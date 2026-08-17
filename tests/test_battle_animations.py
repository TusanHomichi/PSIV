import unittest
from pathlib import Path

from psiv_tools.battle_animations import build_enemy_animations
from psiv_tools.core import EXPECTED_SHA256, read_rom

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestBattleAnimations(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.payload = build_enemy_animations(read_rom(ROM))

    def test_retail_provenance_is_fail_closed_against_grand_cross(self):
        source = self.payload["source"]
        self.assertEqual(source["rom_sha256"], EXPECTED_SHA256)
        self.assertEqual(source["grand_cross"], 0)
        self.assertEqual(source["reference_grand_cross"], 1)
        self.assertEqual(source["record_count"], 153)
        self.assertTrue(source["tables"])
        self.assertTrue(all(table["grand_cross"] == 0 for table in source["tables"]))

    def test_every_enemy_has_exact_sfx_and_no_generic_fallback(self):
        self.assertEqual(self.payload["count"], 153)
        census = self.payload["census"]
        self.assertEqual(census["exact_sfx"], 153)
        self.assertEqual(census["generic_sfx"], 0)
        self.assertEqual(census["frame_sequence_records"], 79)
        self.assertEqual(census["timed_frame_sequences"], 79)
        self.assertEqual(
            census["timed_frame_sequences"] + census["frame_sequence_deferred"],
            153,
        )
        for animation in self.payload["animations"]:
            self.assertTrue(
                any(
                    write["dispatch"] == "Sound_Index"
                    and write["sound_id"] == animation["sfx_id"]
                    for write in animation["sfx_writes"]
                ),
                animation["enemy_id"],
            )

    def test_two_enemy_records_retain_distinct_retail_ids(self):
        records = {record["enemy_id"]: record for record in self.payload["animations"]}
        self.assertEqual(
            (records[2]["sfx_id"], records[2]["sfx_name"]),
            (0xD6, "MechEnemyAlarm"),
        )
        self.assertEqual(
            (records[10]["sfx_id"], records[10]["sfx_name"]),
            (0xD8, "EnemyAttack4"),
        )
        self.assertNotEqual(records[2]["sfx_id"], records[10]["sfx_id"])

    def test_frame_records_are_structured_or_explicitly_deferred(self):
        for animation in self.payload["animations"]:
            sequence = animation["frame_sequence"]
            if sequence is None:
                self.assertFalse(animation["flash_timing_proven"])
                continue
            self.assertEqual(sequence["frame_timer_helper"], "loc_256AE")
            self.assertGreater(sequence["frame_duration"], 0)
            self.assertGreater(sequence["frame_count"], 0)
            self.assertEqual(
                sequence["total_frames"],
                sequence["frame_duration"] * sequence["frame_count"],
            )
            self.assertTrue(animation["flash_timing_proven"])

    def test_mapping_records_carry_the_retail_six_byte_shape(self):
        zoran = self.payload["animations"][10]
        sequence = zoran["frame_sequence"]
        self.assertEqual(zoran["composition"]["status"], "exact")
        self.assertEqual(zoran["sprite_sheet_proven"], True)
        self.assertEqual(sequence["mapping_records"][0]["entry_bytes"], 6)
        entry = sequence["mapping_records"][0]["entries"][0]
        self.assertTrue(
            set(("y", "size", "tile_word", "x", "x_mirror")) <= set(entry)
        )
        self.assertTrue(entry["tile_bank_valid"])

    def test_three_way_census_and_two_distinct_playback_fixtures(self):
        census = self.payload["census"]
        self.assertEqual(
            (census["sprite_sheet_exact"], census["sprite_sheet_partial"],
             census["sprite_sheet_status_deferred"]),
            (60, 15, 78),
        )
        self.assertEqual(
            (census["movement_exact"], census["movement_partial"],
             census["movement_status_deferred"]),
            (30, 49, 74),
        )
        records = {record["enemy_id"]: record for record in self.payload["animations"]}
        zoran = records[10]
        twin_arms = records[87]
        self.assertEqual(zoran["movement"]["runtime"]["initial_offset_pixels"], [0, 16])
        self.assertEqual(twin_arms["movement"]["runtime"]["kind"], "linear")
        self.assertEqual(twin_arms["movement"]["runtime"]["step_pixels"], [0, 64])
        self.assertEqual(
            [zoran["frame_sequence"]["frame_count"], twin_arms["frame_sequence"]["frame_count"]],
            [8, 4],
        )

    def test_extraction_is_deterministic(self):
        self.assertEqual(self.payload, build_enemy_animations(read_rom(ROM)))
