"""The forced-battle capture harness, end to end: every phase, fake oracle.

`tests/test_oracle_force_battle.py` pins the tool's parts - the tape, the
selectors, the patch list, what the log says. This module drives the *whole*
flow (`oracle/force/phases.py`'s `run`) with `oracle.force.runs`'s
`run_oracle` replaced by hand-built logs: the scout, the probe, the preview,
the trimmed capture and its re-run, and everything the report says.

Two options the sweep rests on are pinned here, because both are decisions the
whole flow makes rather than one phase:

* `--durable` - each living party-side fighter's two HP cells
  (`oracle/force/durable.py`) patched at the frame the extractor reads the
  battle's start state from, and the capture *refused* when that frame does not
  read the patch back;
* `--max-rounds` - the tape stopped at the frame the next round's queue is
  built on, so a fixture can cut its window at that round boundary, and the
  report saying `truncated` with the rounds captured.

The fake oracle writes rows up to the frame the tape it was handed ends on, as
a real run's log does, which is what makes the trim observable to the tool.
"""
import json
import os
import pathlib
import unittest
from unittest import mock

from oracle import force as fb
from oracle.force import runs
from tests.test_oracle_force_battle import (BASE_TAPE, LOG_COLUMNS, RAM_MAP,
                                            SCOUT, TRACE_COLUMNS, PackFixture,
                                            cartridge_roll, cartridge_seed,
                                            log_row, trace_row, write_csv,
                                            write_json)

#: The mini RAM map the tool is handed, plus the columns the *battle* readings
#: need: the extractor's own start-frame and round rules
#: (`oracle/fixture/observations.py`), and the party-side HP cells `--durable`
#: patches. A real run is given `oracle/ram_map.json`, which carries all of
#: them; this is the smallest map a whole run can be driven with.
RUN_MAP = {"fields": RAM_MAP["fields"] + [
    {"name": "enemy_count", "addr": "FFFF440C", "size": 2},
    {"name": "turn_00", "addr": "FFFFEFB0", "size": 2, "hex": True},
    {"name": "e1_hp", "addr": "FFFF420E", "size": 2},
    {"name": "e1_maxhp", "addr": "FFFF4210", "size": 2},
    {"name": "e2_hp", "addr": "FFFF428E", "size": 2},
    {"name": "e2_maxhp", "addr": "FFFF4290", "size": 2},
    {"name": "e3_hp", "addr": "FFFF430E", "size": 2},
    {"name": "e3_maxhp", "addr": "FFFF4310", "size": 2},
    {"name": "e4_hp", "addr": "FFFF438E", "size": 2},
    {"name": "e4_maxhp", "addr": "FFFF4390", "size": 2},
    {"name": "chaz_hp", "addr": "FFFFF50E", "size": 2},
    {"name": "chaz_maxhp", "addr": "FFFFF510", "size": 2},
    {"name": "alys_hp", "addr": "FFFFF58E", "size": 2},
    {"name": "alys_maxhp", "addr": "FFFFF590", "size": 2},
    {"name": "hahn_hp", "addr": "FFFFF60E", "size": 2},
    {"name": "hahn_maxhp", "addr": "FFFFF610", "size": 2},
    {"name": "vehicle_fighter_hp", "addr": "FFFF470E", "size": 2},
    {"name": "vehicle_fighter_max_hp", "addr": "FFFF4710", "size": 2},
]}


class WholeRunBase(PackFixture):
    """The whole flow's helpers: a base tape, a scout cache, fake logs.

    The fake battle: the encounter at f40, the enemies built at f48, a round
    opening at f50 (and, when the test asks for more, at f58 and f66), all
    enemies down at f60 - or, with `ended=False`, still standing when the tape
    runs out. `fake_oracle` writes one row per frame of the tape it was handed,
    as a real run does.
    """

    #: The frames the fake battle is described by.
    BATTLE_FIRST = 40
    ENEMIES_LOADED = 48
    ROUND_FRAMES = (50, 58, 66)
    ENDED = 60
    #: The probe's own battle: the enemies fill in at f201, well inside the
    #: probe's 200-policy-block tape.
    PROBE_LOADED = 201
    #: The seed the practice capture runs with: `RNG_Seed` patched at f47, the
    #: frame before the draw, and rotated by the draw itself.
    PATCH_FRAME = 47
    PATCHED_SEED = 0x00116A13
    ROTATED_SEED = 0x80086A13

    def setUp(self):
        super().setUp()
        self.ram_map = os.path.join(self.dir.name, "run_map.json")
        write_json(self.ram_map, RUN_MAP)
        self.layout = fb.field_layout(pathlib.Path(self.ram_map))
        self.out = os.path.join(self.dir.name, "out")
        self.tape = os.path.join(self.dir.name, "base.tape")
        with open(self.tape, "w") as handle:
            handle.write(BASE_TAPE)
        self.calls = []

    def scout_cache(self):
        os.makedirs(self.out, exist_ok=True)
        write_json(os.path.join(self.out, "scout.json"),
                   dict(SCOUT, base_tape=self.tape))

    def probe_rows(self, ids=("10", "10"), party=("53", "25", "21"),
                   vehicle_hp="0"):
        """The probe's log: one row per frame, the enemies built at f201."""
        rows = [log_row(self.BATTLE_FIRST, game_mode="0010", enemy_count="0")]
        for frame in range(self.BATTLE_FIRST + 1, 231):
            built = frame >= self.PROBE_LOADED
            rows.append(log_row(
                frame,
                game_mode="0014" if frame < 230 else "0004",
                enemy_count="2" if built else "0",
                rng_seed="6EA56A13", main_frame_count="23931",
                e1_id=ids[0] if built else "0",
                e1_hp="25" if built else "0", e1_maxhp="25" if built else "0",
                e2_id=ids[1] if built else "0",
                e2_hp="25" if built else "0", e2_maxhp="25" if built else "0",
                e3_hp="0", e3_maxhp="0", e4_hp="0", e4_maxhp="0",
                alys_hp=party[0], chaz_hp=party[1], hahn_hp=party[2],
                vehicle_fighter_hp=vehicle_hp,
                vehicle_fighter_max_hp=vehicle_hp,
                battle_exp_total="0", battle_meseta_total="0"))
        return rows

    def capture_rows(self, frames=70, ended=True, rounds=2, party=None,
                     vehicle_hp="0"):
        """The capture-side log: the fight from f48, a round boundary per
        `rounds`, the kill at f60 when the battle ends."""
        party = party or {"alys_hp": "25", "chaz_hp": "53", "hahn_hp": "21"}
        rows = [log_row(self.BATTLE_FIRST, game_mode="0010", enemy_count="0")]
        for frame in range(self.BATTLE_FIRST + 1, frames + 1):
            built = frame >= self.ENEMIES_LOADED
            over = ended and frame >= self.ENDED
            hp = "65486" if over else "25"
            turns = len([start for start in self.ROUND_FRAMES[:rounds]
                         if frame >= start])
            seed = ("6EA56A13" if frame < self.PATCH_FRAME
                    else f"{self.PATCHED_SEED:08X}" if frame == self.PATCH_FRAME
                    else f"{self.ROTATED_SEED:08X}")
            rows.append(log_row(
                frame,
                game_mode="000C" if (ended and frame > self.ENDED) else "0014",
                enemy_count="2" if built else "0",
                rng_seed=seed, main_frame_count="23931",
                turn_00=f"{turns:04X}",
                e1_id="10" if built else "0",
                e1_hp=hp if built else "0", e1_maxhp="25" if built else "0",
                e2_id="10" if built else "0",
                e2_hp=hp if built else "0", e2_maxhp="25" if built else "0",
                e3_hp="0", e3_maxhp="0", e4_hp="0", e4_maxhp="0",
                e1_ability="02" if over else "00",
                e2_ability="02" if frame >= 52 else "00",
                e3_ability="00", e4_ability="00",
                vehicle_fighter_hp=vehicle_hp,
                vehicle_fighter_max_hp=vehicle_hp,
                battle_exp_total="48" if over else "0",
                battle_meseta_total="12" if over else "0",
                **party))
        return rows

    def fake_oracle(self, probe=None, frames=70, ended=True, rounds=2,
                    party=None, vehicle_hp="0"):
        """A `run_oracle` that writes the probe's log and the capture's.

        The probe's own log is what the tool places the seed patch against, so
        it carries the frames before the draw (f47 last of them) and the
        unpatched seed the trace's roll starts from; every capture-side run
        (preview, capture, verify) gets the same bytes, which is what makes the
        two-run comparison in the tool pass. Each log runs to the frame its own
        tape ends on, which is what the trim is checked against.
        """
        probe = dict(probe or {})

        def run(tape, out_dir, stem, patches, groups=fb.GROUPS):
            self.calls.append({"tape": str(tape),
                               "dir": os.path.basename(str(out_dir)),
                               "patches": list(patches)})
            out_dir = pathlib.Path(out_dir)
            out_dir.mkdir(parents=True, exist_ok=True)
            log = out_dir / f"{stem}.csv"
            trace = out_dir / f"{stem}_rolls.csv"
            if log.parent.name == "probe":
                write_csv(log, self.probe_rows(**probe), LOG_COLUMNS)
                write_csv(trace, [trace_row(48, 0, 0x293E, 23931, 0x6EA56A13,
                                            0x1805)], TRACE_COLUMNS)
            else:
                length = fb.tape_frames(fb.expand_tape(tape.read_text()))
                write_csv(log, self.capture_rows(min(frames, length), ended,
                                                 rounds, party, vehicle_hp),
                          LOG_COLUMNS)
                write_csv(trace, [trace_row(48, 0, 0x293E, 23931,
                                            self.PATCHED_SEED,
                                            cartridge_roll(0x293E, 23931,
                                                           self.PATCHED_SEED))],
                          TRACE_COLUMNS)
            return fb.Run(log, trace, "", 0)
        return run

    def argv(self, *extra):
        return ["--formation", "5", "--out", self.out, "--base-tape", self.tape,
                "--data-dir", self.data, "--ram-map", self.ram_map,
                "--scout", os.path.join(self.out, "scout.json"), *extra]

    def report(self):
        with open(os.path.join(self.out, "report.json")) as handle:
            return json.load(handle)


class WholeRun(WholeRunBase):
    """The whole flow, with `run_oracle` replaced by hand-built logs."""

    def test_the_formation_is_forced_to_the_entry_the_seed_patch_names(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle", self.fake_oracle()):
            self.assertEqual(fb.main(self.argv()), 0)
        report = self.report()
        self.assertEqual(report["formation"], 5)
        self.assertEqual(report["selector"]["group"], 0)
        self.assertEqual(report["selector"]["entry"], 0)
        self.assertEqual(report["selector"]["kind"], "map")
        self.assertEqual(report["draw"]["index_before"], 5)
        self.assertEqual(report["draw"]["index_forced"], 0)
        self.assertEqual(report["draw"]["k"], (0x293E + 23931) & 31)
        seed = int(report["patches"][1].split(":")[2], 16)
        self.assertEqual(seed, cartridge_seed(report["draw"]["k"], 0))
        self.assertEqual(report["patches"][0], "41:FFFFEC28:0007")
        self.assertEqual(report["patches"][2], "49:FFFFEC28:0015")
        self.assertEqual((report["battle_first"], report["battle_last"]),
                         (40, 60))
        self.assertEqual(report["start_frame"], self.ENEMIES_LOADED)
        self.assertEqual(report["outcome"], "victory")
        self.assertEqual(report["abilities"], {"e2=0x02": 52, "e1=0x02": 60})
        self.assertEqual(report["rewards"], {"experience": 48, "meseta": 12})
        self.assertEqual(report["log_sha256"], report["rerun_log_sha256"])
        self.assertEqual(report["rng_trace_check"], "passed")
        self.assertTrue(report["require_ability_met"])
        # Without either option the whole battle is captured, and the report
        # says so rather than leaving a reader to infer it.
        self.assertEqual(report["max_rounds"], 0)
        self.assertFalse(report["truncated"])
        self.assertIsNone(report["cut_frame"])
        self.assertIsNone(report["durable"])
        self.assertEqual(report["rounds_captured"], 2)

    def test_the_capture_and_its_re_run_take_the_same_tape_and_patches(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle", self.fake_oracle()):
            fb.main(self.argv())
        by_dir = {call["dir"]: call for call in self.calls}
        self.assertEqual(by_dir["capture"]["patches"],
                         by_dir["verify"]["patches"])
        self.assertEqual(by_dir["capture"]["tape"], by_dir["verify"]["tape"])
        self.assertEqual(by_dir["probe"]["patches"],
                         by_dir["capture"]["patches"][:1])
        self.assertLessEqual(len(by_dir["probe"]["tape"]), 400)

    def test_the_trimmed_capture_is_the_tape_the_report_names(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle", self.fake_oracle()):
            fb.main(self.argv())
        report = self.report()
        self.assertTrue(report["tape"].endswith("forced_05_attack.tape"))
        self.assertEqual(report["tape_frames"], 60 + fb.TAIL_FRAMES)
        with open(report["tape"]) as handle:
            steps = fb.expand_tape(handle.read())
        # Every command the oracle gets is the composed tape up to the trim:
        # the prefix, the policy, and nothing of the idle tail after it.
        self.assertEqual(fb.tape_frames(steps), report["tape_frames"])
        self.assertEqual([s.mark for s in steps if s.mark], ["walk", "encounter"])
        self.assertEqual(steps[2].buttons, "C")

    def test_an_ability_that_never_fires_fails_the_run(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle", self.fake_oracle()):
            self.assertEqual(fb.main(self.argv("--require-ability", "0x37")), 1)
        report = self.report()
        self.assertEqual(report["require_ability"], ["0x37"])
        self.assertFalse(report["require_ability_met"])
        self.assertEqual(report["abilities"], {"e2=0x02": 52, "e1=0x02": 60})

    def test_a_probe_the_group_table_does_not_explain_is_rejected(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle",
                               self.fake_oracle(probe={"ids": ("99", "10")})):
            self.assertEqual(fb.main(self.argv()), 2)

    def test_a_dry_run_writes_the_tape_and_the_selector_patches_only(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle",
                               side_effect=AssertionError("no oracle run")):
            self.assertEqual(fb.main(self.argv("--dry-run")), 0)
        with open(os.path.join(self.out,
                               "forced_05_attack.full.tape")) as handle:
            tape = handle.read()
        self.assertIn("20 R walk", tape)
        self.assertIn("4 C", tape)
        with open(os.path.join(self.out,
                               "forced_05_attack.patches.txt")) as handle:
            patches = handle.read()
        self.assertIn("41:FFFFEC28:0007", patches)
        self.assertNotIn("FFFFEF0C", patches)

    def test_the_scout_cache_is_reused_rather_than_run_again(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle",
                               side_effect=AssertionError("no oracle run")):
            facts = fb.scout(pathlib.Path(self.tape), BASE_TAPE,
                             pathlib.Path(self.out),
                             pathlib.Path(self.out, "scout.json"), False,
                             True, self.layout)
        self.assertEqual(facts["battle_first"], 40)


class WholeDurableRun(WholeRunBase):
    """`--durable`: the party-side HP cells, patched and verified."""

    #: What the capture's own log reads at the start-state frame when the
    #: patch did land: all three members at 999.
    PATCHED = {"alys_hp": "999", "alys_maxhp": "999", "chaz_hp": "999",
               "chaz_maxhp": "999", "hahn_hp": "999", "hahn_maxhp": "999"}

    def test_every_living_member_is_patched_at_the_start_state_frame(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle",
                               self.fake_oracle(party=self.PATCHED)):
            self.assertEqual(fb.main(self.argv("--durable", "--repeats", "20")),
                             0)
        report = self.report()
        durable = report["durable"]
        # The probe's own start frame is f201 (where its enemy records fill
        # in), which is the frame the extractor reads the start state from.
        self.assertEqual((durable["hp"], durable["frame"]), (999, 201))
        self.assertEqual(durable["cells"], [
            "alys_hp", "alys_maxhp", "chaz_hp", "chaz_maxhp",
            "hahn_hp", "hahn_maxhp"])
        self.assertEqual(durable["skipped"], [])
        self.assertEqual(report["patches"][-6:], [
            "201:FFFFF58E:03E7", "201:FFFFF590:03E7",
            "201:FFFFF50E:03E7", "201:FFFFF510:03E7",
            "201:FFFFF60E:03E7", "201:FFFFF610:03E7"])
        # The patch list's own comment says what the cells are for.
        with open(os.path.join(self.out,
                               "forced_05_attack.patches.txt")) as handle:
            self.assertIn("durable party patch, 999 HP", handle.read())

    def test_a_member_the_tape_left_down_is_not_revived(self):
        self.scout_cache()
        with mock.patch.object(
                runs, "run_oracle",
                self.fake_oracle(probe={"party": ("53", "25", "0")},
                                 party=self.PATCHED)):
            self.assertEqual(fb.main(self.argv("--durable", "--repeats", "20")),
                             0)
        durable = self.report()["durable"]
        self.assertEqual(durable["skipped"], ["hahn"])
        self.assertNotIn("hahn_hp", durable["cells"])
        self.assertEqual(len(durable["cells"]), 4)

    def test_a_capture_whose_start_state_is_not_the_patch_is_refused(self):
        # The probe's party is all living, so the patch is placed - but the
        # capture's own log reads the tape's HP back, which is what a patch the
        # load overwrote looks like. That capture must not become a fixture.
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle", self.fake_oracle()):
            self.assertEqual(fb.main(self.argv("--durable", "--repeats", "20")),
                             2)


class WholeCappedRun(WholeRunBase):
    """`--max-rounds`: the tape stops at the round boundary, and the report
    says the capture is truncated."""

    def test_the_tape_stops_where_round_three_is_drawn_up(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle",
                               self.fake_oracle(ended=False, rounds=3)):
            self.assertEqual(fb.main(self.argv("--max-rounds", "2")), 0)
        report = self.report()
        self.assertEqual(report["outcome"], "truncated")
        self.assertTrue(report["truncated"])
        self.assertEqual(report["rounds_captured"], 2)
        # Round 3's queue is built at f66; round 2 ends the frame before it,
        # and the tape runs exactly to the boundary, so the extractor can see
        # the cut it has to make.
        self.assertEqual(report["cut_frame"], 65)
        self.assertEqual(report["tape_frames"], 66)
        self.assertEqual(report["battle_last"], 66)
        with open(report["tape"]) as handle:
            self.assertEqual(fb.tape_frames(fb.expand_tape(handle.read())), 66)

    def test_a_battle_that_ends_inside_the_cap_is_captured_whole(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle", self.fake_oracle(rounds=2)):
            self.assertEqual(fb.main(self.argv("--max-rounds", "5")), 0)
        report = self.report()
        self.assertEqual(report["outcome"], "victory")
        self.assertFalse(report["truncated"])
        self.assertIsNone(report["cut_frame"])
        self.assertEqual(report["rounds_captured"], 2)
        self.assertEqual((report["battle_first"], report["battle_last"]),
                         (40, 60))

    def test_a_cap_the_tape_never_reaches_is_refused(self):
        # The battle runs out of tape inside round 1: there is no boundary to
        # cut at, and the tool says so rather than guessing one.
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle",
                               self.fake_oracle(ended=False, rounds=1)):
            self.assertEqual(fb.main(self.argv("--max-rounds", "5")), 2)

    def test_a_negative_cap_is_rejected(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle",
                               side_effect=AssertionError("no oracle run")):
            self.assertEqual(fb.main(self.argv("--max-rounds", "-1")), 2)


class WholeVehicleRunBase(WholeRunBase):
    """The whole flow with a pack whose only group is the Motavia vehicle
    table 8, so `--vehicle` reaches the run, the patch list and the report
    exactly as it does on the cartridge."""

    def setUp(self):
        super().setUp()
        self.pack_from({8: [5] * 32}, {"maps": [], "position_grids": []})

    def argv(self, *extra):
        return ["--formation", "5", "--out", self.out, "--base-tape", self.tape,
                "--data-dir", self.pack_dir, "--ram-map", self.ram_map,
                "--scout", os.path.join(self.out, "scout.json"), *extra]


class WholeVehicleRun(WholeVehicleRunBase):
    """The same hand-built logs, one vehicle table's formation forced."""

    def test_the_report_names_the_vehicle_the_capture_was_told_to_use(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle", self.fake_oracle()):
            self.assertEqual(fb.main(self.argv("--vehicle", "2")), 0)
        report = self.report()
        self.assertEqual(report["selector"]["kind"], "vehicle")
        self.assertEqual(report["selector"]["group"], 8)
        self.assertEqual(report["selector"]["cells"][1],
                         ["vehicle_index", 2])
        self.assertEqual(report["vehicle"],
                         {"index": 2, "name": "Ice Digger", "table": 8,
                          "fighter_hp_at_end": report["vehicle_fighter_hp"]})
        # The capture's own files say which vehicle fought it, so two captures
        # of one formation cannot overwrite each other.
        self.assertTrue(report["tape"].endswith("forced_05_attack_v2.tape"))
        self.assertEqual(report["patches"][1], "41:FFFFF43C:0002")
        self.assertNotIn("FFFFF43C", report["patches"][-1])

    def test_without_the_flag_the_tables_own_machine_is_the_default(self):
        self.scout_cache()
        with mock.patch.object(runs, "run_oracle", self.fake_oracle()):
            self.assertEqual(fb.main(self.argv()), 0)
        report = self.report()
        self.assertEqual(report["vehicle"]["index"], 1)
        self.assertEqual(report["selector"]["cells"][1],
                         ["vehicle_index", 1])
        self.assertTrue(report["tape"].endswith("forced_05_attack.tape"))


class WholeVehicleDurableRun(WholeVehicleRunBase):
    """`--durable` on a vehicle table: the vehicle's own HP cells, because a
    vehicle battle's party side is the vehicle (`loc_78EE`, ps4.asm:11408)."""

    def test_a_vehicle_battle_patches_the_vehicle_fighters_own_cells(self):
        self.pack_from({8: [5] * 32}, {"maps": [], "position_grids": []})
        self.scout_cache()
        with mock.patch.object(
                runs, "run_oracle",
                self.fake_oracle(probe={"vehicle_hp": "740"},
                                 vehicle_hp="999")):
            self.assertEqual(fb.main(self.argv("--durable", "--repeats", "20")),
                             0)
        report = self.report()
        self.assertEqual(report["durable"]["cells"],
                         ["vehicle_fighter_hp", "vehicle_fighter_max_hp"])
        self.assertEqual(report["vehicle"]["index"], 1)
        self.assertEqual(report["vehicle_fighter_hp"], 999)

    def test_a_wrecked_vehicle_is_left_alone(self):
        self.pack_from({8: [5] * 32}, {"maps": [], "position_grids": []})
        self.scout_cache()
        with mock.patch.object(
                runs, "run_oracle",
                self.fake_oracle(probe={"vehicle_hp": "0"})):
            self.assertEqual(fb.main(self.argv("--durable", "--repeats", "20")),
                             0)
        durable = self.report()["durable"]
        self.assertEqual(durable["cells"], [])
        self.assertEqual(durable["skipped"], ["vehicle"])



if __name__ == "__main__":
    unittest.main()
