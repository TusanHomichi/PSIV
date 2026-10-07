"""The Air Castle capture recipe, `oracle/sweep/air_castle.py`: constructed controls."""
import hashlib
import json
import pathlib
import tempfile
import unittest

from oracle.sweep import air_castle as recipe


class Recipe(unittest.TestCase):
    def test_every_case_is_a_formation_or_an_event_and_names_its_ability(self):
        names = [case.name for case in recipe.CASES]
        self.assertEqual(len(names), len(set(names)))
        for case in recipe.CASES:
            self.assertNotEqual(case.formation is None, case.event is None, case.name)
            self.assertLessEqual(case.kept, case.rounds, case.name)
            self.assertTrue(0 < case.ability < 0x71, case.name)

    def test_only_an_enemy_action_with_the_ability_counts(self):
        fixture = {"rounds": [
            {"round": 1, "actions": [{"actor": 2, "ability": 0x31},
                                     {"actor": 6, "ability": 0}]},
            {"round": 2, "actions": [{"actor": 7, "ability": 0x31}]},
            {"round": 3, "actions": [{"actor": 6, "ability": 0x30}]},
        ]}
        self.assertEqual(recipe.used_in(fixture, 0x31), [2])
        self.assertEqual(recipe.used_in(fixture, 0x2C), [])

    def write_report(self, work: pathlib.Path, case, **extra) -> None:
        out = work / case.name
        out.mkdir(parents=True)
        (out / "report.json").write_text(json.dumps({
            "trace": "t.csv", "log": "l.csv", "tape": "x.tape",
            "battle_first": 10, "battle_last": 20, "durable": {"hp": 999}, **extra}))

    def test_an_event_case_records_its_index_in_the_fixture(self):
        case = recipe.Case("e", "event", 0x64, 3, event=17)
        with tempfile.TemporaryDirectory() as directory:
            work = pathlib.Path(directory)
            self.write_report(work, case)
            argv = recipe.extract_argv(case, work, work / "out.json")
        self.assertIn("--event-battle", argv)
        self.assertEqual(argv[argv.index("--event-battle") + 1], "17")
        self.assertEqual(argv[argv.index("--max-rounds") + 1], "3")
        self.assertEqual(argv[argv.index("--hp-patch") + 1], "999")

    def test_a_scripted_case_uses_the_script_scout_and_its_party_fixture(self):
        case = recipe.Case("s", "script", 0x2C, 2, keep=1, formation=0x164, script={
            "defaults": {"hp": 999, "max_hp": 999},
            "rounds": [{"1": {"command": "defend"}}]})
        with tempfile.TemporaryDirectory() as directory:
            work = pathlib.Path(directory)
            raw = bytearray(0x10000)
            raw[0xF40A:0xF40F] = bytes([1, 255, 255, 255, 255])
            state = work / "state.bin"
            state.write_bytes(bytes(raw))
            (work / "script-scout.json").write_text(json.dumps({
                "battle_first": 100, "script_state": {
                    "path": str(state), "sha256": hashlib.sha256(raw).hexdigest()}}))
            argv = recipe.capture_argv(case, work)
            commands = json.loads((work / "s" / "commands.json").read_text())
            self.assertEqual(argv[argv.index("--scout") + 1], str(work / "script-scout.json"))
            patches = [argv[i + 1] for i, a in enumerate(argv) if a == "--ram-patch"]
            # Alys's record (character 1 at $F580), in two 64-byte chunks, at the
            # seam frame; HP 999 in its current/maximum words.
            self.assertEqual([p.split(":")[:2] for p in patches],
                             [["101", "FFFFF580"], ["101", "FFFFF5C0"]])
            self.assertEqual(patches[0].split(":")[2][28:36], "03e703e7")
            self.assertTrue(commands["repeat_last"])
            # A changed scout state is refused, not used.
            state.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "changed"):
                recipe.capture_argv(case, work)


if __name__ == "__main__":
    unittest.main()
