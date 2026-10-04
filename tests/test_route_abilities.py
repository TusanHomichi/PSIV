"""Route census controls use constructed tables, not cartridge data."""
import unittest

from oracle.force.pack import Pack
from oracle.sweep.route_abilities import classes, derive, scene_records


class RouteAbilityTests(unittest.TestCase):
    def pack(self):
        return Pack(
            formations={10: {"enemies": [{"enemy": {"id": 1}}]},
                        11: {"enemies": [{"enemy": {"id": 2}}]}},
            enemies={1: {"ai": {"regular_ability_ids": [1, 0],
                                "conditional_ability_ids": [2]}},
                     2: {"ai": {"regular_ability_ids": [3]}},
                     3: {"ai": {"regular_ability_ids": [4]}}},
            groups={7: [10] * 32, 8: [11] * 32},
            maps={5: {"map_symbol": "world", "groups_available": [7],
                      "vehicle_groups": [8]},
                  6: {"map_symbol": "event", "group": None}}, grids={})

    def test_conditional_vehicle_and_event_only_carriers_all_count(self):
        bosses = {9: {"enemies": [{"enemy": {"id": 3}}]}}
        result = derive(self.pack(), bosses, {5, 6}, {9},
                        {i: "damage" for i in range(1, 5)})
        self.assertEqual(result["groups"], [7, 8])
        self.assertEqual([a["id"] for a in result["abilities"]], [1, 2, 3, 4])
        self.assertEqual(result["abilities"][-1]["random_carriers"], [])
        self.assertEqual(result["abilities"][-1]["event_carriers"], [3])
        # Negative control: a conditional-only ability vanishes if the reader
        # omits that half of the AI table, even with identical formations.
        broken = self.pack()
        broken.enemies[1]["ai"]["conditional_ability_ids"] = []
        changed = derive(broken, bosses, {5, 6}, {9}, {i: "damage" for i in range(1, 5)})
        self.assertNotEqual(changed["abilities"], result["abilities"])
        # Negative control: on-foot-only enumeration loses the vehicle carrier.
        broken = self.pack()
        broken.maps[5]["vehicle_groups"] = []
        changed = derive(broken, bosses, {5, 6}, {9}, {i: "damage" for i in range(1, 5)})
        self.assertNotEqual(changed["abilities"], result["abilities"])

    def test_unclassified_ability_is_an_error(self):
        with self.assertRaisesRegex(ValueError, "no inventory classification"):
            derive(self.pack(), {}, {5}, set(), {1: "damage"})

    def test_scene_boundary_does_not_import_an_unselected_battle(self):
        text = """pub static ONE: Scene = Scene { ops: &[
            SceneOp::LoadMap { map: 0x15F, prev_map: 1 },
            SceneOp::StartBattle { index: 8 }, ] };
            pub static TWO: Scene = Scene { ops: &[
            SceneOp::StartBattle { index: 9 }, ] };"""
        self.assertEqual(scene_records(text)["ONE"], ([0x15F], [8]))
        self.assertEqual(scene_records(text)["TWO"], ([], [9]))

    def test_inventory_class_is_not_inferred_from_ability_name(self):
        text = "\n".join((
            "| `$02` (2) damage-ish name | record | chain | effect | carrier | status/stat effect | unsupported |",
            "| `$03` (3) harmless name | record | chain | effect | carrier | damage † | unsupported |",
            "| `$04` (4) name | record | chain | effect | carrier | — | implemented enemy_damage::resolve_damage_skill |"))
        self.assertEqual(classes(text), {2: "status/stat effect", 3: "damage", 4: "damage"})


if __name__ == "__main__":
    unittest.main()
