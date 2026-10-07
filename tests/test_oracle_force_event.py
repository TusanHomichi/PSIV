"""`python3 -m oracle.force --event N`: an event battle forced without a draw.

An event battle's formation comes from `Event_Battle_Index`
(`ps4.asm:11815-11819`), not from a group's 32-entry table, so the tool writes
that one byte and measures nothing. These tests drive it with the same
hand-built packs and fake oracle as `tests/test_oracle_force_battle_run.py`;
the emulator is never started.

What they pin: the event battle is a boss formation the pack numbers apart from
the regular ones, the one cell the tool writes is the cartridge's own address,
the flag is exclusive with `--formation`, and a probe that built some other
formation is refused.
"""
import os
import pathlib
import unittest
from unittest import mock

from oracle import force as fb
from oracle.force import runs
from psiv_tools.extract_stamp import write_table_stamp
from tests.test_oracle_force_battle import FORMATIONS, write_json
from tests.test_oracle_force_battle_run import WholeRunBase

#: Boss formation 8 seats the two enemies formation 5 does, so the shared fake
#: probe (two enemy 10) is exactly what writing `Event_Battle_Index = 8` builds.
BOSS = {"event_battle_index": 8,
        "enemies": [{"slot": 1, "enemy": {"id": 10}},
                    {"slot": 2, "enemy": {"id": 10}}]}
#: `Event_Battle_Index`, `$FFFFECFC` (`ps4.constants.asm:2262`).
EVENT_CELL = "FFFFECFC"


class EventBase(WholeRunBase):
    def setUp(self):
        super().setUp()
        write_json(os.path.join(self.data, "formations.json"),
                   dict(FORMATIONS, boss_formations=[BOSS]))
        write_table_stamp(self.data)  # the stamp lists the table's bytes: stamp again
        self.pack = fb.Pack.load(pathlib.Path(self.data))

    def argv(self, *extra):
        return ["--event", "8", "--out", self.out, "--base-tape", self.tape,
                "--data-dir", self.data, "--ram-map", self.ram_map,
                "--scout", os.path.join(self.out, "scout.json"), *extra]


class EventNumbering(EventBase):
    def test_a_boss_formation_is_numbered_apart_from_the_regular_ones(self):
        self.assertIn(fb.EVENT_BASE + 8, self.pack.formation_ids)
        self.assertEqual(fb.parse_event("8", self.pack), fb.EVENT_BASE + 8)
        self.assertEqual(fb.parse_event("0x8", self.pack), fb.EVENT_BASE + 8)
        self.assertEqual([entry["id"] for entry in
                          self.pack.enemies_of(fb.EVENT_BASE + 8)], [10, 10])

    def test_the_two_flags_do_not_reach_into_each_others_numbering(self):
        # Event index 5 is not formation 5, and formation ids stay out of
        # `--event`: the two share no namespace.
        with self.assertRaises(fb.ForceError):
            fb.parse_event("5", self.pack)
        with self.assertRaises(fb.ForceError):
            fb.parse_formation(str(fb.EVENT_BASE + 8), self.pack)
        self.assertEqual(fb.parse_formation("5", self.pack), 5)
        with self.assertRaises(fb.ForceError):
            fb.parse_event("eight", self.pack)

    def test_the_description_names_the_event_battle(self):
        text = fb.describe(self.pack, fb.EVENT_BASE + 8)
        self.assertIn("event battle 8", text)
        self.assertNotIn("formation 4104", text)


class EventSelector(EventBase):
    def test_the_selector_writes_the_one_event_cell_and_restores_nothing(self):
        selector = fb.event_selector(8)
        self.assertEqual(selector.kind, "event")
        self.assertEqual(selector.cells, [("event_battle_index", 8)])
        self.assertEqual(selector.restore, [])

    def test_the_cell_has_a_layout_entry_even_though_the_log_has_no_column(self):
        field = self.layout["event_battle_index"]
        self.assertEqual((field["addr"], field["size"]), (EVENT_CELL, 1))
        specs = fb.patch_specs(fb.event_selector(8), {"battle_first": 40},
                               self.layout)
        self.assertEqual(specs, [f"41:{EVENT_CELL}:08"])
        # An index the byte cannot hold is refused, not truncated.
        with self.assertRaises(fb.ForceError):
            fb.patch_specs(fb.event_selector(256), {"battle_first": 40},
                           self.layout)


class EventCommandLine(EventBase):
    def test_exactly_one_of_the_two_selectors_is_required(self):
        base = ["--out", self.out]
        for argv in (base, base + ["--formation", "5", "--event", "8"]):
            with self.subTest(argv=argv), self.assertRaises(SystemExit) as raised:
                fb.main(argv)
            self.assertEqual(raised.exception.code, 2)

    def test_a_dry_run_writes_only_the_event_cell(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle",
                               side_effect=AssertionError("no oracle run")):
            self.assertEqual(fb.main(self.argv("--dry-run")), 0)
        patches = pathlib.Path(self.out, "forced_event08_attack.patches.txt").read_text()
        self.assertIn(f"41:{EVENT_CELL}:08", patches)
        self.assertNotIn("FFFFEF0C", patches, "no seed word: there is no draw")
        self.assertNotIn("FFFFEC28", patches, "no map patch: no group is forced")

    def test_an_unknown_event_battle_is_a_usage_error_not_a_capture(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle",
                               side_effect=AssertionError("no oracle run")):
            argv = self.argv("--dry-run")
            argv[1] = "99"
            self.assertEqual(fb.main(argv), 2)


class EventRun(EventBase):
    def test_the_whole_flow_forces_the_boss_formation_and_reports_it(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle", self.fake_oracle()):
            self.assertEqual(fb.main(self.argv()), 0)
        report = self.report()
        self.assertEqual(report["formation"], fb.EVENT_BASE + 8)
        self.assertEqual(report["event_battle"], 8)
        self.assertEqual(report["selector"]["kind"], "event")
        self.assertEqual(report["patches"], [f"41:{EVENT_CELL}:08"])
        self.assertEqual(report["outcome"], "victory")
        self.assertEqual(report["log_sha256"], report["rerun_log_sha256"])
        # Every run of the flow takes the same single patch: the byte is set
        # one frame after the encounter and stays set for the battle.
        self.assertTrue(all(call["patches"] == [f"41:{EVENT_CELL}:08"]
                            for call in self.calls), self.calls)

    def test_a_probe_that_built_another_formation_is_refused(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle",
                               self.fake_oracle(probe={"ids": ("99", "10")})):
            self.assertEqual(fb.main(self.argv()), 2)

    def test_a_regular_encounter_report_has_no_event_battle(self):
        self.scout_cache()
        formation_argv = ["--formation", "5", "--out", self.out, "--base-tape",
                          self.tape, "--data-dir", self.data, "--ram-map",
                          self.ram_map, "--scout",
                          os.path.join(self.out, "scout.json")]
        with mock.patch.object(runs, "run_oracle", self.fake_oracle()):
            self.assertEqual(fb.main(formation_argv), 0)
        self.assertIsNone(self.report()["event_battle"])


if __name__ == "__main__":
    unittest.main()
