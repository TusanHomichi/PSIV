import json
import struct
import tempfile
import unittest
from pathlib import Path

from psiv_tools.battle_pack import (
    ABILITY_EFFECTS_OFFS,
    ABILITY_EFFECT_NONE,
    ABILITY_KINDS,
    BATTLE_DIRECTORY,
    FORMATION_HEADER_RENAMES,
    ABILITIES_NAME,
    ENEMIES_NAME,
    ENEMY_ANIMATIONS_NAME,
    FORMATIONS_NAME,
    LEVELS_NAME,
    BattlePackError,
    ability_effect_count,
    emit_battle,
)
from psiv_tools.core import read_rom
from psiv_tools.pack import PACK_FORMAT_VERSION

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"

#: `docs/BATTLE_SCOUT.md` section 11 finding 2.
BLACK_WAVE_ID = 112
BLACK_WAVE_EFFECT = 0x2C
BLACK_WAVE_TARGET = "0x00B033"
#: The one formation whose declared count disagrees with its entries.
COUNT_MISMATCH_FORMATION = 0x177


class TestFormationRenames(unittest.TestCase):
    """The one place the pack's vocabulary differs from the extractor's."""

    def test_every_rename_lands_on_a_distinct_name(self):
        self.assertEqual(
            len(set(FORMATION_HEADER_RENAMES.values())), len(FORMATION_HEADER_RENAMES)
        )
        self.assertNotEqual(
            set(FORMATION_HEADER_RENAMES), set(FORMATION_HEADER_RENAMES.values())
        )
        self.assertEqual(FORMATION_HEADER_RENAMES["surprise_agility"], "ambush_chance")
        self.assertEqual(FORMATION_HEADER_RENAMES["enemy_count"], "count")


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestBattlePack(unittest.TestCase):
    """One emission, inspected file by file."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls._temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls._temp.name) / "pack"
        cls.fragment = emit_battle(cls.data, cls.root, PACK_FORMAT_VERSION)
        cls.files = {
            name: json.loads((cls.root / name).read_text())
            for name in (
                ENEMIES_NAME,
                ENEMY_ANIMATIONS_NAME,
                FORMATIONS_NAME,
                LEVELS_NAME,
                ABILITIES_NAME,
            )
        }

    @classmethod
    def tearDownClass(cls):
        cls._temp.cleanup()

    # ------------------------------------------------------------ the effects
    def test_the_effect_table_bounds_itself(self):
        # Each entry is an offset from the table base, so the table ends where
        # its lowest target begins: AbilityEffect_None's `rts`.
        count = ability_effect_count(self.data)
        self.assertEqual(count, 44)
        self.assertEqual(ABILITY_EFFECT_NONE - ABILITY_EFFECTS_OFFS, count * 2)
        self.assertEqual(self.data[ABILITY_EFFECT_NONE:ABILITY_EFFECT_NONE + 2], b"\x4e\x75")

    def test_a_rom_whose_effect_table_is_not_bounded_is_refused(self):
        broken = bytearray(self.data)
        broken[ABILITY_EFFECT_NONE:ABILITY_EFFECT_NONE + 2] = b"\x00\x00"
        with self.assertRaises(BattlePackError):
            ability_effect_count(bytes(broken))

    def test_black_wave_is_flagged_rather_than_dispatched(self):
        # Effect $2C is one past the 44-entry table. The dispatcher has no
        # bound, so reading the word where entry 44 would be and adding it to
        # the base gives the odd address the cartridge would jump to.
        abilities = self.files[ABILITIES_NAME]
        black_wave = next(
            record for record in abilities["enemy_skills"]
            if record["id"] == BLACK_WAVE_ID
        )
        self.assertEqual(black_wave["effect_id"], BLACK_WAVE_EFFECT)
        self.assertTrue(black_wave["effect_out_of_range"])
        self.assertEqual(black_wave["effect_dispatch_target"], BLACK_WAVE_TARGET)
        self.assertEqual(black_wave["display_name"], "BLACK WAVE")
        # The target is odd, which is what makes it an address error and not
        # merely a wrong routine.
        self.assertEqual(int(BLACK_WAVE_TARGET, 16) % 2, 1)
        # And it is the only one in the whole set.
        out_of_range = self.fragment["census"]["abilities"]["effect_out_of_range"]
        self.assertEqual(len(out_of_range), 1)
        self.assertEqual(out_of_range[0]["id"], BLACK_WAVE_ID)
        self.assertEqual(
            [record["id"] for kind in ABILITY_KINDS
             for record in abilities[kind] if record["effect_out_of_range"]],
            [BLACK_WAVE_ID],
        )
        self.assertFalse(self.fragment["ability_effects"]["bounds_checked"])

    # ------------------------------------------------------------- the counts
    def test_the_counts_are_what_the_cartridge_holds(self):
        files = self.fragment["files"]
        self.assertEqual(files["enemies"]["count"], 153)
        self.assertEqual(files["formations"]["count"], 504)
        self.assertEqual(files["formations"]["boss_count"], 27)
        self.assertEqual(files["formations"]["encounter_groups"], 68)
        self.assertEqual(files["levels"]["characters"], 11)
        self.assertEqual(files["levels"]["records"], 937)
        self.assertEqual(files["abilities"]["techniques"], 40)
        self.assertEqual(files["abilities"]["skills"], 54)
        self.assertEqual(files["abilities"]["enemy_skills"], 112)
        self.assertEqual(files["abilities"]["item_effects"], 160)
        self.assertEqual(files["enemy_animations"]["count"], 153)
        self.assertEqual(files["enemy_animations"]["exact_sfx"], 153)
        self.assertEqual(files["enemy_animations"]["generic_sfx"], 0)

    def test_every_file_declares_the_pack_version_and_its_kind(self):
        kinds = {
            ENEMIES_NAME: "battle_enemies",
            ENEMY_ANIMATIONS_NAME: "battle_enemy_animations",
            FORMATIONS_NAME: "battle_formations",
            LEVELS_NAME: "battle_levels",
            ABILITIES_NAME: "battle_abilities",
        }
        for name, kind in kinds.items():
            with self.subTest(file=name):
                payload = self.files[name]
                self.assertEqual(payload["format_version"], PACK_FORMAT_VERSION)
                self.assertEqual(payload["kind"], kind)
                self.assertTrue(name.startswith(f"{BATTLE_DIRECTORY}/"))

    # ------------------------------------------------------------- enemies
    def test_an_enemy_carries_stats_properties_ai_and_rewards(self):
        helex = self.files[ENEMIES_NAME]["enemies"][0]
        self.assertEqual((helex["id"], helex["symbol"], helex["display_name"]),
                         (0, "Helex", "HELEX"))
        self.assertEqual(helex["hp"], 90)
        self.assertEqual(helex["stats"]["strength"], 10)
        self.assertEqual(helex["stats"]["attack"], 160)
        # Fourteen element properties, every one a value with its meaning.
        self.assertEqual(len(helex["properties"]), 14)
        self.assertEqual(helex["properties"]["water"], {"value": 4, "meaning": "very_weak"})
        self.assertEqual(helex["properties"]["fire"], {"value": 0, "meaning": "immune"})
        # The AI lists, ids only -- the abilities themselves are in
        # abilities.json and resolving them here would duplicate that file.
        self.assertEqual(len(helex["ai"]["condition_ids"]), 4)
        self.assertEqual(len(helex["ai"]["conditional_ability_ids"]), 4)
        self.assertEqual(len(helex["ai"]["regular_ability_ids"]), 8)
        self.assertEqual(sorted(helex["ai"]), ["condition_ids",
                                               "conditional_ability_ids",
                                               "regular_ability_ids"])
        self.assertIn("experience", helex["rewards"])
        self.assertIn("meseta", helex["rewards"])

    def test_the_property_scale_is_five_values(self):
        # `immune / resistant / normal / weak / very_weak`, and nothing else in
        # 153 records x 14 slots.
        census = self.fragment["census"]["enemies"]
        self.assertEqual(sorted(census["property_values"], key=int),
                         ["0", "1", "2", "3", "4"])
        self.assertEqual(sum(census["property_values"].values()), 153 * 14)
        self.assertEqual(census["property_slots"], 14)

    def test_every_ai_ability_id_resolves_to_an_enemy_skill(self):
        # The AI lists index the enemy-skill table, so an id past it would be a
        # dangling reference the runtime could not follow.
        skills = {record["id"] for record in self.files[ABILITIES_NAME]["enemy_skills"]}
        used = set(self.fragment["census"]["enemies"]["ability_ids_used"])
        # Zero is "no ability", not a skill id.
        self.assertEqual(used - {0} - skills, set())

    # ---------------------------------------------------------- formations
    def test_a_formation_uses_the_packs_header_names(self):
        first = self.files[FORMATIONS_NAME]["formations"][0]
        for old, new in FORMATION_HEADER_RENAMES.items():
            with self.subTest(field=new):
                self.assertIn(new, first)
                self.assertNotIn(old, first)
        self.assertEqual(first["ambush_chance"], 16)
        self.assertEqual(first["run_chance"], 0)
        self.assertEqual(first["drop_rate"], 8)
        self.assertEqual(first["drop_item"], 128)
        self.assertEqual(first["count"], 2)
        self.assertEqual(first["group_1_mask"], "0x03")
        # (enemy, position) pairs, with the slot and the groups it belongs to.
        self.assertEqual(
            first["enemies"],
            [{"slot": 1, "enemy_id": 1, "position": 14, "groups": [1]},
             {"slot": 2, "enemy_id": 1, "position": 26, "groups": [1]}],
        )

    def test_the_one_count_mismatch_is_carried_as_census(self):
        # Formation $177 declares four enemies and lists three. The
        # disassembly's uncompressed source has the same bytes, so it is the
        # ROM's own inconsistency; the pack reports it rather than repairing it.
        census = self.fragment["census"]["formations"]
        self.assertEqual(census["count_mismatches"], [COUNT_MISMATCH_FORMATION])
        formation = next(
            f for f in self.files[FORMATIONS_NAME]["formations"]
            if f["id"] == COUNT_MISMATCH_FORMATION
        )
        self.assertFalse(formation["count_matches_entries"])
        self.assertEqual(formation["count"], 4)
        self.assertEqual(len(formation["enemies"]), 3)

    def test_every_formation_enemy_id_resolves(self):
        enemies = {record["id"] for record in self.files[ENEMIES_NAME]["enemies"]}
        payload = self.files[FORMATIONS_NAME]
        used = {
            slot["enemy_id"]
            for group in ("formations", "boss_formations")
            for formation in payload[group]
            for slot in formation["enemies"]
        }
        self.assertTrue(used)
        self.assertEqual(used - enemies, set())

    def test_the_encounter_groups_and_their_bindings(self):
        payload = self.files[FORMATIONS_NAME]
        groups = payload["encounter_groups"]
        self.assertEqual(groups["group_count"], 68)
        self.assertEqual(groups["entries_per_group"], 32)
        self.assertEqual(groups["group_size_bytes"], 64)
        self.assertTrue(all(len(g["formation_ids"]) == 32 for g in groups["groups"]))
        # Every map id in the table, including the ones with no encounters.
        self.assertEqual(len(payload["map_bindings"]), 417)
        modes = {entry["mode"] for entry in payload["map_bindings"]}
        # Four modes, not three: the map the 416-byte table cannot reach has
        # its own, which is how the overrun stays visible instead of looking
        # like an ordinary "no encounters" map.
        self.assertEqual(modes, {"none", "group", "position_grid", "outside_table"})
        self.assertEqual(
            [e["map_id"] for e in payload["map_bindings"] if e["mode"] == "outside_table"],
            [0x1A0],
        )

    def test_the_two_overworld_position_grids(self):
        grids = {grid["name"]: grid for grid in self.files[FORMATIONS_NAME]["position_grids"]}
        self.assertEqual(sorted(grids), ["dezolis", "motavia"])
        self.assertEqual((grids["motavia"]["columns"], grids["motavia"]["rows"]), (64, 64))
        self.assertEqual((grids["dezolis"]["columns"], grids["dezolis"]["rows"]), (64, 32))
        for grid in grids.values():
            with self.subTest(grid=grid["name"]):
                self.assertEqual(grid["cell_size_pixels"], 64)
                # Rows of columns, so a consumer indexes [y][x] the same way
                # the cartridge's `(y >> 6) * 64 + (x >> 6)` walks it.
                self.assertEqual(len(grid["cells"]), grid["rows"])
                self.assertTrue(all(len(row) == grid["columns"] for row in grid["cells"]))
                self.assertEqual(grid["indexing"],
                                 "(character_y >> 6) * 64 + (character_x >> 6)")

    def test_the_table_overrun_stays_a_census_anomaly(self):
        # `Battle_EnemyFormationIndexes` is 416 bytes for a 417-map id space:
        # MapID $1A0 reads one byte past the end. Dormant because that map has
        # random battles off, and reported rather than padded over.
        census = self.fragment["census"]["formations"]
        self.assertEqual(census["table_covers_maps"], 416)
        self.assertEqual(census["map_ids"], 417)
        self.assertEqual(census["maps_outside_table"], 1)
        outside = [
            entry for entry in self.files[FORMATIONS_NAME]["map_bindings"]
            if not entry["in_table"]
        ]
        self.assertEqual([entry["map_id"] for entry in outside], [0x1A0])

    # -------------------------------------------------------------- levels
    def test_the_progression_covers_eleven_characters(self):
        payload = self.files[LEVELS_NAME]
        self.assertEqual(payload["character_count"], 11)
        self.assertEqual(payload["total_records"], 937)
        self.assertEqual(
            sum(character["record_count"] for character in payload["characters"]), 937
        )
        chaz = payload["characters"][0]
        self.assertEqual((chaz["character"], chaz["starting_level"]), ("Chaz", 1))
        level = chaz["levels"][0]
        self.assertEqual(level["level"], 2)
        self.assertEqual(level["experience_required"], 21)
        self.assertEqual((level["hp"], level["tp"]), (31, 13))
        self.assertEqual(sorted(level["stats"]),
                         ["agility", "dexterity", "mental", "strength"])
        self.assertEqual(len(level["skill_uses"]), 8)

    def test_each_characters_levels_run_from_its_starting_level(self):
        for character in self.files[LEVELS_NAME]["characters"]:
            with self.subTest(character=character["character"]):
                levels = [level["level"] for level in character["levels"]]
                self.assertEqual(levels, sorted(levels))
                self.assertEqual(levels[0], character["starting_level"] + 1)
                self.assertEqual(levels[-1], self.fragment["census"]["levels"]["highest_level"])

    # ----------------------------------------------------------- abilities
    def test_the_four_ability_kinds_share_one_record_shape(self):
        payload = self.files[ABILITIES_NAME]
        self.assertEqual(payload["record_bytes"], 8)
        for kind in ABILITY_KINDS:
            with self.subTest(kind=kind):
                for record in payload[kind]:
                    self.assertEqual(record["kind"], kind)
                    self.assertIn("effect_id", record)
                    self.assertIn("power_or_hit_chance", record)
                    self.assertIn("resistance_stat", record)
                    self.assertIn("element", record)
                    self.assertIn("effect_out_of_range", record)

    def test_the_fields_each_kind_adds(self):
        payload = self.files[ABILITIES_NAME]
        # Only techniques cost TP; only skills declare a weapon requirement.
        self.assertTrue(all("tp_cost" in r for r in payload["techniques"]))
        self.assertTrue(all("requires_weapon" in r for r in payload["skills"]))
        self.assertTrue(all("relevant_stat" in r for r in payload["skills"]))
        self.assertTrue(all("relevant_stat" in r for r in payload["enemy_skills"]))
        foi = payload["techniques"][0]
        self.assertEqual((foi["name"], foi["display_name"], foi["tp_cost"]),
                         ("Foi", "FOI", 3))
        self.assertEqual(foi["element"], {"id": 3, "name": "fire"})

    def test_item_battle_records_are_the_same_eight_bytes(self):
        # An item's battle behaviour is the embedded effect block of its
        # 22-byte inventory record, so the damage pipeline reads it the same way.
        items = self.files[ABILITIES_NAME]["item_effects"]
        self.assertEqual(len(items), 160)
        dagger = items[0]
        self.assertEqual((dagger["id"], dagger["symbol"], dagger["display_name"]),
                         (1, "Dagger", "DAGGER"))
        self.assertIn("battle_object_or_graphic_id", dagger)
        self.assertFalse(dagger["effect_out_of_range"])

    def test_the_effect_census_separates_used_from_unused(self):
        census = self.fragment["census"]["abilities"]
        # The out-of-range id shows up as used, because it is; the unused list
        # is bounded by the table, so the two deliberately do not add up.
        self.assertIn(BLACK_WAVE_EFFECT, census["effect_ids_used"])
        self.assertNotIn(BLACK_WAVE_EFFECT, census["effect_ids_in_table_unused"])
        self.assertEqual(
            set(census["effect_ids_used"]) & set(census["effect_ids_in_table_unused"]),
            set(),
        )
        for value in census["effect_ids_in_table_unused"]:
            self.assertLess(value, self.fragment["ability_effects"]["count"])

    # ------------------------------------------------------------ emission
    def test_emitting_twice_produces_identical_bytes(self):
        with tempfile.TemporaryDirectory() as other:
            second = Path(other) / "pack"
            fragment = emit_battle(self.data, second, PACK_FORMAT_VERSION)
            self.assertEqual(fragment, self.fragment)
            for name in self.files:
                with self.subTest(file=name):
                    self.assertEqual((self.root / name).read_bytes(),
                                     (second / name).read_bytes())

    def test_the_fragment_hashes_match_the_files_on_disk(self):
        import hashlib

        for entry in self.fragment["files"].values():
            with self.subTest(file=entry["file"]):
                blob = (self.root / entry["file"]).read_bytes()
                self.assertEqual(entry["sha256"], hashlib.sha256(blob).hexdigest())

    def test_every_file_is_sorted_and_newline_terminated(self):
        for name in self.files:
            with self.subTest(file=name):
                text = (self.root / name).read_text()
                self.assertTrue(text.endswith("\n"))
                self.assertEqual(
                    text, json.dumps(json.loads(text), indent=2, sort_keys=True) + "\n"
                )


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    REFERENCE.exists(),
    f"Disassembly clone not present at {REFERENCE}; "
    "git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm",
)
class DisassemblyOracleTest(unittest.TestCase):
    """The labels behind the addresses the retail bytes already proved."""

    @classmethod
    def setUpClass(cls):
        cls.source = (REFERENCE / "ps4.asm").read_text(errors="replace")

    def test_the_effect_table_and_its_dispatcher(self):
        self.assertIn("AbilityEffectsOffs:", self.source)
        self.assertIn("AbilityEffect_None:", self.source)
        # `TRAP #2`: `add.w d0,d0 / adda.w (a0,d0.w),a0 / jsr (a0)`, no bound.
        self.assertIn("\tadd.w\td0, d0\n\tadda.w\t(a0,d0.w), a0\n\tjsr\t(a0)", self.source)

    def test_the_effect_table_entries_are_offsets_from_its_own_base(self):
        data = read_rom(ROM)
        count = ability_effect_count(data)
        words = struct.unpack_from(f">{count}H", data, ABILITY_EFFECTS_OFFS)
        # Every entry lands at or after AbilityEffect_None, i.e. inside code
        # rather than back inside the table.
        for index, word in enumerate(words):
            with self.subTest(entry=index):
                self.assertGreaterEqual(ABILITY_EFFECTS_OFFS + word, ABILITY_EFFECT_NONE)


if __name__ == "__main__":
    unittest.main()
