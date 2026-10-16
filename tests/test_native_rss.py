"""The #92 memory verdict, `tools/native_rss.py`, on synthetic and recorded samples.

    PYTHONPATH=. python3 -m unittest tests.test_native_rss -v

Every negative control names the part of the verdict that must fail it, and
every positive control the reason it must not: (a) growth at re-entry, (b) a
leak spread thinly over maps, (c) the shipped #92 build's recorded run, (d)
n92's two-sample scene-and-battle straddle, (e) a ramp in the run's tail, (f)
a transient on a re-entry sample, (g) the drift's limit of detection.
"""

import json
import unittest
from pathlib import Path

from tools.native_rss import FLOOR_SAMPLES, rss_verdict

ROOT = Path(__file__).resolve().parents[1]
CHAPTER14 = ROOT / "tests" / "native_rss_chapter14.json"


def samples(*rows, every=1000, step_ms=1000):
    """One sample per row of (rss_mb, static_mb, objects, map) every `every`
    frames; the first row is frame `every`."""
    return [{"frame": every * (i + 1), "elapsed_ms": step_ms * (i + 1),
             "rss_kb": int(rss * 1024), "static_kb": int(static * 1024), "objects": objects,
             "map": map_id}
            for i, (rss, static, objects, map_id) in enumerate(rows)]


def visits(*plan, extension=150, leak=lambda index: 0.0):
    """Rows for `plan`, a list of (map, static_mb, objects, count): `count`
    samples on the map. `leak(index)` adds MB to static and RSS at the
    sample's index; RSS is static plus `extension`."""
    rows = []
    for map_id, static, objects, count in plan:
        for _ in range(count):
            extra = leak(len(rows))
            rows.append((static + extra + extension, static + extra, objects, map_id))
    return rows


# Warm-up is frames < 2000: the first row of every run below.
WARMUP = 2000
WARM = (9, 100, 2800, 1)


class Floors(unittest.TestCase):
    def test_a_map_swing_with_no_leak_passes(self):
        # Map 1's images take 110 MB of Godot static memory; map 2's, none.
        rows = visits(WARM, (1, 210, 2800, 4), (2, 100, 2800, 4), (1, 210, 2800, 4),
                      (2, 100, 2800, 4))
        verdict = rss_verdict(samples(*rows), WARMUP, 16)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["godot_static"]["growth_mb"], 0.0)
        self.assertEqual(verdict["maps"]["1"]["samples"], 8)
        self.assertEqual(verdict["coverage"]["uncovered_tail_frames"], 0)

    def test_a_growing_floor_at_re_entry_fails(self):
        # (a) Map 1 is 10 MB bigger each time it is entered.
        rows = visits(WARM, (1, 100, 2800, 4), (2, 100, 2800, 4), (1, 110, 2800, 4),
                      (2, 100, 2800, 4), (1, 120, 2800, 4), (2, 100, 2800, 4))
        verdict = rss_verdict(samples(*rows), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["godot_static"]["pass"])
        self.assertEqual((verdict["godot_static"]["worst_map"],
                          verdict["godot_static"]["growth_mb"]), ("1", 20.0))
        self.assertTrue(verdict["extension"]["pass"])
        self.assertTrue(verdict["coverage"]["pass"])

    def test_a_leak_spread_thinly_over_maps_fails_the_drift(self):
        # (b) 0.5 MB a sample on every map. Each map is re-entered 20 samples
        # after its first visit, so its floor grows under the 16 MB bound,
        # but pooled over the run the leak is well past it.
        plan = [(m, 100, 2800, 4) for m in range(1, 6)] * 2
        rows = visits(WARM, *plan, leak=lambda index: 0.5 * index)
        verdict = rss_verdict(samples(*rows), WARMUP, 16)
        self.assertTrue(verdict["godot_static"]["pass"], verdict["godot_static"])
        self.assertLessEqual(verdict["godot_static"]["growth_mb"], 16)
        self.assertFalse(verdict["static_drift"]["pass"])
        self.assertEqual(verdict["static_drift"]["drift_mb"], 18.0)
        self.assertEqual(verdict["static_drift"]["rate_kb_per_kframe"], 512.0)
        self.assertFalse(verdict["pass"])

    def test_two_samples_straddling_a_scene_and_battle_do_not_fail(self):
        # (d) n92's map 208: entered once, its second sample in a scene and
        # battle holding 21 MB and 140 more objects. Fewer samples than a
        # floor window: unjudged, and nothing it held persists.
        rows = visits(WARM, (1, 100, 3000, 6)) + [(269, 119, 3146, 208), (290, 140, 3286, 208)] \
            + visits((1, 100, 3000, 6))
        verdict = rss_verdict(samples(*rows), WARMUP, 16)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["maps_unjudged"],
                         {"208": f"2 sample(s) past warm-up, fewer than {FLOOR_SAMPLES}"})

    def test_a_ramp_in_the_runs_tail_fails(self):
        # (e) The reviewer's tail: the run ends in one long visit to a map not
        # seen before, and static memory climbs 200 MB through it. The old
        # entry-sample verdict compared nothing after the last re-entry.
        head = visits(WARM, (1, 100, 2800, 4), (2, 100, 2800, 4), (1, 100, 2800, 4))
        tail = [(250 + 20 * i, 100 + 20 * i, 2800, 374) for i in range(11)]
        verdict = rss_verdict(samples(*head, *tail), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["godot_static"]["pass"])
        self.assertEqual(verdict["godot_static"]["worst_map"], "374")
        self.assertTrue(verdict["coverage"]["pass"])
        # The same tail in objects: +2,000 fails the object floor.
        tail = [(250, 100, 2800 + 200 * i, 374) for i in range(11)]
        verdict = rss_verdict(samples(*head, *tail), WARMUP, 16)
        self.assertFalse(verdict["objects"]["pass"])
        self.assertEqual(verdict["objects"]["worst_map"], "374")

    def test_a_tail_of_short_visits_fails_coverage(self):
        # (e) A tail made of visits too short to floor is compared with
        # nothing; past one floor window it fails instead of passing.
        head = visits(WARM, (1, 100, 2800, 4), (2, 100, 2800, 4), (1, 100, 2800, 4))
        tail = visits(*[(30 + m, 100 + 20 * m, 2800, 1) for m in range(8)])
        verdict = rss_verdict(samples(*head, *tail), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["coverage"]["pass"])
        self.assertEqual((verdict["coverage"]["uncovered_tail_frames"],
                          verdict["coverage"]["tail_limit_frames"]), (8000, 4000))
        self.assertTrue(verdict["godot_static"]["pass"])
        # A tail within one window passes.
        tail = visits(*[(30 + m, 100, 2800, 1) for m in range(4)])
        self.assertTrue(rss_verdict(samples(*head, *tail), WARMUP, 16)["coverage"]["pass"])

    def test_a_transient_on_a_re_entry_sample_does_not_fail(self):
        # (f) The first sample after re-entering map 1 lands in a 20.3 MB
        # transient (n92 map 368 @3,650,000 in the reviewer's injection).
        rows = visits(WARM, (1, 100, 2800, 4), (2, 100, 2800, 4))
        rows += [(270.3, 120.3, 2940, 1)] + visits((1, 100, 2800, 3), (2, 100, 2800, 4))
        verdict = rss_verdict(samples(*rows), WARMUP, 16)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["godot_static"]["growth_mb"], 0.0)

    def test_the_drift_detects_a_uniform_leak_just_over_the_bound(self):
        # (g) The limit of detection: a uniform leak of 15 MB over the
        # compared span passes and 17 MB fails, on two maps re-entered all
        # through the run. On a recorded run the floors' own noise adds to it
        # (n92: 7.0 MB of drift with no leak injected).
        plan = [(m, 100, 2800, 4) for m in (1, 2)] * 6

        def run(total_mb):
            rows = visits(WARM, *plan, leak=lambda i: total_mb * max(0, i - 1) / (len(plan) * 4 - 4))
            return rss_verdict(samples(*rows), WARMUP, 16)

        self.assertTrue(run(15)["pass"], run(15))
        failed = run(17)
        self.assertFalse(failed["pass"])
        self.assertFalse(failed["static_drift"]["pass"])
        self.assertTrue(failed["godot_static"]["pass"])
        self.assertEqual(failed["static_drift"]["drift_mb"], 17.0)

    def test_a_window_with_no_map_floored_fails(self):
        rows = visits(WARM, *[(m, 100, 2800, 3) for m in range(1, 6)])
        verdict = rss_verdict(samples(*rows), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertIn(f"no map has {FLOOR_SAMPLES} samples", verdict["reason"])


class Extension(unittest.TestCase):
    def test_a_growing_extension_heap_fails_whatever_the_map(self):
        # The shipped #92 shape: the Rust heap grows while maps swing.
        verdict = rss_verdict(samples((250, 100, 2800, 2), (250, 100, 2800, 2),
                                      (380, 210, 2800, 1), (290, 100, 2800, 2),
                                      (420, 210, 2800, 1)), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["extension"]["pass"])
        self.assertEqual(verdict["extension"]["growth_mb"], 60.0)
        self.assertEqual(verdict["extension"]["slope_kb_per_kframe"], 20480.0)

    def test_a_transient_extension_peak_counts_even_if_it_falls_back(self):
        rows = visits(WARM, (1, 100, 1, 2)) + [(300, 100, 1, 1)] + visits((1, 100, 1, 4))
        verdict = rss_verdict(samples(*rows), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["extension"]["pass"])

    def test_the_shipped_92_build_fails_on_its_heap_and_the_fix_passes(self):
        # (c) Recorded 2026-10-15 over the chapter-14 prefix, same options.
        runs = json.loads(CHAPTER14.read_text())
        shipped = rss_verdict(runs["shipped"], 20000, 16)
        self.assertFalse(shipped["pass"])
        self.assertFalse(shipped["extension"]["pass"])
        self.assertEqual(shipped["extension"]["growth_mb"], 115.5)
        fixed = rss_verdict(runs["fixed"], 20000, 16)
        self.assertTrue(fixed["pass"], fixed)
        self.assertEqual(fixed["extension"]["growth_mb"], 4.3)


class Inputs(unittest.TestCase):
    def test_objects_rising_on_a_floor_fail(self):
        rows = visits(WARM, (1, 100, 10, 4), (2, 100, 10, 4), (1, 100, 300, 4))
        verdict = rss_verdict(samples(*rows), WARMUP, 16)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["objects"]["pass"])
        self.assertEqual(verdict["objects"]["growth"], 290)

    def test_the_verdict_leaves_the_report_samples_untouched(self):
        rows = samples(*visits(WARM, (1, 200, 10, 5)))
        before = [dict(item) for item in rows]
        rss_verdict(rows, WARMUP, 16)
        self.assertEqual(rows, before)

    def test_missing_or_unreadable_samples_fail_rather_than_pass(self):
        self.assertFalse(rss_verdict(None, 0, 16)["pass"])
        flat = visits(WARM, (1, 1, 1, 5))
        # No warm-up sample, too few samples past warm-up, an unreadable RSS.
        self.assertFalse(rss_verdict(samples(*flat), 0, 16)["pass"])
        self.assertFalse(rss_verdict(samples(*flat[:2]), WARMUP, 16)["pass"])
        unreadable = samples(*flat)
        unreadable[2]["rss_kb"] = -1
        self.assertFalse(rss_verdict(unreadable, WARMUP, 16)["pass"])
        for key in ("static_kb", "map"):
            missing = samples(*flat)
            del missing[1][key]
            verdict = rss_verdict(missing, WARMUP, 16)
            self.assertFalse(verdict["pass"], key)
            self.assertIn("unreadable", verdict["reason"])


if __name__ == "__main__":
    unittest.main()
