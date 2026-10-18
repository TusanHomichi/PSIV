"""The #92 memory verdict over a native replay's resident-memory samples.

`tools/native/native_tape.gd` records a sample every `--rss-every` gameplay
frames: `frame`, `elapsed_ms`, `rss_kb`, Godot's `static_kb` and `objects`,
the `map` and the session's `activity` (`Session::activity`: `field`,
`battle`, `scene`, `dialogue`, `camp`, `shop`, `destination`, `title`,
`game_over`). `verify_native_tape.py` hands the samples to `rss_verdict` and
fails its receipt on a FAIL.

Total RSS is not the measure. Godot's own allocator holds the current map's
images, so its static memory swings by ~110 MB with the map and returns, and
a battle, a scene or a menu holds art of its own while it runs. A leak is
growth that persists, so the closing measure is a bracket: in the same Godot
process, the driver loads one fixed save through the title's CONTINUE before
the tape starts and again after it ends, lets each settle on the open field,
and samples it (`PROBE_*`). The same save, in the same state, in the same
process: whatever the run left behind and still holds shows as the end probe's
floor over the start probe's, wherever in the run it grew. What the shell
keeps on purpose and fills on first use (texture caches, node pools) is
reported by name with a ceiling from content (`rust/psiv-godot/src/
named_caches.rs`, read through `Field.debug_named_caches`) and subtracted
first, so the bracket judges only what nothing names.

Between the probes, the samples taken on the open field (`activity ==
"field"`) are compared per map across visits, as a diagnostic that still fails
on a breach. Even there a map refresh can briefly hold two copies of the map's
images (n92 map 208, +20.3 MB for one sample at 2,500-frame cadence), so each
map's field samples are judged on floors: the minimum over the last
`FLOOR_FRAMES` frames of field time observed on that map. A persistent leak
raises every later floor; a field transient shorter than the window cannot
raise any. A route's brief late visits leave stretches no map's floors can see
(`coverage_gaps`); the bracket covers them, so coverage is reported.

Godot reports static memory only in debug builds (`MEMORY_STATIC` is 0 under
a release template, which would make the extension term equal RSS and fail
on the map swing), so the verdict is for debug builds of the extension and
engine (#117).
"""

from __future__ import annotations

import math

# The floor window, in frames of field time observed on one map (each sample
# stands for one `--rss-every` interval, so a window is
# FLOOR_FRAMES / cadence consecutive field samples on the map). Measured on
# n92's whole route with activity tags recovered from a headless replay
# (2026-10-16): every field excursion larger than 12 MB lasts one 2,500-frame
# sample (map 208's map-refresh +20.3 MB); excursions of 5 to 12 MB last up to
# 27,500 frames (overworld states) and survive any window short enough to
# judge brief visits, under the 16 MB bound. 10,000 frames is four times the
# longest excursion that approaches the bound.
FLOOR_FRAMES = 10_000

# The longest stretch of the window (head, between two compared spans, or
# tail) in which no map's floors could see a persistent step. A step inside
# such a gap is part of every later baseline and is never seen, so a gap is a
# hole, judged at the verdict's own resolution: one floor window.
GAP_LIMIT_FRAMES = FLOOR_FRAMES

#: A floor window must hold at least this many samples, or the cadence is too
#: coarse to tell a transient from a step and the verdict refuses it.
MIN_FLOOR_SAMPLES = 2

# The bracket probes. After CONTINUE the probe waits PROBE_SETTLE_FRAMES
# consecutive frames on the open field (on n92 every field excursion above
# 12 MB was gone within one 2,500-frame sample), then takes PROBE_SAMPLES
# samples PROBE_SAMPLE_EVERY frames apart and keeps each measure's floor. A
# probe that has not finished within PROBE_TIMEOUT_FRAMES of entering the game
# fails the run: it must not pass for having measured nothing.
PROBE_SETTLE_FRAMES = 2_500
PROBE_SAMPLES = 8
PROBE_SAMPLE_EVERY = 250
PROBE_TIMEOUT_FRAMES = 20_000
#: The CONTINUE slot (zero-based) the probe save is copied into. A retail slot
#: file carries its slot's layout, so a chapter save (a slot-1 file) loads only
#: as slot 1. The route's camp SAVE writes the same slot, so before the end
#: probe the driver keeps the run's slot file as `RUN_SLOT_FILE` and puts the
#: probe save back.
PROBE_SLOT = 0
RUN_SLOT_FILE = "slot_1.run.sram"

FIELD = "field"
PROBE_KEYS = ("rss_kb", "static_kb", "objects")
RSS_SAMPLE_KEYS = ("frame", "elapsed_ms", "rss_kb", "static_kb", "objects", "map")


def floor_samples_for(cadence: int, floor_frames: int = FLOOR_FRAMES) -> int:
    """Field samples per floor window at `cadence` frames between samples."""
    return math.ceil(floor_frames / cadence)


def cadence_error(cadence: int, floor_frames: int = FLOOR_FRAMES) -> str | None:
    """Why `cadence` cannot be judged, or None. A floor needs at least
    `MIN_FLOOR_SAMPLES` samples inside `floor_frames`."""
    if cadence * MIN_FLOOR_SAMPLES > floor_frames:
        return (f"sample cadence {cadence} frames is coarser than "
                f"{floor_frames // MIN_FLOOR_SAMPLES} (a {floor_frames}-frame floor needs "
                f"{MIN_FLOOR_SAMPLES} samples)")
    return None


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


def map_floors(field: list, window: int, key: str) -> tuple[dict, dict]:
    """Per map, the floor of `key` over every `window` consecutive field
    samples on that map, in run order: `(window_start, frame, value)`, the
    frame of the window's first sample and the frame and value of its lowest
    sample (a real sample, so its frame is when that memory was measured).
    Maps with fewer field samples than a window are returned separately with
    their count."""
    by_map: dict[int, list] = {}
    for item in field:
        by_map.setdefault(item["map"], []).append(item)
    floors, short = {}, {}
    for map_id, samples in sorted(by_map.items()):
        if len(samples) < window:
            short[map_id] = len(samples)
            continue
        series = []
        for i in range(len(samples) - window + 1):
            span = samples[i:i + window]
            low = min(span, key=lambda item: item[key])
            series.append((span[0]["frame"], low["frame"], low[key]))
        floors[map_id] = (samples[0]["frame"], series)
    return floors, short


def pooled_drift(floors: dict) -> tuple[float | None, int | None, int]:
    """One growth rate per frame shared by every map: the within-map least-
    squares slope of floor values against frame, pooled over maps (each map
    measured from its own mean, so a map's height and a one-time offset
    between its states are not read as a rate; a regression through each
    map's first floor turns an early bounded step into ~1.5 times the step of
    "drift"). Returns the rate, the frames from the earliest floor to the
    latest, and the number of floors pooled."""
    numerator = denominator = 0.0
    frames, count = [], 0
    for _, series in floors.values():
        points = [(frame, value) for _, frame, value in series]
        frames.extend(frame for frame, _ in points)
        count += len(points)
        mean_x = sum(x for x, _ in points) / len(points)
        mean_y = sum(y for _, y in points) / len(points)
        numerator += sum((x - mean_x) * (y - mean_y) for x, y in points)
        denominator += sum((x - mean_x) ** 2 for x, _ in points)
    rate = numerator / denominator if denominator else None
    return rate, (max(frames) - min(frames)) if frames else None, count


def coverage_gaps(floors: dict, start: int, end: int) -> list:
    """The stretches of [start, end] in which no map's floors can see a
    persistent step. A step at frame t raises every floor whose window starts
    after t, so a map with two or more floors covers (its first field sample,
    its last floor window's first sample]; the gaps are what the union of those
    spans leaves, head and tail included."""
    spans = sorted((first, series[-1][0]) for first, series in floors.values()
                   if len(series) >= 2)
    gaps, cursor = [], start
    for low, high in spans:
        if low > cursor:
            gaps.append((cursor, low))
        cursor = max(cursor, high)
    if end > cursor:
        gaps.append((cursor, end))
    return gaps


def growth(floors: dict, objects: dict, window: int, mb) -> dict:
    """Per judged map: its highest floor over its first, static and objects."""
    maps = {}
    for map_id, (first, series) in floors.items():
        object_series = objects[map_id][1]
        top_static = max(series, key=lambda floor: floor[2])
        top_objects = max(object_series, key=lambda floor: floor[2])
        maps[str(map_id)] = {
            "field_samples": len(series) + window - 1,
            "floors": len(series), "first_field_frame": first,
            "last_floor_start": series[-1][0],
            "first_floor_static_mb": mb(series[0][2]),
            "static_growth_mb": mb(top_static[2] - series[0][2]),
            "static_peak_floor_frame": top_static[1],
            "object_growth": top_objects[2] - object_series[0][2],
            "object_peak_floor_frame": top_objects[1]}
    return maps


CACHE_KEYS = ("entries", "textures", "nodes", "other_objects", "bytes", "ceiling")


def probe_caches(sample: dict) -> dict | None:
    """A probe sample's named caches (`named_caches.rs`), or None when absent
    or malformed."""
    caches = sample.get("caches")
    if not isinstance(caches, dict) or not caches:
        return None
    out = {}
    for name, cache in caches.items():
        if not isinstance(cache, dict):
            return None
        values = {}
        for key in CACHE_KEYS:
            value = cache.get(key)
            # Godot's JSON reads every number back as a float.
            if isinstance(value, float) and value.is_integer():
                value = int(value)
            if not isinstance(value, int) or isinstance(value, bool):
                return None
            values[key] = value
        out[name] = values
    return out


def probe_floor(probe: dict | None) -> tuple[dict | None, str | None]:
    """A probe's floor of each measure, or why it has none: missing, never
    settled on the open field, too few samples, or a sample off the field,
    unreadable or without its named caches.

    Objects and static memory are floored outside the named caches: each
    sample's cache objects and bytes are subtracted first. The caches
    themselves are the last sample's."""
    if not isinstance(probe, dict):
        return None, "missing"
    if probe.get("settled") is not True:
        return None, f"never settled on the open field ({probe.get('reason', 'no reason given')})"
    samples = probe.get("samples")
    if not isinstance(samples, list) or len(samples) < PROBE_SAMPLES:
        return None, f"fewer than {PROBE_SAMPLES} samples"
    if not all(isinstance(item, dict) and item.get("activity") == FIELD
               and all(isinstance(item.get(key), int) for key in PROBE_KEYS + ("orphan_nodes",))
               and item["rss_kb"] > 0 for item in samples):
        return None, "a sample off the open field or unreadable"
    caches = [probe_caches(item) for item in samples]
    if any(cache is None for cache in caches):
        return None, "a sample without its named caches"
    per_texture = probe.get("objects_per_texture")
    if not isinstance(per_texture, int) or per_texture < 1:
        return None, "no measured objects per texture"
    for item_caches in caches:
        for cache in item_caches.values():
            cache["objects"] = (cache["textures"] * per_texture + cache["nodes"]
                                + cache["other_objects"])
    floor = {key: min(item[key] for item in samples) for key in PROBE_KEYS + ("orphan_nodes",)}
    floor["objects_outside_caches"] = min(
        item["objects"] - sum(cache["objects"] for cache in item_caches.values())
        for item, item_caches in zip(samples, caches))
    floor["static_outside_caches_kb"] = min(
        item["static_kb"] - sum(cache["bytes"] for cache in item_caches.values()) / 1024
        for item, item_caches in zip(samples, caches))
    floor["extension_kb"] = min(item["rss_kb"] - item["static_kb"] for item in samples)
    if all(isinstance(item.get("nodes"), int) for item in samples):
        floor["nodes"] = min(item["nodes"] for item in samples)
    floor["caches"] = caches[-1]
    floor["objects_per_texture"] = per_texture
    return floor, None


def bracket_verdict(probes: dict | None, bound_mb: float, object_bound: int) -> dict:
    """The end probe over the start probe, outside the named caches.

    Judged: Godot objects outside the caches may not grow more than
    `object_bound`, Godot static memory outside the caches' bytes and the
    extension heap no more than `bound_mb`; neither probe may hold an orphan
    node; and no cache may hold more entries than its content ceiling at
    either probe. Each cache's growth is reported beside its ceiling. A
    missing or unsettled probe, or one without its caches, fails."""
    probes = probes if isinstance(probes, dict) else {}
    start, start_why = probe_floor(probes.get("start"))
    end, end_why = probe_floor(probes.get("end"))
    if start is None or end is None:
        reasons = [f"{name} probe {why}" for name, why in (("start", start_why), ("end", end_why))
                   if why]
        return {"pass": False, "reason": "; ".join(reasons)}

    def mb(kb: float) -> float:
        return round(kb / 1024, 1)

    static = end["static_outside_caches_kb"] - start["static_outside_caches_kb"]
    extension = end["extension_kb"] - start["extension_kb"]
    objects = end["objects_outside_caches"] - start["objects_outside_caches"]
    caches = {}
    for name in sorted(set(start["caches"]) | set(end["caches"])):
        before, after = start["caches"].get(name), end["caches"].get(name)
        if before is None or after is None:
            caches[name] = {"pass": False, "reason": "reported by one probe only"}
            continue
        caches[name] = {
            "entries": [before["entries"], after["entries"]],
            "objects": [before["objects"], after["objects"]],
            "mb": [mb(before["bytes"] / 1024), mb(after["bytes"] / 1024)],
            "ceiling": after["ceiling"],
            "pass": max(before["entries"], after["entries"]) <= after["ceiling"]}
    orphans = max(start["orphan_nodes"], end["orphan_nodes"])
    return {
        "start_floor": {"static_mb": mb(start["static_kb"]), "objects": start["objects"],
                        "extension_mb": mb(start["extension_kb"]), "rss_mb": mb(start["rss_kb"])},
        "end_floor": {"static_mb": mb(end["static_kb"]), "objects": end["objects"],
                      "extension_mb": mb(end["extension_kb"]), "rss_mb": mb(end["rss_kb"])},
        "static_growth_mb": mb(static), "extension_growth_mb": mb(extension),
        "object_growth": objects,
        "total_static_growth_mb": mb(end["static_kb"] - start["static_kb"]),
        "total_object_growth": end["objects"] - start["objects"],
        "node_growth": end["nodes"] - start["nodes"]
        if "nodes" in end and "nodes" in start else None,
        "orphan_nodes": orphans,
        "objects_per_texture": [start["objects_per_texture"], end["objects_per_texture"]],
        "caches": caches,
        "pass": static / 1024 <= bound_mb and extension / 1024 <= bound_mb
        and objects <= object_bound and orphans == 0
        and all(cache["pass"] for cache in caches.values())}


def rss_verdict(samples: list | None, warmup_frames: int, bound_mb: float,
                object_bound: int = 256, floor_frames: int = FLOOR_FRAMES,
                gap_limit_frames: int = GAP_LIMIT_FRAMES, probes: dict | None = None) -> dict:
    """Memory after warm-up stays bounded (#92). Four parts are judged over the
    window from the first sample at or past `warmup_frames` to the last, and
    three are reported.

    Judged:

    - `bracket` (`bracket_verdict`): the end probe over the start probe. It
      closes the check: it sees a persistent leak anywhere in the run.

    - `extension`: RSS minus Godot static memory (the extension's Rust heap
      and everything else outside Godot's allocator), over every sample, may
      not grow more than `bound_mb` past the window's first sample.
    - `godot_static`: on each map, the highest field floor (`map_floors`) may
      not exceed the map's first floor by more than `bound_mb`: a breach is a
      leak signal mid-run, though a map floored nowhere cannot breach.
    - `objects`: the same for Godot's object count, bounded by `object_bound`.

    Reported, with `within_bound`/`within_limit` for the reader:

    - `static_drift` and `object_drift`: the floors' growth pooled into one
      rate (`pooled_drift`) times the frames from the earliest floor to the
      latest, against the bound.
    - `coverage`: the gaps (`coverage_gaps`) no map's floors can see, against
      `gap_limit_frames`. The bracket sees them.

    A map with fewer field samples than a floor window is listed under
    `maps_unjudged` with the reason. A cadence too coarse for the floor
    window (`cadence_error`), a missing or unsettled probe, and missing or
    unreadable samples (including one without its map or activity) fail
    rather than pass vacuously."""
    samples = samples if isinstance(samples, list) else []
    verdict = {"warmup_frames": warmup_frames, "bound_mb": bound_mb,
               "object_bound": object_bound, "floor_frames": floor_frames,
               "gap_limit_frames": gap_limit_frames, "samples": len(samples)}
    readable = all(isinstance(item, dict) and isinstance(item.get("activity"), str)
                   and all(isinstance(item.get(key), int) for key in RSS_SAMPLE_KEYS)
                   for item in samples)
    warmup = [item for item in samples if readable and item["frame"] < warmup_frames]
    measured = [item for item in samples if readable and item["frame"] >= warmup_frames]
    verdict["samples_after_warmup"] = len(measured)
    if (len(measured) < 2 or not warmup or not readable
            or any(item["rss_kb"] <= 0 for item in samples)):
        verdict.update({"pass": False, "reason": "missing or unreadable samples around warm-up"})
        return verdict
    cadence = min(b["frame"] - a["frame"] for a, b in zip(samples, samples[1:]))
    verdict["cadence_frames"] = cadence
    refused = cadence_error(cadence, floor_frames)
    if refused:
        verdict.update({"pass": False, "reason": refused})
        return verdict
    window = floor_samples_for(cadence, floor_frames)
    measured = [dict(item, extension_kb=item["rss_kb"] - item["static_kb"]) for item in measured]
    baseline, last = measured[0], measured[-1]
    span_s = (last["elapsed_ms"] - baseline["elapsed_ms"]) / 1000

    def mb(kb: float) -> float:
        return round(kb / 1024, 1)

    peak_ext = max(measured, key=lambda item: item["extension_kb"])
    extension_growth = peak_ext["extension_kb"] - baseline["extension_kb"]
    field = [item for item in measured if item["activity"] == FIELD]
    floors, short = map_floors(field, window, "static_kb")
    object_floors, _ = map_floors(field, window, "objects")
    maps = growth(floors, object_floors, window, mb)
    unjudged = {str(map_id): f"{count} field sample(s) past warm-up, fewer than {window}"
                for map_id, count in short.items()}
    static_rate, static_span, static_pooled = pooled_drift(floors)
    object_rate, object_span, object_pooled = pooled_drift(object_floors)
    gaps = coverage_gaps(floors, baseline["frame"], last["frame"])
    widest = max(gaps, key=lambda gap: gap[1] - gap[0], default=None)
    worst_static = max(maps.items(), key=lambda item: item[1]["static_growth_mb"],
                       default=(None, {"static_growth_mb": None, "static_peak_floor_frame": None}))
    worst_objects = max(maps.items(), key=lambda item: item[1]["object_growth"],
                        default=(None, {"object_growth": None, "object_peak_floor_frame": None}))
    static_drift = static_rate * static_span if static_rate is not None else None
    object_drift = object_rate * object_span if object_rate is not None else None
    activities: dict[str, int] = {}
    for item in measured:
        activities[item["activity"]] = activities.get(item["activity"], 0) + 1
    verdict.update({
        "baseline_frame": baseline["frame"], "last_frame": last["frame"],
        "floor_samples": window, "span_s": round(span_s, 1),
        "activities": dict(sorted(activities.items())),
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
                         "maps_judged": len(maps),
                         "pass": all(value["static_growth_mb"] <= bound_mb
                                     for value in maps.values())},
        "objects": {"worst_map": worst_objects[0], "growth": worst_objects[1]["object_growth"],
                    "floor_frame": worst_objects[1]["object_peak_floor_frame"],
                    "pass": all(value["object_growth"] <= object_bound
                                for value in maps.values())},
        "static_drift": {"floors_pooled": static_pooled,
                         "rate_kb_per_kframe": round(static_rate * 1000, 2)
                         if static_rate is not None else None,
                         "drift_mb": mb(static_drift) if static_drift is not None else None,
                         "within_bound": static_drift is not None
                         and static_drift / 1024 <= bound_mb},
        "object_drift": {"floors_pooled": object_pooled,
                         "rate_per_kframe": round(object_rate * 1000, 3)
                         if object_rate is not None else None,
                         "drift": round(object_drift, 1) if object_drift is not None else None,
                         "within_bound": object_drift is not None
                         and object_drift <= object_bound},
        "coverage": {"gaps": [{"from": low, "to": high, "frames": high - low}
                              for low, high in gaps],
                     "widest_gap_frames": widest[1] - widest[0] if widest else 0,
                     "uncovered_frames": sum(high - low for low, high in gaps),
                     "within_limit": bool(maps) and all(high - low <= gap_limit_frames
                                                        for low, high in gaps)},
        "bracket": bracket_verdict(probes, bound_mb, object_bound),
    })
    verdict["pass"] = all(verdict[part]["pass"] for part in
                          ("extension", "bracket", "godot_static", "objects"))
    return verdict
