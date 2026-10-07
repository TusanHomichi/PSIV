"""Scripts are validated before the emulator and expanded from observed menus."""
import contextlib
import io
import json
import pathlib
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock

from oracle.force import ForceError, cli, phases, runs
from oracle.force.script import Command, Script, party_state, patch_state, validate
from oracle.force.script_menu import (command_buttons, confirmation_ready,
                                     list_buttons, presses, verify_commands)


def state():
    raw = bytearray(65536)
    raw[0xF40A:0xF40F] = bytes([1, 0xFF, 0xFF, 0xFF, 0xFF])
    raw[0xF58E:0xF594] = bytes.fromhex("00350035000A")
    raw[0xF5D2:0xF5D5] = bytes([1, 30, 31])
    raw[0xF5E2] = 6
    raw[0xF5EA] = 2
    raw[0xF410] = 57
    return bytes(raw)


def records():
    return {"technique": {1: {"targeting": {"raw": 17}, "tp_cost": 3}},
            "skill": {6: {"targeting": {"raw": 18}}},
            "item": {57: {"targeting_or_parameter_3": 2,
                          "battle_object_or_graphic_id": 2}}}


class Parsing(unittest.TestCase):
    def test_party_script_refuses_vehicle_only_selector_before_any_capture(self):
        args = cli.parser().parse_args(["--formation", "1", "--party-script", "commands.json",
                                       "--out", "unused"])
        with mock.patch.object(phases, "field_layout", return_value={}), \
                mock.patch.object(phases.Pack, "load"), \
                mock.patch.object(phases, "parse_formation", return_value=1), \
                mock.patch.object(phases, "choose_selector", return_value=SimpleNamespace(kind="vehicle")), \
                mock.patch.object(phases, "scout") as scout:
            with self.assertRaisesRegex(ForceError, "on-foot formation"):
                phases.run(args)
            scout.assert_not_called()

    def test_all_commands_and_hex_ids(self):
        kinds = [{"command": "attack", "target": 6},
                 {"command": "technique", "id": "0x1", "target": 7},
                 {"command": "skill", "id": 6, "target": -1},
                 {"command": "item", "id": 57, "target": -1},
                 {"command": "defend"}]
        script = Script.parse({"rounds": [{"1": value} for value in kinds], "repeat_last": True})
        self.assertEqual([r[1].kind for r in script.rounds],
                         ["attack", "technique", "skill", "item", "defend"])
        self.assertEqual(script.round(99)[1], Command("defend"))

    def test_strict_schema(self):
        values = [{}, {"rounds": []}, {"rounds": [{}]},
                  {"rounds": [{"6": {"command": "defend"}}]},
                  {"rounds": [{"1": {"command": "technique", "id": True, "target": 6}}]},
                  {"rounds": [{"1": {"command": "defend", "target": 6}}]},
                  {"rounds": [{"1": {"command": "attack"}}]},
                  {"rounds": [{"1": {"command": "item", "id": 0, "target": -1}}]},
                  {"rounds": [{"1": {"command": "defend"}}], "repeat_last": 1}]
        for value in values:
            with self.subTest(value=value), self.assertRaises(ForceError):
                Script.parse(value)

    def test_finite_script_refuses_an_unwritten_round(self):
        with self.assertRaisesRegex(ForceError, "round 2"):
            Script.parse({"rounds": [{"1": {"command": "defend"}}]}).round(1)

    def test_cap_boundary_does_not_require_an_extra_finite_command_round(self):
        script = Script.parse({"rounds": [{"1": {"command": "attack", "target": 6}}]})
        self.assertEqual(script.command(1, 0, 0, 1), Command("attack", 6))
        self.assertEqual(script.command(1, 1, 1, 1), Command("defend"))
        with self.assertRaises(ForceError):
            script.command(1, 1, 1, 0)
        with self.assertRaises(ForceError):
            script.round(-1)


class Validation(unittest.TestCase):
    def test_actual_party_and_inventory(self):
        self.assertEqual(party_state(state())[1]["technique"][:3], [1, 30, 31])
        for value in ({"command": "technique", "id": 1, "target": 6},
                      {"command": "skill", "id": 6, "target": -1},
                      {"command": "item", "id": 57, "target": -1}):
            validate(Script.parse({"rounds": [{"1": value}]}), state(), records())

    def test_unknown_technique_is_rejected_before_any_frame_runs(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory)
            script = path / "script.json"
            script.write_text(json.dumps({"rounds": [{"1": {
                "command": "technique", "id": 2, "target": 6}}]}))
            raw = path / "state.bin"
            raw.write_bytes(state())
            args = cli.parser().parse_args(["--formation", "0x126", "--out", directory,
                                            "--party-script", str(script)])
            facts = {"battle_first": 40, "script_state": {
                "path": str(raw), "sha256": runs.sha256(raw), "frame": 40}}
            defs = records()
            defs["technique"][2] = defs["technique"][1]
            stderr = io.StringIO()
            with mock.patch.object(phases.Pack, "load"), \
                    mock.patch.object(phases, "parse_formation", return_value=0x126), \
                    mock.patch.object(phases, "choose_selector", return_value=mock.Mock(kind="event")), \
                    mock.patch.object(phases, "scout", return_value=facts), \
                    mock.patch("oracle.force.script.definitions", return_value=defs), \
                    mock.patch.object(runs, "run_oracle") as run, \
                    contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(stderr):
                self.assertEqual(cli.main(["--formation", "0x126", "--out", directory,
                                           "--party-script", str(script)]), 2)
                run.assert_not_called()
                self.assertIn("does not know technique 2", stderr.getvalue())

    def test_invalid_target_missing_item_and_insufficient_tp(self):
        for value in ({"command": "attack", "target": 1},
                      {"command": "technique", "id": 1, "target": -1},
                      {"command": "item", "id": 57, "target": 6}):
            with self.assertRaises(ForceError):
                validate(Script.parse({"rounds": [{"1": value}]}), state(), records())
        raw = bytearray(state())
        raw[0xF410] = 0
        with self.assertRaisesRegex(ForceError, "no item"):
            validate(Script.parse({"rounds": [{"1": {"command": "item", "id": 57,
                                                       "target": -1}}]}), raw, records())
        raw = bytearray(state())
        raw[0xF592:0xF594] = b"\x00\x02"
        with self.assertRaisesRegex(ForceError, "cannot afford"):
            validate(Script.parse({"rounds": [{"1": {"command": "technique", "id": 1,
                                                       "target": 6}}]}), raw, records())

    def test_fixture_patches_cannot_write_commands_or_use_a_stale_frame(self):
        self.assertEqual(patch_state(state(), ["41:FFFFF410:39"], 41)[0xF410], 57)
        for spec in ("41:FFFF410A:0201", "40:FFFFF410:39", "41:FFFFFA7F:0102"):
            with self.assertRaises(ForceError):
                patch_state(state(), [spec], 41)


class MenuSequence(unittest.TestCase):
    def test_initialized_results_confirm_but_high_bit_command_lists_do_not(self):
        self.assertTrue(confirmation_ready(0x2D))
        self.assertTrue(confirmation_ready(0x802D))
        self.assertTrue(confirmation_ready(0x802F))
        self.assertTrue(confirmation_ready(0x8020))
        for routine in (0x8005, 0x8011, 0x8017, 0x8001, 0):
            self.assertFalse(confirmation_ready(routine))

    def row(self, routine):
        return {"battle_routine_2": f"{routine:04X}", "battle_total_comd": "0",
                "menu_command_0": "0"}

    def test_each_command_on_horizontal_strip(self):
        expected = {"attack": ["C"], "technique": ["R", "C"],
                    "skill": ["R", "R", "C"], "item": ["L", "L", "C"],
                    "defend": ["L", "C"]}
        for kind, buttons in expected.items():
            self.assertEqual(command_buttons(self.row(5), Command(kind)), buttons)
            self.assertEqual([(s.frames, s.buttons) for s in presses(buttons)],
                             [value for b in buttons for value in [(4, b), (12, ".")]])

    def test_actual_reverse_tech_skill_and_equipment_first_item_lists(self):
        for routine, kind, count, values, ability in (
                (8, "technique", 16, [31, 30, 1], 1),
                (0x11, "skill", 8, [9, 6], 6),
                (0x17, "item", 44, [2, 5, 4, 126, 57], 57)):
            row = self.row(routine)
            row.update({f"menu_{kind}_{i}": str(values[i] if i < len(values) else 0)
                        for i in range(count)})
            row[f"menu_{kind}_cursor_0"] = "0"
            self.assertEqual(command_buttons(row, Command(kind, -1, ability)),
                             ["R"] if ability == 57 else ["D"] * values.index(ability) + ["C"])
        self.assertEqual(list_buttons(4, 5), ["D", "C"])
        self.assertEqual(list_buttons(7, 4), ["D", "C"])

    def test_disabled_or_missing_live_entry_refuses_the_command(self):
        row = self.row(8)
        row.update({f"menu_technique_{i}": "129" if i == 0 else "0" for i in range(16)})
        row["menu_technique_cursor_0"] = "0"
        with self.assertRaisesRegex(ForceError, "live technique list rejects"):
            command_buttons(row, Command("technique", 6, 1))

    def test_queue_readback_rejects_ignored_command_id_or_target_inputs(self):
        raw = bytearray(65536)
        raw[0x410A:0x410E] = bytes([2, 1, 0, 6])
        commands = {1: Command("technique", 6, 1)}
        verify_commands(raw, commands, records(), [1, 6])
        for offset, value in ((0x410A, 1), (0x410B, 30), (0x410D, 7)):
            changed = bytearray(raw)
            changed[offset] = value
            with self.assertRaises(ForceError):
                verify_commands(changed, commands, records(), [1, 6])

    def test_sparse_enemy_and_party_target_cursors(self):
        for routine, target, current, wanted in ((0xC, 9, 0, ["R", "C"]),
                                                (0xD, 3, 1, ["C"])):
            row = self.row(routine)
            row.update({f"menu_object_{i}": str(int(i in (1, 3, 6, 9))) for i in range(1, 10)})
            row["battle_enemy_index" if routine == 0xC else "battle_char_index"] = str(current)
            row["menu_target_x"] = "320"
            row[f"menu_object_x_{target}"] = "320"
            row["menu_object_x_1"] = "200"
            self.assertEqual(command_buttons(row, Command("technique", target, 1)), wanted)
        row["battle_char_index"] = "0"
        row["menu_target_x"] = "312"  # different sprite/body anchors
        self.assertEqual(command_buttons(row, Command("technique", target, 1)), ["R", "C"])
