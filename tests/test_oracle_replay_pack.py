"""The replay mirror keeps party and enemy command namespaces separate."""
import json
import pathlib
import tempfile
import unittest

from oracle.sweep.replay_pack import fixture_enemies, party_records


class ReplayPack(unittest.TestCase):
    def write(self, path, document):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(document))

    def test_party_action_ids_do_not_select_enemy_skill_records(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            self.write(root / "capture.json", {
                "formation": {"enemies": [{"enemy_id": 123}]},
                "rounds": [{"actions": [
                    {"actor": 1, "kind": "technique", "ability": 99},
                    {"actor": 2, "kind": "item", "ability": 98},
                    {"actor": 6, "kind": "ability", "ability": 7},
                    {"actor": 7, "kind": "attack", "ability": 8},
                ]}],
            })
            self.write(root / "mirror.json", {"enemies": []})
            self.assertEqual(fixture_enemies(root), ({123}, {7}))

    def test_only_selected_party_records_are_copied_from_decoded_pack(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            fixtures, runtime = root / "fixtures", root / "runtime"
            self.write(fixtures / "capture.json", {"rounds": [{"commands": [
                {"command": "technique", "ability": 3},
                {"command": "skill", "ability": 4},
                {"command": "item", "ability": 5},
                {"command": "defend"},
            ]}]})
            common = {"display_name": "synthetic", "effect_id": 42,
                      "power_or_hit_chance": 61, "resistance_stat": {"id": 2},
                      "element": {"id": 3}}
            self.write(runtime / "battle/abilities.json", {
                "techniques": [dict(common, id=3, targeting={"raw": 17}, tp_cost=9),
                               dict(common, id=99, targeting={"raw": 18}, tp_cost=1)],
                "skills": [dict(common, id=4, targeting={"raw": 18},
                                relevant_stat={"id": 1}, requires_weapon=True)],
                "item_effects": [dict(common, id=5, parameter_2=63,
                                      targeting_or_parameter_3=18,
                                      battle_object_or_graphic_id=7)],
            })
            self.write(runtime / "battle/equipment.json", {
                "items": [{"id": 5, "type": {"id": 8}}],
            })
            records = party_records(runtime, fixtures)
            self.assertEqual([record["id"] for record in records["techniques"]], [3])
            technique = records["techniques"][0]
            self.assertEqual((technique["effect"], technique["power"], technique["cost"]),
                             (42, 61, 9))
            skill = records["skills"][0]
            self.assertEqual((skill["targeting"], skill["power_stat"],
                              skill["requires_weapon"]), (18, 1, True))
            item = records["battle_items"][0]
            self.assertEqual((item["actor_power"], item["targeting"], item["object"],
                              item["consumable"]), (63, 2, 7, True))

    def test_missing_decoded_command_record_is_not_silently_omitted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            fixtures, runtime = root / "fixtures", root / "runtime"
            self.write(fixtures / "capture.json", {"rounds": [{"commands": [
                {"command": "technique", "ability": 3},
            ]}]})
            self.write(runtime / "battle/abilities.json", {
                "techniques": [], "skills": [], "item_effects": [],
            })
            self.write(runtime / "battle/equipment.json", {"items": []})
            with self.assertRaises(KeyError):
                party_records(runtime, fixtures)
