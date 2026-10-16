"""The #92 memory verdict over a native replay's resident-memory samples.

`tools/native/native_tape.gd` records a sample every `--rss-every` gameplay
frames (`frame`, `elapsed_ms`, `rss_kb`, Godot's `static_kb` and `objects`,
and the `map` the sample was taken on); `verify_native_tape.py` hands the
samples to `rss_verdict` and fails its receipt on a FAIL.

Total RSS is not the measure. Godot's own allocator holds the current map's
images, so its static memory swings by ~110 MB with the map and returns, and
a scene, a battle or a map state holds art of its own while it lasts. A leak
is growth that persists, so Godot's memory is judged on per-map floors: the
minimum over every `floor_samples` consecutive samples taken on one map
(within a visit and across visits, in run order). A persistent leak raises
every later floor; a transient shorter than `floor_samples` samples cannot
raise any. A state that lasts longer survives the floor at its own height:
see `FLOOR_SAMPLES` for what was measured. Samples that record the battle or
scene state they were taken in would let the verdict compare like with like
without floors (#139).

Godot reports static memory only in debug builds (`MEMORY_STATIC` is 0 under
a release template, which would make the extension term equal RSS and fail
on the map swing), so the verdict is for debug builds of the extension and
engine.
"""

from __future__ import annotations

# Samples per floor window. Measured on the recorded runs (n92's whole route
# and the 2026-10-15 chapter-14 arms, `--rss-every 2500`): every static-memory
# excursion lasting 2 samples or more stays within 10.4 MB of its visit's
# minimum, and the only larger one (n92 map 208, +20.3 MB: a scene and a
# battle) lasts 1 sample. Excursions of up to 10.4 MB recur for up to 19
# samples (overworld states), so no window short enough to cover a run's end
# removes those; they survive the floor under the 16 MB bound. 4 samples
# removes every excursion of up to 3 samples, three times the longest
# excursion that exceeds 10.4 MB.
FLOOR_SAMPLES = 4

RSS_SAMPLE_KEYS = ("frame", "elapsed_ms", "rss_kb", "static_kb", "objects", "map")


def rss_slope(samples: list, key: str = "rss_kb") -> float | None:
    """Least-squares slope of `key` in KiB per 1,000 gameplay frames. Reported,
    not judged: wall-clock work such as audio rendering makes growth per frame
    depend on the host's frame rate."""
    if len(samples) < 2:
        return None
    frames = [item["frame"] / 1000 for item in samples]
    values = [item[key] for item in samples]
    mean_f = sum(frames) / len(frames)
    mean_v = sum(values) / len(values)
    spread = sum((f - mean_f) ** 2 for f in frames)
    if spread == 0:
        return None
    return round(sum((f - mean_f) * (v - mean_v) for f, v in zip(frames, values)) / spread, 1)


def map_floors(measured: list, floor_samples: int, key: str) -> tuple[dict, dict]:
    """Per map, the floor of `key` over every window of `floor_samples`
    consecutive samples on that map, in run order, as `(window_end, frame,
    value)`: the frame of the window's last sample, and the frame and value of
    its lowest sample (the floor is a real sample, so its frame is when that
    memory was measured). Maps with fewer samples than a window are returned
    separately with their sample count."""
    by_map: dict[int, list] = {}
    for item in measured:
        by_map.setdefault(item["map"], []).append(item)
    floors, short = {}, {}
    for map_id, samples in sorted(by_map.items()):
        if len(samples) < floor_samples:
            short[map_id] = len(samples)
            continue
        series = []
        for i in range(len(samples) - floor_samples + 1):
            window = samples[i:i + floor_samples]
            low = min(window, key=lambda item: item[key])
            series.append((window[-1]["frame"], low["frame"], low[key]))
        floors[map_id] = series
    return floors, short


def rss_verdict(samples: list | None, warmup_frames: int, bound_mb: float,
                object_bound: int = 256, floor_samples: int = FLOOR_SAMPLES) -> dict:
    """Memory after warm-up stays bounded (#92), judged in five parts over the
    window from the first sample at or past `warmup_frames` to the last.

    - `extension`: RSS minus Godot static memory (the extension's Rust heap
      and everything else outside Godot's allocator) may not grow more than
      `bound_mb` past the window's first sample.
    - `godot_static`: on each map, the highest static floor (`map_floors`)
      may not exceed the map's first floor by more than `bound_mb`.
    - `objects`: the same for Godot's object count, bounded by
      `object_bound` (leaked nodes, images or textures).
    - `static_drift`: a leak spread over maps that each stay under the
      per-map bound still adds up. Every floor's growth over its map's first
      floor is pooled into one rate per frame (least squares through the
      origin: growth against frames since that map's first floor); the rate
      times the frames from the earliest floor to the latest may not exceed
      `bound_mb`. The limit of detection is that bound: a uniform leak of at
      most `bound_mb` over those frames passes.
    - `coverage`: the compared span runs from the earliest first floor to the
      last sample of any map's last floor window. Frames after it are compared
      with nothing, so
      the uncovered tail may not exceed one floor window (`floor_samples`
      sample intervals), the verdict's own resolution: a transient that short
      is invisible to the floors by design, and anything longer is a hole.
      The head before the compared span is reported, not judged: memory
      grown there is part of every later floor's baseline, as in warm-up.

    A map with fewer than `floor_samples` samples past warm-up is listed under
    `maps_unjudged` with the reason; a window in which no map has a floor
    fails. Missing or unreadable samples (including a sample without its map)
    prove nothing, so they fail rather than pass vacuously."""
    samples = samples if isinstance(samples, list) else []
    verdict = {"warmup_frames": warmup_frames, "bound_mb": bound_mb,
               "object_bound": object_bound, "floor_samples": floor_samples,
               "samples": len(samples)}
    readable = all(isinstance(item, dict) and isinstance(item.get(key), int)
                   for item in samples for key in RSS_SAMPLE_KEYS)
    warmup = [item for item in samples if readable and item["frame"] < warmup_frames]
    measured = [item for item in samples if readable and item["frame"] >= warmup_frames]
    verdict["samples_after_warmup"] = len(measured)
    if (len(measured) < 2 or not warmup or not readable
            or any(item["rss_kb"] <= 0 for item in samples)):
        verdict.update({"pass": False, "reason": "missing or unreadable samples around warm-up"})
        return verdict
    measured = [dict(item, extension_kb=item["rss_kb"] - item["static_kb"]) for item in measured]
    baseline = measured[0]
    last = measured[-1]
    span_s = (last["elapsed_ms"] - baseline["elapsed_ms"]) / 1000
    interval = min(b["frame"] - a["frame"] for a, b in zip(measured, measured[1:]))

    def mb(kb: float) -> float:
        return round(kb / 1024, 1)

    peak_ext = max(measured, key=lambda item: item["extension_kb"])
    extension_growth = peak_ext["extension_kb"] - baseline["extension_kb"]
    floors, short = map_floors(measured, floor_samples, "static_kb")
    object_floors, _ = map_floors(measured, floor_samples, "objects")
    maps = {}
    pooled = []  # (frames since the map's first floor, static growth KiB)
    for map_id, series in floors.items():
        _, first_frame, first_static = series[0]
        objects = object_floors[map_id]
        top_static = max(series, key=lambda floor: floor[2])
        top_objects = max(objects, key=lambda floor: floor[2])
        maps[str(map_id)] = {
            "samples": len(series) + floor_samples - 1,
            "first_floor_frame": first_frame, "last_window_end": series[-1][0],
            "first_floor_static_mb": mb(first_static),
            "static_growth_mb": mb(top_static[2] - first_static),
            "static_peak_floor_frame": top_static[1],
            "object_growth": top_objects[2] - objects[0][2],
            "object_peak_floor_frame": top_objects[1]}
        pooled.extend((frame - first_frame, static - first_static)
                      for _, frame, static in series[1:])
    unjudged = {str(map_id): f"{count} sample(s) past warm-up, fewer than {floor_samples}"
                for map_id, count in short.items()}
    worst_static = max(maps.items(), key=lambda item: item[1]["static_growth_mb"],
                       default=(None, {"static_growth_mb": None, "static_peak_floor_frame": None}))
    worst_objects = max(maps.items(), key=lambda item: item[1]["object_growth"],
                        default=(None, {"object_growth": None, "object_peak_floor_frame": None}))
    if maps:
        compared_first = min(value["first_floor_frame"] for value in maps.values())
        compared_last = max(value["last_window_end"] for value in maps.values())
        last_floor = max(frame for series in floors.values() for _, frame, _ in series)
    else:
        compared_first = compared_last = last_floor = None
    spread = sum(frames * frames for frames, _ in pooled)
    rate = sum(frames * growth for frames, growth in pooled) / spread if spread else None
    drift_kb = rate * (last_floor - compared_first) if rate is not None else None
    tail_limit = floor_samples * interval
    tail = last["frame"] - compared_last if compared_last is not None else None
    verdict.update({
        "baseline_frame": baseline["frame"], "last_frame": last["frame"],
        "sample_interval_frames": interval, "span_s": round(span_s, 1),
        "rss_mb": {"baseline": mb(baseline["rss_kb"]), "last": mb(last["rss_kb"]),
                   "peak": mb(max(item["rss_kb"] for item in measured)),
                   "slope_kb_per_kframe": rss_slope(measured)},
        "extension": {"baseline_mb": mb(baseline["extension_kb"]),
                      "peak_mb": mb(peak_ext["extension_kb"]), "peak_frame": peak_ext["frame"],
                      "last_mb": mb(last["extension_kb"]), "growth_mb": mb(extension_growth),
                      "growth_mb_per_min": round(extension_growth / 1024 / span_s * 60, 2)
                      if span_s > 0 else None,
                      "slope_kb_per_kframe": rss_slope(measured, "extension_kb"),
                      "pass": extension_growth / 1024 <= bound_mb},
        "maps": maps,
        "maps_unjudged": unjudged,
        "godot_static": {"worst_map": worst_static[0],
                         "growth_mb": worst_static[1]["static_growth_mb"],
                         "floor_frame": worst_static[1]["static_peak_floor_frame"],
                         "pass": bool(maps) and all(value["static_growth_mb"] <= bound_mb
                                                    for value in maps.values())},
        "objects": {"worst_map": worst_objects[0], "growth": worst_objects[1]["object_growth"],
                    "floor_frame": worst_objects[1]["object_peak_floor_frame"],
                    "pass": bool(maps) and all(value["object_growth"] <= object_bound
                                               for value in maps.values())},
        "static_drift": {"floors_pooled": len(pooled),
                         "rate_kb_per_kframe": round(rate * 1000, 2) if rate is not None else None,
                         "drift_mb": mb(drift_kb) if drift_kb is not None else None,
                         "pass": drift_kb is not None and drift_kb / 1024 <= bound_mb},
        "coverage": {"compared_first_frame": compared_first,
                     "compared_last_frame": compared_last,
                     "uncovered_head_frames": compared_first - baseline["frame"]
                     if compared_first is not None else None,
                     "uncovered_tail_frames": tail, "tail_limit_frames": tail_limit,
                     "pass": tail is not None and tail <= tail_limit},
    })
    if not maps:
        verdict["reason"] = (f"no map has {floor_samples} samples past warm-up; "
                             "Godot memory is unjudged")
    verdict["pass"] = all(verdict[part]["pass"] for part in
                          ("extension", "godot_static", "objects", "static_drift", "coverage"))
    return verdict
