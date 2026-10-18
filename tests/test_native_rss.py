"""The #92 memory verdict, `tools/native_rss.py`, on synthetic and recorded samples.

    PYTHONPATH=. python3 -m unittest tests.test_native_rss -v

The check closes on a bracket: the same save loaded through the title before
and after the run, settled on the open field and floored. Between the probes,
field samples are compared per map on floors over `FLOOR_FRAMES` of observed
field time, and a per-map breach still fails. Every negative control names
the part of the verdict that must fail it, and every positive control the
reason it must not.

The bracket (the 2026-10-17 ruling's controls): (a) a leak injected in each of
the gap windows no map's floors see fails the bracket, (b) a 20 MB transient
released before the end passes, (c) the shipped #92 shape (live Rust bytes
climbing) fails on the heap, and recorded runs without the heap counter are
refused, (d) a probe that never settles on the field fails. The per-map
floors: growth at re-entry, a tail step, n92's map-208 straddle at 2,500- and
500-frame cadence, a transient on a re-entry sample. Reported, not judged: the
drifts and coverage.
"""

import json
import unittest
from pathlib import Path

from tools.native_rss import (FLOOR_FRAMES, GAP_LIMIT_FRAMES, PROBE_SAMPLES,
                               PROBE_SETTLE_FRAMES, cadence_error, rss_verdict)

ROOT = Path(__file__).resolve().parents[1]
CHAPTER14 = ROOT / "tests" / "native_rss_chapter14.json"
EVERY = 2500
# Warm-up ends at 5,000: the first row (frame 2,500) of every run below.
WARMUP = 5000


def samples(*rows, every=EVERY, residual=0.0):
    """One sample per row of (rss_mb, static_mb, objects, map[, activity])
    every `every` frames (activity defaults to the field); the first row is
    frame `every`. Live Rust bytes are RSS minus static minus `residual` MB,
    so a row's heap is what it does not give to Godot."""
    out = []
    for i, row in enumerate(rows):
        rss, static, objects, map_id, *rest = row
        out.append({"frame": every * (i + 1), "elapsed_ms": 1000 * (i + 1),
                    "rss_kb": int(rss * 1024), "static_kb": int(static * 1024),
                    "rust_live_bytes": int((rss - static - residual) * 1024 * 1024),
                    "objects": int(objects), "map": map_id,
                    "activity": rest[0] if rest else "field"})
    return out


def visits(*plan, extension=150, static_leak=lambda i: 0.0, object_leak=lambda i: 0):
    """Rows for `plan`, a list of (map, static_mb, objects, count[, activity]):
    `count` samples on the map. The leaks add to static (and RSS) and to
    objects at each row's index; RSS is static plus `extension`."""
    rows = []
    for map_id, static, objects, count, *rest in plan:
        for _ in range(count):
            extra = static_leak(len(rows))
            rows.append((static + extra + extension, static + extra,
                         objects + object_leak(len(rows)), map_id, *rest))
    return rows


WARM = (9, 100, 2800, 1)
# Two maps re-entered in turn: map 1 holds 110 MB more images than map 2.
SWING = [(1, 210, 2800, 4), (2, 100, 2800, 4)] * 3


def cache(entries=0, textures=None, nodes=0, other=0, mb=0.0, ceiling=100):
    """One named cache as the debug accessor reports it (Godot's JSON reads
    every number back as a float)."""
    return {"entries": float(entries),
            "textures": float(entries if textures is None else textures),
            "nodes": float(nodes), "other_objects": float(other),
            "bytes": float(int(mb * 1024 * 1024)), "ceiling": float(ceiling)}


def probe(static=100, objects=2800, extension=150, settled=True, count=PROBE_SAMPLES,
          caches=None, orphans=0, per_texture=2, residual=0.0, monitor=True):
    """A settled bracket probe: `count` field samples at these values, with
    its named caches (none filled by default)."""
    caches = caches if caches is not None else {"battle_command_tiles": cache()}
    return {"settled": settled, "frames_waited": PROBE_SETTLE_FRAMES + 2000,
            "objects_per_texture": per_texture, "orphan_monitor_alive": monitor,
            "samples": [{"frame": PROBE_SETTLE_FRAMES + 250 * (i + 1),
                         "rss_kb": int((static + extension + residual) * 1024),
                         "rust_live_bytes": int(extension * 1024 * 1024),
                         "static_kb": int(static * 1024), "objects": objects,
                         "orphan_nodes": orphans, "nodes": 30, "caches": caches,
                         "activity": "field", "map": 19} for i in range(count)]}


def bracket(start_static=100, end_static=100, start_objects=2800, end_objects=2800,
            start_extension=150, end_extension=150):
    """Start and end probes; the defaults are a run that left nothing behind."""
    return {"start": probe(start_static, start_objects, start_extension),
            "end": probe(end_static, end_objects, end_extension)}


def judge(rows, **options):
    return rss_verdict(samples(*rows, every=options.pop("every", EVERY)),
                       options.pop("warmup", WARMUP), 16,
                       probes=options.pop("probes", bracket()), **options)


class Bracket(unittest.TestCase):
    def test_a_run_that_leaves_nothing_behind_passes(self):
        verdict = judge(visits(WARM, *SWING))
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["bracket"]["static_growth_mb"], 0.0)
        self.assertEqual(verdict["bracket"]["end_floor"]["rust_live_mb"], 150.0)

    def gap_run(self, where, leak_mb=200):
        """A stretch of brief visits (one sample each, never floored) at
        `where` (n92's gaps: 2.53M-2.60M, 3.60M-3.66M, 4.17M-end), with maps
        re-entered on either side that never cross it; a leak lands inside
        the stretch and stays, so the end probe holds it."""
        brief = [(40 + m, 100, 2800, 1) for m in range(6)]
        before = [(1, 210, 2800, 4), (2, 100, 2800, 4)]
        after = [(3, 210, 2800, 4), (4, 100, 2800, 4)]
        plan = {"head": brief + after * 3, "middle": before * 2 + brief + after * 2,
                "tail": before * 3 + brief}[where]
        step_at = {"head": 3, "middle": 19, "tail": 28}[where]
        return judge(visits(WARM, *plan, static_leak=lambda i: leak_mb * (i >= step_at)),
                     probes=bracket(end_static=100 + leak_mb))

    def test_a_leak_in_each_gap_window_fails_the_bracket(self):
        # (a) No map's floors can see the step (coverage reports the gap),
        # but the end probe holds it.
        for where in ("head", "middle", "tail"):
            verdict = self.gap_run(where)
            self.assertFalse(verdict["pass"], where)
            self.assertFalse(verdict["bracket"]["pass"], where)
            self.assertEqual(verdict["bracket"]["static_growth_mb"], 200.0, where)
            self.assertTrue(verdict["godot_static"]["pass"], where)
            self.assertFalse(verdict["coverage"]["within_limit"], where)
        # 17 MB in a gap is past the bound too; 15 MB is inside it.
        self.assertFalse(self.gap_run("middle", 17)["bracket"]["pass"])
        self.assertTrue(self.gap_run("middle", 15)["bracket"]["pass"])

    def test_a_transient_released_before_the_end_passes(self):
        # (b) 20 MB held for a stretch mid-run, then released: both probes
        # read the same floor.
        rows = visits(WARM, *SWING, static_leak=lambda i: 20 * (10 <= i < 14))
        verdict = judge(rows)
        self.assertTrue(verdict["bracket"]["pass"], verdict["bracket"])
        self.assertEqual(verdict["bracket"]["static_growth_mb"], 0.0)

    def test_live_rust_bytes_and_objects_are_bracketed_too(self):
        # A synthetic leak of live Rust bytes fails the bracket.
        heap = judge(visits(WARM, *SWING), probes=bracket(end_extension=170))
        self.assertFalse(heap["bracket"]["pass"])
        self.assertEqual(heap["bracket"]["rust_live_growth_mb"], 20.0)
        objects = judge(visits(WARM, *SWING), probes=bracket(end_objects=3100))
        self.assertFalse(objects["bracket"]["pass"])
        self.assertEqual(objects["bracket"]["object_growth"], 300)

    def test_a_probe_that_never_settles_fails(self):
        # (d) The probe waited out its timeout without a settled field: no
        # floor, so the run cannot pass on it.
        unsettled = bracket()
        unsettled["end"] = {"settled": False, "frames_waited": 20000,
                            "reason": "no 2500 field frames plus 8 samples within 20000 frames"}
        verdict = judge(visits(WARM, *SWING), probes=unsettled)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["bracket"]["pass"])
        self.assertIn("end probe never settled", verdict["bracket"]["reason"])
        for broken, why in ((None, "start probe missing"),
                            (probe(count=PROBE_SAMPLES - 1), "fewer than"),
                            ({**probe(), "samples": [dict(probe()["samples"][0], activity="battle")]
                              * PROBE_SAMPLES}, "off the open field")):
            probes = bracket()
            probes["start"] = broken
            verdict = judge(visits(WARM, *SWING), probes=probes)
            self.assertFalse(verdict["pass"], why)
            self.assertIn(why, verdict["bracket"]["reason"])
        self.assertIn("missing", judge(visits(WARM, *SWING), probes=None)["bracket"]["reason"])

    def test_cache_growth_within_its_ceiling_passes(self):
        # (f) The academy smoke's shape: 36 command tiles and 14 sprite
        # sheets filled on first use, 2 objects a texture (a headless dummy
        # renderer keeps each texture's image), and their pixel bytes in
        # static memory. Outside the caches nothing grew.
        start = probe(caches={"battle_command_tiles": cache(0, ceiling=152),
                              "field_sprite_sheets": cache(8, mb=0.2, ceiling=245)})
        end = probe(static=100.3, objects=2800 + 72 + 28,
                    caches={"battle_command_tiles": cache(36, ceiling=152),
                            "field_sprite_sheets": cache(22, mb=0.5, ceiling=245)})
        verdict = judge(visits(WARM, *SWING), probes={"start": start, "end": end})
        bracket_part = verdict["bracket"]
        self.assertTrue(bracket_part["pass"], bracket_part)
        self.assertEqual((bracket_part["object_growth"], bracket_part["total_object_growth"]),
                         (0, 100))
        self.assertEqual(bracket_part["static_growth_mb"], 0.0)
        self.assertEqual(bracket_part["caches"]["battle_command_tiles"]["entries"], [0, 36])
        self.assertEqual(bracket_part["caches"]["battle_command_tiles"]["objects"], [0, 72])

    def test_an_object_leak_outside_the_caches_fails(self):
        # (e) The same caches, plus 300 objects nothing names.
        start = probe(caches={"battle_command_tiles": cache(0, ceiling=152)})
        end = probe(objects=2800 + 72 + 300,
                    caches={"battle_command_tiles": cache(36, ceiling=152)})
        verdict = judge(visits(WARM, *SWING), probes={"start": start, "end": end})
        self.assertFalse(verdict["bracket"]["pass"])
        self.assertEqual(verdict["bracket"]["object_growth"], 300)
        self.assertTrue(verdict["bracket"]["caches"]["battle_command_tiles"]["pass"])

    def test_a_cache_past_its_ceiling_fails(self):
        # (g) A cache cannot hold more entries than the pack has assets.
        start = probe(caches={"dialogue_portraits": cache(0, ceiling=39)})
        end = probe(objects=2800 + 80,
                    caches={"dialogue_portraits": cache(40, ceiling=39)})
        verdict = judge(visits(WARM, *SWING), probes={"start": start, "end": end})
        self.assertFalse(verdict["bracket"]["pass"])
        self.assertFalse(verdict["bracket"]["caches"]["dialogue_portraits"]["pass"])
        self.assertEqual(verdict["bracket"]["object_growth"], 0)

    def test_an_orphan_node_fails(self):
        # The flash-attack class: a node made and never parented.
        verdict = judge(visits(WARM, *SWING),
                        probes={"start": probe(), "end": probe(objects=2801, orphans=1)})
        self.assertFalse(verdict["bracket"]["pass"])
        self.assertEqual(verdict["bracket"]["orphan_nodes"], 1)

    def test_static_memory_is_judged_outside_the_cache_bytes(self):
        start = probe(caches={"field_sprite_sheets": cache(8, mb=0.2)})
        within = probe(static=100.3, objects=2828,
                       caches={"field_sprite_sheets": cache(22, mb=0.5)})
        leak = probe(static=120.3, objects=2828,
                     caches={"field_sprite_sheets": cache(22, mb=0.5)})
        self.assertTrue(judge(visits(WARM, *SWING),
                              probes={"start": start, "end": within})["bracket"]["pass"])
        failed = judge(visits(WARM, *SWING), probes={"start": start, "end": leak})["bracket"]
        self.assertFalse(failed["pass"])
        self.assertEqual(failed["static_growth_mb"], 20.0)

    def test_a_probe_without_its_caches_or_texture_ratio_fails(self):
        no_caches = probe()
        for sample in no_caches["samples"]:
            del sample["caches"]
        no_ratio = probe()
        del no_ratio["objects_per_texture"]
        for broken, why in ((no_caches, "named caches"), (no_ratio, "objects per texture")):
            verdict = judge(visits(WARM, *SWING), probes={"start": probe(), "end": broken})
            self.assertFalse(verdict["pass"], why)
            self.assertIn(why, verdict["bracket"]["reason"])

    def test_the_rss_residual_is_reported_never_judged(self):
        # 40 MB more RSS that is neither Godot static nor live Rust bytes:
        # the allocator kept freed pages. Reported, and the run passes.
        verdict = judge(visits(WARM, *SWING),
                        probes={"start": probe(), "end": probe(residual=40)})
        self.assertTrue(verdict["pass"], verdict["bracket"])
        self.assertEqual(verdict["bracket"]["residual_growth_mb"], 40.0)
        self.assertEqual(verdict["bracket"]["rust_live_growth_mb"], 0.0)
        rows = samples(*visits(WARM, *SWING))
        for index, item in enumerate(rows):
            item["rss_kb"] += 1024 * 3 * index  # 3 MB more retained per sample
        whole = rss_verdict(rows, WARMUP, 16, probes=bracket())
        self.assertTrue(whole["pass"], whole["rust_heap"])
        self.assertGreater(whole["residual"]["last_mb"] - whole["residual"]["baseline_mb"], 16)

    def test_a_dead_orphan_monitor_fails_the_probe(self):
        verdict = judge(visits(WARM, *SWING),
                        probes={"start": probe(), "end": probe(monitor=False)})
        self.assertFalse(verdict["pass"])
        self.assertIn("orphan-node monitor", verdict["bracket"]["reason"])

    def test_recorded_runs_without_the_heap_counter_are_refused(self):
        # (c) The 2026-10-15 shipped and fixed chapter-14 runs predate the
        # counter: they fail closed rather than be judged on RSS. Their
        # verdict at the time (heap +115.5 vs +4.3 MB as RSS minus static) is
        # in the commit messages; the shipped shape, live bytes climbing
        # 3.8 MB a minute, is the synthetic case below.
        runs = json.loads(CHAPTER14.read_text())
        for arm in ("shipped", "fixed"):
            verdict = rss_verdict(runs[arm], 20000, 16, probes=bracket())
            self.assertFalse(verdict["pass"], arm)
            self.assertIn("unreadable", verdict["reason"], arm)
        rows = samples(*visits(WARM, *SWING))
        for index, item in enumerate(rows):
            item["rust_live_bytes"] += int(3.8 * 1024 * 1024) * index
        leaking = rss_verdict(rows, WARMUP, 16, probes=bracket())
        self.assertFalse(leaking["pass"])
        self.assertFalse(leaking["rust_heap"]["pass"])


class Floors(unittest.TestCase):
    """Per-map field floors: diagnostics between the probes that still fail on
    a breach."""

    def test_battles_and_scenes_are_never_compared(self):
        # A battle on map 1 holds 40 MB and 300 objects of its own art.
        rows = visits(WARM, (1, 210, 2800, 4), (1, 250, 3100, 3, "battle"),
                      (2, 100, 2800, 4), (1, 210, 2800, 4), (1, 260, 3200, 2, "scene"),
                      (2, 100, 2800, 4), (1, 210, 2800, 4), (2, 100, 2800, 4))
        verdict = judge(rows)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["activities"], {"battle": 3, "field": 24, "scene": 2})
        self.assertEqual(verdict["maps"]["1"]["field_samples"], 12)

    def test_a_growing_floor_at_re_entry_fails(self):
        rows = visits(WARM, (1, 100, 2800, 4), (2, 100, 2800, 4), (1, 110, 2800, 4),
                      (2, 100, 2800, 4), (1, 120, 2800, 4), (2, 100, 2800, 4))
        verdict = judge(rows)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["godot_static"]["pass"])
        self.assertEqual((verdict["godot_static"]["worst_map"],
                          verdict["godot_static"]["growth_mb"]), ("1", 20.0))

    def test_a_step_in_a_long_tail_visit_fails(self):
        head = visits(WARM, *SWING[:3])
        tail = [(250, 100, 2800, 374)] * 6 + [(450, 300, 2800, 374)] * 8
        verdict = judge(head + tail)
        self.assertFalse(verdict["godot_static"]["pass"])
        self.assertEqual((verdict["godot_static"]["worst_map"],
                          verdict["godot_static"]["growth_mb"]), ("374", 200.0))
        tail = [(250, 100, 2800, 374)] * 6 + [(250, 100, 4800, 374)] * 8
        self.assertFalse(judge(head + tail)["objects"]["pass"])

    def test_map_208s_straddle_does_not_fail(self):
        # n92 at 2,500-frame cadence: map 208's two field samples, the second
        # during a map refresh holding +20.3 MB and +140 objects.
        rows = visits(WARM, *SWING[:4]) + [(269, 119, 3146, 208), (300, 150, 3300, 208, "battle"),
                                           (290, 160, 3300, 208, "scene"),
                                           (290, 140, 3286, 208)] + visits(*SWING[:2])
        verdict = judge(rows)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["maps_unjudged"],
                         {"208": "2 field sample(s) past warm-up, fewer than 4"})

    def test_map_208s_straddle_at_a_500_frame_cadence_does_not_fail(self):
        # The refresh's +20.3 MB spans 5 field samples at 500 frames; a floor
        # window is FLOOR_FRAMES of field time (20 samples), so it holds.
        before = [(250, 100, 3000, 1)] * 24 + [(250, 100, 3000, 2)] * 24
        visit = ([(269, 119, 3146, 208)] * 20 + [(300, 150, 3300, 208, "battle")] * 10
                 + [(290, 160, 3300, 208, "scene")] * 10 + [(290, 140, 3286, 208)] * 5
                 + [(269, 119, 3146, 208)] * 20)
        after = [(250, 100, 3000, 1)] * 24 + [(250, 100, 3000, 2)] * 24
        verdict = judge([(250, 100, 3000, 9)] * 10 + before + visit + after,
                        every=500, warmup=5000)
        self.assertEqual(verdict["floor_samples"], FLOOR_FRAMES // 500)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["maps"]["208"]["static_growth_mb"], 0.0)

    def test_a_transient_on_a_re_entry_sample_does_not_fail(self):
        rows = visits(WARM, *SWING[:2]) + [(380.3, 230.3, 2940, 1)] + visits(
            (1, 210, 2800, 3), *SWING[1:])
        verdict = judge(rows)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["godot_static"]["growth_mb"], 0.0)

    def test_no_floored_map_cannot_breach(self):
        # Brief visits only: nothing per map to judge, which is no breach;
        # the bracket decides.
        rows = visits(WARM, *[(m, 100, 2800, 3) for m in range(1, 6)])
        verdict = judge(rows)
        self.assertEqual(verdict["godot_static"]["maps_judged"], 0)
        self.assertTrue(verdict["pass"], verdict)
        self.assertFalse(judge(rows, probes=bracket(end_static=140))["pass"])


class Reported(unittest.TestCase):
    """Drift and coverage are reported for the reader; the bracket judges."""

    def test_a_thin_static_leak_shows_in_the_drift(self):
        plan = [(m, 100, 2800, 4) for m in range(1, 6)] * 2
        verdict = judge(visits(WARM, *plan, static_leak=lambda i: 0.5 * i),
                        probes=bracket(end_static=120))
        self.assertTrue(verdict["godot_static"]["pass"])
        self.assertEqual(verdict["static_drift"]["rate_kb_per_kframe"], 204.8)
        self.assertFalse(verdict["static_drift"]["within_bound"])
        self.assertFalse(verdict["bracket"]["pass"])

    def test_the_drift_reads_a_uniform_leak_exactly(self):
        plan = SWING * 3
        span = len(plan) * 4 - 4
        rows = visits(WARM, *plan, static_leak=lambda i: 15 * max(0, i - 1) / span)
        self.assertEqual(judge(rows)["static_drift"]["drift_mb"], 15.0)

    def test_an_objects_leak_over_once_entered_maps_shows_in_the_object_drift(self):
        plan = [(m, 100, 2800, 8) for m in range(1, 11)]
        verdict = judge(visits(WARM, *plan, object_leak=lambda i: 4 * i),
                        probes=bracket(end_objects=2800 + 320))
        self.assertTrue(verdict["objects"]["pass"])
        self.assertFalse(verdict["object_drift"]["within_bound"])
        self.assertFalse(verdict["bracket"]["pass"])
        self.assertEqual(verdict["bracket"]["object_growth"], 320)

    def test_coverage_gaps_are_reported_not_judged(self):
        rows = visits(WARM, *SWING) + visits(*[(30 + m, 100, 2800, 1) for m in range(6)])
        verdict = judge(rows)
        self.assertFalse(verdict["coverage"]["within_limit"])
        self.assertGreater(verdict["coverage"]["widest_gap_frames"], GAP_LIMIT_FRAMES)
        self.assertTrue(verdict["pass"], verdict)


class Cadence(unittest.TestCase):
    def test_a_cadence_too_coarse_for_the_floor_window_is_refused(self):
        self.assertIsNone(cadence_error(2500))
        self.assertIsNone(cadence_error(FLOOR_FRAMES // 2))
        self.assertIn("coarser", cadence_error(25000))
        verdict = judge(visits(WARM, *SWING), every=25000, warmup=25001)
        self.assertFalse(verdict["pass"])
        self.assertIn("coarser", verdict["reason"])

    def test_the_floor_window_is_frames_not_samples(self):
        rows = visits(WARM, *SWING)
        self.assertEqual(judge(rows)["floor_samples"], FLOOR_FRAMES // EVERY)
        self.assertEqual(judge(rows, every=500, warmup=501)["floor_samples"],
                         FLOOR_FRAMES // 500)


class RustHeap(unittest.TestCase):
    def test_a_growing_rust_heap_fails_whatever_the_map(self):
        verdict = rss_verdict(samples((250, 100, 2800, 2), (250, 100, 2800, 2),
                                      (380, 210, 2800, 1), (290, 100, 2800, 2),
                                      (420, 210, 2800, 1), every=1000), 2000, 16,
                              floor_frames=2000, gap_limit_frames=2000, probes=bracket())
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["rust_heap"]["pass"])
        self.assertEqual(verdict["rust_heap"]["growth_mb"], 60.0)
        self.assertEqual(verdict["rust_heap"]["slope_kb_per_kframe"], 20480.0)

    def test_a_transient_rust_heap_peak_counts_even_if_it_falls_back(self):
        rows = visits(WARM, (1, 100, 1, 2)) + [(300, 100, 1, 1)] + visits(*SWING)
        verdict = judge(rows)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["rust_heap"]["pass"])


class Inputs(unittest.TestCase):
    def test_the_verdict_leaves_the_report_samples_untouched(self):
        rows = samples(*visits(WARM, *SWING))
        probes = bracket()
        before = ([dict(item) for item in rows], json.dumps(probes))
        rss_verdict(rows, WARMUP, 16, probes=probes)
        self.assertEqual(([dict(item) for item in rows], json.dumps(probes)), before)

    def test_missing_or_unreadable_samples_fail_rather_than_pass(self):
        self.assertFalse(rss_verdict(None, 0, 16, probes=bracket())["pass"])
        flat = visits(WARM, *SWING)
        self.assertFalse(rss_verdict(samples(*flat), 0, 16, probes=bracket())["pass"])
        self.assertFalse(rss_verdict(samples(*flat[:2]), WARMUP, 16, probes=bracket())["pass"])
        unreadable = samples(*flat)
        unreadable[2]["rss_kb"] = -1
        self.assertFalse(rss_verdict(unreadable, WARMUP, 16, probes=bracket())["pass"])
        for key in ("static_kb", "map", "activity", "rust_live_bytes"):
            missing = samples(*flat)
            del missing[1][key]
            verdict = rss_verdict(missing, WARMUP, 16, probes=bracket())
            self.assertFalse(verdict["pass"], key)
            self.assertIn("unreadable", verdict["reason"])


if __name__ == "__main__":
    unittest.main()
