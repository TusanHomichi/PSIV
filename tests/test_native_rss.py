"""The #92 memory verdict, `tools/native_rss.py`, on synthetic and recorded samples.

    PYTHONPATH=. python3 -m unittest tests.test_native_rss -v

Godot's memory is compared only between field samples (`activity ==
"field"`), per map, on floors over `FLOOR_FRAMES` of observed field time.
Every negative control names the part of the verdict that must fail it, and
every positive control the reason it must not: (a) growth at re-entry, (b) a
static leak spread thinly over maps, (c) the shipped #92 build's recorded run,
(d) n92's map-208 straddle, at 2,500- and 500-frame cadence, (e) a step in
the run's tail, (f) a transient on a re-entry sample, (g) the drift's limit of
detection; then the 2026-10-16 review's steps in coverage gaps, an
objects-only leak over once-entered maps, and cadences too coarse to judge.
"""

import json
import unittest
from pathlib import Path

from tools.native_rss import FLOOR_FRAMES, GAP_LIMIT_FRAMES, cadence_error, rss_verdict

ROOT = Path(__file__).resolve().parents[1]
CHAPTER14 = ROOT / "tests" / "native_rss_chapter14.json"
EVERY = 2500
# Warm-up ends at 5,000: the first row (frame 2,500) of every run below.
WARMUP = 5000


def samples(*rows, every=EVERY):
    """One sample per row of (rss_mb, static_mb, objects, map[, activity])
    every `every` frames (activity defaults to the field); the first row is
    frame `every`."""
    out = []
    for i, row in enumerate(rows):
        rss, static, objects, map_id, *rest = row
        out.append({"frame": every * (i + 1), "elapsed_ms": 1000 * (i + 1),
                    "rss_kb": int(rss * 1024), "static_kb": int(static * 1024),
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


def judge(rows, **options):
    return rss_verdict(samples(*rows, every=options.pop("every", EVERY)),
                       options.pop("warmup", WARMUP), 16, **options)


class Floors(unittest.TestCase):
    def test_a_map_swing_with_no_leak_passes(self):
        verdict = judge(visits(WARM, *SWING))
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["floor_samples"], 4)
        self.assertEqual(verdict["godot_static"]["growth_mb"], 0.0)
        self.assertEqual(verdict["maps"]["1"]["field_samples"], 12)
        self.assertLessEqual(verdict["coverage"]["widest_gap_frames"], GAP_LIMIT_FRAMES)

    def test_battles_and_scenes_are_never_compared(self):
        # A battle on map 1 holds 40 MB and 300 objects of its own art.
        rows = visits(WARM, (1, 210, 2800, 4), (1, 250, 3100, 3, "battle"),
                      (2, 100, 2800, 4), (1, 210, 2800, 4), (1, 260, 3200, 2, "scene"),
                      (2, 100, 2800, 4), (1, 210, 2800, 4), (2, 100, 2800, 4))
        verdict = judge(rows)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["activities"], {"battle": 3, "field": 24, "scene": 2})

    def test_a_growing_floor_at_re_entry_fails(self):
        # (a) Map 1 is 10 MB bigger each time it is entered.
        rows = visits(WARM, (1, 100, 2800, 4), (2, 100, 2800, 4), (1, 110, 2800, 4),
                      (2, 100, 2800, 4), (1, 120, 2800, 4), (2, 100, 2800, 4))
        verdict = judge(rows)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["godot_static"]["pass"])
        self.assertEqual((verdict["godot_static"]["worst_map"],
                          verdict["godot_static"]["growth_mb"]), ("1", 20.0))
        self.assertTrue(verdict["extension"]["pass"])
        self.assertTrue(verdict["coverage"]["pass"])

    def test_a_static_leak_spread_thinly_over_maps_fails_the_drift(self):
        # (b) 0.5 MB a sample on every map. Each map is re-entered 20 samples
        # after its first visit, so its floor grows +10 MB, under the 16 MB
        # bound; pooled over the run the leak is past it.
        plan = [(m, 100, 2800, 4) for m in range(1, 6)] * 2
        verdict = judge(visits(WARM, *plan, static_leak=lambda i: 0.5 * i))
        self.assertTrue(verdict["godot_static"]["pass"], verdict["godot_static"])
        self.assertEqual(verdict["godot_static"]["growth_mb"], 10.0)
        self.assertFalse(verdict["static_drift"]["pass"])
        self.assertEqual(verdict["static_drift"]["rate_kb_per_kframe"], 204.8)
        self.assertGreater(verdict["static_drift"]["drift_mb"], 16)
        self.assertTrue(verdict["coverage"]["pass"], verdict["coverage"])
        self.assertFalse(verdict["pass"])

    def test_map_208s_straddle_does_not_fail(self):
        # (d) n92 at 2,500-frame cadence: map 208's two field samples, the
        # second during a map refresh holding +20.3 MB and +140 objects, with
        # a battle and a scene between them. Two field samples are fewer than
        # a floor window: listed unjudged, nothing compared.
        rows = visits(WARM, *SWING[:4]) + [(269, 119, 3146, 208), (300, 150, 3300, 208, "battle"),
                                           (290, 160, 3300, 208, "scene"),
                                           (290, 140, 3286, 208)] + visits(*SWING[:2])
        verdict = judge(rows)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["maps_unjudged"],
                         {"208": "2 field sample(s) past warm-up, fewer than 4"})

    def test_map_208s_straddle_at_a_500_frame_cadence_does_not_fail(self):
        # (d) The same visit sampled every 500 frames: the map refresh's
        # +20.3 MB now spans 5 field samples (2,500 frames) of a long visit.
        # A floor window is FLOOR_FRAMES of field time, 20 samples here, so
        # the transient is still inside every window's minimum.
        before = [(250, 100, 3000, 1)] * 24 + [(250, 100, 3000, 2)] * 24
        visit = ([(269, 119, 3146, 208)] * 20 + [(300, 150, 3300, 208, "battle")] * 10
                 + [(290, 160, 3300, 208, "scene")] * 10 + [(290, 140, 3286, 208)] * 5
                 + [(269, 119, 3146, 208)] * 20)
        after = [(250, 100, 3000, 1)] * 24 + [(250, 100, 3000, 2)] * 24
        verdict = judge([(250, 100, 3000, 9)] * 10 + before + visit + after,
                        every=500, warmup=5000)
        self.assertEqual(verdict["floor_samples"], FLOOR_FRAMES // 500)
        self.assertTrue(verdict["pass"], {k: verdict[k] for k in
                                          ("godot_static", "objects", "coverage")})
        self.assertEqual(verdict["maps"]["208"]["static_growth_mb"], 0.0)

    def test_a_step_in_the_runs_tail_fails(self):
        # (e) The run ends in one long visit to a map not seen before, and
        # Godot's static memory steps up 200 MB in it and stays there.
        head = visits(WARM, *SWING[:3])
        tail = [(250, 100, 2800, 374)] * 6 + [(450, 300, 2800, 374)] * 8
        verdict = judge(head + tail)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["godot_static"]["pass"])
        self.assertEqual((verdict["godot_static"]["worst_map"],
                          verdict["godot_static"]["growth_mb"]), ("374", 200.0))
        # The same step in objects: +2,000.
        tail = [(250, 100, 2800, 374)] * 6 + [(250, 100, 4800, 374)] * 8
        verdict = judge(head + tail)
        self.assertFalse(verdict["objects"]["pass"])
        self.assertEqual(verdict["objects"]["worst_map"], "374")

    def test_a_step_in_a_tail_too_short_to_floor_fails_coverage(self):
        # (e) The step lands in the last frames, after the last floor window
        # could start: no floor can see it, so the uncovered tail fails.
        head = visits(WARM, *SWING)
        tail = visits(*[(30 + m, 300, 2800, 1) for m in range(6)])
        verdict = judge(head + tail)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["coverage"]["pass"])
        self.assertGreater(verdict["coverage"]["widest_gap_frames"], GAP_LIMIT_FRAMES)
        self.assertTrue(verdict["godot_static"]["pass"])

    def test_a_transient_on_a_re_entry_sample_does_not_fail(self):
        # (f) The first field sample after re-entering map 1 lands in a
        # 20.3 MB transient.
        rows = visits(WARM, *SWING[:2]) + [(380.3, 230.3, 2940, 1)] + visits(
            (1, 210, 2800, 3), *SWING[1:])
        verdict = judge(rows)
        self.assertTrue(verdict["pass"], verdict)
        self.assertEqual(verdict["godot_static"]["growth_mb"], 0.0)

    def test_the_drift_detects_a_uniform_leak_over_the_bound(self):
        # (g) The limit of detection on clean data: a uniform leak of 15 MB
        # over the floors' frames passes and 17 MB fails. A recorded run's
        # own floor noise adds to it (n92: 4.6 MB of drift, nothing injected).
        plan = SWING * 3

        def run(total_mb):
            # The floors span samples 1..(N - 4): spread the leak over those.
            span = len(plan) * 4 - 4
            rows = visits(WARM, *plan, static_leak=lambda i: total_mb * max(0, i - 1) / span)
            return judge(rows)

        self.assertTrue(run(15)["pass"], run(15)["static_drift"])
        self.assertEqual(run(15)["static_drift"]["drift_mb"], 15.0)
        failed = run(17)
        self.assertFalse(failed["pass"])
        self.assertFalse(failed["static_drift"]["pass"])
        self.assertTrue(failed["godot_static"]["pass"])

    def test_an_objects_leak_over_once_entered_maps_fails_the_object_drift(self):
        # Ten maps, each entered once for 8 samples, while 4 objects leak a
        # sample: each map's floor grows +16 objects, the run leaks ~300.
        plan = [(m, 100, 2800, 8) for m in range(1, 11)]
        verdict = judge(visits(WARM, *plan, object_leak=lambda i: 4 * i))
        self.assertTrue(verdict["objects"]["pass"], verdict["objects"])
        self.assertEqual(verdict["objects"]["growth"], 16)
        self.assertFalse(verdict["object_drift"]["pass"])
        self.assertGreater(verdict["object_drift"]["drift"], 256)
        self.assertFalse(verdict["pass"])

    def test_a_window_with_no_map_floored_fails(self):
        verdict = judge(visits(WARM, *[(m, 100, 2800, 3) for m in range(1, 6)]))
        self.assertFalse(verdict["pass"])
        self.assertIn("no map has 4 field samples", verdict["reason"])


class Coverage(unittest.TestCase):
    """The 2026-10-16 review: a persistent step inside a stretch no floor can
    see passed. Every gap, head and internal and tail, is judged in frames."""

    def gap_run(self, where):
        """A stretch of brief visits (one sample each, never floored) at
        `where`, with maps re-entered on either side that never cross it
        (maps 1 and 2 before, 3 and 4 after); a +200 MB step lands inside the
        stretch and stays. No floor brackets it, so no part but coverage can
        see it."""
        brief = [(40 + m, 100, 2800, 1) for m in range(6)]
        before = [(1, 210, 2800, 4), (2, 100, 2800, 4)]
        after = [(3, 210, 2800, 4), (4, 100, 2800, 4)]
        plan = {"head": brief + after * 3, "middle": before * 2 + brief + after * 2,
                "tail": before * 3 + brief}[where]
        step_at = {"head": 3, "middle": 19, "tail": 28}[where]
        return judge(visits(WARM, *plan, static_leak=lambda i: 200 * (i >= step_at)))

    def test_a_step_in_a_head_gap_fails(self):
        verdict = self.gap_run("head")
        self.assertFalse(verdict["coverage"]["pass"])
        self.assertEqual(verdict["coverage"]["gaps"][0]["from"], 5000)

    def test_a_step_in_an_internal_gap_fails(self):
        verdict = self.gap_run("middle")
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["coverage"]["pass"])
        self.assertTrue(verdict["godot_static"]["pass"])

    def test_a_step_bracketed_by_a_re_entered_map_is_seen(self):
        # The control for the three above: when map 1 is re-entered after
        # the brief stretch, its floors see the step and the stretch is no
        # gap.
        brief = [(40 + m, 100, 2800, 1) for m in range(6)]
        cycle = [(1, 210, 2800, 4), (2, 100, 2800, 4)]
        verdict = judge(visits(WARM, *cycle * 2, *brief, *cycle * 2,
                               static_leak=lambda i: 200 * (i >= 19)))
        self.assertTrue(verdict["coverage"]["pass"], verdict["coverage"])
        self.assertFalse(verdict["godot_static"]["pass"])
        self.assertEqual(verdict["godot_static"]["growth_mb"], 200.0)

    def test_a_step_in_the_tail_gap_fails(self):
        verdict = self.gap_run("tail")
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["coverage"]["pass"])
        self.assertEqual(verdict["coverage"]["gaps"][-1]["to"], verdict["last_frame"])


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


class Extension(unittest.TestCase):
    def test_a_growing_extension_heap_fails_whatever_the_map(self):
        # The shipped #92 shape: the Rust heap grows while maps swing.
        verdict = rss_verdict(samples((250, 100, 2800, 2), (250, 100, 2800, 2),
                                      (380, 210, 2800, 1), (290, 100, 2800, 2),
                                      (420, 210, 2800, 1), every=1000), 2000, 16,
                              floor_frames=2000, gap_limit_frames=2000)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["extension"]["pass"])
        self.assertEqual(verdict["extension"]["growth_mb"], 60.0)
        self.assertEqual(verdict["extension"]["slope_kb_per_kframe"], 20480.0)

    def test_a_transient_extension_peak_counts_even_if_it_falls_back(self):
        rows = visits(WARM, (1, 100, 1, 2)) + [(300, 100, 1, 1)] + visits(*SWING)
        verdict = judge(rows)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["extension"]["pass"])

    def test_the_shipped_92_build_fails_on_its_heap_and_the_fix_does_not(self):
        # (c) Recorded 2026-10-15 over the chapter-14 prefix, same options.
        # That prefix has 12 field samples, 4 of them on one map: Godot's
        # memory gets one floor and no coverage, so both arms fail coverage;
        # the heap is what separates them.
        runs = json.loads(CHAPTER14.read_text())
        shipped = rss_verdict(runs["shipped"], 20000, 16)
        fixed = rss_verdict(runs["fixed"], 20000, 16)
        self.assertFalse(shipped["extension"]["pass"])
        self.assertEqual(shipped["extension"]["growth_mb"], 115.5)
        self.assertTrue(fixed["extension"]["pass"])
        self.assertEqual(fixed["extension"]["growth_mb"], 4.3)
        for verdict in (shipped, fixed):
            self.assertFalse(verdict["pass"])
            self.assertFalse(verdict["coverage"]["pass"])
            self.assertEqual(verdict["activities"]["field"], 11)


class Inputs(unittest.TestCase):
    def test_objects_rising_on_a_floor_fail(self):
        rows = visits(WARM, (1, 100, 10, 4), (2, 100, 10, 4), (1, 100, 300, 4),
                      (2, 100, 10, 4))
        verdict = judge(rows)
        self.assertFalse(verdict["pass"])
        self.assertFalse(verdict["objects"]["pass"])
        self.assertEqual(verdict["objects"]["growth"], 290)

    def test_the_verdict_leaves_the_report_samples_untouched(self):
        rows = samples(*visits(WARM, *SWING))
        before = [dict(item) for item in rows]
        rss_verdict(rows, WARMUP, 16)
        self.assertEqual(rows, before)

    def test_missing_or_unreadable_samples_fail_rather_than_pass(self):
        self.assertFalse(rss_verdict(None, 0, 16)["pass"])
        flat = visits(WARM, *SWING)
        # No warm-up sample, too few samples past warm-up, an unreadable RSS.
        self.assertFalse(rss_verdict(samples(*flat), 0, 16)["pass"])
        self.assertFalse(rss_verdict(samples(*flat[:2]), WARMUP, 16)["pass"])
        unreadable = samples(*flat)
        unreadable[2]["rss_kb"] = -1
        self.assertFalse(rss_verdict(unreadable, WARMUP, 16)["pass"])
        for key in ("static_kb", "map", "activity"):
            missing = samples(*flat)
            del missing[1][key]
            verdict = rss_verdict(missing, WARMUP, 16)
            self.assertFalse(verdict["pass"], key)
            self.assertIn("unreadable", verdict["reason"])


if __name__ == "__main__":
    unittest.main()
