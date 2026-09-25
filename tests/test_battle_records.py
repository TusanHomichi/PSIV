import json
import tempfile
import unittest
from pathlib import Path

from psiv_tools.battle_pack import (
    CHARACTERS_NAME,
    EQUIPMENT_NAME,
    _display_names,
    emit_battle,
)
from psiv_tools.battle_records import build_characters, build_equipment
from psiv_tools.battle_rules import (
    BONUS_FIELDS,
    EQUIPMENT_SLOTS,
    SIGNATURES,
    BattleRecordError,
    read_attack_rules,
    read_char_init,
    read_element_rules,
    read_equip_rules,
    read_mod_stats,
    read_rules,
    type_rules,
)
from psiv_tools.core import TABLES, read_rom
from psiv_tools.pack import PACK_FORMAT_VERSION

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"

#: `docs/battle/BATTLE_SCOUT.md` section 12 works the opening party's derived stats by
#: hand. Reproducing them is the end-to-end check on the whole module.
OPENING_DERIVED = {0: (18, 10), 1: (13, 18)}
#: The one inventory record carrying an equip mask its type can never reach.
NOTHING_ITEM_ID = 118
#: Chaz and Rika, the two characters who start with a weapon in the left hand.
DUAL_WIELDERS = [0, 5]


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestTheRules(unittest.TestCase):
    """The five routines, decoded out of retail rather than transcribed."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)

    def test_every_signature_occurs_exactly_as_often_as_declared(self):
        for label, (signature, expected) in SIGNATURES.items():
            with self.subTest(routine=label):
                pattern = bytes.fromhex(signature)
                count = 0
                at = self.data.find(pattern)
                while at != -1:
                    count += 1
                    at = self.data.find(pattern, at + 1)
                self.assertEqual(count, expected)

    def test_a_rom_that_lost_a_signature_is_refused(self):
        broken = bytearray(self.data)
        site = self.data.find(bytes.fromhex(SIGNATURES["UpdateCharModStats"][0]))
        broken[site:site + 2] = b"\x4e\x71"
        with self.assertRaises(BattleRecordError):
            read_mod_stats(bytes(broken))

    # --------------------------------------------------- InitializeCharStats
    def test_the_record_layout_comes_out_of_the_routine(self):
        init = read_char_init(self.data)
        spec = TABLES["characters"]
        self.assertEqual(init["table"], spec["offset"])
        self.assertEqual(init["count"], spec["count"])
        self.assertEqual(init["record_bytes"], spec["record_size"])
        self.assertEqual(init["character_stats_ram"], "0xFFFFF500")
        self.assertEqual(init["struct_bytes"], 0x80)
        # The fields tile the record with no gap and no overlap.
        at = 0
        for field in init["fields"]:
            self.assertEqual(field["record_offset"], at)
            at += field["bytes"]
        self.assertEqual(at, spec["record_size"])

    def test_the_three_mirrors_the_routine_makes(self):
        mirrors = {
            field["field"]: field["mirrored_to"]
            for field in read_char_init(self.data)["fields"]
            if field["mirrored_to"]
        }
        self.assertEqual(mirrors, {
            "curr_hp": "max_hp",
            "curr_tp": "max_tp",
            "curr_skill_uses": "max_skill_uses",
        })

    def test_the_three_array_copies(self):
        arrays = {
            field["field"]: field["count"]
            for field in read_char_init(self.data)["fields"]
            if field["count"] > 1
        }
        self.assertEqual(arrays, {"techniques": 16, "skills": 8, "curr_skill_uses": 8})

    # ---------------------------------------------------- UpdateCharModStats
    def test_the_seven_derived_stat_passes(self):
        passes = read_mod_stats(self.data)["passes"]
        self.assertEqual(
            [(entry["stat"], entry["base_stat"], entry["item_offsets"]) for entry in passes],
            [
                ("strength_mod", "strength", ["0x0B"]),
                ("mental_mod", "mental", ["0x0C"]),
                ("agility_mod", "agility", ["0x0D"]),
                ("dexterity_mod", "dexterity", ["0x0E"]),
                ("atk_pow", "strength", ["0x0B", "0x0F"]),
                ("dfs_pow", "agility", ["0x0D", "0x10"]),
                ("magic_dfs", "mental", ["0x0C", "0x11"]),
            ],
        )

    def test_the_single_stat_passes_wrap_and_the_derived_ones_sign_extend(self):
        for entry in read_mod_stats(self.data)["passes"]:
            with self.subTest(stat=entry["stat"]):
                if entry["stat"].endswith("_mod"):
                    self.assertEqual((entry["arithmetic"], entry["signed"]), ("add.b", False))
                else:
                    self.assertTrue(entry["signed"])
                    self.assertEqual(entry["result_bytes"], 2)

    def test_the_adders_agree_with_the_item_table(self):
        rules = read_mod_stats(self.data)
        spec = TABLES["items"]
        self.assertEqual(rules["inventory_data"], f"0x{spec['offset']:06X}")
        self.assertEqual(rules["record_bytes"], spec["record_size"])
        self.assertEqual(rules["equipment_slots"], len(EQUIPMENT_SLOTS))

    def test_every_bonus_byte_is_summed_by_some_pass(self):
        used = {
            offset for entry in read_mod_stats(self.data)["passes"]
            for offset in entry["item_offsets"]
        }
        self.assertEqual(used, {f"0x{offset:02X}" for offset in BONUS_FIELDS})

    # ------------------------------------------------------- UpdateEquipment
    def test_the_equip_jump_table_places_each_type(self):
        rules = read_equip_rules(self.data)
        self.assertEqual(rules["max_equippable_type"], 7)
        self.assertEqual(rules["type_offset"], "0x0A")
        self.assertEqual(rules["usable_by_offset"], "0x08")
        self.assertEqual(len(rules["filters"]), 2)
        placed = {
            item_type: handler["equips_to"]
            for item_type, handler in rules["handlers"].items()
        }
        self.assertEqual(placed, {
            1: ["right_hand"], 2: ["right_hand"], 3: ["right_hand"],
            4: ["right_hand"], 5: ["left_hand"], 6: ["head"], 7: ["body"],
        })

    def test_only_a_shield_ever_reaches_the_left_hand(self):
        types = type_rules(read_rules(self.data))
        left = [key for key, rule in types.items() if rule["slot"] == "left_hand"]
        self.assertEqual(left, [5])

    def test_the_two_handed_types_clear_the_left_hand(self):
        types = type_rules(read_rules(self.data))
        self.assertEqual(
            [key for key, rule in types.items() if rule["two_handed"]], [3, 4]
        )
        for key in (3, 4):
            self.assertEqual(types[key]["clears"], ["left_hand"])

    def test_a_shield_displaces_a_two_handed_weapon(self):
        shield = type_rules(read_rules(self.data))[5]
        self.assertEqual(shield["clears_when_other_hand_holds"], [3, 4])
        self.assertEqual(shield["clears_conditionally"], ["right_hand"])
        self.assertFalse(shield["two_handed"])

    # -------------------------------------------------- Battle_AttackCommand
    def test_which_types_can_attack(self):
        rules = read_attack_rules(self.data)
        self.assertEqual(rules["weapon_types"], [1, 2, 3, 4])
        self.assertEqual(rules["multi_target_types"], [2, 4])

    # ----------------------------------------------------- UpdateCharElems
    def test_the_element_byte_has_two_readings(self):
        rules = read_element_rules(self.data)
        self.assertEqual(rules["element_offset"], "0x12")
        self.assertEqual(rules["shield_type"], 5)
        self.assertEqual(rules["elements"], 14)
        self.assertEqual(rules["resistance_value"], 1)
        self.assertEqual(rules["property_base"], "0x30")
        self.assertEqual(
            rules["weapon_element_slots"], {"right_hand": "0x50", "left_hand": "0x51"}
        )

    def test_a_weapon_carries_an_element_and_armour_grants_one(self):
        types = type_rules(read_rules(self.data))
        for key, rule in types.items():
            with self.subTest(item_type=key):
                if rule["is_weapon"]:
                    self.assertEqual(rule["element_role"], "attack_element")
                elif rule["equippable"]:
                    self.assertEqual(rule["element_role"], "resistance_granted")
                else:
                    self.assertIsNone(rule["element_role"])


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestTheRecords(unittest.TestCase):
    """What the two files hold, built straight from the ROM."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.display = _display_names(cls.data)
        cls.characters = build_characters(cls.data, cls.display)
        cls.equipment = build_equipment(cls.data, cls.display["items"])
        cls.items = {item["id"]: item for item in cls.equipment["items"]}

    def test_eleven_characters_and_a_hundred_and_sixty_items(self):
        self.assertEqual(self.characters["count"], 11)
        self.assertEqual(len(self.characters["characters"]), 11)
        self.assertEqual(self.equipment["count"], 160)
        self.assertEqual(len(self.equipment["items"]), 160)

    def test_hp_and_tp_start_at_their_maxima(self):
        for character in self.characters["characters"]:
            with self.subTest(character=character["symbol"]):
                self.assertEqual(character["hp"], character["max_hp"])
                self.assertEqual(character["tp"], character["max_tp"])
        for character in self.characters["characters"]:
            for skill in character["skills"]["known"]:
                self.assertEqual(skill["uses"], skill["max_uses"])

    def test_the_ability_arrays_stay_positional(self):
        for character in self.characters["characters"]:
            with self.subTest(character=character["symbol"]):
                self.assertEqual(len(character["techniques"]["slots"]), 16)
                self.assertEqual(len(character["skills"]["slots"]), 8)
                self.assertEqual(len(character["skills"]["uses"]), 8)
                for entry in character["techniques"]["known"]:
                    self.assertEqual(
                        character["techniques"]["slots"][entry["slot"]], entry["id"]
                    )
                for entry in character["skills"]["known"]:
                    self.assertEqual(
                        character["skills"]["slots"][entry["slot"]], entry["id"]
                    )
                # Learned abilities go in the first empty slot, so the known
                # ones are a prefix with no holes.
                known = len(character["skills"]["known"])
                self.assertEqual(
                    character["skills"]["slots"][known:], [0] * (8 - known)
                )

    def test_chaz_is_the_record_the_disassembly_documents(self):
        chaz = self.characters["characters"][0]
        self.assertEqual(chaz["symbol"], "Chaz")
        self.assertEqual(chaz["display_name"], "Chaz")
        self.assertEqual(chaz["rom_offset"], "0x2A8ACA")
        self.assertEqual(chaz["profession"]["symbol"], "Hunter")
        self.assertEqual(chaz["profession"]["display_name"], "HUNTER")
        self.assertEqual((chaz["level"], chaz["hp"]), (1, 25))
        self.assertEqual(chaz["stats"],
                         {"strength": 8, "mental": 6, "agility": 7, "dexterity": 5})
        self.assertEqual(
            [slot and slot["item_id"] for slot in chaz["equipment"].values()],
            [2, 2, 5, 4],
        )

    def test_the_opening_party_derives_the_stats_the_scout_worked_by_hand(self):
        for character_id, (attack, defence) in OPENING_DERIVED.items():
            character = self.characters["characters"][character_id]
            with self.subTest(character=character["symbol"]):
                initialized = character["initialized"]
                self.assertEqual(initialized["atk_pow"], attack)
                self.assertEqual(initialized["dfs_pow"], defence)

    def test_the_initialized_block_covers_every_derived_stat(self):
        passes = [entry["stat"] for entry in read_mod_stats(self.data)["passes"]]
        for character in self.characters["characters"]:
            initialized = character["initialized"]
            with self.subTest(character=character["symbol"]):
                for stat in passes:
                    if stat.endswith("_mod"):
                        self.assertIn(stat[:-4], initialized["stats"])
                        self.assertIn("mod", initialized["stats"][stat[:-4]])
                    else:
                        self.assertIn(stat, initialized)
                self.assertEqual(len(initialized["element_props"]), 14)

    def test_an_unequipped_character_derives_its_base_stats(self):
        for character in self.characters["characters"]:
            initialized = character["initialized"]
            with self.subTest(character=character["symbol"]):
                for name, value in character["stats"].items():
                    self.assertEqual(initialized["stats"][name]["base"], value)

    def test_a_weapon_hand_carries_its_items_element(self):
        for character in self.characters["characters"]:
            for slot in ("right_hand", "left_hand"):
                entry = character["equipment"][slot]
                observed = character["initialized"]["weapon_elements"][slot]["id"]
                with self.subTest(character=character["symbol"], slot=slot):
                    if entry is None or entry["type"] == 5:
                        self.assertEqual(observed, 0)
                    else:
                        self.assertEqual(
                            observed, self.items[entry["item_id"]]["element"]["id"]
                        )

    def test_every_initial_item_is_one_its_owner_could_re_equip(self):
        census = self.characters["census"]
        self.assertEqual(census["equipment_the_owner_cannot_equip"], [])
        for character in self.characters["characters"]:
            for slot, entry in character["equipment"].items():
                if entry is None:
                    continue
                item = self.items[entry["item_id"]]
                with self.subTest(character=character["symbol"], slot=slot):
                    self.assertTrue(item["equippable"])
                    self.assertIn(
                        character["character_id"],
                        [who["character_id"] for who in item["equippable_by"]],
                    )

    def test_the_left_hand_weapons_the_equip_menu_could_never_replace(self):
        self.assertEqual(self.characters["census"]["weapon_in_left_hand"], DUAL_WIELDERS)
        for character_id in DUAL_WIELDERS:
            entry = self.characters["characters"][character_id]["equipment"]["left_hand"]
            with self.subTest(character=character_id):
                self.assertTrue(self.items[entry["item_id"]]["is_weapon"])
                # Nothing but a shield is ever placed there by the menu.
                self.assertEqual(self.items[entry["item_id"]]["slot"], "right_hand")

    # ------------------------------------------------------------ equipment
    def test_the_type_byte_decides_slot_weapon_and_multi_target(self):
        knife = self.items[2]
        self.assertEqual(knife["display_name"], "HUNT-KNIFE")
        self.assertEqual(knife["type"], {"id": 1, "name": "one_handed_single_target_weapon"})
        self.assertEqual(knife["slot"], "right_hand")
        self.assertTrue(knife["is_weapon"])
        self.assertFalse(knife["multi_target"])
        self.assertEqual(knife["bonuses"]["attack"], 5)
        self.assertEqual(knife["element"], {"id": 1, "name": "physical",
                                            "role": "attack_element"})

    def test_the_boomerang_is_the_one_handed_multi_target_case(self):
        boomerang = self.items[3]
        self.assertEqual(boomerang["display_name"], "BOOMERANG")
        self.assertEqual(boomerang["type"]["id"], 2)
        self.assertTrue(boomerang["multi_target"])
        self.assertFalse(boomerang["two_handed"])

    def test_every_item_resolves_to_a_type_rule(self):
        types = {rule["type"] for rule in self.equipment["types"]}
        for item in self.equipment["items"]:
            with self.subTest(item=item["id"]):
                self.assertIn(item["type"]["id"], types)
                self.assertEqual(item["equippable"], item["slot"] is not None)
                if item["multi_target"]:
                    self.assertTrue(item["is_weapon"])

    def test_the_equip_mask_only_names_real_characters(self):
        for item in self.equipment["items"]:
            mask = int(item["equippable_by_mask"], 16)
            with self.subTest(item=item["id"]):
                self.assertEqual(mask >> 11, 0)
                self.assertEqual(len(item["equippable_by"]), bin(mask).count("1"))

    def test_the_one_mask_no_filter_can_reach(self):
        census = self.equipment["census"]
        self.assertEqual(
            [entry["id"] for entry in census["equip_mask_on_unequippable"]],
            [NOTHING_ITEM_ID],
        )
        nothing = self.items[NOTHING_ITEM_ID]
        self.assertFalse(nothing["equippable"])
        self.assertGreater(nothing["type"]["id"], census["max_equippable_type"])

    def test_no_item_declares_the_unused_type_zero(self):
        census = self.equipment["census"]
        self.assertEqual(census["types_declared_unused"], [0])
        self.assertNotIn(0, census["types_present"])

    def test_the_bonus_fields_are_the_ones_the_passes_name(self):
        for item in self.equipment["items"]:
            with self.subTest(item=item["id"]):
                self.assertEqual(set(item["bonuses"]), set(BONUS_FIELDS.values()))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestInThePack(unittest.TestCase):
    """The two files as `emit_battle` writes them."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls._temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls._temp.name) / "pack"
        cls.fragment = emit_battle(cls.data, cls.root, PACK_FORMAT_VERSION)
        cls.files = {
            name: json.loads((cls.root / name).read_text())
            for name in (CHARACTERS_NAME, EQUIPMENT_NAME)
        }

    @classmethod
    def tearDownClass(cls):
        cls._temp.cleanup()

    def test_both_files_are_versioned_and_named(self):
        self.assertEqual(self.files[CHARACTERS_NAME]["kind"], "battle_characters")
        self.assertEqual(self.files[EQUIPMENT_NAME]["kind"], "battle_equipment")
        for payload in self.files.values():
            self.assertEqual(payload["format_version"], PACK_FORMAT_VERSION)

    def test_the_manifest_fragment_names_both(self):
        files = self.fragment["files"]
        self.assertEqual(files["characters"]["file"], CHARACTERS_NAME)
        self.assertEqual(files["characters"]["count"], 11)
        self.assertEqual(files["equipment"]["file"], EQUIPMENT_NAME)
        self.assertEqual(files["equipment"]["count"], 160)
        self.assertEqual(files["equipment"]["equippable"], 132)

    def test_the_manifest_headline_names_the_routines(self):
        rules = self.fragment["equipment_rules"]
        self.assertEqual(set(rules["routines"]),
                         {"equip", "attack", "derived_stats", "elements"})
        self.assertEqual(rules["weapon_types"], [1, 2, 3, 4])
        self.assertEqual(rules["multi_target_types"], [2, 4])
        self.assertEqual(len(rules["derived_stats"]), 7)

    def test_the_census_reaches_the_manifest(self):
        census = self.fragment["census"]
        self.assertEqual(census["characters"]["weapon_in_left_hand"], DUAL_WIELDERS)
        self.assertEqual(census["equipment"]["equippable"], 132)

    def test_every_character_equips_items_the_equipment_file_holds(self):
        items = {item["id"] for item in self.files[EQUIPMENT_NAME]["items"]}
        for character in self.files[CHARACTERS_NAME]["characters"]:
            for entry in character["equipment"].values():
                if entry is not None:
                    self.assertIn(entry["item_id"], items)

    def test_emitting_twice_produces_identical_bytes(self):
        with tempfile.TemporaryDirectory() as other:
            second = Path(other) / "pack"
            emit_battle(self.data, second, PACK_FORMAT_VERSION)
            for name in self.files:
                with self.subTest(file=name):
                    self.assertEqual((self.root / name).read_bytes(),
                                     (second / name).read_bytes())


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    REFERENCE.exists(),
    f"Disassembly clone not present at {REFERENCE}; "
    "git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm",
)
class DisassemblyOracleTest(unittest.TestCase):
    """The labels behind the addresses retail already proved."""

    @classmethod
    def setUpClass(cls):
        cls.source = (REFERENCE / "ps4.asm").read_text(errors="replace")

    def test_the_routines_exist_under_the_names_used_here(self):
        for label in ("InitializeCharStats:", "UpdateCharModStats:",
                      "UpdateCharElems:", "UpdateEquipment:",
                      "EquipItemTypeJmpTbl:", "Battle_AttackCommand:",
                      "AddItemBonusToCharStats:", "AddItemBonusToCharStats2:"):
            with self.subTest(label=label):
                self.assertIn(label, self.source)

    def test_the_mirrors_are_the_ones_the_source_writes(self):
        self.assertIn("move.w\tcurr_hp(a0), max_hp(a0)", self.source)
        self.assertIn("move.w\tcurr_tp(a0), max_tp(a0)", self.source)

    def test_the_seven_handlers_the_jump_table_reaches(self):
        for label in ("ModStatsUpdate_Strength", "ModStatsUpdate_Mental",
                      "ModStatsUpdate_Agility", "ModStatsUpdate_Dexterity",
                      "ModStatsUpdate_Attack", "ModStatsUpdate_Defense",
                      "ModStatsUpdate_MagicDefense"):
            with self.subTest(label=label):
                self.assertIn(f"{label}:", self.source)

    def test_the_five_equip_handlers(self):
        for label in ("EquipItemType_OneHanded", "EquipItemType_TwoHanded",
                      "EquipItemType_Shields", "EquipItemType_Head",
                      "EquipItemType_Body"):
            with self.subTest(label=label):
                self.assertIn(f"{label}:", self.source)


if __name__ == "__main__":
    unittest.main()
