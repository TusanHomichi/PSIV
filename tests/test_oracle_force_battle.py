"""Tests for oracle/force_battle.py, the forced-battle capture harness.

Everything here is a hand-built input: a tape written as a few steps, a pack of
two formations, a probe trace with one roll in it, and a RAM log carrying the
frames the tool reads. The emulator is never started - `run_oracle` is replaced
- so what these tests pin is the composition the tool would hand the oracle:
which frames get padded, which RAM cells are patched at which frames, and what
the report says about the log that comes back.

Where a number can be, it is re-derived here instead of being read from the
module under test: `cartridge_seed` writes out the roll's arithmetic from the
disassembly, so a wrong index or a wrong patch frame has to fail.
"""
import hashlib
import json
import os
import pathlib
import tempfile
import unittest
from unittest import mock

from oracle import force_battle as fb

#: A pack with three groups: 0 is drawn by a map, 1 by the Motavia position
#: grid, 2 is a vehicle table. Formation 5 sits in 0 and 2, formation 6 in 1.
FORMATIONS = {
    "formations": [
        {"id": 5, "enemies": [{"slot": 1, "enemy": {"id": 10}},
                              {"slot": 2, "enemy": {"id": 10}}]},
        {"id": 6, "enemies": [{"slot": 1, "enemy": {"id": 15}}]},
    ]
}
ENEMIES = [
    {"id": 10, "symbol": "TestFoe", "hp": 25,
     "ai": {"regular_ability_ids": [2] * 8, "conditional_ability_ids": [0] * 4}},
    {"id": 15, "symbol": "OtherFoe", "hp": 261,
     "ai": {"regular_ability_ids": [0, 0, 0, 0, 0, 0, 8, 8],
            "conditional_ability_ids": [0] * 4}},
]
GROUP_IDS = {0: [5] * 32, 1: [4, 4, 6] + [4] * 29, 2: [5] * 20 + [6] * 12}
ENCOUNTERS = {
    "maps": [{"map_id": 7, "map_symbol": "TestCave", "in_table": True,
              "group": 0, "value": 0},
             {"map_id": 9, "map_symbol": "TestTown", "in_table": True,
              "group": None, "value": 255}],
    "position_grids": [{"map": {"id": 0}, "columns": 64, "rows": 2,
                        "cells": [[255, 255, 255, 1] + [255] * 60,
                                  [255] * 64]}],
}
RAM_MAP = {"fields": [
    {"name": "map_index", "addr": "FFFFEC28", "size": 2, "hex": True},
    {"name": "c1_x_px", "addr": "FFFFC030", "size": 2},
    {"name": "c1_y_px", "addr": "FFFFC034", "size": 2},
    {"name": "vehicle_index", "addr": "FFFFF43C", "size": 2, "hex": True},
    {"name": "mota_battle_bg_index", "addr": "FFFFECEF", "size": 1, "hex": True},
    {"name": "rng_hi", "addr": "FFFFEF0C", "size": 2, "hex": True},
]}

#: The base tape: two steps that walk, and the encounter fires at f40.
BASE_TAPE = "20 R walk\n20 R encounter\n40 . settled\n"
BASE_SHA = hashlib.sha256(BASE_TAPE.encode()).hexdigest()
SCOUT = {
    "base_tape": "BASE", "base_sha256": BASE_SHA,
    "battle_first": 40, "battle_last": 300,
    "cells": {"map_index": 0x15, "c1_x_px": 100, "c1_y_px": 200,
              "vehicle_index": 0, "mota_battle_bg_index": 0},
}
#: The columns of a real log the tool reads (the RAM map's own groups).
LOG_COLUMNS = ("frame,game_mode,enemy_count,rng_seed,main_frame_count,"
               "e1_id,e1_hp,e1_maxhp,e1_ability,e2_id,e2_hp,e2_maxhp,"
               "e2_ability,e3_id,e3_hp,e3_maxhp,e3_ability,e4_id,e4_hp,"
               "e4_maxhp,e4_ability,chaz_hp,alys_hp,hahn_hp,"
               "vehicle_fighter_hp,battle_exp_total,"
               "battle_meseta_total").split(",")
TRACE_COLUMNS = ("frame,call_index_in_frame,pc,hv,frame_count,seed_before,"
                 "roll,seed_after").split(",")


def cartridge_roll(hv, frame_count, seed):
    """The roll's own arithmetic, written out again from `UpdateRNGSeed2`."""
    return (hv + frame_count - ((seed >> 16) & 0xFFFF)) & 0xFFFF


def cartridge_seed(k, index):
    """The `RNG_Seed` high word that puts the draw on `index`.

    The roll is `(hv + Main_Frame_Count - RNG_Seed_high) & $FFFF`
    (`UpdateRNGSeed2`, ps4.asm:86097) and the entry is its low five bits, so
    the high word has to be `K - index` modulo 32."""
    return (k - index) % 32


def write_json(path, document):
    with open(path, "w") as handle:
        json.dump(document, handle)


def write_csv(path, rows, columns):
    with open(path, "w") as handle:
        handle.write(",".join(columns) + "\n")
        for row in rows:
            handle.write(",".join(str(row.get(c, "")) for c in columns) + "\n")


def log_row(frame, **columns):
    return {"frame": str(frame), **columns}


def trace_row(frame, index, hv, frame_count, seed_before, roll):
    """One trace row, with `seed_after` the rotation `ror RNG_Seed.w` makes."""
    hi, lo = (seed_before >> 16) & 0xFFFF, seed_before & 0xFFFF
    rotated = (((hi >> 1) | ((hi & 1) << 15)) << 16) | lo
    return {"frame": str(frame), "call_index_in_frame": str(index),
            "pc": "0423A2", "hv": f"{hv:04X}", "frame_count": str(frame_count),
            "seed_before": f"{seed_before:08X}", "roll": f"{roll:04X}",
            "seed_after": f"{rotated:08X}"}


class PackFixture(unittest.TestCase):
    """A temp directory holding a pack and a RAM map the tool can load."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.data = os.path.join(self.dir.name, "generated")
        os.makedirs(self.data)
        write_json(os.path.join(self.data, "formations.json"), FORMATIONS)
        write_json(os.path.join(self.data, "enemies.json"), ENEMIES)
        write_json(os.path.join(self.data, "formation_indexes.json"),
                   {"groups": [{"group": group, "formation_ids": ids}
                               for group, ids in sorted(GROUP_IDS.items())]})
        write_json(os.path.join(self.data, "encounters.json"), ENCOUNTERS)
        self.ram_map = os.path.join(self.dir.name, "ram_map.json")
        write_json(self.ram_map, RAM_MAP)
        self.layout = fb.field_layout(pathlib.Path(self.ram_map))
        self.pack = fb.Pack.load(pathlib.Path(self.data))

    def pack_from(self, groups, encounters):
        """A pack built from just these groups and encounters."""
        root = tempfile.mkdtemp(dir=self.dir.name)
        write_json(os.path.join(root, "formations.json"), FORMATIONS)
        write_json(os.path.join(root, "enemies.json"), ENEMIES)
        write_json(os.path.join(root, "formation_indexes.json"),
                   {"groups": [{"group": group, "formation_ids": ids}
                               for group, ids in sorted(groups.items())]})
        write_json(os.path.join(root, "encounters.json"), encounters)
        return fb.Pack.load(pathlib.Path(root))


class TapeComposition(PackFixture):
    def test_a_repeat_block_expands_to_its_steps_in_order(self):
        steps = fb.expand_tape("2 R\nrepeat 3\n4 C\n12 .\nend\n6 . done\n")
        self.assertEqual([(s.frames, s.buttons) for s in steps],
                         [(2, "R"), (4, "C"), (12, "."), (4, "C"), (12, "."),
                          (4, "C"), (12, "."), (6, ".")])
        self.assertEqual(steps[-1].mark, "done")
        self.assertEqual(fb.tape_frames(steps), 2 + 3 * 16 + 6)

    def test_an_unterminated_repeat_and_a_stray_end_are_rejected(self):
        with self.assertRaises(fb.ForceError):
            fb.expand_tape("repeat 2\n4 C\n")
        with self.assertRaises(fb.ForceError):
            fb.expand_tape("4 C\nend\n")

    def test_trimming_cuts_inside_a_step_and_keeps_the_frame_count(self):
        steps = fb.expand_tape(BASE_TAPE)
        cut = fb.trim_tape(steps, 30)
        self.assertEqual(fb.tape_frames(cut), 30)
        self.assertEqual([(s.frames, s.buttons, s.mark) for s in cut],
                         [(20, "R", "walk"), (10, "R", "encounter")])
        self.assertEqual(fb.tape_frames(fb.trim_tape(steps, 41)), 41)

    def test_emitting_merges_markless_runs_and_keeps_marks(self):
        text = fb.emit_tape([fb.Step(4, "C"), fb.Step(12, "."),
                             fb.Step(4, "C"), fb.Step(12, "."),
                             fb.Step(3, ".", "done")])
        self.assertEqual(text, "4 C\n12 .\n4 C\n12 .\n3 . done\n")

    def test_the_attack_policy_is_tapes_07_and_09s_own_press(self):
        self.assertEqual([(s.frames, s.buttons)
                          for s in fb.policy_steps("attack", 3)],
                         [(4, "C"), (12, ".")] * 3)

    def test_the_defend_policy_walks_right_to_defend_as_tape_14_does(self):
        buttons = [s.buttons for s in fb.policy_steps("defend", 1)]
        self.assertEqual(buttons.count("R"), 4)
        self.assertEqual(buttons[0], "C")
        self.assertEqual(buttons[-2:], ["C", "."])

    def test_an_unknown_policy_is_rejected(self):
        with self.assertRaises(fb.ForceError):
            fb.policy_steps("sing", 1)

    def test_the_composed_tape_keeps_the_prefix_its_delay_and_the_policy(self):
        steps = fb.compose(fb.expand_tape(BASE_TAPE), 40, 5, 2, "attack")
        started = []
        frame = 1
        for step in steps:
            started.append((frame, step.frames, step.buttons, step.mark))
            frame += step.frames
        self.assertEqual(started[0], (1, 20, "R", "walk"))
        self.assertEqual(started[2], (41, 5, ".", "delay"))
        self.assertEqual(started[3], (46, 4, "C", ""))
        self.assertEqual(fb.tape_frames(steps), 40 + 5 + 2 * 16 + 600)

    def test_a_delay_does_not_touch_the_frames_before_the_seam(self):
        base = fb.expand_tape(BASE_TAPE)
        plain = fb.trim_tape(fb.compose(base, 40, 0, 1, "attack"), 40)
        delayed = fb.trim_tape(fb.compose(base, 40, 7, 1, "attack"), 40)
        self.assertEqual([(s.frames, s.buttons) for s in plain],
                         [(s.frames, s.buttons) for s in delayed])


class Selectors(PackFixture):
    def test_a_formation_a_map_draws_is_forced_by_that_map(self):
        selector = fb.choose_selector(self.pack, 5)
        self.assertEqual((selector.kind, selector.group), ("map", 0))
        self.assertEqual(selector.cells, [("map_index", 7)])
        self.assertEqual(selector.restore, ["map_index"])

    def test_a_grid_only_formation_is_forced_by_the_world_map_and_a_cell(self):
        selector = fb.choose_selector(self.pack, 6)
        self.assertEqual((selector.kind, selector.group), ("grid", 1))
        self.assertEqual(selector.cells, [("map_index", 0),
                                          ("c1_x_px", 3 * 64),
                                          ("c1_y_px", 0)])
        self.assertEqual(selector.restore,
                         ["map_index", "c1_x_px", "c1_y_px"])

    def test_a_map_selector_wins_over_a_grid_one_for_the_same_formation(self):
        pack = self.pack_from({0: [5] * 32, 1: [5] * 32}, ENCOUNTERS)
        self.assertEqual(fb.choose_selector(pack, 5).kind, "map")

    def test_a_vehicle_group_is_the_last_resort_and_says_so(self):
        pack = self.pack_from({8: [5] * 24 + [6] * 8},
                              {"maps": [], "position_grids": []})
        selector = fb.choose_selector(pack, 5)
        self.assertEqual((selector.kind, selector.group), ("vehicle", 8))
        self.assertEqual(selector.cells[1], ("vehicle_index", 1))
        self.assertEqual(selector.cells[2], ("mota_battle_bg_index", 0))
        self.assertNotIn("vehicle_index", selector.restore)

    def test_a_formation_in_no_reachable_group_names_the_groups(self):
        pack = self.pack_from({3: [5] * 32},
                              {"maps": [], "position_grids": []})
        with self.assertRaises(fb.ForceError) as caught:
            fb.choose_selector(pack, 5)
        self.assertIn("group(s) [3]", str(caught.exception))


class DrawAndPatches(PackFixture):
    def probe_inputs(self, frame=48, hv=0x293E, count=23931, seed=0x6EA56A13,
                     roll=0x1814):
        trace = [trace_row(frame, 0, hv, count, seed, roll)]
        rows = [log_row(40, game_mode="0014"),
                log_row(frame - 1, game_mode="0014", rng_seed=f"{seed:08X}",
                        main_frame_count=str(count))]
        return trace, rows

    def draw_of(self, **kwargs):
        trace, rows = self.probe_inputs(**kwargs)
        return fb.find_draw(trace, rows, 40)

    def test_the_draw_is_the_first_roll_after_the_battle_and_its_low_bits(self):
        draw = self.draw_of(roll=0x1850)
        self.assertEqual(draw.frame, 48)
        self.assertEqual(draw.index, 0x10)
        self.assertEqual(draw.k, (0x293E + 23931) & 31)
        self.assertEqual(draw.frame_before, 47)
        self.assertEqual(draw.rows_in_frame, 1)

    def test_a_roll_before_the_battle_is_not_the_draw(self):
        trace, rows = self.probe_inputs()
        trace.insert(0, trace_row(30, 0, 0x1000, 5, 0x11223344, 0x0000))
        self.assertEqual(fb.find_draw(trace, rows, 40).frame, 48)

    def test_a_battle_with_no_roll_is_reported(self):
        trace, rows = self.probe_inputs(frame=30)
        with self.assertRaises(fb.ForceError):
            fb.find_draw(trace, rows, 40)

    def test_the_seed_that_forces_an_index_is_k_minus_the_index(self):
        draw = self.draw_of()
        for index in range(32):
            self.assertEqual((draw.k - draw.seed_for(index)) % 32, index)
            self.assertEqual(cartridge_seed(draw.k, index),
                             draw.seed_for(index))

    def test_a_map_selector_patches_the_cell_then_the_seed_then_the_write_back(self):
        draw = self.draw_of()
        facts = dict(SCOUT, cells=dict(SCOUT["cells"]))
        specs = fb.patch_specs(fb.choose_selector(self.pack, 5), facts,
                               self.layout, draw, index=5)
        self.assertEqual(specs, [
            "41:FFFFEC28:0007",
            f"{draw.frame_before}:FFFFEF0C:{cartridge_seed(draw.k, 5):04X}",
            f"{draw.frame + 1}:FFFFEC28:0015"])

    def test_a_grid_selector_patches_three_cells_and_writes_all_three_back(self):
        draw = self.draw_of()
        facts = dict(SCOUT, cells=dict(SCOUT["cells"]))
        specs = fb.patch_specs(fb.choose_selector(self.pack, 6), facts,
                               self.layout, draw, index=1)
        self.assertEqual(specs[:3], ["41:FFFFEC28:0000", "41:FFFFC030:00C0",
                                     "41:FFFFC034:0000"])
        self.assertEqual(len(specs), 1 + 3 + 3)
        self.assertEqual(specs[-1], f"{draw.frame + 1}:FFFFC034:00C8")
        self.assertEqual(specs[3], f"{draw.frame_before}:FFFFEF0C:"
                                   f"{cartridge_seed(draw.k, 1):04X}")

    def test_a_value_too_wide_for_its_cell_is_rejected(self):
        selector = fb.Selector(0, "map", "test",
                               [("mota_battle_bg_index", 256)], [])
        with self.assertRaises(fb.ForceError):
            fb.patch_specs(selector, dict(SCOUT), self.layout)


class LogReading(PackFixture):
    def capture_rows(self, enemy_hp, party_hp=(53, 25, 21), vehicle_hp=0,
                     ended=True):
        rows = [log_row(40, game_mode="0010", enemy_count="0"),
                log_row(48, game_mode="0014", enemy_count="2", e1_id="10",
                        e1_hp=str(enemy_hp[0]), e1_maxhp="25", e2_id="10",
                        e2_hp=str(enemy_hp[1]), e2_maxhp="25",
                        chaz_hp=str(party_hp[0]), alys_hp=str(party_hp[1]),
                        hahn_hp=str(party_hp[2]),
                        vehicle_fighter_hp=str(vehicle_hp))]
        last = log_row(60, game_mode="0014", enemy_count="2", e1_id="10",
                       e1_hp=str(enemy_hp[0]), e1_maxhp="25", e2_id="10",
                       e2_hp=str(enemy_hp[1]), e2_maxhp="25",
                       chaz_hp=str(party_hp[0]), alys_hp=str(party_hp[1]),
                       hahn_hp=str(party_hp[2]),
                       vehicle_fighter_hp=str(vehicle_hp),
                       battle_exp_total="24", battle_meseta_total="6")
        rows.append(last)
        if ended:
            rows.append(log_row(70, game_mode="000C", enemy_count="2",
                                e1_id="10", e1_hp=str(enemy_hp[0]),
                                e1_maxhp="25", e2_id="10",
                                e2_hp=str(enemy_hp[1]), e2_maxhp="25",
                                chaz_hp=str(party_hp[0]),
                                alys_hp=str(party_hp[1]),
                                hahn_hp=str(party_hp[2]),
                                vehicle_fighter_hp=str(vehicle_hp),
                                battle_exp_total="24",
                                battle_meseta_total="6"))
        return rows

    def test_a_negative_hp_reads_as_the_cartridge_stores_it(self):
        self.assertEqual(fb.hp_of({"hp": "65485"}, "hp"), -51)
        self.assertEqual(fb.hp_of({"hp": "53"}, "hp"), 53)

    def test_every_enemy_out_is_a_victory(self):
        rows = self.capture_rows((65511, 65485), (10, 5, 3))
        self.assertEqual(fb.classify(rows, (40, 60)), "victory")

    def test_a_party_wipe_is_a_defeat(self):
        rows = self.capture_rows((25, 25), (0, 0, 0))
        self.assertEqual(fb.classify(rows, (40, 60)), "defeat")

    def test_a_wrecked_vehicle_is_a_defeat_even_with_the_party_untouched(self):
        rows = self.capture_rows((25, 25), vehicle_hp=0)
        self.assertEqual(fb.classify(rows, (40, 60), vehicle=True), "defeat")
        self.assertEqual(fb.classify(rows, (40, 60)), "withdrawal")

    def test_a_battle_still_running_at_the_tape_end_is_unfinished(self):
        rows = self.capture_rows((25, 25), ended=False)
        self.assertEqual(fb.classify(rows, (40, 60)), "unfinished")

    def test_ability_uses_are_the_nonzero_ids_from_the_draw_onward(self):
        rows = [log_row(frame, e1_ability=e1, e2_ability=e2,
                        e3_ability="00", e4_ability="00")
                for frame, e1, e2 in ((40, "00", "FC"), (48, "00", "02"),
                                      (52, "00", "02"), (60, "37", "02"))]
        self.assertEqual(fb.ability_uses(rows, (40, 60), 48),
                         {"e2=0x02": 48, "e1=0x37": 60})

    def test_the_reported_enemies_are_the_slots_the_battle_built(self):
        rows = self.capture_rows((65511, 65485))
        self.assertEqual(fb.enemy_slots(rows, (40, 60)),
                         [{"slot": 1, "id": 10, "maxhp": 25},
                          {"slot": 2, "id": 10, "maxhp": 25}])


class WholeRun(PackFixture):
    """The whole flow, with `run_oracle` replaced by hand-built logs."""

    def setUp(self):
        super().setUp()
        self.out = os.path.join(self.dir.name, "out")
        self.tape = os.path.join(self.dir.name, "base.tape")
        with open(self.tape, "w") as handle:
            handle.write(BASE_TAPE)
        self.calls = []

    def scout_cache(self):
        os.makedirs(self.out, exist_ok=True)
        write_json(os.path.join(self.out, "scout.json"),
                   dict(SCOUT, base_tape=self.tape))

    def probe_rows(self, ids=("10", "10")):
        """One row per frame, as a real log has: 40 opens the battle, 48 draws."""
        rows = [log_row(40, game_mode="0010", enemy_count="0")]
        for frame in range(41, 231):
            built = frame >= 201
            rows.append(log_row(
                frame,
                game_mode="0014" if frame < 230 else "0004",
                enemy_count="2" if built else "0",
                rng_seed="6EA56A13", main_frame_count="23931",
                e1_id=ids[0] if built else "0",
                e1_hp="25" if built else "0", e1_maxhp="25" if built else "0",
                e2_id=ids[1] if built else "0",
                e2_hp="25" if built else "0", e2_maxhp="25" if built else "0",
                chaz_hp="53", alys_hp="25", hahn_hp="21",
                vehicle_fighter_hp="0", battle_exp_total="0",
                battle_meseta_total="0"))
        return rows

    #: The seed the practice capture runs with: `RNG_Seed` patched at f47, the
    #: frame before the draw, and rotated by the draw itself. A real log reads
    #: exactly this way, and oracle/rng_trace.py check insists on it.
    PATCH_FRAME = 47
    PATCHED_SEED = 0x00116A13
    ROTATED_SEED = 0x80086A13

    def capture_rows(self):
        """One row per frame: the fight at f48, the kill at f60, field after."""
        rows = [log_row(40, game_mode="0010", enemy_count="0")]
        for frame in range(41, 71):
            built = frame >= 48
            over = frame >= 60
            hp = "65486" if over else "25"
            seed = ("6EA56A13" if frame < self.PATCH_FRAME
                    else f"{self.PATCHED_SEED:08X}" if frame == self.PATCH_FRAME
                    else f"{self.ROTATED_SEED:08X}")
            rows.append(log_row(
                frame,
                game_mode="0014" if frame <= 60 else "000C",
                enemy_count="2" if built else "0",
                rng_seed=seed, main_frame_count="23931",
                e1_id="10" if built else "0",
                e1_hp=hp if built else "0", e1_maxhp="25" if built else "0",
                e2_id="10" if built else "0",
                e2_hp=hp if built else "0", e2_maxhp="25" if built else "0",
                e1_ability="02" if over else "00",
                e2_ability="02" if frame >= 52 else "00",
                e3_ability="00", e4_ability="00",
                chaz_hp="53", alys_hp="25", hahn_hp="21",
                vehicle_fighter_hp="0",
                battle_exp_total="48" if over else "0",
                battle_meseta_total="12" if over else "0"))
        return rows

    def fake_oracle(self, probe_ids=("10", "10")):
        """A `run_oracle` that writes a probe log and the capture log.

        The probe's own log is what the tool places the seed patch against, so
        it carries the frames before the draw (f47 last of them) and the
        unpatched seed the trace's roll starts from; every capture-side run
        (preview, capture, verify) gets the same bytes, which is what makes the
        two-run comparison in the tool pass."""
        def run(tape, out_dir, stem, patches, groups=fb.GROUPS):
            self.calls.append({"tape": str(tape),
                               "dir": os.path.basename(str(out_dir)),
                               "patches": list(patches)})
            os.makedirs(out_dir, exist_ok=True)
            log = pathlib.Path(out_dir) / f"{stem}.csv"
            trace = pathlib.Path(out_dir) / f"{stem}_rolls.csv"
            if log.parent.name == "probe":
                write_csv(log, self.probe_rows(probe_ids), LOG_COLUMNS)
                write_csv(trace, [trace_row(48, 0, 0x293E, 23931, 0x6EA56A13,
                                            0x1805)], TRACE_COLUMNS)
            else:
                write_csv(log, self.capture_rows(), LOG_COLUMNS)
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

    def test_the_formation_is_forced_to_the_entry_the_seed_patch_names(self):
        self.scout_cache()
        with mock.patch.object(fb, "run_oracle", self.fake_oracle()):
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
        self.assertEqual(report["outcome"], "victory")
        self.assertEqual(report["abilities"], {"e2=0x02": 52, "e1=0x02": 60})
        self.assertEqual(report["rewards"], {"experience": 48, "meseta": 12})
        self.assertEqual(report["log_sha256"], report["rerun_log_sha256"])
        self.assertEqual(report["rng_trace_check"], "passed")
        self.assertTrue(report["require_ability_met"])

    def test_the_capture_and_its_re_run_take_the_same_tape_and_patches(self):
        self.scout_cache()
        with mock.patch.object(fb, "run_oracle", self.fake_oracle()):
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
        with mock.patch.object(fb, "run_oracle", self.fake_oracle()):
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
        with mock.patch.object(fb, "run_oracle", self.fake_oracle()):
            self.assertEqual(fb.main(self.argv("--require-ability", "0x37")), 1)
        report = self.report()
        self.assertEqual(report["require_ability"], ["0x37"])
        self.assertFalse(report["require_ability_met"])
        self.assertEqual(report["abilities"], {"e2=0x02": 52, "e1=0x02": 60})

    def test_a_probe_the_group_table_does_not_explain_is_rejected(self):
        self.scout_cache()
        with mock.patch.object(fb, "run_oracle",
                               self.fake_oracle(probe_ids=("99", "10"))):
            self.assertEqual(fb.main(self.argv()), 2)

    def test_a_dry_run_writes_the_tape_and_the_selector_patches_only(self):
        self.scout_cache()
        with mock.patch.object(fb, "run_oracle",
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
        with mock.patch.object(fb, "run_oracle",
                               side_effect=AssertionError("no oracle run")):
            facts = fb.scout(pathlib.Path(self.tape), BASE_TAPE,
                             pathlib.Path(self.out),
                             pathlib.Path(self.out, "scout.json"), False,
                             True, self.layout)
        self.assertEqual(facts["battle_first"], 40)


class Arguments(PackFixture):
    def argv(self, *extra):
        return ["--formation", "5", "--out", os.path.join(self.dir.name, "o"),
                "--base-tape", os.path.join(self.dir.name, "missing.tape"),
                "--data-dir", self.data, "--ram-map", self.ram_map, *extra]

    def test_an_unknown_formation_id_is_rejected_with_its_range(self):
        status = fb.main(self.argv("--formation", "0x7F"))
        self.assertEqual(status, 2)

    def test_a_formation_that_is_not_a_number_is_rejected(self):
        self.assertEqual(fb.main(self.argv("--formation", "five")), 2)

    def test_a_negative_delay_or_no_repeats_is_rejected(self):
        self.assertEqual(fb.main(self.argv("--delay", "-1")), 2)
        self.assertEqual(fb.main(self.argv("--repeats", "0")), 2)

    def test_a_missing_base_tape_is_rejected(self):
        self.assertEqual(fb.main(self.argv()), 2)

    def test_an_unknown_policy_is_rejected_by_the_parser(self):
        with self.assertRaises(SystemExit):
            fb.main(self.argv("--policy", "sing"))

    def test_parse_formation_takes_decimal_and_hex(self):
        self.assertEqual(fb.parse_formation("5", self.pack), 5)
        self.assertEqual(fb.parse_formation("0x5", self.pack), 5)


if __name__ == "__main__":
    unittest.main()
