import hashlib
import struct
import unittest
from pathlib import Path

from psiv_tools.core import read_rom
from psiv_tools.layouts import (
    COLLISION_CELL_PIXELS,
    CHUNK_PIXELS_X,
    collision_at,
    collision_type_name,
    is_blocking,
    is_walkable,
)
from psiv_tools.maps import extract_maps
from psiv_tools.overworld import (
    DEZOLIS,
    EVENT_FLAG_SYMBOLS,
    GET_MAP_LAYOUT_CHUNK_BG,
    GET_MAP_LAYOUT_CHUNK_FG,
    HEIGHT_CHUNKS,
    HEIGHT_PIXELS,
    MAP_CHANGE_COLLISION_TYPE,
    MAP_LAYOUT_BG,
    MAP_LAYOUT_FG,
    MOTAVIA,
    OVERWORLD_MAP_IDS,
    PAGE_BYTES,
    PAGE_COUNT,
    PAGE_ROWS,
    PAGE_TABLE_SITES,
    PATCH_TABLE_SITES,
    ROW_BYTES,
    WIDTH_CHUNKS,
    WIDTH_PIXELS,
    WINDOW_PAGES,
    WINDOW_ROWS,
    OverworldError,
    _PatchDecoder,
    apply_patches,
    check_code_sites,
    decode_overworld,
    decode_patches,
    decode_plane,
    is_overworld,
    opening_patches,
    overworld_spec,
    patched_cells,
    read_page_table,
    read_patch_table,
)
from psiv_tools.pack import (
    MAP_CHANGE_COLLISION_TYPE as PACK_MAP_CHANGE,
    warp_rect,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"

MAP_PIATA = 0x10
#: Where Piata's south-edge transition puts the party, after the standing-cell
#: shift `psiv_tools.pack` applies to every stored Y.
PIATA_ENTRANCE_CELL = (46, 143)


class TestGeometry(unittest.TestCase):
    """The numbers, checked against each other rather than restated."""

    def test_a_page_is_eight_rows_of_the_hardcoded_stride(self):
        # `move.w #$FF,d2` copies 256 longs and `lsl.w #7,d1` is the row.
        self.assertEqual(PAGE_BYTES, 0x400)
        self.assertEqual(ROW_BYTES, 128)
        self.assertEqual(PAGE_ROWS, PAGE_BYTES // ROW_BYTES)
        self.assertEqual(PAGE_ROWS, 8)

    def test_sixteen_pages_make_the_grid_the_records_declare(self):
        self.assertEqual(PAGE_COUNT, 16)
        self.assertEqual(HEIGHT_CHUNKS, PAGE_COUNT * PAGE_ROWS)
        self.assertEqual(WIDTH_CHUNKS, ROW_BYTES)
        self.assertEqual((WIDTH_CHUNKS, HEIGHT_CHUNKS), (128, 128))
        self.assertEqual(WIDTH_PIXELS, WIDTH_CHUNKS * CHUNK_PIXELS_X)
        self.assertEqual((WIDTH_PIXELS, HEIGHT_PIXELS), (4096, 4096))

    def test_the_window_is_four_pages_deep(self):
        # `andi.w #$1F` in GetMapLayoutChunk and in GetChunkAndCollision, and
        # `andi.w #$FFF,d2` on the destination offset: 4KB, 32 rows, 4 pages.
        self.assertEqual(WINDOW_ROWS, 32)
        self.assertEqual(WINDOW_PAGES, WINDOW_ROWS // PAGE_ROWS)
        self.assertEqual(WINDOW_ROWS * ROW_BYTES, MAP_LAYOUT_BG - MAP_LAYOUT_FG)

    def test_the_two_map_ids_are_the_ones_the_loader_branches_on(self):
        self.assertEqual(OVERWORLD_MAP_IDS, (MOTAVIA, DEZOLIS))
        # `andi.w #$FFFE,d0 / beq` over the whole 417-entry id space.
        self.assertEqual(
            [map_id for map_id in range(417) if is_overworld(map_id)],
            list(OVERWORLD_MAP_IDS),
        )

    def test_the_map_change_type_is_the_one_the_pack_names(self):
        self.assertEqual(MAP_CHANGE_COLLISION_TYPE, PACK_MAP_CHANGE)
        self.assertEqual(collision_type_name(MAP_CHANGE_COLLISION_TYPE), "map_change")
        self.assertFalse(is_blocking(MAP_CHANGE_COLLISION_TYPE))

    def test_bit_zero_of_the_map_index_picks_the_planet(self):
        for plane, site in {**PAGE_TABLE_SITES, **PATCH_TABLE_SITES}.items():
            with self.subTest(plane=plane, site=site.label):
                self.assertEqual(site.table_for(MOTAVIA), site.motavia)
                self.assertEqual(site.table_for(DEZOLIS), site.dezolis)
                self.assertNotEqual(site.motavia, site.dezolis)

    def test_an_interior_map_is_refused_by_every_entry_point(self):
        for call in (
            lambda: read_page_table(b"", MAP_PIATA, "fg"),
            lambda: read_patch_table(b"", MAP_PIATA, "fg"),
            lambda: decode_patches(b"", MAP_PIATA),
            lambda: decode_overworld(b"", MAP_PIATA),
            lambda: overworld_spec({"id": MAP_PIATA, "symbol": "Piata"}),
        ):
            with self.assertRaises(OverworldError):
                call()


class _Assembler:
    """Just enough 68000 to build a hook routine at a known address.

    A hook reaches `GetMapLayoutChunk` through a 16-bit `bsr.w` and skips a
    block through an 8-bit `beq.s`, so both displacements depend on where the
    routine sits and how long the block turned out; assembling is the only way
    to write these tests without hand-counting bytes.
    """

    EVENT_FLAGS_TEST = 0x057624
    #: Far enough below `GetMapLayoutChunkFG` for a `bsr.w` to reach it.
    ORIGIN = GET_MAP_LAYOUT_CHUNK_FG - 0x1000

    def __init__(self):
        self.out = bytearray()

    def emit(self, data: bytes) -> "_Assembler":
        self.out += data
        return self

    def rom(self) -> bytes:
        data = bytearray(GET_MAP_LAYOUT_CHUNK_FG + 0x100)
        data[self.ORIGIN:self.ORIGIN + len(self.out)] = self.out
        return bytes(data)

    def chunk_pointer(self, column: int, row: int, plane: str) -> "_Assembler":
        target = GET_MAP_LAYOUT_CHUNK_FG if plane == "fg" else GET_MAP_LAYOUT_CHUNK_BG
        self.emit(struct.pack(">HH", 0x303C, column))
        self.emit(struct.pack(">HH", 0x323C, row))
        self.emit(struct.pack(">H", 0x6100))
        # `bsr.w` counts from the displacement word, which is where we are now.
        return self.emit(struct.pack(">h", target - (self.ORIGIN + len(self.out))))

    def gate(self, flag: int, block) -> "_Assembler":
        self.emit(struct.pack(">HH", 0x303C, flag))
        self.emit(struct.pack(">HI", 0x4EB9, self.EVENT_FLAGS_TEST))
        branch = len(self.out)
        self.emit(b"\x67\x00")
        block(self)
        self.out[branch + 1] = len(self.out) - (branch + 2)
        return self

    def rts(self) -> "_Assembler":
        return self.emit(struct.pack(">H", 0x4E75))

    def decode(self):
        return _PatchDecoder(self.rom(), "bg", 5, self.ORIGIN).decode()


class TestPatchDecoder(unittest.TestCase):
    """The instruction reader, on bytes this test assembles itself.

    The retail hook routines are rebuilt here rather than quoted so that every
    form the decoder claims to accept is exercised, including the negative
    displacement that reaches the other plane.
    """

    def test_a_routine_that_is_only_rts_patches_nothing(self):
        self.assertEqual(_Assembler().rts().decode(), [])

    def test_a_long_immediate_writes_four_chunk_ids(self):
        def block(asm):
            asm.chunk_pointer(0x18, 0x35, "bg")
            asm.emit(struct.pack(">HI", 0x22BC, 0x0A0B0C0D))
            asm.emit(struct.pack(">HIH", 0x237C, 0x01020304, 0x0080))

        (patch,) = _Assembler().gate(0xDA, block).rts().decode()
        self.assertEqual(patch.event_flag, 0xDA)
        self.assertEqual(patch.event_symbol, "EventFlag_Reunion")
        first, second = patch.writes
        self.assertEqual(
            (first.plane, first.chunk_x, first.chunk_y, first.chunk_ids),
            ("bg", 0x18, 0x35, (0x0A, 0x0B, 0x0C, 0x0D)),
        )
        # `$80(a1)` is exactly one row further down.
        self.assertEqual((second.chunk_x, second.chunk_y), (0x18, 0x36))
        self.assertEqual(second.chunk_ids, (0x01, 0x02, 0x03, 0x04))

    def test_a_byte_immediate_writes_one(self):
        def block(asm):
            asm.chunk_pointer(6, 0x24, "bg").emit(struct.pack(">HH", 0x12BC, 0x002C))

        (patch,) = _Assembler().gate(0x82, block).rts().decode()
        (write,) = patch.writes
        self.assertEqual((write.chunk_x, write.chunk_y, write.chunk_ids), (6, 0x24, (0x2C,)))

    def test_a_negative_displacement_crosses_to_the_other_plane(self):
        # $F000 is -$1000: the same cell of Map_Layout_FG. $EF80 is one row
        # above it there. Both shapes occur in retail.
        def block(asm):
            asm.chunk_pointer(28, 47, "bg")
            asm.emit(struct.pack(">HHH", 0x137C, 0x0000, 0xF000))
            asm.emit(struct.pack(">HHH", 0x137C, 0x0000, 0xEF80))

        (patch,) = _Assembler().gate(0x65, block).rts().decode()
        same, above = patch.writes
        self.assertEqual((same.plane, same.chunk_x, same.chunk_y), ("fg", 28, 47))
        self.assertEqual((above.plane, above.chunk_x, above.chunk_y), ("fg", 28, 46))

    def test_a_d0_store_needs_its_moveq(self):
        def cleared(asm):
            asm.chunk_pointer(24, 53, "fg")
            asm.emit(struct.pack(">H", 0x7000))
            asm.emit(struct.pack(">HH", 0x2340, 0x0004))

        def bare(asm):
            asm.chunk_pointer(24, 53, "fg").emit(struct.pack(">HH", 0x2340, 0x0004))

        (patch,) = _Assembler().gate(0xDA, cleared).rts().decode()
        self.assertEqual(patch.writes[0].chunk_ids, (0, 0, 0, 0))
        with self.assertRaises(OverworldError):
            _Assembler().gate(0xDA, bare).rts().decode()

    def test_an_unknown_opcode_is_a_failure_and_not_a_skip(self):
        def block(asm):
            asm.chunk_pointer(0, 0, "bg").emit(struct.pack(">H", 0x4E71))  # nop

        with self.assertRaises(OverworldError):
            _Assembler().gate(0xDA, block).rts().decode()

    def test_a_store_before_the_pointer_is_a_failure(self):
        def block(asm):
            asm.emit(struct.pack(">HH", 0x12BC, 0x0001))

        with self.assertRaises(OverworldError):
            _Assembler().gate(0xDA, block).rts().decode()

    def test_a_flag_test_that_is_not_one_is_a_failure(self):
        assembler = _Assembler()
        assembler.emit(struct.pack(">HH", 0x303C, 0xDA)).emit(struct.pack(">H", 0x4E71))
        with self.assertRaises(OverworldError):
            assembler.decode()

    def test_a_store_that_lands_off_the_map_is_a_failure(self):
        def block(asm):
            # $8000 is -$8000: nowhere near either Map_Layout region.
            asm.chunk_pointer(0, 0, "fg").emit(struct.pack(">HHH", 0x137C, 0x0000, 0x8000))

        with self.assertRaises(OverworldError):
            _Assembler().gate(0xDA, block).rts().decode()

    def test_later_writes_win(self):
        def block(asm):
            asm.chunk_pointer(4, 4, "bg").emit(struct.pack(">HH", 0x12BC, 0x0011))
            asm.chunk_pointer(4, 4, "bg").emit(struct.pack(">HH", 0x12BC, 0x0022))

        patches = _Assembler().gate(0xDA, block).rts().decode()
        self.assertEqual(patched_cells(patches, "bg"), {(4, 4): 0x22})


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestOverworldsAgainstTheRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.records = extract_maps(cls.data)["maps"]
        cls.worlds = {
            map_id: decode_overworld(cls.data, map_id, cls.records[map_id])
            for map_id in OVERWORLD_MAP_IDS
        }

    # ------------------------------------------------------------ code sites
    def test_every_hardcoded_address_is_a_retail_lea_operand(self):
        report = check_code_sites(self.data)
        self.assertEqual(report["page_copy"]["compression"], None)
        self.assertEqual(report["page_copy"]["longs_copied"], PAGE_BYTES // 4)
        self.assertEqual(report["get_map_layout_chunk"]["window_rows"], WINDOW_ROWS)
        self.assertEqual(report["get_map_layout_chunk"]["row_bytes"], ROW_BYTES)
        # `GetMapLayoutChunkBG` is the `lea` six bytes into the same signature,
        # so pinning one pins the pair.
        self.assertEqual(GET_MAP_LAYOUT_CHUNK_BG - GET_MAP_LAYOUT_CHUNK_FG, 6)
        self.assertEqual(
            self.data[GET_MAP_LAYOUT_CHUNK_FG:GET_MAP_LAYOUT_CHUNK_FG + 4],
            struct.pack(">HH", 0x43F8, MAP_LAYOUT_FG),
        )
        self.assertEqual(
            self.data[GET_MAP_LAYOUT_CHUNK_BG:GET_MAP_LAYOUT_CHUNK_BG + 4],
            struct.pack(">HH", 0x43F8, MAP_LAYOUT_BG),
        )

    def test_a_rom_that_does_not_match_the_load_path_is_refused(self):
        broken = bytearray(self.data)
        broken[PAGE_TABLE_SITES["fg"].rom_offset + 2] ^= 0xFF
        with self.assertRaises(OverworldError):
            check_code_sites(bytes(broken))
        with self.assertRaises(OverworldError):
            read_page_table(bytes(broken), MOTAVIA, "fg")

    # ----------------------------------------------------------- page tables
    def test_the_page_tables_sit_where_the_load_path_says(self):
        expected = {
            (MOTAVIA, "fg"): (0x107DC2, 0x107E02, 0x10BE02, 16),
            (MOTAVIA, "bg"): (0x10BE02, 0x10BE42, 0x10FE42, 16),
            (DEZOLIS, "fg"): (0x115584, 0x1155C4, 0x1175C4, 8),
            (DEZOLIS, "bg"): (0x1175C4, 0x117604, 0x119604, 8),
        }
        for (map_id, plane), (table, start, end, distinct) in expected.items():
            with self.subTest(map=map_id, plane=plane):
                read = read_page_table(self.data, map_id, plane)
                self.assertEqual(read.rom_offset, table)
                self.assertEqual((read.data_start, read.data_end), (start, end))
                self.assertEqual(len(read.pages), PAGE_COUNT)
                self.assertEqual(len(read.distinct_pages), distinct)

    def test_each_table_is_flush_against_what_follows_it(self):
        # Motavia's FG pages end exactly where its BG table begins, and its BG
        # pages end exactly at the palette pointer its record stores. Same for
        # Dezolis. Nothing here is a guessed extent.
        for map_id in OVERWORLD_MAP_IDS:
            with self.subTest(map=map_id):
                fg = read_page_table(self.data, map_id, "fg")
                bg = read_page_table(self.data, map_id, "bg")
                self.assertEqual(fg.data_end, bg.rom_offset)
                self.assertEqual(
                    bg.data_end, int(self.records[map_id]["palette"]["pointer"], 16)
                )

    def test_dezolis_repeats_its_last_page_for_the_bottom_half(self):
        for plane in ("fg", "bg"):
            with self.subTest(plane=plane):
                table = read_page_table(self.data, DEZOLIS, plane)
                self.assertEqual(
                    [page.aliases for page in table.pages],
                    [None] * 8 + [7] * 8,
                )
                layout = decode_plane(self.data, table)
                self.assertEqual(layout.anomaly["kind"], "aliased_pages")
                self.assertEqual(layout.anomaly["distinct_pages"], 8)
                # Rows 64..127 are page 7's eight rows, over and over.
                page7 = layout.cells[56 * ROW_BYTES:64 * ROW_BYTES]
                for row in range(64, HEIGHT_CHUNKS):
                    start = row * ROW_BYTES
                    self.assertEqual(
                        layout.cells[start:start + ROW_BYTES],
                        page7[(row % PAGE_ROWS) * ROW_BYTES:(row % PAGE_ROWS + 1) * ROW_BYTES],
                    )

    def test_motavia_has_sixteen_distinct_pages_and_no_anomaly(self):
        for plane in ("fg", "bg"):
            with self.subTest(plane=plane):
                table = read_page_table(self.data, MOTAVIA, plane)
                self.assertEqual([page.aliases for page in table.pages], [None] * 16)
                layout = decode_plane(self.data, table)
                self.assertIsNone(layout.anomaly)
                # With no repeats the assembled grid *is* the ROM region, which
                # is what "the pages are contiguous and in order" means.
                self.assertEqual(
                    hashlib.sha256(layout.cells).hexdigest(), layout.blob.sha256
                )

    # ------------------------------------------------------------ the layouts
    def test_the_decoded_layouts_are_what_this_rom_holds(self):
        expected = {
            MOTAVIA: {
                "fg": ("ab6e2f394359e9f1d1546cb23ac2f9663c96d0d88afdbc029da776f4b9f2cf43", 181, 30),
                "bg": ("165bcc517a664d2c5c6fa7eede4fd4335ee7d5e30c8042cfc2c0fdc0c37211f4", 210, 158),
            },
            DEZOLIS: {
                "fg": ("c3cac8bb3da1e03a68e8875b35efc6f2be7196e18c0f9fbe466be45357b43c11", 103, 14),
                "bg": ("8d0407fc8edbb2abdad231c3e168e069a2b63d50eddbc1ebce0834880cdffa9c", 110, 82),
            },
        }
        for map_id, planes in expected.items():
            world = self.worlds[map_id]
            for plane, (digest, highest, distinct) in planes.items():
                with self.subTest(map=world.symbol, plane=plane):
                    layout = getattr(world.layout, plane)
                    self.assertEqual(len(layout.cells), WIDTH_CHUNKS * HEIGHT_CHUNKS)
                    self.assertEqual(hashlib.sha256(layout.cells).hexdigest(), digest)
                    self.assertEqual(max(layout.cells), highest)
                    self.assertEqual(len(layout.distinct_chunks), distinct)
                    self.assertLess(highest, len(world.layout.chunks))

    def test_both_records_declare_the_grid_the_paged_path_builds(self):
        for map_id in OVERWORLD_MAP_IDS:
            with self.subTest(map=map_id):
                spec = overworld_spec(self.records[map_id])
                self.assertEqual(
                    (spec.width_chunks_fg, spec.height_chunks_fg,
                     spec.width_chunks_bg, spec.height_chunks_bg),
                    (WIDTH_CHUNKS, HEIGHT_CHUNKS, WIDTH_CHUNKS, HEIGHT_CHUNKS),
                )
                # `loc_51AB2`'s first byte: both overworlds read plane B.
                self.assertEqual(spec.collision_plane, 1)
                self.assertEqual(spec.collision_plane_name, "bg")
                self.assertEqual(spec.layout_fg, PAGE_TABLE_SITES["fg"].table_for(map_id))
                self.assertEqual(spec.layout_bg, PAGE_TABLE_SITES["bg"].table_for(map_id))

    def test_the_chunk_tables_come_from_the_record_like_any_other_map(self):
        expected = {MOTAVIA: (0x1065A2, 0x107DB7, 256), DEZOLIS: (0x114914, 0x11557C, 128)}
        for map_id, (start, end, count) in expected.items():
            world = self.worlds[map_id]
            with self.subTest(map=world.symbol):
                (blob,) = world.layout.chunks.blobs
                self.assertEqual((blob.rom_offset, blob.rom_end), (start, end))
                self.assertEqual(len(world.layout.chunks), count)
                # The chunk stream stops short of the page table that follows
                # it, which is the boundary proof for both.
                self.assertLess(blob.rom_end, world.tables["fg"].rom_offset)

    # ---------------------------------------------------------- the collision
    def test_the_collision_grids_are_what_this_rom_holds(self):
        expected = {
            MOTAVIA: (
                "52131f9ec19486c105834dcc57dda3cbbf173df8710e81950e285d98c1a7b77e",
                {0x0: 14956, 0x1: 123, 0x8: 6307, 0x9: 43518, 0xA: 632},
            ),
            DEZOLIS: (
                "c527e67dff4e36f170b32d48cf1ad55addac60ca84b28f85b7f92016d7f0ad5e",
                {0x0: 8267, 0x1: 78, 0x8: 55487, 0xB: 1704},
            ),
        }
        for map_id, (digest, histogram) in expected.items():
            world = self.worlds[map_id]
            with self.subTest(map=world.symbol):
                grid = world.layout.collision
                self.assertEqual((grid.width, grid.height), (256, 256))
                self.assertEqual(grid.width * COLLISION_CELL_PIXELS, WIDTH_PIXELS)
                self.assertEqual(hashlib.sha256(grid.types).hexdigest(), digest)
                self.assertEqual(grid.histogram(), histogram)

    def test_sand_and_ice_exist_and_only_on_the_planet_that_has_them(self):
        # `TileCollNormalPtrs` routes types $A and $B to TileColl_Solid, and
        # until the overworlds were decoded neither appeared anywhere in the
        # cartridge. They are the desert and the ice sheet, one planet each.
        motavia = self.worlds[MOTAVIA].layout.collision.histogram()
        dezolis = self.worlds[DEZOLIS].layout.collision.histogram()
        self.assertIn(0xA, motavia)
        self.assertNotIn(0xB, motavia)
        self.assertIn(0xB, dezolis)
        self.assertNotIn(0xA, dezolis)
        for value in (0xA, 0xB):
            self.assertTrue(is_blocking(value))

    def test_the_cell_piata_puts_you_on(self):
        grid = self.worlds[MOTAVIA].layout.collision
        self.assertEqual(grid.type_at(*PIATA_ENTRANCE_CELL), MAP_CHANGE_COLLISION_TYPE)
        self.assertTrue(grid.walkable_at(*PIATA_ENTRANCE_CELL))
        # The northwest corner is open ocean, which blocks.
        self.assertEqual(grid.name_at(0, 0), "water")
        self.assertFalse(grid.walkable_at(0, 0))

    def test_dezolis_bottom_half_is_its_last_page_over_and_over(self):
        grid = self.worlds[DEZOLIS].layout.collision
        # Cell rows 112..127 are chunk rows 56..63, i.e. page 7; every cell row
        # from 128 down is that block again.
        page7 = grid.types[112 * grid.width:128 * grid.width]
        self.assertEqual(grid.types[128 * grid.width:], page7 * 8)
        walkable = sum(
            1
            for y in range(128, grid.height)
            for x in range(grid.width)
            if is_walkable(grid.type_at(x, y))
        )
        # A rim, not a place: 128 of 32,768 cells, and the map's 14 doorways
        # are all in the top third.
        self.assertEqual(walkable, 128)
        for entry in self.records[DEZOLIS]["transitions_2"]["entries"]:
            self.assertLess(entry["source"]["y_tile"], 128)

    # -------------------------------------------------------------- transitions
    def test_the_forty_three_overworld_transitions_are_all_doorways(self):
        total = 0
        for map_id in OVERWORLD_MAP_IDS:
            record = self.records[map_id]
            self.assertEqual(record["transitions"]["count"], 0)
            total += record["transitions_2"]["count"]
        self.assertEqual(total, 43)

    def test_every_transition_rect_lands_on_the_map(self):
        for map_id in OVERWORLD_MAP_IDS:
            world = self.worlds[map_id]
            grid = world.layout.collision
            for entry in self.records[map_id]["transitions_2"]["entries"]:
                source = entry["source"]
                with self.subTest(map=world.symbol, target=entry["target"]["symbol"]):
                    rect = warp_rect(
                        entry["range"]["id"], source["x_tile"], source["y_tile"],
                        grid.width, grid.height,
                    )
                    self.assertIsNotNone(rect)
                    self.assertLessEqual(rect.x + rect.width, grid.width)
                    self.assertLessEqual(rect.y + rect.height, grid.height)

    def test_the_inverse_of_piatas_exit_exists_and_is_coherent(self):
        # Piata's own south-edge transition sends the party to Motavia at
        # PIATA_ENTRANCE_CELL; the return trip has to be a transition whose
        # trigger covers that same cell, or the two towns are not connected.
        piata = self.records[MAP_PIATA]
        exits = [
            entry
            for section in ("transitions", "transitions_2")
            for entry in piata[section]["entries"]
            if entry["target"]["id"] == MOTAVIA
        ]
        self.assertEqual(len(exits), 1)
        destination = exits[0]["destination"]
        self.assertEqual(
            (destination["x_tile"], destination["y_tile"] + 1), PIATA_ENTRANCE_CELL
        )

        grid = self.worlds[MOTAVIA].layout.collision
        entrances = [
            entry
            for entry in self.records[MOTAVIA]["transitions_2"]["entries"]
            if entry["target"]["id"] == MAP_PIATA
        ]
        self.assertEqual(len(entrances), 1)
        source = entrances[0]["source"]
        rect = warp_rect(
            entrances[0]["range"]["id"], source["x_tile"], source["y_tile"],
            grid.width, grid.height,
        )
        self.assertEqual(rect.to_json(), {"x": 44, "y": 140, "width": 4, "height": 4})
        self.assertIn(PIATA_ENTRANCE_CELL, list(rect.cells()))
        # Every cell of it is a map-change cell, which is the only way
        # MapTransTile_MapChange is ever reached.
        self.assertTrue(all(
            grid.type_at(x, y) == MAP_CHANGE_COLLISION_TYPE for x, y in rect.cells()
        ))

    # ------------------------------------------------------------- event patches
    def test_the_patches_are_the_ones_the_hook_tables_reach(self):
        counts = {MOTAVIA: 9, DEZOLIS: 3}
        for map_id, expected in counts.items():
            world = self.worlds[map_id]
            with self.subTest(map=world.symbol):
                self.assertEqual(len(world.patches), expected)
                for patch in world.patches:
                    self.assertIn(patch.event_flag, EVENT_FLAG_SYMBOLS)
                    self.assertIn(
                        patch.routine, read_patch_table(self.data, map_id, patch.plane)
                    )
                    # A hook runs right after its page is copied, so every cell
                    # it writes has to be inside that page's eight rows.
                    for write in patch.writes:
                        self.assertEqual(write.chunk_y // PAGE_ROWS, patch.page)

    def test_the_patch_free_pages_really_are_patch_free(self):
        for map_id in OVERWORLD_MAP_IDS:
            for plane in ("fg", "bg"):
                routines = read_patch_table(self.data, map_id, plane)
                patched = {p.page for p in self.worlds[map_id].patches if p.plane == plane}
                bare = {page for page, r in enumerate(routines) if page not in patched}
                for page in bare:
                    with self.subTest(map=map_id, plane=plane, page=page):
                        # `rts`, and the table points several pages at the same
                        # one, which is why an entry is an address and not an
                        # index into a per-page list.
                        self.assertEqual(
                            self.data[routines[page]:routines[page] + 2],
                            struct.pack(">H", 0x4E75),
                        )

    def test_each_shut_door_is_opened_by_exactly_one_event(self):
        # The six overworld doorways whose trigger covers no map-change cell in
        # the stored layout, and the flag that writes one in. Proven by
        # applying the patch to the collision plane, not by reading a comment.
        expected = {
            (MOTAVIA, "MotaSpaceport"): "EventFlag_MotaSpaceport",
            (MOTAVIA, "MachineCenter"): "EventFlag_MachineCenter",
            (MOTAVIA, "TheEdge"): "EventFlag_Reunion",
            (DEZOLIS, "DezoSpaceport"): "EventFlag_DezoSpaceport",
        }
        seen = set()
        for map_id in OVERWORLD_MAP_IDS:
            world = self.worlds[map_id]
            grid = world.layout.collision
            for entry in self.records[map_id]["transitions_2"]["entries"]:
                source = entry["source"]
                rect = warp_rect(
                    entry["range"]["id"], source["x_tile"], source["y_tile"],
                    grid.width, grid.height,
                )
                cells = list(rect.cells())
                if any(grid.type_at(x, y) == MAP_CHANGE_COLLISION_TYPE for x, y in cells):
                    continue
                symbol = entry["target"]["symbol"]
                with self.subTest(map=world.symbol, target=symbol):
                    patches = opening_patches(world, cells)
                    self.assertEqual(len(patches), 1)
                    self.assertEqual(
                        patches[0].event_symbol, expected[(map_id, symbol)]
                    )
                seen.add((map_id, symbol))
        self.assertEqual(seen, set(expected))

    def test_applying_every_patch_leaves_no_door_shut(self):
        for map_id in OVERWORLD_MAP_IDS:
            world = self.worlds[map_id]
            plane = apply_patches(world.layout.collision_layout, world.patches)
            chunks = world.layout.chunks
            for entry in self.records[map_id]["transitions_2"]["entries"]:
                source = entry["source"]
                rect = warp_rect(
                    entry["range"]["id"], source["x_tile"], source["y_tile"],
                    world.layout.collision.width, world.layout.collision.height,
                )
                with self.subTest(map=world.symbol, target=entry["target"]["symbol"]):
                    # Not every door is open at once in the cartridge -- these
                    # flags contradict each other in story order -- so this is
                    # the weaker claim that every door exists in some state.
                    self.assertTrue(any(
                        collision_at(chunks, plane, x, y) == MAP_CHANGE_COLLISION_TYPE
                        or world.layout.collision.type_at(x, y) == MAP_CHANGE_COLLISION_TYPE
                        for x, y in rect.cells()
                    ))

    def test_no_retail_patch_writes_a_cell_twice(self):
        # `patched_cells` has to resolve a conflict somehow and takes the last
        # writer; the cartridge never makes it choose, which is worth knowing
        # before a runtime starts applying these in some other order.
        for map_id in OVERWORLD_MAP_IDS:
            world = self.worlds[map_id]
            for plane in ("fg", "bg"):
                with self.subTest(map=world.symbol, plane=plane):
                    written = [
                        (write.chunk_x + step, write.chunk_y)
                        for patch in world.patches
                        for write in patch.writes
                        if write.plane == plane
                        for step in range(len(write.chunk_ids))
                    ]
                    cells = patched_cells(world.patches, plane)
                    self.assertEqual(len(cells), len(written))
                    self.assertEqual(set(cells), set(written))

    # ------------------------------------------------------------------- shape
    def test_the_json_says_the_format_out_loud(self):
        payload = self.worlds[DEZOLIS].to_json()
        self.assertEqual(payload["compression"], None)
        self.assertEqual(payload["page_bytes"], PAGE_BYTES)
        self.assertEqual(payload["page_count"], PAGE_COUNT)
        self.assertEqual(payload["window_rows"], WINDOW_ROWS)
        self.assertEqual(payload["wraps"], {"x_pixels": 4096, "y_pixels": 4096})
        self.assertEqual(payload["patch_count"], 3)
        self.assertEqual(
            payload["patched_events"],
            ["EventFlag_DarkForce2", "EventFlag_DezoSpaceport"],
        )
        self.assertEqual(len(payload["anomalies"]), 2)
        self.assertEqual(payload["tables"]["bg"]["distinct_pages"], 8)

    def test_a_record_handed_to_the_wrong_map_is_refused(self):
        with self.assertRaises(OverworldError):
            decode_overworld(self.data, MOTAVIA, self.records[DEZOLIS])
        with self.assertRaises(OverworldError):
            overworld_spec(self.records[MAP_PIATA])


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    REFERENCE.exists(),
    f"Disassembly clone not present at {REFERENCE}; "
    "git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm",
)
class DisassemblyOracleTest(unittest.TestCase):
    """What the clone's labels say, after the retail bytes have already said it.

    The clone is a Grand Cross build, so these only confirm that the routines
    this module was transcribed from are the ones it names.
    """

    @classmethod
    def setUpClass(cls):
        cls.source = (REFERENCE / "ps4.asm").read_text(errors="replace")
        cls.constants = (REFERENCE / "ps4.constants.asm").read_text(errors="replace")

    def test_the_branch_that_skips_the_layout_pointers(self):
        self.assertIn("loc_539E2:", self.source)
        self.assertIn("\tbeq.w\tloc_53A26", self.source)
        self.assertIn("\tbeq.w\tloc_53A8E", self.source)

    def test_field_map_index_is_the_word_the_planet_test_reads_a_byte_of(self):
        # This is the whole proof that `btst #0,($FFFFEC29).w` means "map id is
        # odd", and therefore that Dezolis takes the second table.
        self.assertIn("Field_Map_Index = ramaddr($FFFFEC28)", self.constants)
        self.assertIn("btst\t#0, ($FFFFEC29).w", self.source)

    def test_the_page_tables_are_named_by_the_labels_this_module_uses(self):
        for label in ("loc_107DC2", "loc_10BE02", "loc_115584", "loc_1175C4"):
            with self.subTest(label=label):
                self.assertIn(f"lea\t({label}).l, a2", self.source)

    def test_get_map_layout_chunk_hardcodes_the_overworld_stride(self):
        self.assertIn("GetMapLayoutChunkFG:", self.source)
        self.assertIn("GetMapLayoutChunkBG:", self.source)
        self.assertIn("\tandi.w\t#$1F, d1\n\tlsl.w\t#7, d1", self.source)

    def test_the_event_flags_the_patches_test(self):
        for value, symbol in EVENT_FLAG_SYMBOLS.items():
            with self.subTest(symbol=symbol):
                literal = f"${value:X}" if value > 9 else str(value)
                self.assertIn(f"{symbol} = {literal}", self.source + self.constants)

    def test_the_overworld_map_data_manager_entries_touch_no_layout(self):
        # Both records list entry $14; Dezolis also lists $26. Neither writes
        # Map_Layout: $14 clears temporary flags and $26 rewrites Chunk_Table.
        self.assertIn("bra.w\tMapDataMan_MovingPlatforms\t; $14", self.source)
        self.assertIn("bra.w\tMapDataMan_Dezolis\t; $26", self.source)
        body = self.source.split("MapDataMan_Dezolis:")[1].split("; ---")[0]
        self.assertIn("Chunk_Table", body)
        self.assertNotIn("Map_Layout", body)


if __name__ == "__main__":
    unittest.main()
