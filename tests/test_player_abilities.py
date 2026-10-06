"""Inventory proof boundaries: decoded universe, observed route and executions."""
import contextlib
import hashlib
import io
import json
import pathlib
import tempfile
import unittest
from unittest import mock

from oracle.fixture import FixtureError
from oracle.fixture.player_state import attach_start_ram
from oracle.sweep import player_abilities as ledger
from oracle.sweep.player_capture import patches, require_executed, verify_report
from oracle.fixture.observations import battle_start
from oracle.sweep.player_chains import Chains
from oracle.force.script import patch_state, party_state
from oracle.force.script_menu import party_address, write_map
from oracle.force import runs
from oracle.sweep.replay_pack import party_records
from oracle.fixture.roles import animation_hit_pass_actors


def row(kind="technique", id_=1, name="SAMPLE"):
    return [kind, str(id_), name, "Effect (ps4.asm:1)", "Object (ps4.asm:2)",
            "single", "damage", "unsupported", "not captured", "-"]


def records():
    return {"techniques": [{"id": 1, "display_name": "SAMPLE", "targeting": {"raw": 17}}],
            "skills": [{"id": 2, "display_name": "OTHER", "targeting": {"raw": 17}}]}


def engine():
    return [{"kind": kind, "id": id_, "battle_supported": True}
            for kind, id_ in (("technique", 1), ("skill", 2))]


class Inventory(unittest.TestCase):
    def test_animation_ids_come_only_from_battle_object_tables(self):
        source = """AbilityEffectsOffs:
 dc.w AbilityEffect_None-AbilityEffectsOffs ; 1
AbilityEffect_None:
 rts
BattleObjsGroup6Ptrs:
 dc.l BattleObj_Sample ; $43C
BattleObj_Sample:
 rts
TechObj_Sample:
 move.w #$43C, d0
 bra.w CharTech_LoadBattleObj
TechObj_Next:
 rts
EventTable:
 dc.l Event_Unrelated ; $43C
Event_Unrelated:
 rts
"""
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "sample.asm"
            path.write_text(source)
            chains = Chains(path)
            animation = chains.animation("TechObj_Sample", "TechObj_")
            self.assertIn("BattleObj_Sample", animation)
            self.assertNotIn("Event_Unrelated", animation)
            path.write_text(source.replace("EventTable:", "BattleObjsGroup7Ptrs:"))
            with self.assertRaisesRegex(ValueError, "duplicate battle object"):
                Chains(path)

    def test_learning_uses_earliest_initial_or_progression_level(self):
        characters = [{"id": 0, "name": "Learner", "level": 4,
                       "initial_techniques": [{"id": 1}], "initial_skills": []}]
        progression = {"characters": [{"character_id": 0, "character": "Learner", "levels": [
            {"level": 3, "new_technique": {"id": 1}, "new_skill": {"id": 2}},
            {"level": 9, "new_technique": {"id": 1}, "new_skill": None}]}]}
        learned = ledger.learn_tables(characters, progression)
        self.assertEqual(learned["technique"][1][0]["level"], 3)
        self.assertEqual(learned["skill"][2][0]["level"], 3)

    def test_route_maxima_require_a_completed_observed_report(self):
        report = {"result": "completed", "chapters": [
            {"party": [{"character_id": 0, "level": 2}]},
            {"party": [{"character_id": 0, "level": 7}, {"character_id": 3, "level": 5}]}]}
        self.assertEqual(ledger.route_levels(report), {0: 7, 3: 5})
        for value in ({"result": "failed", "chapters": report["chapters"]},
                      {"result": "completed", "chapters": []},
                      {"result": "completed", "chapters": [{"party": [{"character_id": True, "level": 1}]}]}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                ledger.route_levels(value)

    def test_ledger_duplicate_or_malformed_rows_are_rejected(self):
        text = "| " + " | ".join(row()) + " |\n"
        self.assertEqual(ledger.ledger_rows(text)[("technique", 1)], row())
        for bad in (text + text, "| technique | 1 | incomplete |"):
            with self.assertRaises(ValueError):
                ledger.ledger_rows(bad)

    def test_saved_grants_are_observed_levels_and_do_not_replace_progression(self):
        learned = {"technique": {1: {0: {"name": "Learner", "level": 3}}}, "skill": {}}
        characters = [{"id": 0, "name": "Learner"}]
        report = {"chapters": [{"party": [{"character_id": 0, "level": level,
                    "techniques": [1], "skills": [2]}]} for level in (9, 7)]}
        ledger.include_saved_learning(learned, report, characters, records())
        self.assertEqual(learned["technique"][1][0]["level"], 3)
        self.assertNotIn("source", learned["technique"][1][0])
        self.assertEqual(learned["skill"][2][0], {"name": "Learner", "level": 7,
                                                 "source": "route snapshot"})
        report["chapters"][0]["party"][0]["skills"] = [255]
        with self.assertRaisesRegex(ValueError, "unknown saved skill 255"):
            ledger.include_saved_learning(learned, report, characters, records())

    def test_missing_ability_names_it_before_any_rust_process(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "battle").mkdir()
            (root / "battle/abilities.json").write_text(json.dumps(records()))
            path = root / "ledger.md"
            path.write_text("| " + " | ".join(row()) + " |\n")
            error = io.StringIO()
            with mock.patch.object(ledger, "fresh_engine") as rust, contextlib.redirect_stderr(error):
                status = ledger.main(["--runtime-pack", str(root), "--ledger", str(path)])
            self.assertEqual(status, 2)
            self.assertIn("ledger lacks skill 2 OTHER", error.getvalue())
            rust.assert_not_called()

    def test_engine_universe_cannot_omit_or_duplicate_an_ability(self):
        rows = {("technique", 1): row(), ("skill", 2): row("skill", 2, "OTHER")}
        for gates in (engine()[:1], engine() + engine()[:1]):
            with self.assertRaisesRegex(ValueError, "decoded ability universe"):
                ledger.inventory(records(), {"technique": {}, "skill": {}}, {}, rows, gates, {})

    def test_successful_cargo_without_fresh_output_cannot_reuse_an_old_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            output = root / "old.json"
            output.write_text(json.dumps(engine()))
            result = mock.Mock(returncode=0, stdout="running 0 tests", stderr="")
            with mock.patch.object(ledger.subprocess, "run", return_value=result):
                with self.assertRaisesRegex(ValueError, "did not emit fresh output"):
                    ledger.fresh_engine(root, output)
            self.assertEqual(json.loads(output.read_text()), engine())

    def test_route_and_status_are_derived_and_update_is_idempotent(self):
        rows = {("technique", 1): row(), ("skill", 2): row("skill", 2, "OTHER")}
        learned = {"technique": {1: {0: {"name": "Learner", "level": 5}}}, "skill": {}}
        values = ledger.inventory(records(), learned, {0: 5}, rows, engine(), {})
        self.assertTrue(values[0]["route"])
        self.assertFalse(values[1]["route"])
        text = "\n".join("| " + " | ".join(r) + " |" for r in rows.values()) + "\n"
        updated = ledger.update(text, values)
        self.assertIn("implemented | not captured | route; Learner L5", updated)
        self.assertEqual(ledger.update(updated, values), updated)

    def test_capture_counts_executed_party_actions_not_commands_or_enemies(self):
        fixture = {"formation": {}, "rounds": [{"round": 2, "commands": [
            {"command": "skill", "ability": 99}], "actions": [
            {"actor": 2, "kind": "skill", "ability": 2, "start_frame": 12},
            {"actor": 6, "kind": "technique", "ability": 1, "start_frame": 13}]}]}
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory)
            (path / "case.json").write_text(json.dumps(fixture))
            found = ledger.captures(path)
        self.assertEqual(set(found), {("skill", 2)})
        self.assertEqual(found[("skill", 2)][0]["round"], 2)
        case = {"rounds": [{"1": {"command": "skill", "id": 99, "target": 6}}]}
        with self.assertRaisesRegex(ValueError, "did not execute.*99"):
            require_executed(case, fixture)

    def test_patch_recipes_are_bounded_and_preserve_unrequested_record_bytes(self):
        raw = bytearray(65536)
        raw[0xF40A:0xF40F] = bytes((1, 255, 255, 255, 255))
        raw[0xF580] = 77
        case = {"defaults": {"tp": 500, "mental": 7}, "rounds": [{"1": {
            "command": "skill", "id": 2, "target": 6}}]}
        specs = patches(case, raw, 9)
        self.assertEqual(len(specs), 2)
        first = bytes.fromhex(specs[0].split(":")[2])
        self.assertEqual(len(first), 64)
        self.assertEqual(first[0], 77)
        self.assertEqual(int.from_bytes(first[18:20], "big"), 500)
        self.assertEqual(first[27:30], bytes((7, 7, 7)))
        case["defaults"]["cursor"] = 1
        with self.assertRaisesRegex(ValueError, "unknown party field cursor"):
            patches(case, raw, 9)

    def test_resumed_extraction_rejects_changed_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "evidence"
            path.write_bytes(b"stock observation")
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            report = {key: str(path) for key in
                      ("trace", "log", "rerun_log", "start_ram", "party_script")}
            report.update({key + "_sha256": digest for key in tuple(report)})
            verify_report(report)
            path.write_bytes(b"changed observation")
            with self.assertRaisesRegex(ValueError, "capture trace hash changed"):
                verify_report(report)

    def test_substituted_party_observations_follow_actual_records(self):
        raw = bytearray(65536)
        raw[0xF40A:0xF40F] = bytes((1, 0, 2, 255, 255))
        raw[0xF780] = 99
        case = {"characters": [5, 0, 7], "rounds": [{str(i): {"command": "defend"}
                                                   for i in range(1, 4)}]}
        changed = patch_state(raw, patches(case, raw, 9), 9)
        party = party_state(changed)
        self.assertEqual([party[i]["character"] for i in range(1, 4)], [5, 0, 7])
        self.assertEqual(changed[0xF780], 99)
        self.assertEqual(party_address("alys_hp", 0xF58E, party), 0xF78E)
        self.assertEqual(party_address("menu_party_0", 0xF40A, party), 0xF40A)
        with tempfile.TemporaryDirectory() as directory:
            out = pathlib.Path(directory) / "map.tsv"
            write_map(runs.DEFAULT_RAM_MAP_TSV, out, party)
            fields = {f["name"]: f for f in json.loads(out.with_suffix(".json").read_text())["fields"]}
            self.assertEqual(fields["alys_hp"]["addr"], "FFFFF78E")
            self.assertIn("alys_hp\tFFFFF78E\t", out.read_text())
            self.assertLess(len(fields), 1024)
        case["characters"] = [5, 5, 7]
        with self.assertRaisesRegex(ValueError, "distinct character"):
            patches(case, raw, 9)

    def test_replay_pack_includes_equipment_from_observed_party_records(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            runtime, fixtures = root / "runtime", root / "fixtures"
            (runtime / "battle").mkdir(parents=True)
            fixtures.mkdir()
            (runtime / "battle/abilities.json").write_text(json.dumps(
                {"techniques": [], "skills": [], "item_effects": []}))
            names = ("strength", "mental", "agility", "dexterity", "attack", "defense", "magic_defense")
            item = {"id": 206, "display_name": "SYNTHETIC", "type": {"id": 5},
                    "element": {"id": 3}, "bonuses": dict.fromkeys(names, -1)}
            (runtime / "battle/equipment.json").write_text(json.dumps({"items": [item]}))
            record = [0] * 128
            record[0x4C] = 206
            (fixtures / "capture.json").write_text(json.dumps({"party": [{"record": record}]}))
            result = party_records(runtime, fixtures)
            self.assertEqual(result["equipment"], [{"id": 206, "name": "SYNTHETIC", "kind": 5,
                                                    "element": 3, "bonuses": [-1] * 7}])
            record[0x4D] = 207
            (fixtures / "capture.json").write_text(json.dumps({"party": [{"record": record}]}))
            with self.assertRaises(KeyError):
                party_records(runtime, fixtures)

    def test_animation_hit_pass_keys_on_substituted_character_identity(self):
        party = [{"id": 1, "name": "ALYS", "character_id": 5},
                 {"id": 2, "name": "CHAZ", "character_id": 9}]
        self.assertEqual(animation_hit_pass_actors(party), {2})


class ObservedStart(unittest.TestCase):
    def test_scripted_start_uses_live_battle_stats_not_base_stat_columns(self):
        class Log:
            @staticmethod
            def has(name):
                return "_player_" in name

            @staticmethod
            def num(frame, name):
                if name.endswith("_maxhp") and name.startswith("e"):
                    return 0
                return 7 if "_player_" in name else 1

        party, enemies = battle_start(Log(), 1)
        self.assertFalse(enemies)
        for member in party:
            for stat in ("strength", "mental", "dexterity", "attack", "defence"):
                self.assertEqual(member[stat], 7)
            self.assertEqual(member["hp"], 1)

    def test_independent_hash_and_csv_cells_are_mandatory(self):
        raw = bytearray(65536)
        raw[0xF40A] = 1
        member = {"id": 1, "level": 7, "hp": 10, "max_hp": 30, "tp": 8,
                  "max_tp": 20, "status": 0, "strength": 2, "mental": 3,
                  "agility": 4, "dexterity": 5, "attack": 6, "defence": 9}
        offsets = {"level": (8, 2), "hp": (14, 2), "max_hp": (16, 2),
                   "tp": (18, 2), "max_tp": (20, 2), "status": (22, 1),
                   "strength": (26, 1), "mental": (29, 1), "agility": (32, 1),
                   "dexterity": (35, 1), "attack": (36, 2), "defence": (40, 2)}
        for field, (offset, width) in offsets.items():
            raw[0xF580 + offset:0xF580 + offset + width] = member[field].to_bytes(width, "big")
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "state.bin"
            path.write_bytes(raw)
            digest = hashlib.sha256(raw).hexdigest()
            fixture = {"party": [dict(member)], "provenance": {"start_frame": 10}}
            attach_start_ram(fixture, path, digest)
            self.assertEqual(fixture["party"][0]["character_id"], 1)
            self.assertEqual(len(fixture["party"][0]["record"]), 128)
            for expected in (None, "wrong"):
                with self.assertRaisesRegex(FixtureError, "hash"):
                    attach_start_ram(fixture, path, expected)
            fixture["party"][0]["tp"] = 99
            with self.assertRaisesRegex(FixtureError, "tp.*8 != CSV 99"):
                attach_start_ram(fixture, path, digest)


if __name__ == "__main__":
    unittest.main()
