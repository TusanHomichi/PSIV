import json
import re
import struct
import tempfile
import unittest
from pathlib import Path

from psiv_tools.core import read_rom
from psiv_tools.map_effects import (
    FIELD_OBJ_SECONDARY,
    FIELD_OBJ_STRIDE,
    FLAG_BANKS,
    FLAG_CLEAR_BLOCK,
    FLAG_CLEAR_DOORS,
    FLAG_TEST_BLOCK,
    GET_MAP_LAYOUT_OFFSET,
    MAP_DATA_MANAGER_ROUTINES,
    MAP_LAYOUT_BYTES,
    Gate,
    MapEffectsError,
    Write,
    decode_entry,
    dispatch_table,
    extract_map_effects,
    flag_clears,
    map_layout_bases,
    flag_tests,
    resolve_layout_write,
    routine_address,
)
from psiv_tools.maps import extract_maps
from psiv_tools.pack import PACK_FORMAT_VERSION, build_pack

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"

MAP_ZEMA = 0x024
MAP_BIRTH_VALLEY_B1 = 0x02C
MAP_PIATA_ACADEMY_F1 = 0x013
MAP_GARUBERK_PART2 = 0x19A
#: `MapDataMan_ChkZemaNormal`, and the flag it waits on.
ZEMA_ENTRY = 0x05
EVENT_FLAG_IGGLANOVA_ZEMA = 0x33
#: `MapDataMan_PiataAcademy_F1` clears the Alys object once she is found.
ALYS_ENTRY = 0x01
EVENT_FLAG_ALYS_FOUND = 0x08


#: The two layout buffers, as `GetMapLayoutOffset` names them.
BASES = {"fg": 0xA000, "bg": 0xB000}


class TestResolveLayoutWrite(unittest.TestCase):
    """A displacement is applied to an address, so it needs the buffer bases."""

    def write(self, plane="bg", x=4, y=3, disp=0, chunk=0x59):
        return Write("layout_write", 0x1234, {
            "plane": plane, "chunk_x": x, "chunk_y": y,
            "displacement": disp, "chunk_id": chunk,
        })

    def resolve(self, write, widths, heights=None):
        return resolve_layout_write(write, widths, BASES, heights)

    def test_a_displacement_wraps_onto_the_next_row(self):
        widths = {"fg": 32, "bg": 32}
        self.assertEqual(
            self.resolve(self.write(x=31, disp=1), widths)["chunk_x"], 0)
        self.assertEqual(
            self.resolve(self.write(x=31, disp=1), widths)["chunk_y"], 4)
        # One row down is exactly the row size.
        moved = self.resolve(self.write(disp=32), widths)
        self.assertEqual((moved["chunk_x"], moved["chunk_y"]), (4, 4))

    def test_the_same_write_resolves_differently_on_two_maps(self):
        # This is why resolution happens at emission and not at decode: one
        # routine serves maps of different widths.
        narrow = self.resolve(self.write(disp=1), {"fg": 32, "bg": 32})
        wide = self.resolve(self.write(x=31, disp=1), {"fg": 48, "bg": 48})
        self.assertEqual((narrow["chunk_x"], narrow["chunk_y"]), (5, 3))
        self.assertEqual((wide["chunk_x"], wide["chunk_y"]), (32, 3))

    def test_the_plane_picks_which_row_size_applies(self):
        widths = {"fg": 32, "bg": 48}
        self.assertEqual(
            self.resolve(self.write(plane="fg", x=31, disp=1), widths)["chunk_y"], 4)
        self.assertEqual(
            self.resolve(self.write(plane="bg", x=31, disp=1), widths)["chunk_y"], 3)

    def test_cells_are_twice_the_chunk(self):
        resolved = self.resolve(self.write(x=5, y=6), {"fg": 32, "bg": 32})
        self.assertEqual((resolved["cell_x"], resolved["cell_y"]), (10, 12))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestTheMechanism(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.table = dispatch_table(cls.data)

    def test_the_jump_table_is_named_by_its_own_dispatcher(self):
        self.assertEqual(self.table, 0x051B64)
        for index in (0, 1, MAP_DATA_MANAGER_ROUTINES - 1):
            with self.subTest(entry=index):
                self.assertEqual(
                    struct.unpack_from(">H", self.data, self.table + index * 4)[0],
                    0x6000,  # every entry is a bra.w
                )

    def test_there_are_four_flag_doors_and_no_temp_bank(self):
        # The disassembly names a `TempEveFlags_Test` reading $FFFFF156. No
        # routine in the retail image loads that address; the routine it labels
        # that way is the $FFFFF140 door it elsewhere calls ChestFlags_Test.
        tests = flag_tests(self.data)
        self.assertEqual(len(tests), 4)
        self.assertEqual(sorted(tests.values()), sorted(FLAG_BANKS.values()))
        self.assertEqual(tests[FLAG_TEST_BLOCK], "event_flags")
        self.assertEqual(
            [m.start() for m in re.finditer(re.escape(b"\x41\xf8\xf1\x56"), self.data)],
            [],
        )

    def test_the_clear_block_is_one_door_short(self):
        # Nothing in the cartridge clears a town flag.
        clears = flag_clears(self.data)
        self.assertEqual(len(clears), FLAG_CLEAR_DOORS)
        self.assertEqual(clears[FLAG_CLEAR_BLOCK], "event_flags")
        self.assertNotIn("town_flags", clears.values())

    def test_get_map_layout_offset_is_where_this_module_says(self):
        # `tst.w d3 / move.b Map_Row_Size_FG,d3 / lea Map_Layout_FG,a1`
        self.assertEqual(
            self.data[GET_MAP_LAYOUT_OFFSET:GET_MAP_LAYOUT_OFFSET + 12].hex(),
            "4a43660a1638ec6443f8a000",
        )

    def test_an_entry_past_the_table_is_refused(self):
        with self.assertRaises(MapEffectsError):
            routine_address(self.data, self.table, MAP_DATA_MANAGER_ROUTINES)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestDecodedEntries(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.maps = extract_maps(cls.data)["maps"]
        cls.effects = extract_map_effects(cls.data, cls.maps)
        cls.census = cls.effects["census"]

    def test_the_zema_doors(self):
        # The headline: four house doorways that cover no map-change cell in
        # the stored layout, opened by writing chunks $59/$5A once
        # EventFlag_IgglanovaZema is set. The routine walks a four-entry table,
        # so the decoder has to run its loop to see them at all.
        entry = decode_entry(self.data, ZEMA_ENTRY)
        self.assertIsNone(entry.undecoded)
        (path,) = [p for p in entry.paths if p.writes]
        self.assertEqual(
            [(g.bank, g.flag, g.required) for g in path.gates],
            [("event_flags", EVENT_FLAG_IGGLANOVA_ZEMA, True)],
        )
        self.assertEqual(len(path.writes), 8)
        self.assertEqual({w.kind for w in path.writes}, {"layout_write"})
        self.assertEqual(
            [(w.detail["chunk_x"], w.detail["chunk_y"], w.detail["chunk_id"])
             for w in path.writes],
            [(21, 11, 0x59), (21, 11, 0x5A), (20, 21, 0x59), (20, 21, 0x5A),
             (12, 15, 0x59), (12, 15, 0x5A), (17, 15, 0x59), (17, 15, 0x5A)],
        )
        # The second of each pair is the neighbouring chunk, reached by a
        # displacement of one rather than a fresh coordinate.
        self.assertEqual([w.detail["displacement"] for w in path.writes],
                         [0, 1, 0, 1, 0, 1, 0, 1])

    def test_alys_leaves_the_academy_when_she_is_found(self):
        entry = decode_entry(self.data, ALYS_ENTRY)
        self.assertIsNone(entry.undecoded)
        (path,) = [p for p in entry.paths if p.writes]
        self.assertEqual(
            [(g.bank, g.flag, g.required) for g in path.gates],
            [("event_flags", EVENT_FLAG_ALYS_FOUND, True)],
        )
        (write,) = path.writes
        self.assertEqual(write.kind, "object_despawn")
        # Alys_Piata is $FFFFC4C0, which is object 7 of the map's own list.
        self.assertEqual(write.detail["object_index"], 7)
        record = next(m for m in self.maps if m["id"] == MAP_PIATA_ACADEMY_F1)
        self.assertEqual(record["objects"]["entries"][7]["symbol"], "NPCAlysPiata")

    def test_object_indices_are_slots_of_the_map_they_patch(self):
        # The binding that makes the emitted index meaningful: `Field_LoadObject`
        # is a first-free scan from $FFFFC300, so record object N is slot N.
        # A handful of routines clear more slots than their map has objects;
        # those are the census's, and every other write is in range.
        records = {m["id"]: m for m in self.maps if not m["is_null"]}
        reported = {(e["map"], e["at"], e["object_index"])
                    for e in self.census["object_writes_past_map_object_count"]}
        checked = 0
        for map_id, entries in self.effects["per_map"].items():
            count = records[map_id]["objects"]["count"]
            for entry in entries:
                if not entry["decoded"]:
                    continue
                for path in entry["paths"]:
                    for write in path["writes"]:
                        if "object_index" not in write:
                            continue
                        checked += 1
                        if write["object_index"] >= count:
                            with self.subTest(map=hex(map_id), at=write["at"]):
                                self.assertIn(
                                    (map_id, write["at"], write["object_index"]), reported)
        self.assertGreater(checked, 300)

    def test_the_over_clearing_routines_are_a_short_known_list(self):
        # Two routines, on two maps. Clearing a slot the map never filled is a
        # no-op on hardware, so this is defensive code rather than a defect --
        # but a consumer indexing blindly would panic, hence the census.
        past = self.census["object_writes_past_map_object_count"]
        self.assertEqual(sorted({e["map"] for e in past}), [0x0F6, 0x0FB])
        self.assertEqual(sorted({e["entry"] for e in past}), ["0x64", "0x65"])
        for entry in past:
            with self.subTest(at=entry["at"]):
                self.assertGreaterEqual(entry["object_index"], entry["map_objects"])

    def test_gate_polarity_comes_from_the_branch(self):
        # A flag test leaves Z set when the flag is CLEAR, so `beq` is the
        # clear arm and `bne` the set arm. Both appear in retail.
        polarities = {
            gate["required"]
            for entries in self.effects["per_map"].values()
            for entry in entries if entry["decoded"]
            for path in entry["paths"] for gate in path["gates"]
        }
        self.assertEqual(polarities, {"set", "clear"})

    def test_no_retail_routine_aborts_the_rest_of_the_list(self):
        # The dispatcher allows it and the data carries the bit; nothing uses it.
        aborts = [
            (map_id, entry["entry_hex"])
            for map_id, entries in self.effects["per_map"].items()
            for entry in entries if entry["decoded"]
            for path in entry["paths"] if path["aborts_remaining_entries"]
        ]
        self.assertEqual(aborts, [])

    def test_the_census_counts(self):
        census = self.census
        self.assertEqual(census["table_entries"], MAP_DATA_MANAGER_ROUTINES)
        self.assertEqual(census["referenced_entries"], 127)
        self.assertEqual(census["unreferenced_entries"], 33)
        self.assertEqual(census["maps_with_entries"], 139)
        self.assertEqual(census["map_entry_pairs"], 188)
        self.assertEqual(census["decoded_entries"], 116)
        self.assertEqual(len(census["undecoded_entries"]), 11)
        self.assertEqual(census["evaluated"], "map load only")
        self.assertEqual(sorted(census["kinds"]), [
            "flag_clear", "layout_replace", "layout_write", "object_despawn",
            "object_dialogue", "object_rewrite",
        ])
        # 335, not the 339 an earlier pass counted: four of those came from a
        # path whose gates required one flag both set and clear, which no map
        # load can satisfy. Pruning it removed the path and its writes.
        self.assertEqual(census["kinds"]["object_despawn"], 335)
        self.assertEqual(census["kinds"]["layout_write"], 191)
        self.assertEqual(census["kinds"]["layout_replace"], 4)
        self.assertEqual(census["maps_per_kind"]["layout_replace"], 2)

    def test_only_two_banks_gate_anything(self):
        # Event flags and the $F140 bank. Extended and town flags gate no map
        # effect in the cartridge, which is worth knowing before modelling them.
        self.assertEqual(sorted(self.census["gate_banks"]), ["chest_flags", "event_flags"])
        self.assertEqual(sorted(self.census["flags_per_bank"]), ["chest_flags", "event_flags"])

    def test_undecoded_entries_are_reported_not_dropped(self):
        for entry in self.census["undecoded_entries"]:
            with self.subTest(entry=entry["entry_hex"]):
                self.assertIn("reason", entry)
                self.assertTrue(entry["maps"])
        # Every referenced entry is either decoded or listed.
        listed = {e["entry"] for e in self.census["undecoded_entries"]}
        self.assertEqual(len(listed), 11)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestInThePack(unittest.TestCase):
    """A three-map pack: the two door maps and the one with a layout variant."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls._temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls._temp.name) / "pack"
        cls.manifest = build_pack(
            cls.data, cls.root,
            map_ids=[MAP_ZEMA, MAP_BIRTH_VALLEY_B1, MAP_GARUBERK_PART2],
        )
        cls.maps = {
            entry["id"]: json.loads((cls.root / entry["json"]).read_text())
            for entry in cls.manifest["maps"]
        }

    @classmethod
    def tearDownClass(cls):
        cls._temp.cleanup()

    def test_the_effects_arrive_with_the_map_they_patch(self):
        for payload in self.maps.values():
            with self.subTest(map=payload["symbol"]):
                self.assertIn("map_effects", payload)
                self.assertIn("layout_variants", payload)
        self.assertTrue(self.maps[MAP_ZEMA]["map_effects"])

    def test_the_five_stuck_doors_are_the_ones_the_patches_open(self):
        # These five have been in `doors_without_map_change_cell` since the
        # pack first reported it. Slice 1 is what opens them.
        waiting = self.manifest["warps"]["doors_without_map_change_cell"]
        self.assertEqual(len(waiting), 5)
        for door in waiting:
            payload = self.maps[door["id"]]
            patched = {
                (write["chunk_x"], write["chunk_y"])
                for entry in payload["map_effects"] if entry["decoded"]
                for path in entry["paths"] for write in path["writes"]
                if write["kind"] == "layout_write"
            }
            rect = door["rect"]
            cells = {(x, y)
                     for y in range(rect["y"], rect["y"] + rect["height"])
                     for x in range(rect["x"], rect["x"] + rect["width"])}
            covered = any(
                (cx * 2 + dx, cy * 2 + dy) in cells
                for cx, cy in patched for dx in (0, 1) for dy in (0, 1)
            )
            with self.subTest(door=door["target"]["symbol"]):
                self.assertTrue(covered, "no layout write covers this doorway")

    def test_the_layout_variant_ships_decoded(self):
        # The lead's requirement: a replacement layout is a first-class variant,
        # not a blob pointer, so no consumer needs a decompressor.
        (variant,) = self.maps[MAP_GARUBERK_PART2]["layout_variants"]
        self.assertEqual(len(variant["planes"]), 2)
        # One plane of the pair is the map's own base blob: only the other
        # actually changes.
        self.assertEqual([p["identical_to_base"] for p in variant["planes"]], [True, False])
        self.assertGreater(variant["differs_from_base_cells"], 0)
        self.assertEqual(variant["collision"]["width_cells"], 96)
        self.assertEqual(len(variant["collision"]["rows"]), 96)
        for name in ("png", "png_over"):
            with self.subTest(file=name):
                self.assertTrue((self.root / variant[name]).exists())
        base = self.maps[MAP_GARUBERK_PART2]["collision"]
        self.assertEqual(
            (variant["collision"]["width_cells"], variant["collision"]["height_cells"]),
            (base["width_cells"], base["height_cells"]),
        )

    def test_the_manifest_census(self):
        census = self.manifest["map_effects"]
        self.assertEqual(census["slice"], 1)
        self.assertEqual(census["evaluated"], "map load only")
        self.assertEqual(
            [m["id"] for m in census["maps_with_layout_variants"]], [MAP_GARUBERK_PART2]
        )
        self.assertIn("object_despawn", census["kinds_emitted"])
        # Pack-wide, not per map: a filtered build still censuses everything.
        self.assertEqual(census["referenced_entries"], 127)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    REFERENCE.exists(),
    f"Disassembly clone not present at {REFERENCE}; "
    "git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm",
)
class DisassemblyOracleTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = (REFERENCE / "ps4.asm").read_text(errors="replace")
        cls.constants = (REFERENCE / "ps4.constants.asm").read_text(errors="replace")

    def test_the_dispatcher_can_abort_the_list(self):
        self.assertIn("MapDataManagerJmpTbl:", self.source)
        self.assertIn("\tjsr\t(a1,d0.w)", self.source)
        self.assertIn("\tbra.w\tGoPast_FFFF_Terminator", self.source)

    def test_the_slot_allocator_is_first_free(self):
        self.assertIn("Field_LoadObject:", self.source)
        self.assertIn("Field_Obj_Secondary = ramaddr($FFFFC300)", self.constants)
        self.assertEqual(FIELD_OBJ_SECONDARY, 0xC300)
        self.assertEqual(FIELD_OBJ_STRIDE, 0x40)

    def test_the_clone_names_a_temp_bank_the_cartridge_does_not_have(self):
        # Recorded so a clone update that fixes this fails loudly here.
        self.assertIn("Temp_Event_Flags = ramaddr($FFFFF156)", self.constants)
        self.assertIn("TempEveFlags_Test:", self.source)
        self.assertIn("Chest_Flags = ramaddr($FFFFF140)", self.constants)

    def test_the_zema_door_table_is_where_the_loop_reads_it(self):
        self.assertIn("Zema_LockedDoorsOffs:", self.source)
        body = self.source.split("Zema_LockedDoorsOffs:")[1].split("; ===")[0]
        for pair in ("$15, $0B", "$14, $15", "$0C, $0F", "$11, $0F"):
            with self.subTest(pair=pair):
                self.assertIn(pair, body)


if __name__ == "__main__":
    unittest.main()


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestCrossPlaneWrites(unittest.TestCase):
    """A displacement can leave the plane it was computed for.

    `GetMapLayoutOffset` returns a pointer into one of two adjacent buffers, so
    `-$1000(a1)` off a BG pointer is the same byte offset in the FG buffer.
    Three retail routines use exactly that to stamp a tile on both planes.
    """

    #: Krup's inn, whose vase-and-flowers routine writes to both planes.
    MAP_KRUP_INN_F1 = 0x03F
    KRUP_ENTRY = 32
    #: The Inner Sanctuary, the other cross-plane routine.
    MAP_INNER_SANCTUARY_B1 = 0x16F
    SANCTUARY_ENTRY = 85

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.effects = extract_map_effects(cls.data, extract_maps(cls.data)["maps"])

    def writes(self, map_id, entry_index):
        return [
            write
            for entry in self.effects["per_map"][map_id] if entry["entry"] == entry_index
            for path in entry["paths"] for write in path["writes"]
            if write["kind"] == "layout_write"
        ]

    def test_the_buffers_come_from_the_routine_that_names_them(self):
        bases = map_layout_bases(self.data)
        self.assertEqual(bases, {"fg": 0xA000, "bg": 0xB000})
        self.assertEqual(abs(bases["bg"] - bases["fg"]), MAP_LAYOUT_BYTES)

    def test_no_write_anywhere_resolves_to_a_negative_cell(self):
        # The defect this replaced: ten writes carried cell_y in the -200s,
        # which is not a coordinate and which psiv-data cannot load.
        for map_id, entries in self.effects["per_map"].items():
            for entry in entries:
                for path in entry.get("paths", ()):
                    for write in path["writes"]:
                        if write["kind"] != "layout_write" or write["out_of_bounds"]:
                            continue
                        with self.subTest(map=map_id, entry=entry["entry_hex"]):
                            self.assertGreaterEqual(write["cell_x"], 0)
                            self.assertGreaterEqual(write["cell_y"], 0)
                            self.assertGreaterEqual(write["chunk_x"], 0)
                            self.assertGreaterEqual(write["chunk_y"], 0)

    def test_krups_flowers_land_on_the_other_plane(self):
        # The disassembly's own comments: the vase is "on Plane B" and the two
        # flower tiles "on Plane A" -- the same cell and the row above it.
        writes = self.writes(self.MAP_KRUP_INN_F1, self.KRUP_ENTRY)
        self.assertEqual(len(writes), 3)
        vase, flower_a, flower_b = writes
        self.assertEqual((vase["plane"], vase["cell_x"], vase["cell_y"]), ("bg", 34, 32))
        self.assertFalse(vase["crosses_plane"])
        self.assertEqual(
            (flower_a["plane"], flower_a["cell_x"], flower_a["cell_y"]), ("fg", 34, 32)
        )
        self.assertEqual(
            (flower_b["plane"], flower_b["cell_x"], flower_b["cell_y"]), ("fg", 34, 30)
        )
        for flower in (flower_a, flower_b):
            self.assertTrue(flower["crosses_plane"])
            self.assertEqual(flower["requested_plane"], "bg")
        # -$1000 is the buffer gap; -$1020 is that plus one row of this map.
        self.assertEqual(flower_a["displacement"], -MAP_LAYOUT_BYTES)
        self.assertEqual(flower_b["displacement"], -(MAP_LAYOUT_BYTES + 32))

    def test_the_sanctuary_crosses_once(self):
        writes = self.writes(self.MAP_INNER_SANCTUARY_B1, self.SANCTUARY_ENTRY)
        crossing = [w for w in writes if w["crosses_plane"]]
        self.assertEqual(len(crossing), 1)
        self.assertEqual(
            (crossing[0]["plane"], crossing[0]["cell_x"], crossing[0]["cell_y"]),
            ("fg", 30, 22),
        )

    def test_a_write_outside_both_buffers_is_refused_not_numbered(self):
        # No retail write does this; the branch exists so that one ever
        # appearing is skippable rather than plausible-looking.
        write = Write("layout_write", 0x1234, {
            "plane": "fg", "chunk_x": 0, "chunk_y": 0,
            "displacement": -0x4000, "chunk_id": 1,
        })
        resolved = resolve_layout_write(write, {"fg": 32, "bg": 32}, BASES)
        self.assertTrue(resolved["out_of_bounds"])
        self.assertNotIn("cell_x", resolved)
        self.assertNotIn("cell_y", resolved)
        self.assertEqual(resolved["displacement"], -0x4000)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestImpossiblePaths(unittest.TestCase):
    """A backward branch re-tests a flag; one arm then contradicts the path."""

    MAP_KRUP_INN_F1 = 0x03F
    KRUP_ENTRY = 32

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.effects = extract_map_effects(cls.data, extract_maps(cls.data)["maps"])

    def test_no_path_requires_a_flag_both_ways(self):
        for map_id, entries in self.effects["per_map"].items():
            for entry in entries:
                for index, path in enumerate(entry.get("paths", ())):
                    seen: dict[tuple[str, int], str] = {}
                    for gate in path["gates"]:
                        key = (gate["bank"], gate["flag"])
                        with self.subTest(map=map_id, entry=entry["entry_hex"],
                                          path=index, flag=gate["flag_hex"]):
                            self.assertEqual(
                                seen.setdefault(key, gate["required"]),
                                gate["required"],
                                "a path requires one flag both set and clear",
                            )

    def test_krups_three_paths_are_the_three_real_ones(self):
        entry = next(
            e for e in self.effects["per_map"][self.MAP_KRUP_INN_F1]
            if e["entry"] == self.KRUP_ENTRY
        )
        gates = [
            [(g["flag_hex"], g["required"]) for g in path["gates"]]
            for path in entry["paths"]
        ]
        self.assertEqual(gates, [
            [("0x67", "clear"), ("0x42", "clear")],
            [("0x67", "set"), ("0x42", "clear")],
            [("0x67", "set"), ("0x42", "set")],
        ])
        # Only the both-set path plants the flowers.
        self.assertEqual([len(p["writes"]) for p in entry["paths"]], [4, 4, 7])


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestFlagClears(unittest.TestCase):
    """Map-load flag clears, and the table core-lane transcribed by hand."""

    #: `rust/psiv-core/src/map_load.rs`, `MAP_LOAD_FLAG_CLEARS`.
    CORE_TABLE = {
        0x14: [0x09, 0x0A, 0x0D, 0x0E, 0x0F, 0x10],
        0x17: [0x0B, 0x0C, 0x11, 0x12],
        0x18: [0x13],
        0x3D: [0x15],
        0x3E: [0x17],
        0x47: [0x19],
        0x84: [0x08],
    }

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.census = extract_map_effects(
            cls.data, extract_maps(cls.data)["maps"]
        )["census"]

    def test_the_decoded_clears_match_the_transcribed_table(self):
        decoded = self.census["flag_clears"]["chest_flags"]["by_entry"]
        expected = {f"0x{k:02X}": v for k, v in self.CORE_TABLE.items()}
        self.assertEqual(decoded, expected)
        self.assertEqual(self.census["flag_clears"]["chest_flags"]["entries"], 7)
        self.assertEqual(
            len(self.census["flag_clears"]["chest_flags"]["flag_ids"]), 15
        )

    def test_the_bank_address_is_carried_beside_the_name(self):
        # The `$F140` bank's name is under review; an address does not move.
        self.assertEqual(
            self.census["flag_clears"]["chest_flags"]["bank_address"], "0xFFFFF140"
        )

    def test_one_entry_clears_an_event_flag_not_a_temp_one(self):
        # Entry $1A goes through the *first* clear door, so it is an event-flag
        # clear and correctly absent from a table scoped to $F140.
        event = self.census["flag_clears"]["event_flags"]
        self.assertEqual(event["by_entry"], {"0x1A": [0x14]})
        self.assertEqual(event["bank_address"], "0xFFFFF100")

    def test_a_position_gated_path_is_not_called_unconditional(self):
        # Two Garuberk routines clear a flag and then put it back if the player
        # is past a coordinate. The put-back path is conditional on something
        # this slice does not model, and must not read as "always".
        self.assertGreater(self.census["paths_position_gated"], 0)
