"""ROM-free controls for the scene-load timing observation lane."""

import copy
import unittest

from oracle.scene_loads import (
    FRAME_CYCLES, ROUTINES, absolute, annotate, attach_work, check_coverage, spans,
)


def boundary(routine, end, frame, cycles):
    return {"frame": str(frame), "pc": f"{ROUTINES[routine][int(end)]:06X}",
            "cycles": str(cycles)}


def frame(number, count, seed="12345678", panel="010D"):
    return {"frame": str(number), "main_frame_count": str(count),
            "rng_seed": seed, "panel_index": panel}


class SceneLoadObservations(unittest.TestCase):
    def test_nested_decompressors_keep_their_own_boundaries(self):
        trace = [boundary("panel", False, 10, 500),
                 boundary("nemesis", False, 10, 1000),
                 boundary("nemesis", True, 12, 2000),
                 boundary("enigma", False, 12, 2100),
                 boundary("enigma", True, 12, 3000),
                 boundary("panel", True, 12, 4000)]
        loads = annotate(spans(trace), [frame(10, 20), frame(11, 20), frame(12, 20)])
        self.assertEqual(len(loads), 1)
        load = loads[0]
        self.assertEqual(load["master_cycles"], 2 * FRAME_CYCLES + 3500)
        self.assertEqual(load["held_frames"], [11, 12])
        self.assertEqual(load["query"], "panel:10d")
        nemesis, enigma = load["components"]
        self.assertEqual(nemesis["lag_frames"], 2)
        self.assertEqual(enigma["lag_frames"], 0)
        self.assertEqual(enigma["master_cycles"], 900)

    def test_elapsed_frames_are_not_all_lag_frames(self):
        trace = [boundary("load_art", False, 10, 100),
                 boundary("nemesis", False, 10, 200),
                 boundary("nemesis", True, 13, 200),
                 boundary("load_art", True, 13, 400)]
        loads = annotate(spans(trace), [frame(10, 65535), frame(11, 65535),
                                       frame(12, 65535), frame(13, 0, "ABCDEF12")])
        self.assertEqual(loads[0]["lag_frames"], 2)
        self.assertEqual(loads[0]["query"], "art:1d712c")

    def test_portrait_work_outside_a_measured_load_is_ignored(self):
        self.assertEqual(spans([boundary("nemesis", False, 10, 0),
                                boundary("nemesis", True, 12, 0)]), [])

    def test_instruction_overshoot_survives_frame_rebasing(self):
        before = boundary("panel", False, 10, FRAME_CYCLES + 20)
        after = boundary("panel", True, 11, 100)
        self.assertEqual(absolute(after) - absolute(before), 80)
        self.assertEqual(spans([before, after])[0]["master_cycles"], 80)

    def test_missing_or_misordered_boundary_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "ends inside"):
            spans([boundary("panel", False, 10, 0)])
        with self.assertRaisesRegex(ValueError, "without an entry"):
            spans([boundary("panel", True, 10, 0)])
        with self.assertRaisesRegex(ValueError, "unmatched"):
            spans([boundary("panel", False, 10, 0), boundary("nemesis", True, 10, 30)])
        with self.assertRaisesRegex(ValueError, "execution order"):
            spans([boundary("panel", False, 10, 40), boundary("panel", True, 10, 30)])

    def test_a_changed_seed_invalidates_a_claimed_lag_frame(self):
        loads = spans([boundary("panel", False, 10, 0), boundary("panel", True, 11, 0)])
        rows = [frame(10, 20), frame(11, 20)]
        self.assertEqual(annotate(copy.deepcopy(loads), rows)[0]["lag_frames"], 1)
        rows[1]["rng_seed"] = "BAD0BEEF"
        with self.assertRaisesRegex(ValueError, "seed changed"):
            annotate(loads, rows)

    def test_two_panel_returns_in_one_frame_cannot_borrow_the_last_id(self):
        loads = spans([boundary("panel", False, 10, 0), boundary("panel", True, 10, 100),
                       boundary("panel", False, 10, 200), boundary("panel", True, 10, 300)])
        with self.assertRaisesRegex(ValueError, "RAM id is ambiguous"):
            annotate(loads, [frame(10, 20)])

    def test_decoded_inputs_must_select_the_observed_decompressors(self):
        call = {"routine": "panel", "query": "panel:10d", "master_cycles": 100,
                "components": [{"routine": "nemesis", "master_cycles": 60},
                               {"routine": "enigma", "master_cycles": 30}]}
        work = {"art": {"source": "0x123456"}, "mappings": [{"source": "0x234567"}]}
        attach_work(call, work)
        self.assertEqual(call["outside_decompressors_master_cycles"], 10)
        self.assertEqual(call["components"][0]["input"], work["art"])
        with self.assertRaisesRegex(ValueError, "different calls"):
            attach_work(call, {"art": None, "mappings": work["mappings"]})

    def test_a_later_panel_id_write_invalidates_frame_end_identification(self):
        call = spans([boundary("panel", False, 10, 0), boundary("panel", True, 12, 0)])
        rows = [frame(10, 20), frame(11, 20), frame(12, 20, panel="0143")]
        with self.assertRaisesRegex(ValueError, "panel id changed"):
            annotate(call, rows)

    def test_art_alone_does_not_prove_the_torch_panels_load_was_measured(self):
        calls = [{"query": "panel:10d"}, {"query": "art:1d712c"}]
        check_coverage(46, calls)
        with self.assertRaisesRegex(ValueError, "missed required loads"):
            check_coverage(46, calls[1:])


if __name__ == "__main__":
    unittest.main()
