import re
import unittest
from pathlib import Path

from psiv_tools.core import read_rom
from psiv_tools.maps import MapError, extract_encounter_binding, extract_maps
from psiv_tools.maps.encounters import (
    ENEMY_FORMATION_INDEXES,
    ENEMY_FORMATION_INDEXES_SIZE,
    POSITION_GRIDS,
)
from psiv_tools.maps.records import (
    ERROR_TRAP,
    FIELD_MAP_PTRS,
    MAP_COUNT,
    MAP_DATA_MANAGER_ROUTINES,
    MAP_UPDATE_ROUTINES,
    XY_RANGES,
    _Cursor,
    _decode_chest,
    _decode_interaction,
    _decode_object,
    _decode_transition,
    _read_fixed_records,
    _read_index_list,
    _read_scroll,
    _read_word_long_list,
)
from psiv_tools.shops import extract_shops
from psiv_tools.symbols import (
    FIELD_OBJECT_SYMBOLS,
    MAP_SYMBOLS,
    MUSIC_ID_BASE,
    MUSIC_SYMBOLS,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
DISASM = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"
ASM = DISASM / "ps4.asm"
CONSTANTS = DISASM / "ps4.constants.asm"

# Sections in the exact order GameMode_LoadFieldMap advances a0 through them.
# The two leading bytes (Map_General_Var, music id) are not sections.
SECTION_ORDER = [
    "tilesets", "sprites", "sprite_data", "dimensions", "scroll", "chunks",
    "map_updates", "transitions", "transitions_2", "objects", "treasure_chests",
    "tile_animations", "layout", "dialogue", "interaction_areas", "events",
    "palette", "flags", "map_data_manager",
]


def cursor(hex_bytes: str) -> _Cursor:
    """A synthetic record in a buffer big enough for its own pointers."""
    raw = bytes.fromhex(hex_bytes)
    return _Cursor(raw.ljust(0x1000, b"\x00"), 0, 0x10, "Piata")


def read_jump_table(label: str, stride: int = 1) -> list[str]:
    """Read consecutive `bra.w` entries under a jump-table label.

    Anything else -- an `if`, a comment, a stray directive -- ends the read,
    because a jump table with a conditional in it is not the same table in
    every build and the entry count would silently change meaning. `stride`
    is 4 for the one table the disassembly annotates with byte offsets rather
    than entry numbers, which is also how the game indexes it.
    """
    lines = ASM.read_text(encoding="utf-8", errors="replace").splitlines()
    start = next(i for i, line in enumerate(lines) if line.startswith(f"{label}:"))
    entries = []
    for raw in lines[start + 1:]:
        line = raw.strip()
        if not line:
            continue
        match = re.match(r"^bra\.w\s+(\S+)\s*;\s*(\S+)", line)
        if not match:
            break
        expected = match.group(2)
        value = int(expected.lstrip("$"), 16) if expected.startswith("$") else int(expected)
        if value != len(entries) * stride:
            raise AssertionError(
                f"{label} entry {len(entries)} is annotated {expected}, which is not "
                f"{len(entries) * stride}"
            )
        entries.append(match.group(1))
    return entries


def read_enemy_formation_indexes() -> list[int]:
    """Rebuild Battle_EnemyFormationIndexes from its `dc.b` listing."""
    lines = ASM.read_text(encoding="utf-8", errors="replace").splitlines()
    start = next(
        i for i, line in enumerate(lines)
        if line.startswith("Battle_EnemyFormationIndexes:")
    )
    values: list[int] = []
    for lineno, raw in enumerate(lines[start + 1:], start=start + 2):
        line = raw.split(";", 1)[0].strip()
        if not line:
            continue
        if not line.startswith("dc.b"):
            break
        operand = line.split(None, 1)[1].strip()
        if not re.fullmatch(r"\$[0-9A-Fa-f]{1,2}", operand):
            raise AssertionError(f"ps4.asm:{lineno}: unexpected operand {operand!r}")
        values.append(int(operand.lstrip("$"), 16))
    return values


def read_piata_transition_targets() -> list[str]:
    """Pull the `MapID_*` operands out of Map_Piata's transition blocks."""
    lines = ASM.read_text(encoding="utf-8", errors="replace").splitlines()
    start = next(i for i, line in enumerate(lines) if line.startswith("Map_Piata:"))
    targets = []
    for raw in lines[start:]:
        if raw.strip().startswith("; Objects"):
            break
        match = re.search(r"dc\.w\s+MapID_(\w+)\s*$", raw.split(";")[0].rstrip())
        if match:
            targets.append(match.group(1))
    return targets


class TestPackageSurface(unittest.TestCase):
    """The three modules are an implementation detail of one import path."""

    def test_public_entry_points_come_from_the_package(self):
        import psiv_tools.maps as maps

        self.assertEqual(sorted(maps.__all__), ["MapError", "extract_encounter_binding", "extract_maps"])
        for name in maps.__all__:
            self.assertTrue(hasattr(maps, name), name)

    def test_symbol_tables_live_with_the_other_disassembly_symbols(self):
        import psiv_tools.symbols as symbols

        self.assertEqual(len(symbols.MUSIC_SYMBOLS), 52)
        self.assertEqual(len(symbols.FIELD_OBJECT_SYMBOLS), 222)
        self.assertEqual(symbols.MUSIC_ID_BASE, 0x81)


class TestRecordDecoders(unittest.TestCase):
    """Structural unit tests. These need no ROM and no disassembly."""

    def test_transition_record(self):
        decoded = _decode_transition(bytes.fromhex("002f000700002e8e0000"))
        self.assertEqual(decoded["source"], {"x_tile": 0, "y_tile": 47, "x": 0, "y": 752})
        self.assertEqual(decoded["range"], {"id": 7, "name": "YHigher"})
        self.assertEqual(decoded["target"]["symbol"], "Motavia")
        self.assertEqual(decoded["destination"]["x"], 0x2E * 16)
        self.assertEqual(decoded["destination"]["y"], 0x8E * 16)

    def test_object_id_is_a_jump_table_byte_offset(self):
        decoded = _decode_object(bytes.fromhex("00a0000002d0003c005c"))
        self.assertEqual(decoded["object_id"], 0xA0)
        self.assertEqual(decoded["symbol"], "TreasureChest")
        self.assertEqual(decoded["x"], 0x3C * 8)

    def test_chest_item_and_meseta_are_different_fields(self):
        item = _decode_chest(bytes.fromhex("0000187d2808"))
        self.assertEqual(item["contents_type"], "item")
        self.assertEqual(item["item_id"], 0x7D)
        self.assertIsNone(item["meseta"])
        self.assertFalse(item["white_chest"])

        meseta = _decode_chest(bytes.fromhex("010118322808"))
        self.assertEqual(meseta["contents_type"], "meseta")
        self.assertIsNone(meseta["item_id"])
        self.assertEqual(meseta["meseta"], 0x32 * 100)
        self.assertTrue(meseta["white_chest"])
        self.assertEqual(meseta["object_symbol"], "WhiteTreasureChest")

    def test_interaction_parameter_is_not_the_record_index(self):
        decoded = _decode_interaction(bytes.fromhex("002c004a000c00000001"))
        self.assertEqual(decoded["interaction_type"], 0)
        self.assertEqual(decoded["parameter"], 1)
        self.assertNotIn("index", decoded)

    def test_scroll_reads_step_counters_only_when_the_mode_byte_is_zero(self):
        """loc_51AB2's `bne` skips the four longs when the byte is non-zero."""
        short = _read_scroll(cursor("010000010001" + "dead"))
        self.assertEqual(short["size_bytes"], 6)
        self.assertIsNone(short["fg_step_counters"])
        self.assertIsNone(short["bg_step_counters"])

        both = _read_scroll(cursor("0100" + "0000" + "11111111" + "22222222"
                                   + "0000" + "33333333" + "44444444"))
        self.assertEqual(both["size_bytes"], 22)
        self.assertEqual(both["fg_step_counters"], {"x": "0x11111111", "y": "0x22222222"})
        self.assertEqual(both["bg_step_counters"], {"x": "0x33333333", "y": "0x44444444"})

    def test_tileset_list_ends_on_any_negative_word(self):
        """loc_519D2 uses `tst.w` + `bmi`, not a $FFFF comparison."""
        section = _read_word_long_list(
            cursor("001000000010" + "fffe"), "tileset art", "vram_tile",
            "kosinski", negative_terminator=True,
        )
        self.assertEqual(section["entry_count"], 1)
        self.assertEqual(section["terminator"], "0xFFFE")
        self.assertEqual(section["size_bytes"], 8)

    def test_kosinski_sprite_list_ends_only_on_ffff(self):
        """loc_51A5C compares the word against $FFFF, so $FFFE is an address."""
        section = _read_word_long_list(
            cursor("fffe00000010" + "ffff"), "sprite data", "ram_address",
            "kosinski", negative_terminator=False, sign_extend=True,
        )
        self.assertEqual(section["entry_count"], 1)
        self.assertEqual(section["entries"][0]["ram_address"], "0xFFFFFFFE")

    def test_index_list_rejects_an_out_of_range_routine(self):
        with self.assertRaises(MapError):
            _read_index_list(cursor("40ff"), 1, MAP_UPDATE_ROUTINES, "map update")
        with self.assertRaises(MapError):
            _read_index_list(cursor("00a0ffff"), 2, MAP_DATA_MANAGER_ROUTINES, "MapDataManager")

    def test_record_walk_and_word_scan_must_agree(self):
        """A $FFFE terminator would split the loader's two paths for objects."""
        record = "004c045302d0003c005c"
        self.assertEqual(
            _read_fixed_records(
                cursor(record + "ffff"), 10, "objects", _decode_object,
                negative_terminator=True,
            )["count"],
            1,
        )
        with self.assertRaises(MapError):
            _read_fixed_records(
                cursor(record + "fffe" + "ffff"), 10, "objects", _decode_object,
                negative_terminator=True,
            )

    def test_music_symbols_are_contiguous_from_the_base_id(self):
        self.assertEqual(MUSIC_ID_BASE, 0x81)
        self.assertEqual(MUSIC_SYMBOLS[0x84 - MUSIC_ID_BASE], "MotabiaTown")

    def test_field_object_symbols_cover_the_whole_jump_table(self):
        self.assertEqual(len(FIELD_OBJECT_SYMBOLS), 222)
        self.assertEqual(FIELD_OBJECT_SYMBOLS[0xA0 // 4], "TreasureChest")
        self.assertEqual(FIELD_OBJECT_SYMBOLS[0x1D4 // 4], "WhiteTreasureChest")


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestMapsFromRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.result = extract_maps(cls.data)
        cls.maps = cls.result["maps"]
        cls.real = [m for m in cls.maps if not m["is_null"]]
        cls.piata = cls.maps[0x10]

    def test_pointer_table_geometry(self):
        table = self.result["pointer_table"]
        self.assertEqual(table["rom_offset"], "0x100000")
        self.assertEqual(table["rom_end_exclusive"], "0x100684")
        self.assertEqual(table["count"], MAP_COUNT)
        self.assertEqual(table["entry_size"], 4)
        self.assertEqual(table["error_trap"], "0x000200")

    def test_pointer_table_is_flush_against_the_first_map_record(self):
        """FieldMapPtrs[0] is Map_Motavia, and it is the byte after the table."""
        end = FIELD_MAP_PTRS + MAP_COUNT * 4
        self.assertEqual(int.from_bytes(self.data[FIELD_MAP_PTRS:FIELD_MAP_PTRS + 4], "big"), end)
        self.assertEqual(self.maps[0]["rom_offset"], f"0x{end:06X}")

    def test_real_and_null_split(self):
        self.assertEqual(self.result["real_map_count"], 361)
        self.assertEqual(self.result["null_map_count"], 56)
        self.assertEqual(len(self.maps), MAP_COUNT)
        for entry in self.maps:
            with self.subTest(entry["id_hex"]):
                self.assertEqual(entry["is_null"], entry["symbol"].startswith("Null"))
                if entry["is_null"]:
                    self.assertEqual(entry["pointer"], f"0x{ERROR_TRAP:06X}")

    def test_altered_data_fails_closed(self):
        cases = {
            "pointer table": FIELD_MAP_PTRS,
            "encounter table": ENEMY_FORMATION_INDEXES,
            # Piata's map-update byte; $02 -> $FD is past MapUpdateJmpTbl.
            "map update index": 0x11C6E8,
            # Piata's object-list terminator; $FFFF -> $0000 keeps the record
            # walk running into the treasure-chest section.
            "object terminator": 0x11C79A,
        }
        for label, offset in cases.items():
            with self.subTest(label):
                corrupt = bytearray(self.data)
                corrupt[offset] ^= 0xFF
                with self.assertRaises(MapError):
                    extract_maps(bytes(corrupt))

    def test_sections_tile_the_record_exactly(self):
        """No gaps and no overlaps: the sections are the whole record."""
        for entry in self.real:
            with self.subTest(entry["symbol"]):
                start = int(entry["rom_offset"], 16)
                # Map_General_Var and the music id come before the first section.
                cursor_at = start + 2
                for key in SECTION_ORDER:
                    section = entry[key]
                    self.assertEqual(section["rom_offset"], f"0x{cursor_at:06X}", key)
                    cursor_at = int(section["rom_end_exclusive"], 16)
                    self.assertEqual(
                        section["size_bytes"],
                        cursor_at - int(section["rom_offset"], 16),
                        key,
                    )
                self.assertEqual(entry["rom_end_exclusive"], f"0x{cursor_at:06X}")
                self.assertEqual(entry["size_bytes"], cursor_at - start)
                self.assertEqual(bytes.fromhex(entry["raw_hex"]), self.data[start:cursor_at])

    def test_records_do_not_overlap(self):
        spans = sorted(
            (int(m["rom_offset"], 16), int(m["rom_end_exclusive"], 16), m["symbol"])
            for m in self.real
        )
        for (_, prev_end, prev_name), (start, _, name) in zip(spans, spans[1:]):
            self.assertLessEqual(prev_end, start, f"{prev_name} overlaps {name}")

    def test_piata_record(self):
        piata = self.piata
        self.assertEqual(piata["symbol"], "Piata")
        self.assertEqual(piata["rom_offset"], "0x11C690")
        self.assertEqual(piata["rom_end_exclusive"], "0x11C808")
        self.assertEqual(piata["general_var"], 3)
        self.assertEqual(piata["music"], {
            "id": 0x84, "id_hex": "0x84", "symbol": "MotabiaTown", "changes_music": True,
        })
        self.assertEqual(piata["tilesets"]["entry_count"], 3)
        self.assertEqual(piata["sprites"]["entry_count"], 7)
        self.assertEqual(piata["sprite_data"]["entry_count"], 0)
        self.assertEqual(
            [piata["dimensions"][k] for k in
             ("fg_row_size", "fg_column_size", "bg_row_size", "bg_column_size")],
            [0x1F] * 4,
        )
        self.assertEqual(piata["chunks"]["pointers"], ["0x122A90", "0x123380"])
        self.assertEqual(piata["map_updates"]["ids"], [2])
        self.assertEqual(piata["layout"]["fg_offset"], "0x123710")
        self.assertEqual(piata["palette"]["pointer"], "0x123990")
        self.assertEqual(piata["map_data_manager"]["ids"], [])

    def test_piata_record_ends_where_its_own_art_begins(self):
        """The record's last byte is followed by loc_11C808, its first tileset.

        That is an independent end-of-record proof: the walk cannot have
        stopped early or late without landing somewhere other than the
        address the record itself stores.
        """
        self.assertEqual(
            self.piata["tilesets"]["entries"][0]["source_offset"],
            self.piata["rom_end_exclusive"],
        )

    def test_piata_warps(self):
        """Piata exits to the Motavia world map and into six interiors."""
        self.assertEqual(self.piata["transitions"]["count"], 1)
        world = self.piata["transitions"]["entries"][0]
        self.assertEqual(world["target"]["symbol"], "Motavia")
        self.assertEqual(world["source"], {"x_tile": 0, "y_tile": 0x2F, "x": 0, "y": 0x2F * 16})
        self.assertEqual(world["range"], {"id": 7, "name": "YHigher"})
        self.assertEqual(
            (world["destination"]["x_tile"], world["destination"]["y_tile"]),
            (0x2E, 0x8E),
        )
        self.assertEqual(
            [t["target"]["symbol"] for t in self.piata["transitions_2"]["entries"]],
            ["PiataAcademy", "PiataDorm", "PiataInn", "PiataHouse1",
             "PiataItemShop", "PiataHouse2"],
        )

    def test_every_transition_targets_a_real_map(self):
        total = 0
        for entry in self.real:
            for key in ("transitions", "transitions_2"):
                for record in entry[key]["entries"]:
                    total += 1
                    target = record["target"]
                    self.assertLess(target["id"], MAP_COUNT)
                    self.assertFalse(self.maps[target["id"]]["is_null"], record)
                    self.assertEqual(record["range"]["name"], XY_RANGES[record["range"]["id"]])
        self.assertEqual(total, self.result["totals"]["transitions"])
        self.assertEqual(total, 930)

    def test_every_map_binds_exactly_one_dialogue_tree(self):
        for entry in self.real:
            with self.subTest(entry["symbol"]):
                self.assertIn(entry["dialogue"]["tree"], range(1, 44))
                self.assertEqual(
                    entry["dialogue"]["label"], f"DialogueTree{entry['dialogue']['tree']}"
                )
        bound = self.result["dialogue_trees_bound"]
        self.assertEqual(len(bound), 36)
        # Trees 6 and 27-32 are never named by a map header. 31 and 32 are the
        # "talk to your party" trees the text slice already treats specially;
        # the rest are reached through WorldDialogueTreePtrs instead.
        self.assertEqual(sorted(set(range(1, 44)) - set(bound)), [6, 27, 28, 29, 30, 31, 32])

    def test_totals(self):
        self.assertEqual(self.result["totals"], {
            "tilesets": 1107,
            "sprite_art": 509,
            "chunk_pointers": 438,
            "transitions": 930,
            "objects": 949,
            "treasure_chests": 155,
            "tile_animations": 19,
            "interaction_areas": 649,
            "record_bytes": 59512,
        })

    def test_world_maps_carry_no_layout_pointers(self):
        for map_id in (0, 1):
            with self.subTest(MAP_SYMBOLS[map_id]):
                layout = self.maps[map_id]["layout"]
                self.assertFalse(layout["present"])
                self.assertEqual(layout["size_bytes"], 0)
        for entry in self.real:
            if entry["id"] > 1:
                self.assertTrue(entry["layout"]["present"], entry["symbol"])
                self.assertEqual(entry["layout"]["size_bytes"], 8)

    def test_every_pointer_lands_inside_the_rom(self):
        for entry in self.real:
            with self.subTest(entry["symbol"]):
                pointers = [entry["palette"]["pointer"], entry["dialogue"]["pointer"]]
                pointers += entry["chunks"]["pointers"]
                pointers += [e["source_offset"] for e in entry["tilesets"]["entries"]]
                pointers += [e["source_offset"] for e in entry["sprites"]["entries"]]
                pointers += [e["source_offset"] for e in entry["sprite_data"]["entries"]]
                if entry["layout"]["present"]:
                    pointers += [entry["layout"]["fg_offset"], entry["layout"]["bg_offset"]]
                for pointer in pointers:
                    self.assertTrue(0 < int(pointer, 16) < len(self.data), pointer)

    def test_chest_contents_resolve(self):
        from psiv_tools.symbols import ITEM_SYMBOLS

        chests = [c for m in self.real for c in m["treasure_chests"]["entries"]]
        self.assertEqual(len(chests), 155)
        for chest in chests:
            with self.subTest(chest["rom_offset"]):
                if chest["contents_type"] == "item":
                    self.assertTrue(1 <= chest["item_id"] <= len(ITEM_SYMBOLS))
                else:
                    self.assertGreater(chest["meseta"], 0)

    def test_interaction_routines_used_by_retail_maps(self):
        """Only four of the eight InteractionRoutines are ever selected.

        Nothing in the map data reaches routine 7, the one this clone adds for
        the Grand Cross hack, so the retail records do not depend on it.
        """
        used = {
            area["interaction_type"]
            for entry in self.real
            for area in entry["interaction_areas"]["entries"]
        }
        self.assertEqual(sorted(used), [0, 1, 2, 5])

    def test_shop_locations_land_on_real_maps(self):
        locations = extract_shops(self.data)["locations"]["entries"]
        for entry in locations:
            with self.subTest(entry["rom_offset"]):
                self.assertLess(entry["map_id"], MAP_COUNT)
                self.assertFalse(self.maps[entry["map_id"]]["is_null"], entry)

    def test_piata_shops_are_reachable_from_piata(self):
        """The shop table and the map graph agree about where Piata's shops are."""
        reachable = {t["target"]["symbol"] for t in self.piata["transitions_2"]["entries"]}
        locations = extract_shops(self.data)["locations"]["entries"]
        piata_shops = {
            e["map_symbol"] for e in locations
            if e["map_symbol"] and e["map_symbol"].startswith("Piata")
        }
        self.assertEqual(piata_shops, {"PiataInn", "PiataItemShop"})
        self.assertTrue(piata_shops <= reachable)

    def test_map_symbols_and_pointers_agree_about_which_maps_are_null(self):
        pointers = [
            int.from_bytes(self.data[FIELD_MAP_PTRS + i * 4:FIELD_MAP_PTRS + i * 4 + 4], "big")
            for i in range(MAP_COUNT)
        ]
        self.assertEqual(
            [i for i, p in enumerate(pointers) if p == ERROR_TRAP],
            [i for i, s in enumerate(MAP_SYMBOLS) if s.startswith("Null")],
        )


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestEncounterBindingFromRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.result = extract_encounter_binding(cls.data)

    def test_table_geometry_is_one_byte_short_of_the_map_space(self):
        table = self.result["table"]
        self.assertEqual(table["rom_offset"], "0x008050")
        self.assertEqual(table["rom_end_exclusive"], "0x0081F0")
        self.assertEqual(table["count"], ENEMY_FORMATION_INDEXES_SIZE)
        self.assertEqual(table["count"], 416)
        self.assertEqual(table["map_count"], 417)
        self.assertEqual(table["maps_outside_table"], 1)

    def test_the_map_outside_the_table_never_rolls_an_encounter(self):
        """MapID $1A0 reads past the table; only its cleared flag saves it."""
        outside = [m for m in self.result["maps"] if m["mode"] == "outside_table"]
        self.assertEqual([m["map_symbol"] for m in outside], ["AirCastleSpace"])
        maps = extract_maps(self.data)["maps"]
        self.assertEqual(maps[416]["flags"]["random_battles"], 0)

    def test_group_ids_are_inside_the_formation_index_table(self):
        self.assertEqual(self.result["formation_group_count"], 68)
        for entry in self.result["maps"]:
            if entry["mode"] == "group":
                self.assertLess(entry["group"], 68)
        for grid in self.result["position_grids"]:
            for group in grid["groups_used"]:
                self.assertLess(group, 68)
        for vehicle in self.result["vehicle_groups"]:
            self.assertLess(vehicle["group"], 68)

    def test_position_grids_decompress_to_their_documented_ranges(self):
        mota, dezo = self.result["position_grids"]
        self.assertEqual(mota["label"], "Battle_MotaFormationGroupIndexes")
        self.assertEqual((mota["rom_offset"], mota["rom_end_exclusive"]), ("0x28501C", "0x2853C8"))
        self.assertEqual(mota["decompressed_size"], 4096)
        self.assertEqual((mota["rows"], mota["columns"]), (64, 64))
        self.assertEqual(mota["groups_used"], list(range(8)))
        self.assertEqual(mota["fallback_cells"], 2460)

        self.assertEqual(dezo["label"], "Battle_DezoFormationGroupIndexes")
        self.assertEqual((dezo["rom_offset"], dezo["rom_end_exclusive"]), ("0x2853CC", "0x28541F"))
        self.assertEqual(dezo["decompressed_size"], 2048)
        self.assertEqual((dezo["rows"], dezo["columns"]), (32, 64))
        self.assertEqual(dezo["groups_used"], [11, 12])
        self.assertEqual(dezo["fallback_cells"], 0)

    def test_world_maps_use_the_position_grids(self):
        for map_id, grid in ((0, "motavia"), (1, "dezolis")):
            entry = self.result["maps"][map_id]
            self.assertEqual(entry["mode"], "position_grid")
            self.assertEqual(entry["position_grid"], grid)
            self.assertEqual(entry["value"], map_id)

    def test_encounter_modes_are_exhaustive(self):
        modes = {}
        for entry in self.result["maps"]:
            modes[entry["mode"]] = modes.get(entry["mode"], 0) + 1
        self.assertEqual(modes, {
            "position_grid": 2, "group": 147, "none": 267, "outside_table": 1,
        })

    def test_the_three_flag_disagreements_are_reported(self):
        """The table and Random_Battles_Flag are stored apart and disagree thrice."""
        anomalies = extract_maps(self.data)["encounter_anomalies"]
        self.assertEqual(
            [(a["id"], a["symbol"], a["mode"], a["random_battles_flag"]) for a in anomalies],
            [
                (0x02D, "ValleyMazeUnused", "none", 1),
                (0x15B, "MystVale_Part4", "group", 0),
                (0x184, "AirCastleXeAThoulRoom", "group", 0),
            ],
        )

    def test_a_group_count_that_disagrees_with_the_data_fails_closed(self):
        with self.assertRaises(MapError):
            extract_encounter_binding(self.data, group_count=13)


@unittest.skipUnless(ASM.is_file(), f"disassembly oracle not present at {DISASM}")
class TestMapsAgainstDisassembly(unittest.TestCase):
    def test_music_symbols_match_the_constants(self):
        pattern = re.compile(r"^MusicID_(\w+)\s*=\s*id\(PtrMusic_\w+\)\s*;\s*\$([0-9A-Fa-f]+)")
        constants = {}
        for line in CONSTANTS.read_text(encoding="utf-8", errors="replace").splitlines():
            match = pattern.match(line.strip())
            if match:
                constants[int(match.group(2), 16)] = match.group(1)
        self.assertEqual(len(constants), len(MUSIC_SYMBOLS))
        self.assertEqual(min(constants), MUSIC_ID_BASE)
        for value, name in constants.items():
            self.assertEqual(MUSIC_SYMBOLS[value - MUSIC_ID_BASE], name)

    def test_field_object_symbols_match_the_jump_table(self):
        entries = read_jump_table("FieldObjectsJmpTbl", stride=4)
        self.assertEqual(len(entries), len(FIELD_OBJECT_SYMBOLS))
        for index, label in enumerate(entries):
            expected = label[len("FieldObj_"):] if label.startswith("FieldObj_") else label
            self.assertEqual(FIELD_OBJECT_SYMBOLS[index], expected)

    def test_xy_ranges_match_the_jump_table(self):
        entries = read_jump_table("XYRangeJmpTbl")
        self.assertEqual(
            [e[len("XYRange_"):] for e in entries],
            list(XY_RANGES),
        )

    def test_jump_table_bounds(self):
        self.assertEqual(len(read_jump_table("MapUpdateJmpTbl")), MAP_UPDATE_ROUTINES)
        self.assertEqual(
            len(read_jump_table("MapDataManagerJmpTbl")), 0x83
        )

    def test_map_data_manager_table_is_a0_entries_including_its_option_block(self):
        """The table continues past an `if revision=0` block to index $9F.

        `Map_LoadChunks` reaches into it with the raw byte offset $98, which is
        index $26, so the table has to be counted as a whole rather than
        stopped at the first conditional.
        """
        text = ASM.read_text(encoding="utf-8", errors="replace")
        start = text.index("MapDataManagerJmpTbl:")
        block = text[start:text.index("; =================================================================", start)]
        self.assertEqual(len(re.findall(r"^\tbra\.w", block, re.M)), MAP_DATA_MANAGER_ROUTINES + 2)
        self.assertIn("if revision=0", block)

    @unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
    def test_enemy_formation_indexes_round_trip(self):
        expected = read_enemy_formation_indexes()
        self.assertEqual(len(expected), ENEMY_FORMATION_INDEXES_SIZE)
        data = read_rom(ROM)
        actual = data[ENEMY_FORMATION_INDEXES:ENEMY_FORMATION_INDEXES + len(expected)]
        self.assertEqual(list(actual), expected)
        self.assertEqual(data.count(bytes(expected)), 1)

    @unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
    def test_piata_transition_targets_match_the_source(self):
        expected = read_piata_transition_targets()
        piata = extract_maps(read_rom(ROM))["maps"][0x10]
        actual = [
            t["target"]["symbol"]
            for t in piata["transitions"]["entries"] + piata["transitions_2"]["entries"]
        ]
        self.assertEqual(actual, expected)

    @unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
    def test_position_grid_ranges_match_the_inline_annotations(self):
        text = ASM.read_text(encoding="utf-8", errors="replace")
        for spec in POSITION_GRIDS:
            with self.subTest(spec["label"]):
                self.assertIn(
                    f"(0x{spec['start']:08X}-0x{spec['end']:08X}", text
                )


if __name__ == "__main__":
    unittest.main()
