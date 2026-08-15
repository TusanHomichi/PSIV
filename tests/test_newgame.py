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
    EVENT_FLAG_PIATA_CHAZ_CONTROL,
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
    _pack_flags,
    event_routine,
    event_routines,
    flag_setters,
    read_trigger,
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


class TestFlagPacking2(unittest.TestCase):
    """Packing is the inverse of reading, so a round trip is the proof."""

    def test_pack_and_unpack_are_inverses(self):
        for flags, base in (({7, 21}, 0), ({0x127, 0x1A7}, 0x100),
                            ({0, 0x10, 0x19}, 0), (set(), 0), ({255}, 0)):
            with self.subTest(flags=sorted(flags)):
                self.assertEqual(set(_flag_ids(_pack_flags(flags, base), base)), flags)

    def test_the_oracles_first_long_falls_out_of_the_packing(self):
        # oracle/logs/01_newgame.csv: eflags_00 = 01000400 with flags $07/$15.
        self.assertEqual(_pack_flags({0x07, 0x15})[:4].hex(), "01000400")
        # town_flags_00 = 80008040 with town flags 0/$10/$19.
        self.assertEqual(_pack_flags({0x00, 0x10, 0x19})[:4].hex(), "80008040")

    def test_a_flag_outside_its_bank_is_refused(self):
        with self.assertRaises(NewGameError):
            _pack_flags({0x127}, 0)      # extended id in the base bank
        with self.assertRaises(NewGameError):
            _pack_flags({0x100}, 0)      # past 256 flags


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

    def test_two_scenes_run_before_control_and_the_chain_stops_itself(self):
        # The defect this replaced: only Event_GameStart was scanned, but
        # control does not reach the player when it ends. RunEvents walks the
        # start map's trigger list first, and one of those triggers plays
        # another scene.
        chain = self.payload["first_control"]["scene_chain"]
        self.assertEqual([scene["event_hex"] for scene in chain], ["0x9F", "0xA0"])
        self.assertEqual(chain[0]["routine"], "0x073946")
        # Event_PiataChazAlone is eighteen bytes ending in a tail jump.
        self.assertEqual((chain[1]["routine"], chain[1]["rom_end"]),
                         ("0x073ECE", "0x073EE0"))
        self.assertEqual([s["flag_hex"] for s in chain[0]["sets"]], ["0x07"])
        self.assertEqual([s["flag_hex"] for s in chain[1]["sets"]], ["0x15"])
        # The chain terminates because the second scene sets the very flag its
        # own trigger tests.
        trigger = next(t for t in chain[1]["triggers_after"] if t["event"] == "0xA0")
        self.assertEqual(trigger["gates"],
                         [{"flag": 0x15, "flag_hex": "0x15", "required": "clear"}])

    def test_the_start_maps_other_triggers_do_not_fire(self):
        # Both are reported rather than dropped, and each is excluded for a
        # reason the extraction can state.
        chain = self.payload["first_control"]["scene_chain"]
        triggers = {t["index"]: t for t in chain[0]["triggers_after"]}
        self.assertEqual(sorted(triggers), [3, 10, 124])
        # RunEvent_FindingAlys waits for the party to stand somewhere, and the
        # party cannot move before it has control.
        self.assertTrue(triggers[3]["position_gated"])
        # RunEvent_SuspicionOnPrincipal wants a flag a new game has not set.
        self.assertFalse(triggers[10]["position_gated"])
        self.assertIn({"flag": 9, "flag_hex": "0x09", "required": "set"},
                      triggers[10]["gates"])

    def test_both_flags_are_set_at_first_control(self):
        flags = self.payload["first_control"]["event_flags_set"]
        self.assertEqual([f["id"] for f in flags],
                         [EVENT_FLAG_PIATA_FIRST_TIME, EVENT_FLAG_PIATA_CHAZ_CONTROL])
        self.assertEqual([f["symbol"] for f in flags],
                         ["EventFlag_PiataFirstTime", "EventFlag_PiataChazControl"])

    def test_the_flag_banks_match_the_emulator_oracle(self):
        # oracle/README.md and oracle/logs/01_newgame.csv, last frame (6938):
        #   eflags_00 = 01000400, eflags_04..eflags_1C = 00000000
        #   ext_eflags_00 = 00000000
        #   town_flags_00 = 80008040
        banks = self.payload["first_control"]["flag_banks"]
        self.assertEqual(banks["event_flags"]["first_long"], "0x01000400")
        self.assertEqual(banks["event_flags"]["raw_hex"], "01000400" + "00" * 28)
        self.assertEqual(banks["extended_event_flags"]["first_long"], "0x00000000")
        self.assertEqual(banks["town_flags"]["first_long"], "0x80008040")
        self.assertEqual(banks["chest_flags"]["raw_hex"], "00" * 32)
        for bank in banks.values():
            self.assertEqual(bank["bytes"], 32)

    def test_all_four_banks_are_scanned_not_just_the_base_one(self):
        # The four setters share one bit routine and differ only in the `lea`
        # that picks the bank, so a scan that knew about one would miss three.
        setters = flag_setters(self.data)
        self.assertEqual(sorted(setters.values()),
                         ["chest_flags", "event_flags", "extended_event_flags",
                          "town_flags"])
        self.assertEqual(setters[EVENT_FLAGS_SET], "event_flags")
        control = self.payload["first_control"]
        self.assertEqual(control["chest_flags_set"], [])
        self.assertEqual(control["town_flags_set"], [0x00, 0x10, 0x19])
        self.assertEqual(len(control["extended_event_flags_set"]), 11)
        # Neither scene touches anything but the base bank, so the other three
        # are exactly what the initialiser left.
        init = self.payload["new_game_init"]["flags"]
        self.assertEqual(
            [f"0x{flag:03X}" for flag in control["extended_event_flags_set"]],
            init["extended_event_flags"]["set"],
        )

    # ------------------------------------------------------------ fail-closed
    def test_the_trigger_decoder_reads_gates_and_dispatch(self):
        # RunEvent_PiataChazAlone: one flag gate, one dispatch, no position.
        trigger = read_trigger(self.data, 124)
        self.assertEqual(trigger.event, 0xA0)
        self.assertEqual(trigger.gates, ((EVENT_FLAG_PIATA_CHAZ_CONTROL, False),))
        self.assertFalse(trigger.position_gated)
        self.assertTrue(trigger.fires(frozenset()))
        self.assertFalse(trigger.fires(frozenset({EVENT_FLAG_PIATA_CHAZ_CONTROL})))
        # RunEvent_Null00 dispatches nothing, so it can never fire.
        null = read_trigger(self.data, 0)
        self.assertIsNone(null.event)
        self.assertFalse(null.fires(frozenset()))

    def test_the_event_table_ends_where_the_code_after_it_begins(self):
        routines = event_routines(self.data)
        self.assertEqual(len(routines), 0xA1)
        self.assertEqual(EVENT_PTRS + len(routines) * 4, 0x05A538)
        # Event_PiataChazAlone is the last routine by address, which is why its
        # extent comes from its tail jump rather than from the next entry.
        self.assertEqual(max(routines), routines[0xA0])

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
