import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from psiv_tools.core import read_rom
from psiv_tools.pack import PACK_FORMAT_VERSION, build_pack
from psiv_tools.newgame import (
    CHARACTER_SYMBOLS,
    EMPTY_SLOT,
    EVENT_FLAGS_SET,
    EVENT_FLAG_PIATA_FIRST_TIME,
    EVENT_PTRS,
    EXTENDED_EVENT_FLAGS,
    GAME_START_EVENT_INDEX,
    NEW_GAME_INIT,
    PARTY_SLOTS,
    PARTY_SLOTS_PLACED,
    START_POS_PIXELS,
    STANDING_CELL_Y_OFFSET,
    TITLE_SET_MAP,
    NewGameError,
    _flag_ids,
    event_routine,
    extract_new_game,
    read_first_control,
    read_new_game_init,
    read_title_handoff,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"

MAP_PIATA_ACADEMY = 0x11
MAP_PIATA_ACADEMY_F1 = 0x13


class TestFlagPacking(unittest.TestCase):
    """`EventFlags_Set`'s bit arithmetic, on bytes this test writes."""

    def test_flags_are_packed_most_significant_bit_first(self):
        # `lsr.w #3,d0` picks the byte, `move.w #7,d1 / sub.w d2,d1 / bset d1`
        # puts flag 0 in bit 7 and flag 7 in bit 0.
        self.assertEqual(_flag_ids(bytes([0x80]), 0), (0,))
        self.assertEqual(_flag_ids(bytes([0x01]), 0), (7,))
        self.assertEqual(_flag_ids(bytes([0x00, 0x80]), 0), (8,))
        self.assertEqual(_flag_ids(bytes([0xFF]), 0), tuple(range(8)))

    def test_the_base_offsets_extended_ids(self):
        # `Extended_Event_Flags` holds ids from $100 up.
        self.assertEqual(_flag_ids(bytes([0x80]), 0x100), (0x100,))
        self.assertEqual(_flag_ids(bytes([0x00, 0x00, 0x00, 0x00, 0x01]), 0x100), (0x127,))

    def test_nothing_set_is_no_ids(self):
        self.assertEqual(_flag_ids(bytes(32), 0x100), ())


class TestCharacterSymbols(unittest.TestCase):
    def test_eleven_characters_in_id_order(self):
        self.assertEqual(len(CHARACTER_SYMBOLS), 11)
        self.assertEqual(CHARACTER_SYMBOLS[0], "Chaz")
        self.assertEqual(CHARACTER_SYMBOLS[1], "Alys")
        self.assertEqual(CHARACTER_SYMBOLS[-1], "Seth")

    def test_a_party_slot_is_a_character_or_empty(self):
        self.assertEqual(EMPTY_SLOT, 0xFF)
        self.assertEqual(PARTY_SLOTS, 6)
        # `loc_535F8` runs `moveq #4,d7`, so only five slots are ever placed.
        self.assertEqual(PARTY_SLOTS_PLACED, 5)
        self.assertLess(PARTY_SLOTS_PLACED, PARTY_SLOTS)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestNewGameAgainstTheRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.payload = extract_new_game(cls.data)

    # --------------------------------------------------------- title handoff
    def test_the_title_screen_starts_an_event_not_a_map(self):
        handoff = self.payload["title_handoff"]
        # `Title_StartOption` never loads a map: it sets the field mode's
        # "play event" routine and an event index, so a new game runs a script.
        self.assertEqual(handoff["game_mode_routine"]["value"], 0xC)
        self.assertEqual(handoff["event_index"]["value"], GAME_START_EVENT_INDEX)
        # And the map it set beforehand is the academy's ground floor, which is
        # where the scene plays -- not where the player is handed control.
        self.assertEqual(handoff["scene_map"]["id"], MAP_PIATA_ACADEMY)
        self.assertEqual(handoff["scene_map"]["symbol"], "PiataAcademy")

    def test_the_event_pointer_table_resolves_where_expected(self):
        routine = event_routine(self.data, GAME_START_EVENT_INDEX)
        self.assertEqual(routine, 0x073946)
        self.assertEqual(event_routine(self.data, GAME_START_EVENT_INDEX + 1), 0x073ECE)
        self.assertEqual(EVENT_PTRS, 0x05A2B4)

    # ------------------------------------------------------------ init state
    def test_the_initialiser_seats_chaz_and_alys(self):
        # This is the state the user memory is about, and it is real -- it just
        # does not survive the opening scene.
        init = self.payload["new_game_init"]
        self.assertEqual(init["routine"], f"0x{NEW_GAME_INIT:06X}")
        self.assertEqual(
            [slot["symbol"] for slot in init["party_slots"]],
            ["Chaz", "Alys", None, None, None, None],
        )
        self.assertEqual([slot["id"] for slot in init["party_slots"][:2]], [0, 1])

    def test_a_new_game_carries_500_meseta_and_no_items(self):
        init = self.payload["new_game_init"]
        self.assertEqual(init["money"], 500)
        self.assertEqual(init["inventory"]["slots"], 40)
        self.assertTrue(init["inventory"]["all_empty"])

    def test_the_settings_a_new_game_starts_with(self):
        self.assertEqual(self.payload["new_game_init"]["settings"], {
            "message_speed": 2, "battle_speed": 2, "world_index": 0,
        })

    def test_eleven_extended_event_flags_start_set(self):
        # The headline correction: a new game does *not* start with every flag
        # clear. The 512-byte clear wipes all four flag regions, and then a
        # 32-byte table is copied back over the extended ones.
        flags = self.payload["new_game_init"]["flags"]
        self.assertEqual(flags["cleared_region"]["bytes"], 512)
        extended = flags["extended_event_flags"]
        self.assertEqual(extended["address"], f"0xFFFF{EXTENDED_EVENT_FLAGS:04X}")
        self.assertEqual(extended["source"], "0x044632")
        self.assertEqual(extended["bytes"], 32)
        self.assertEqual(extended["set"], [
            "0x127", "0x128", "0x12A", "0x134", "0x138", "0x150",
            "0x15B", "0x15D", "0x16B", "0x178", "0x1A7",
        ])
        self.assertEqual(
            extended["sha256"],
            "41da6d8b4b4621dbb036e6c2665bae39d997b5e33851c57a2607286883ccaccd",
        )

    def test_the_extended_table_really_goes_to_the_extended_region(self):
        # The clone says `Chest_Flags` here and retail says `Extended_Event_Flags`.
        # $F120 and $F140 are 32 bytes apart, so the two readings differ by a
        # whole flag region; the retail `lea` operand settles it.
        init = read_new_game_init(self.data)
        lea = next(r for r in init["reads"]
                   if r["label"] == "loc_44414: Extended_Event_Flags table")
        self.assertIn("43f8f120", lea["raw_hex"])
        self.assertNotIn("43f8f140", lea["raw_hex"])
        self.assertEqual(init["flags"]["chest_flags"]["set"], [])

    def test_town_flags_are_four_bytes_not_two(self):
        town = self.payload["new_game_init"]["flags"]["town_flags"]
        self.assertEqual(town["bytes"], 4)
        self.assertEqual(town["raw_hex"], "80008040")
        self.assertEqual(town["set"], ["0x000", "0x010", "0x019"])

    def test_character_stats_are_a_separate_system(self):
        stats = self.payload["new_game_init"]["character_stats"]
        # `InitialCharStats`, the offset SOURCE_NOTES already documents, applied
        # to all eleven characters rather than just the party.
        self.assertEqual(stats["source"], "0x2A8ACA")
        self.assertEqual(stats["characters"], len(CHARACTER_SYMBOLS))

    # --------------------------------------------------------- first control
    def test_the_player_starts_alone_as_chaz(self):
        control = self.payload["first_control"]
        self.assertEqual(
            [slot["symbol"] for slot in control["party"]],
            ["Chaz", None, None, None, None, None],
        )
        # And during the scene it was Alys leading, which is the memory people
        # have of "starting with Alys and Chaz".
        self.assertEqual(
            [slot["symbol"] for slot in control["event"]["party_during_scene"]],
            ["Alys", "Chaz", None, None],
        )

    def test_where_the_player_is_handed_control(self):
        control = self.payload["first_control"]
        self.assertEqual(control["map"]["id"], MAP_PIATA_ACADEMY_F1)
        self.assertEqual(control["map"]["symbol"], "PiataAcademy_F1")
        self.assertEqual(control["facing"], {"id": 0, "name": "down"})
        self.assertEqual(control["character_alignment"], 0)
        self.assertEqual(control["music"]["symbol"], "MotabiaTown")
        position = control["position"]
        # The retail immediates, not the clone's $58/$22/right.
        self.assertEqual((position["x_start_word"], position["y_start_word"]), (0x60, 0x24))
        self.assertEqual((position["x_pixels"], position["y_pixels"]), (768, 288))
        self.assertEqual((position["x_cell"], position["y_cell"]), (48, 19))
        # And the arithmetic that got there, restated from the constants.
        self.assertEqual(position["x_pixels"], position["x_start_word"] * START_POS_PIXELS)
        self.assertEqual(
            position["y_cell"], position["y_pixels"] // 16 + STANDING_CELL_Y_OFFSET
        )

    def test_exactly_one_event_flag_is_set_by_the_opening(self):
        flags = self.payload["first_control"]["event_flags_set"]
        self.assertEqual(len(flags), 1)
        self.assertEqual(flags[0]["id"], EVENT_FLAG_PIATA_FIRST_TIME)
        self.assertEqual(flags[0]["symbol"], "EventFlag_PiataFirstTime")
        # `read_first_control` refuses a routine that reaches EventFlags_Set
        # more than once, so this list being short is a checked claim.
        self.assertEqual(EVENT_FLAGS_SET, 0x057666)

    # ------------------------------------------------------------ fail-closed
    def test_a_rom_whose_init_moved_is_refused(self):
        for site, reader in (
            (NEW_GAME_INIT + 0x50, read_new_game_init),
            (TITLE_SET_MAP, read_title_handoff),
        ):
            with self.subTest(site=hex(site)):
                broken = bytearray(self.data)
                broken[site:site + 8] = b"\x00" * 8
                with self.assertRaises(NewGameError):
                    reader(bytes(broken))

    def test_an_event_table_that_does_not_resolve_is_refused(self):
        broken = bytearray(self.data)
        broken[0x05A280:0x05A282] = b"\x00\x00"  # the lea's displacement
        with self.assertRaises(NewGameError):
            read_first_control(bytes(broken))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestAgainstThePackedMap(unittest.TestCase):
    """The start cell has to be somewhere the player can actually stand.

    Builds its own one-map pack rather than reading the repository's, so the
    cross-check is against this code's output and not against whatever was on
    disk from an earlier run.
    """

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls._temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls._temp.name) / "pack"
        cls.manifest = build_pack(cls.data, cls.root, map_ids=[MAP_PIATA_ACADEMY_F1])
        cls.start = cls.manifest["game_start"]
        entry = cls.manifest["maps"][0]
        cls.map_json = json.loads((cls.root / entry["json"]).read_text())

    @classmethod
    def tearDownClass(cls):
        cls._temp.cleanup()

    def test_the_start_map_is_the_one_the_manifest_names(self):
        self.assertEqual(self.start["map"]["id"], MAP_PIATA_ACADEMY_F1)
        self.assertEqual(self.map_json["id"], self.start["map"]["id"])
        # The file the manifest points at is on disk and hashes to what it says.
        blob = (self.root / self.start["file"]).read_bytes()
        self.assertEqual(
            self.start["sha256"], hashlib.sha256(blob).hexdigest()
        )
        self.assertEqual(json.loads(blob)["format_version"], PACK_FORMAT_VERSION)

    def test_the_start_cell_is_walkable_on_the_map_it_names(self):
        start, payload = self.start, self.map_json
        rows = payload["collision"]["rows"]
        x, y = start["x_cell"], start["y_cell"]
        self.assertLess(x, payload["dimensions"]["width_cells"])
        self.assertLess(y, payload["dimensions"]["height_cells"])
        # Type 0 is ordinary ground, and so is every neighbour: the game does
        # not drop the player onto a warp or into a wall.
        self.assertEqual(rows[y][x], 0)
        self.assertEqual(
            [rows[y + dy][x + dx] for dy in (-1, 0, 1) for dx in (-1, 0, 1)], [0] * 9
        )

    def test_alys_is_standing_on_that_map_as_an_npc(self):
        # The party is Chaz alone and Alys is a field object, which is what
        # makes the opening "go and find her" rather than a party of two.
        self.assertEqual(self.start["party"], ["Chaz"])
        alys = [npc for npc in self.map_json["npcs"] if npc["symbol"] == "NPCAlysPiata"]
        self.assertEqual(len(alys), 1)

    def test_the_music_the_start_names_is_the_maps_own(self):
        self.assertEqual(self.start["music"]["id"], self.map_json["music"]["id"])


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    REFERENCE.exists(),
    f"Disassembly clone not present at {REFERENCE}; "
    "git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm",
)
class DisassemblyOracleTest(unittest.TestCase):
    """What the clone says, after the retail bytes have already said it."""

    @classmethod
    def setUpClass(cls):
        cls.source = (REFERENCE / "ps4.asm").read_text(errors="replace")
        cls.constants = (REFERENCE / "ps4.constants.asm").read_text(errors="replace")

    def test_the_start_option_dispatches_the_game_start_event(self):
        self.assertIn("Title_StartOption:", self.source)
        self.assertIn("move.w\t#$9F, (Event_Index).w", self.source)
        self.assertIn("dc.l\tEvent_GameStart\t; $9F", self.source)

    def test_the_character_ids_are_the_clones(self):
        for index, symbol in enumerate(CHARACTER_SYMBOLS):
            with self.subTest(symbol=symbol):
                self.assertRegex(
                    self.constants, rf"CharID_{symbol} = +id\(Char_{symbol}\)"
                )

    def test_the_two_flag_regions_the_clone_confuses(self):
        self.assertIn("Extended_Event_Flags = ramaddr($FFFFF120)", self.constants)
        self.assertIn("Chest_Flags = ramaddr($FFFFF140)", self.constants)
        # The clone's own loc_44414 names the wrong one of the two; retail's
        # `lea` is checked in TestNewGameAgainstTheRom.
        self.assertIn("lea\t(Chest_Flags).w, a1", self.source)

    def test_the_clone_edited_the_start_position(self):
        # Three Grand Cross edits with "was" comments, which is why this module
        # reads the immediates out of the cartridge.
        self.assertIn("move.w\t#$58, (Map_Start_X_Pos).w ; was 60", self.source)
        self.assertIn("move.w\t#$22, (Map_Start_Y_Pos).w ; was 24", self.source)
        self.assertIn(
            "move.w\t#FacingDir_Right, (Map_Start_Facing_Dir).w ; was 0 (down)",
            self.source,
        )

    def test_the_flag_the_opening_sets_is_annotated_as_such(self):
        self.assertIn(
            "EventFlag_PiataFirstTime = 7\t; Set when Chaz is alone in Piata "
            "at the start of the game",
            self.constants,
        )


if __name__ == "__main__":
    unittest.main()
