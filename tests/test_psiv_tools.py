import unittest
from pathlib import Path

from psiv_tools.core import (
    EXPECTED_SHA256,
    TABLES,
    extract_all,
    genesis_checksum,
    hash_info,
    inspect_rom,
    read_rom,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestRetailRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.result = extract_all(cls.data)

    def test_identity(self):
        self.assertEqual(hash_info(self.data)["sha256"], EXPECTED_SHA256)
        self.assertEqual(len(self.data), 3_145_728)

    def test_checksum(self):
        self.assertEqual(genesis_checksum(self.data), 0x05CB)
        self.assertTrue(inspect_rom(self.data)["checksum_ok"])

    def test_layout_signatures(self):
        self.assertTrue(all(v["ok"] for v in self.result["layout_validation"]))

    def test_counts(self):
        self.assertEqual(len(self.result["characters"]), 11)
        self.assertEqual(len(self.result["techniques"]), 40)
        self.assertEqual(len(self.result["skills"]), 54)
        self.assertEqual(len(self.result["combos"]), 15)
        self.assertEqual(len(self.result["vehicles"]), 3)
        self.assertEqual(len(self.result["items"]), 160)
        self.assertEqual(len(self.result["enemies"]), 153)
        self.assertEqual(len(self.result["enemy_skills"]), 112)
        self.assertEqual(self.result["progression"]["total_level_records"], 937)

    def test_chaz(self):
        chaz = self.result["characters"][0]
        self.assertEqual(chaz["name"], "Chaz")
        self.assertEqual(chaz["profession"]["name"], "Hunter")
        self.assertEqual(chaz["level"], 1)
        self.assertEqual(chaz["experience"], 0)
        self.assertEqual(chaz["hp"], 25)
        self.assertEqual(chaz["tp"], 10)
        self.assertEqual(chaz["stats"], {"strength": 8, "mental": 6, "agility": 7, "dexterity": 5})
        self.assertEqual(chaz["initial_techniques"][0]["name"], "Res")
        self.assertEqual(chaz["initial_skills"][0]["name"], "Earth")
        self.assertEqual(chaz["initial_skills"][0]["initial_uses"], 3)

    def test_ability_examples(self):
        foi = self.result["techniques"][0]
        self.assertEqual(foi["name"], "Foi")
        self.assertEqual(foi["tp_cost"], 3)
        self.assertEqual(foi["element"]["name"], "fire")

        crosscut = self.result["skills"][0]
        self.assertEqual(crosscut["name"], "Crosscut")
        self.assertTrue(crosscut["requires_weapon"])
        self.assertEqual(crosscut["relevant_stat"]["name"], "attack")

        combo = self.result["combos"][1]
        self.assertEqual(combo["name"], "Paradinblw")
        self.assertEqual(combo["effect_id"], 1)

    def test_item_examples(self):
        dagger = self.result["items"][0]
        self.assertEqual(dagger["id"], 1)
        self.assertEqual(dagger["symbol"], "Dagger")
        self.assertEqual(dagger["rom_offset"], "0x2A8E28")
        self.assertEqual(dagger["type"]["name"], "one_handed_single_target_weapon")
        self.assertEqual(dagger["bonuses"]["attack"], 2)
        self.assertEqual(dagger["meseta_cost"], 40)
        self.assertEqual([c["name"] for c in dagger["used_by"]], ["Chaz", "Hahn", "Seth"])

        shadow_blade = self.result["items"][136]
        self.assertEqual(shadow_blade["symbol"], "ShadwBlade")
        self.assertEqual(shadow_blade["bonuses"]["mental"], -10)
        self.assertEqual(shadow_blade["bonuses"]["agility"], -5)
        self.assertEqual(shadow_blade["bonuses"]["dexterity"], -10)

        monomate = self.result["items"][124]
        self.assertEqual(monomate["symbol"], "Monomate")
        self.assertEqual(monomate["type"]["name"], "disposable_item")
        self.assertEqual(monomate["meseta_cost"], 20)

    def test_enemy_example(self):
        helex = self.result["enemies"][0]
        self.assertEqual(helex["symbol"], "Helex")
        self.assertEqual(helex["rom_offset"], "0x2816BC")
        self.assertEqual(helex["hp"], 90)
        self.assertEqual(helex["stats"]["attack"], 160)
        self.assertEqual(helex["basic_attack"]["element"]["name"], "physical")
        self.assertEqual(helex["ai"]["regular_ability_ids"], [2] * 8)
        self.assertEqual([a["symbol"] for a in helex["ai"]["regular_abilities"]], ["FlameBolt"] * 8)

    def test_enemy_skill_example(self):
        nothing = self.result["enemy_skills"][0]
        self.assertEqual(nothing["id"], 1)
        self.assertEqual(nothing["symbol"], "Nothing")
        self.assertEqual(nothing["rom_offset"], "0x28336C")
        self.assertEqual(nothing["raw_hex"], "0d00030100000000")

        flame_bolt = self.result["enemy_skills"][1]
        self.assertEqual(flame_bolt["symbol"], "FlameBolt")

    def test_level_progression(self):
        progression = self.result["progression"]
        self.assertTrue(progression["table_block"]["mirror_is_exact"])
        self.assertEqual(progression["table_block"]["size_bytes"], 20_614)
        self.assertEqual(progression["pointer_table"]["rom_offset"], "0x004074")

        chaz = progression["characters"][0]
        self.assertEqual(chaz["starting_level"], 1)
        self.assertEqual(chaz["record_count"], 98)
        level2 = chaz["levels"][0]
        self.assertEqual(level2["level"], 2)
        self.assertEqual(level2["experience_required"], 21)
        self.assertEqual(level2["hp"], 31)
        self.assertEqual(level2["tp"], 13)
        self.assertEqual(level2["stats"], {"strength": 9, "mental": 7, "agility": 8, "dexterity": 6})
        self.assertEqual(level2["skill_uses"], [4, 0, 0, 0, 0, 0, 0, 0])
        self.assertEqual(chaz["levels"][-1]["level"], 99)

    def test_table_boundaries(self):
        item_end = TABLES["items"]["offset"] + TABLES["items"]["record_size"] * TABLES["items"]["count"]
        self.assertEqual(item_end, TABLES["techniques"]["offset"])
        enemy_end = TABLES["enemies"]["offset"] + TABLES["enemies"]["record_size"] * TABLES["enemies"]["count"]
        self.assertEqual(enemy_end, TABLES["enemy_skills"]["offset"])
        mirror_end = int(self.result["progression"]["table_block"]["mirror_end_exclusive"], 16)
        self.assertEqual(mirror_end + 2, TABLES["characters"]["offset"])


if __name__ == "__main__":
    unittest.main()
