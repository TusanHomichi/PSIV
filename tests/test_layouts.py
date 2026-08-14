import hashlib
import struct
import unittest
from pathlib import Path

from psiv_tools import kosinski
from psiv_tools.core import read_rom
from psiv_tools.layouts import (
    BLOCKING_COLLISION_TYPES,
    CHUNK_BYTES,
    CHUNK_PIXELS_X,
    CHUNK_PIXELS_Y,
    CHUNK_TILES_X,
    CHUNK_TILES_Y,
    CHUNK_WORDS,
    COLLISION_BIT,
    COLLISION_CELL_PIXELS,
    COLLISION_TYPE_NAMES,
    LAYOUT_RAM_BYTES,
    MAP_PALETTE_BYTES,
    MAX_CHUNKS,
    PLANE_BG,
    PLANE_FG,
    VDP_WORD_MASK,
    ChunkTable,
    Layout,
    LayoutError,
    MapLayoutSpec,
    chunk_palette,
    collision_at,
    collision_type_name,
    compose_layout,
    decode_chunks,
    decode_collision,
    decode_layout,
    decode_map_layout,
    decode_map_palette,
    decode_tilesets,
    dimension_from_header,
    h_flip,
    is_blocking,
    is_walkable,
    palette_line,
    priority,
    collision_flag,
    render_collision,
    render_layout,
    tile_index,
    v_flip,
    vdp_word,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"


# ---------------------------------------------------------------------------
# The three hand-walked maps.
#
# Every offset here was walked out of the retail image by following
# GameMode_LoadFieldMap through the map record, starting from the field-map
# pointer table at 0x100000. They are pinned rather than rediscovered so that
# this module stays a decoder of the layout section and does not grow a second
# copy of the map-record grammar.
# ---------------------------------------------------------------------------
FIELD_MAP_PTRS = 0x100000

MAP_PIATA = {
    "map_id": 0x10,
    "record": 0x11C690,
    "spec": MapLayoutSpec.from_header(
        chunk_blobs=(0x122A90, 0x123380),
        layout_fg=0x123710,
        layout_bg=0x1237B0,
        dimension_bytes=(0x1F, 0x1F, 0x1F, 0x1F),
        collision_plane=1,
        tilesets=((0x010, 0x11C808), (0x111, 0x11DE68), (0x213, 0x11F4A8)),
        palette=0x123990,
        label="piata",
    ),
}

MAP_PIATA_ITEM_SHOP = {
    "map_id": 0x1B,
    "record": 0x130084,
    "spec": MapLayoutSpec.from_header(
        chunk_blobs=(0x12E1C0,),
        layout_fg=0x1300F4,
        layout_bg=0x130134,
        dimension_bytes=(0x1F, 0x1F, 0x1F, 0x1F),
        collision_plane=1,
        tilesets=((0x010, 0x129864), (0x110, 0x12AAF4), (0x210, 0x12BE54), (0x310, 0x12D344)),
        palette=0x12F510,
        label="piata_item_shop",
    ),
}

MAP_ISLAND_CAVE = {
    "map_id": 0x92,
    "record": 0x164ED6,
    "spec": MapLayoutSpec.from_header(
        chunk_blobs=(0x166DF0,),
        layout_fg=0x168310,
        layout_bg=0x168630,
        dimension_bytes=(0x2F, 0x2F, 0x2F, 0x2F),
        collision_plane=0,
        tilesets=((0x010, 0x164F64), (0x110, 0x166254)),
        palette=0x168770,
        label="island_cave",
    ),
}

HAND_WALKED = (MAP_PIATA, MAP_PIATA_ITEM_SHOP, MAP_ISLAND_CAVE)

# Map_Piata's second transition table (`Map_Transition_Data_2_Addr`), the one
# `MapTransTile_MapChange` consults. Ten-byte records, `$FFFF`-terminated.
PIATA_TRANSITIONS_2 = 0x11C6F6
PIATA_TRANSITION_SIZE = 10

# The first sprite VRAM slot in Map_Piata's `loc_51A1A` list. The three tileset
# blobs tile into VRAM back to back and stop exactly here.
PIATA_FIRST_SPRITE_TILE = 0x2D0

# `loc_65D12` walks the shop-location table; PiataItemShop's entry.
PIATA_ITEM_SHOP_COUNTER = (0x210 // COLLISION_CELL_PIXELS, 0x1D0 // COLLISION_CELL_PIXELS)

# `ChunkTilesToVRAM` and `ChunkTilesToBuffer`, the two places the collision bit
# is masked off: `andi.w #$BFFF,d3`.
COLLISION_MASK_OPCODES = (0x054492, 0x0544B2)

# `loc_45B0A`, the four `btst #6` reads that assemble the collision nibble, and
# `loc_45AB4`, the layout stride and base selection.
COLLISION_ASSEMBLY_OPCODE = 0x045B0A
COLLISION_ASSEMBLY_BYTES = bytes.fromhex(
    "7800"                      # moveq #0,d4
    "0829000600006702" "5244"   # btst #6,$0(a1)  / beq / addq.w #1,d4
    "0829000600026702" "5444"   # btst #6,$2(a1)  / beq / addq.w #2,d4
    "0829000600086702" "5844"   # btst #6,$8(a1)  / beq / addq.w #4,d4
    "08290006000a6702" "5044"   # btst #6,$A(a1)  / beq / addq.w #8,d4
    "4e75"                      # rts
)


# ---------------------------------------------------------------------------
# Fixtures that need no ROM
# ---------------------------------------------------------------------------
def kos_literals(payload: bytes) -> bytes:
    """Build a Kosinski stream that is nothing but literal bytes.

    Description fields carry one bit per element, consumed LSB first, and the
    routine reloads a field the instant its sixteenth bit is consumed -- before
    that element's operand bytes are read. An encoder that emits the next field
    lazily, after the operands, produces a stream that decodes to garbage, so
    this one places the new field word at exactly the point the reload happens.

    Elements are a literal (one set bit, one operand byte) and the end marker
    (a clear bit then a set bit, then a displacement word with a zero count and
    a zero extra byte).
    """
    elements: list[tuple[tuple[int, ...], bytes]] = [((1,), bytes([b])) for b in payload]
    elements.append(((0, 1), b"\x00\x00\x00"))

    stream = bytearray()
    fields: list[tuple[int, list[int]]] = []

    def open_field() -> None:
        fields.append((len(stream), []))
        stream.extend(b"\x00\x00")

    open_field()
    for bits, operands in elements:
        for bit in bits:
            _, current = fields[-1]
            current.append(bit)
            if len(current) == 16:
                open_field()  # the reload, before this element's operands
        stream += operands

    for position, bits in fields:
        value = sum(bit << index for index, bit in enumerate(bits))
        struct.pack_into("<H", stream, position, value)
    return bytes(stream)


def chunk_bytes(words) -> bytes:
    return struct.pack(f">{CHUNK_WORDS}H", *words)


def flat_chunk(word: int) -> bytes:
    return chunk_bytes([word] * CHUNK_WORDS)


def collision_chunk(nibbles) -> bytes:
    """Build one chunk whose four cells carry the given collision types.

    `nibbles` is the four cells in reading order. Within a cell the type's bits
    1, 2, 4 and 8 live on its top-left, top-right, bottom-left and bottom-right
    tile.
    """
    words = [0] * CHUNK_WORDS
    for cell, value in enumerate(nibbles):
        cell_x, cell_y = cell % 2, cell // 2
        base = cell_y * 2 * CHUNK_TILES_X + cell_x * 2
        for bit, step in enumerate((0, 1, CHUNK_TILES_X, CHUNK_TILES_X + 1)):
            if value >> bit & 1:
                words[base + step] |= 1 << COLLISION_BIT
    return chunk_bytes(words)


class KosLiteralHelperTest(unittest.TestCase):
    """The fixture builder has to be right before it can prove anything."""

    def test_round_trips_through_the_real_decoder(self):
        for length in (0, 1, 14, 16, 17, 32, 46, 100):
            with self.subTest(length=length):
                payload = bytes((i * 7 + 3) & 0xFF for i in range(length))
                stream = kos_literals(payload)
                decoded, consumed = kosinski.decompress(stream)
                self.assertEqual(decoded, payload)
                self.assertEqual(consumed, len(stream))

    def test_handles_the_lengths_that_straddle_a_field_boundary(self):
        # 15 and 16 literals put the end marker's two bits either side of a
        # reload; those are exactly the cases a naive encoder gets wrong.
        for length in (13, 14, 15, 16, 31):
            with self.subTest(length=length):
                payload = bytes(range(length))
                self.assertEqual(kosinski.decompress(kos_literals(payload))[0], payload)


# ---------------------------------------------------------------------------
# Pattern-name words
# ---------------------------------------------------------------------------
class PatternWordTest(unittest.TestCase):
    def test_field_extraction(self):
        word = 0xFFFF  # priority, collision, palette, both flips, tile $7FF
        self.assertEqual(tile_index(word), 0x7FF)
        self.assertTrue(h_flip(word))
        self.assertTrue(v_flip(word))
        self.assertEqual(palette_line(word), 1)
        self.assertTrue(priority(word))
        self.assertEqual(collision_flag(word), 1)

    def test_collision_bit_is_the_high_palette_bit(self):
        # Bit 14 would be palette bit 1 on real hardware. PSIV takes it, and
        # ChunkTilesToBuffer masks it off, so chunk tiles reach CRAM lines 0
        # and 1 only.
        self.assertEqual(COLLISION_BIT, 14)
        self.assertEqual(VDP_WORD_MASK, 0xFFFF & ~(1 << COLLISION_BIT))
        flagged = 1 << COLLISION_BIT | 0x0123
        self.assertEqual(collision_flag(flagged), 1)
        self.assertEqual(vdp_word(flagged), 0x0123)
        self.assertEqual(palette_line(flagged), 0)
        self.assertEqual(palette_line(flagged | 1 << 13), 1)

    def test_geometry_constants_agree_with_each_other(self):
        self.assertEqual(CHUNK_WORDS, CHUNK_TILES_X * CHUNK_TILES_Y)
        self.assertEqual(CHUNK_BYTES, 32)
        self.assertEqual(CHUNK_PIXELS_X, 32)
        self.assertEqual(CHUNK_PIXELS_Y, 32)
        self.assertEqual(COLLISION_CELL_PIXELS, 16)

    def test_dimension_from_header(self):
        # The record stores one less than the count; every reader adds one.
        self.assertEqual(dimension_from_header(0x1F), 32)
        self.assertEqual(dimension_from_header(0x2F), 48)
        self.assertEqual(dimension_from_header(0x7F), 128)
        with self.assertRaises(LayoutError):
            dimension_from_header(0x100)


class CollisionTypeTest(unittest.TestCase):
    def test_blocking_set_matches_TileCollNormalPtrs(self):
        # Types 8, 9, $A and $B route to TileColl_Solid and $C to
        # TileColl_Shop; both are `moveq #1,d2`. Everything else is
        # TileColl_Empty, `moveq #0,d2`.
        self.assertEqual(BLOCKING_COLLISION_TYPES, frozenset({0x8, 0x9, 0xA, 0xB, 0xC}))
        for value in range(16):
            self.assertEqual(is_blocking(value), value in {0x8, 0x9, 0xA, 0xB, 0xC})
            self.assertEqual(is_walkable(value), not is_blocking(value))

    def test_map_change_is_walkable(self):
        # MapTransTile_MapChange runs the transition and returns d2 = 0 when no
        # transition matches, so the walker steps onto the cell either way.
        self.assertFalse(is_blocking(0x1))
        self.assertEqual(collision_type_name(0x1), "map_change")

    def test_unnamed_types_are_not_invented(self):
        self.assertEqual(collision_type_name(0x7), "unnamed_7")
        self.assertNotIn(0x7, COLLISION_TYPE_NAMES)
        with self.assertRaises(LayoutError):
            collision_type_name(0x10)


# ---------------------------------------------------------------------------
# Decoders, against hand-built streams
# ---------------------------------------------------------------------------
class DecodeChunksTest(unittest.TestCase):
    def test_blobs_concatenate_into_one_array(self):
        # Map_LoadChunks loads a1 once and lets each KosDecomp leave it past
        # what it wrote, so two blobs are one chunk array, not two.
        first = kos_literals(flat_chunk(0x0001) + flat_chunk(0x0002))
        second = kos_literals(flat_chunk(0x0003))
        rom = bytes(4) + first + second
        table = decode_chunks(rom, (4, 4 + len(first)))
        self.assertEqual(len(table), 3)
        self.assertEqual(table[2], (0x0003,) * CHUNK_WORDS)
        self.assertEqual([b.rom_offset for b in table.blobs], [4, 4 + len(first)])
        self.assertEqual(table.blobs[0].decompressed_length, 2 * CHUNK_BYTES)

    def test_partial_definition_is_rejected(self):
        rom = kos_literals(bytes(CHUNK_BYTES + 2))
        with self.assertRaises(LayoutError) as caught:
            decode_chunks(rom, (0,))
        self.assertIn("whole number", str(caught.exception))

    def test_more_chunks_than_a_layout_byte_can_name_is_rejected(self):
        rom = kos_literals(bytes(CHUNK_BYTES * (MAX_CHUNKS + 1)))
        with self.assertRaises(LayoutError) as caught:
            decode_chunks(rom, (0,))
        self.assertIn(str(MAX_CHUNKS), str(caught.exception))

    def test_no_blobs_is_rejected(self):
        with self.assertRaises(LayoutError):
            decode_chunks(b"", ())

    def test_unknown_chunk_id_is_rejected(self):
        table = decode_chunks(kos_literals(flat_chunk(0)), (0,))
        with self.assertRaises(LayoutError):
            table[1]


class DecodeLayoutTest(unittest.TestCase):
    def test_row_major_with_a_width_stride(self):
        cells = bytes(range(48))
        rom = kos_literals(cells)
        layout = decode_layout(rom, 0, 8, 6, PLANE_BG)
        self.assertEqual(layout.plane, PLANE_BG)
        self.assertEqual(layout.chunk_at(0, 0), 0)
        self.assertEqual(layout.chunk_at(7, 0), 7)
        # SetupChunksBG: row * (row_size + 1) + column
        self.assertEqual(layout.chunk_at(3, 5), 5 * 8 + 3)
        self.assertEqual(layout.width_pixels, 8 * CHUNK_PIXELS_X)
        self.assertEqual(layout.height_pixels, 6 * CHUNK_PIXELS_Y)

    def test_size_mismatch_is_fatal(self):
        rom = kos_literals(bytes(48))
        with self.assertRaises(LayoutError) as caught:
            decode_layout(rom, 0, 8, 5)
        self.assertIn("40", str(caught.exception))

    def test_layout_larger_than_its_ram_region_is_rejected(self):
        with self.assertRaises(LayoutError) as caught:
            decode_layout(b"", 0, 128, 33)
        self.assertIn(str(LAYOUT_RAM_BYTES), str(caught.exception))

    def test_out_of_range_cell_is_rejected(self):
        layout = decode_layout(kos_literals(bytes(48)), 0, 8, 6)
        with self.assertRaises(LayoutError):
            layout.chunk_at(8, 0)
        with self.assertRaises(LayoutError):
            layout.chunk_at(0, 6)

    def test_bad_plane_name_is_rejected(self):
        with self.assertRaises(LayoutError):
            Layout(plane="middle", width_chunks=1, height_chunks=1,
                   cells=b"\x00", blob=None)  # type: ignore[arg-type]


class DecodeCollisionTest(unittest.TestCase):
    """The nibble arithmetic of GetChunkAndCollision, on synthetic chunks."""

    def _table(self, *chunks) -> ChunkTable:
        rom = kos_literals(b"".join(chunks))
        return decode_chunks(rom, (0,))

    def test_each_cell_reads_its_own_quadrant(self):
        table = self._table(collision_chunk((0x0, 0x1, 0x8, 0xC)))
        layout = decode_layout(kos_literals(b"\x00"), 0, 1, 1)
        grid = decode_collision(table, layout)
        self.assertEqual((grid.width, grid.height), (2, 2))
        self.assertEqual(grid.type_at(0, 0), 0x0)
        self.assertEqual(grid.type_at(1, 0), 0x1)
        self.assertEqual(grid.type_at(0, 1), 0x8)
        self.assertEqual(grid.type_at(1, 1), 0xC)

    def test_bit_order_is_top_left_top_right_bottom_left_bottom_right(self):
        # loc_45B0A adds 1, 2, 4 and 8 for the words at +0, +2, +8 and +$A,
        # which are the cell's four tiles in reading order.
        for bit, word_offset in enumerate((0, 1, CHUNK_TILES_X, CHUNK_TILES_X + 1)):
            words = [0] * CHUNK_WORDS
            words[word_offset] = 1 << COLLISION_BIT
            table = self._table(chunk_bytes(words))
            layout = decode_layout(kos_literals(b"\x00"), 0, 1, 1)
            self.assertEqual(decode_collision(table, layout).type_at(0, 0), 1 << bit)

    def test_only_bit_14_counts(self):
        # Every other bit of the word is tile index, flips, palette or priority.
        table = self._table(flat_chunk(0xBFFF))
        layout = decode_layout(kos_literals(b"\x00"), 0, 1, 1)
        self.assertEqual(decode_collision(table, layout).type_at(0, 0), 0)

    def test_grid_is_two_cells_per_chunk_in_each_axis(self):
        table = self._table(flat_chunk(0), collision_chunk((0x8, 0x8, 0x8, 0x8)))
        layout = decode_layout(kos_literals(bytes([0, 1, 1, 0])), 0, 2, 2)
        grid = decode_collision(table, layout)
        self.assertEqual((grid.width, grid.height), (4, 4))
        self.assertTrue(grid.walkable_at(0, 0))
        self.assertFalse(grid.walkable_at(2, 0))
        self.assertFalse(grid.walkable_at(0, 2))
        self.assertTrue(grid.walkable_at(2, 2))
        self.assertEqual(grid.histogram(), {0: 8, 8: 8})

    def test_out_of_range_cell_is_rejected(self):
        table = self._table(flat_chunk(0))
        grid = decode_collision(table, decode_layout(kos_literals(b"\x00"), 0, 1, 1))
        with self.assertRaises(LayoutError):
            grid.type_at(2, 0)

    def test_collision_at_matches_the_grid(self):
        table = self._table(collision_chunk((0x2, 0x9, 0xB, 0x0)))
        layout = decode_layout(kos_literals(b"\x00"), 0, 1, 1)
        grid = decode_collision(table, layout)
        for cell_y in range(2):
            for cell_x in range(2):
                self.assertEqual(
                    collision_at(table, layout, cell_x, cell_y),
                    grid.type_at(cell_x, cell_y),
                )


class RenderTest(unittest.TestCase):
    def _one_tile_map(self):
        # A single chunk whose top-left tile is pattern 1 and the rest pattern
        # 0, and a tileset holding two patterns at VRAM tile 0.
        words = [0] * CHUNK_WORDS
        words[0] = 1
        chunks = decode_chunks(kos_literals(chunk_bytes(words)), (0,))
        layout = decode_layout(kos_literals(b"\x00"), 0, 1, 1)
        art = bytes(32) + b"\x12" * 32  # pattern 0 blank, pattern 1 all 1s/2s
        patterns = decode_tilesets(kos_literals(art), ((0, 0),))
        return chunks, layout, patterns

    def test_compose_places_tiles_and_reports_nothing_missing(self):
        chunks, layout, patterns = self._one_tile_map()
        width, height, pixels, missing = compose_layout(chunks, layout, patterns)
        self.assertEqual((width, height), (CHUNK_PIXELS_X, CHUNK_PIXELS_Y))
        self.assertEqual(missing, set())
        self.assertEqual(pixels[0], 1)
        self.assertEqual(pixels[1], 2)
        self.assertEqual(pixels[8], 0)  # second tile of the row is pattern 0

    def test_unloaded_patterns_are_reported_not_guessed(self):
        words = [0] * CHUNK_WORDS
        words[0] = 5
        chunks = decode_chunks(kos_literals(chunk_bytes(words)), (0,))
        layout = decode_layout(kos_literals(b"\x00"), 0, 1, 1)
        patterns = decode_tilesets(kos_literals(bytes(32)), ((0, 0),))
        _, _, pixels, missing = compose_layout(chunks, layout, patterns)
        self.assertEqual(missing, {5})
        self.assertEqual(set(pixels), {0})

    def test_overlay_treats_colour_zero_as_transparent(self):
        chunks, layout, patterns = self._one_tile_map()
        _, _, base, _ = compose_layout(chunks, layout, patterns)
        filled = bytearray(b"\x0F" * len(base))
        _, _, over, _ = compose_layout(chunks, layout, patterns, filled)
        self.assertEqual(over[0], 1)   # opaque pixel wins
        self.assertEqual(over[8], 0x0F)  # colour 0 leaves the base alone
        self.assertEqual(len(over), len(base))

    def test_overlay_size_mismatch_is_rejected(self):
        chunks, layout, patterns = self._one_tile_map()
        with self.assertRaises(LayoutError):
            compose_layout(chunks, layout, patterns, bytes(4))

    def test_palette_line_shifts_the_index(self):
        words = [0] * CHUNK_WORDS
        words[0] = 1 | 1 << 13
        chunks = decode_chunks(kos_literals(chunk_bytes(words)), (0,))
        layout = decode_layout(kos_literals(b"\x00"), 0, 1, 1)
        patterns = decode_tilesets(kos_literals(bytes(32) + b"\x12" * 32), ((0, 0),))
        _, _, pixels, _ = compose_layout(chunks, layout, patterns)
        self.assertEqual(pixels[0], 1 + 16)

    def test_flips(self):
        art = bytes(32) + bytes([0x10]) + bytes(31)  # pattern 1: one lit top-left pixel
        patterns = decode_tilesets(kos_literals(art), ((0, 0),))
        for flip, expect in (
            (0, (0, 0)),
            (1 << 11, (7, 0)),
            (1 << 12, (0, 7)),
            (1 << 11 | 1 << 12, (7, 7)),
        ):
            with self.subTest(flip=flip):
                words = [0] * CHUNK_WORDS
                words[0] = 1 | flip
                chunks = decode_chunks(kos_literals(chunk_bytes(words)), (0,))
                layout = decode_layout(kos_literals(b"\x00"), 0, 1, 1)
                _, _, pixels, _ = compose_layout(chunks, layout, patterns)
                lit = [(i % CHUNK_PIXELS_X, i // CHUNK_PIXELS_X)
                       for i, v in enumerate(pixels) if v]
                self.assertEqual(lit, [expect])

    def test_render_layout_emits_a_png_of_the_right_size(self):
        chunks, layout, patterns = self._one_tile_map()
        data = render_layout(chunks, layout, patterns)
        self.assertEqual(data[:8], png_signature())
        self.assertEqual(png_size(data), (CHUNK_PIXELS_X, CHUNK_PIXELS_Y))

    def test_render_collision_scales_to_map_pixels(self):
        table = decode_chunks(kos_literals(collision_chunk((0x0, 0x8, 0x1, 0x2))), (0,))
        grid = decode_collision(table, decode_layout(kos_literals(b"\x00"), 0, 1, 1))
        data = render_collision(grid)
        self.assertEqual(png_size(data), (2 * COLLISION_CELL_PIXELS, 2 * COLLISION_CELL_PIXELS))
        self.assertEqual(png_size(render_collision(grid, scale=1)), (2, 2))
        with self.assertRaises(LayoutError):
            render_collision(grid, scale=0)


def png_signature() -> bytes:
    return b"\x89PNG\r\n\x1a\n"


def png_size(data: bytes) -> tuple[int, int]:
    return struct.unpack_from(">II", data, 16)


class SpecTest(unittest.TestCase):
    def test_from_header_adds_one_to_every_dimension(self):
        spec = MapLayoutSpec.from_header(
            chunk_blobs=(1,), layout_fg=2, layout_bg=3,
            dimension_bytes=(0x1F, 0x0F, 0x7F, 0x00),
        )
        self.assertEqual(spec.width_chunks_fg, 32)
        self.assertEqual(spec.height_chunks_fg, 16)
        self.assertEqual(spec.width_chunks_bg, 128)
        self.assertEqual(spec.height_chunks_bg, 1)

    def test_collision_plane_byte_selects_the_layout(self):
        base = dict(chunk_blobs=(1,), layout_fg=2, layout_bg=3,
                    dimension_bytes=(0, 0, 0, 0))
        self.assertEqual(
            MapLayoutSpec.from_header(collision_plane=0, **base).collision_plane_name,
            PLANE_FG,
        )
        self.assertEqual(
            MapLayoutSpec.from_header(collision_plane=1, **base).collision_plane_name,
            PLANE_BG,
        )

    def test_wrong_number_of_dimension_bytes_is_rejected(self):
        with self.assertRaises(LayoutError):
            MapLayoutSpec.from_header(
                chunk_blobs=(1,), layout_fg=2, layout_bg=3, dimension_bytes=(0, 0, 0),
            )


# ---------------------------------------------------------------------------
# The cartridge
# ---------------------------------------------------------------------------
@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class RetailLayoutTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.maps = {
            entry["spec"].label: decode_map_layout(cls.data, entry["spec"], with_tiles=True)
            for entry in HAND_WALKED
        }

    def test_map_records_sit_where_the_pointer_table_says(self):
        for entry in HAND_WALKED:
            with self.subTest(map_id=entry["map_id"]):
                pointer = struct.unpack_from(">L", self.data, FIELD_MAP_PTRS + entry["map_id"] * 4)[0]
                self.assertEqual(pointer, entry["record"])

    def test_piata_dimensions(self):
        piata = self.maps["piata"]
        self.assertEqual((piata.fg.width_chunks, piata.fg.height_chunks), (32, 32))
        self.assertEqual((piata.bg.width_chunks, piata.bg.height_chunks), (32, 32))
        self.assertEqual((piata.fg.width_pixels, piata.fg.height_pixels), (1024, 1024))
        self.assertEqual(len(piata.chunks), 183)

    def test_piata_chunk_blobs_consume_exactly_their_range(self):
        # 0x122A90 is shared by fourteen Motavia maps; 0x123380 is Piata's own.
        # Both are zero-padded to a 16-byte boundary before the next blob.
        blobs = self.maps["piata"].chunks.blobs
        self.assertEqual(blobs[0].rom_offset, 0x122A90)
        self.assertEqual(blobs[0].compressed_length, 0x8E1)
        self.assertEqual(blobs[0].rom_end, 0x123371)
        self.assertEqual(blobs[0].decompressed_length, 4544)
        self.assertEqual(blobs[1].rom_offset, 0x123380)
        self.assertEqual(blobs[1].compressed_length, 0x381)
        self.assertEqual(blobs[1].rom_end, 0x123701)
        self.assertEqual(blobs[1].decompressed_length, 1312)
        self.assertEqual(sum(b.decompressed_length for b in blobs), 183 * CHUNK_BYTES)
        for blob in blobs:
            self.assertEqual(set(self.data[blob.rom_end:(blob.rom_end + 15) & ~15]), {0} if
                             blob.rom_end % 16 else set())

    def test_piata_layout_blobs_chain_into_the_palette(self):
        # FG layout, BG layout and the palette blob are consecutive, so each
        # decoder's consumed length is checked by the next thing's address.
        piata = self.maps["piata"]
        self.assertEqual(piata.fg.blob.rom_offset, 0x123710)
        self.assertEqual(piata.fg.blob.compressed_length, 0x9D)
        self.assertEqual(piata.fg.blob.rom_end, 0x1237AD)
        self.assertLessEqual(piata.fg.blob.rom_end, piata.bg.blob.rom_offset)
        self.assertEqual(piata.bg.blob.rom_offset, 0x1237B0)
        self.assertEqual(piata.bg.blob.compressed_length, 0x1D8)
        self.assertEqual(piata.bg.blob.rom_end, 0x123988)
        self.assertLessEqual(piata.bg.blob.rom_end, MAP_PIATA["spec"].palette)
        self.assertLess(MAP_PIATA["spec"].palette - piata.bg.blob.rom_end, 16)

    def test_piata_tilesets_tile_into_vram_up_to_the_first_sprite(self):
        # loc_519D2's three entries name VRAM tiles 0x10, 0x111 and 0x213; each
        # blob's tile count is exactly the gap to the next, and the last one
        # stops where loc_51A1A's first sprite begins.
        sets = self.maps["piata"].patterns.tilesets
        self.assertEqual([t.first_tile for t in sets], [0x010, 0x111, 0x213])
        self.assertEqual([t.tile_count for t in sets], [257, 258, 189])
        for earlier, later in zip(sets, sets[1:]):
            self.assertEqual(earlier.end_tile, later.first_tile)
        self.assertEqual(sets[-1].end_tile, PIATA_FIRST_SPRITE_TILE)
        self.assertEqual(
            struct.unpack_from(">H", self.data, 0x11C6A6)[0], PIATA_FIRST_SPRITE_TILE
        )

    def test_every_hand_walked_map_pins(self):
        expected = {
            "piata": {
                "chunks": 183,
                "fg_cells": "d7d53f3a9f8da150680e82bd43e264d2faacb7dfb42d91e57e8828cb40c1fb46",
                "bg_cells": "a7b83198d68b5afcd166b57846f9acd5effde2bb0dc41b72fe814c757ca8acb7",
                "collision": "533b6c0aba2054b3c54d425556937679e0cb77831670ed7e389c3d856df6771f",
                "grid": (64, 64),
                "histogram": {0: 3236, 1: 7, 8: 734, 0xC: 119},
                "plane": PLANE_BG,
            },
            "piata_item_shop": {
                "chunks": 256,
                "fg_cells": "824547589a4bd1790ee59744703c16a9beffd4b894c2cffa909370469e6aed80",
                "bg_cells": "2f5cee4ee3aed0bb48bc22349483b00eccd7fe7dcc8ffd8aa1ac96c3b12388ab",
                "collision": "c2bb1c9da760531b5d324fff16273adc83beab218cf58ec2e5a258aad65f95e5",
                "grid": (64, 64),
                "histogram": {0: 4000, 8: 70, 0xC: 26},
                "plane": PLANE_BG,
            },
            "island_cave": {
                "chunks": 256,
                "fg_cells": "014bd35f2fd536e04d0af0c19a87f8831a42a40fe4ce154a273cc8de5ca3b7a9",
                "bg_cells": "16721e791ccb5b3c8c066dee9e606c159202a1aab7e84e132d58a4fb56347f65",
                "collision": "175945f8f5b354c91aa713ef0ec6bb483081580a0b10ae82103ac631e157e1e0",
                "grid": (96, 96),
                "histogram": {0: 7868, 1: 8, 8: 1340},
                "plane": PLANE_FG,
            },
        }
        for label, want in expected.items():
            with self.subTest(map=label):
                decoded = self.maps[label]
                self.assertEqual(len(decoded.chunks), want["chunks"])
                self.assertEqual(hashlib.sha256(decoded.fg.cells).hexdigest(), want["fg_cells"])
                self.assertEqual(hashlib.sha256(decoded.bg.cells).hexdigest(), want["bg_cells"])
                self.assertEqual(
                    hashlib.sha256(decoded.collision.types).hexdigest(), want["collision"]
                )
                self.assertEqual(
                    (decoded.collision.width, decoded.collision.height), want["grid"]
                )
                self.assertEqual(decoded.collision.histogram(), want["histogram"])
                self.assertEqual(decoded.collision_layout.plane, want["plane"])

    def test_the_island_cave_reads_collision_off_the_other_plane(self):
        # Its $FFFFEC24 byte is zero, so GetChunkAndCollision uses Map_Layout_FG
        # where Piata uses Map_Layout_BG. Decoding the wrong plane gives a grid
        # that is nothing like the map.
        cave = self.maps["island_cave"]
        self.assertEqual(cave.spec.collision_plane, 0)
        self.assertIs(cave.collision_layout, cave.fg)
        wrong = decode_collision(cave.chunks, cave.bg)
        self.assertNotEqual(wrong.types, cave.collision.types)
        # Reading the other plane calls four fifths of the map solid, which is
        # not a cave anyone could walk through; the right plane blocks a sixth.
        cells = cave.collision.width * cave.collision.height
        self.assertLess(sum(1 for v in cave.collision.types if is_blocking(v)), cells // 4)
        self.assertGreater(sum(1 for v in wrong.types if is_blocking(v)), cells * 3 // 4)

    def test_piata_doorways_are_map_change_cells(self):
        # Every entry in Map_Piata's second transition table names the cell the
        # player walks from; GetChunkAndCollision probes 16 pixels below that,
        # and the cell it lands on must be collision type 1 or the door would
        # never fire. Six doors, six hits, nothing else in the map is type 1.
        piata = self.maps["piata"]
        offset = PIATA_TRANSITIONS_2
        doors = []
        while struct.unpack_from(">H", self.data, offset)[0] != 0xFFFF:
            x, y = self.data[offset], self.data[offset + 1]
            destination = struct.unpack_from(">H", self.data, offset + 4)[0]
            doors.append((x, y, destination))
            offset += PIATA_TRANSITION_SIZE
        self.assertEqual(len(doors), 6)
        self.assertEqual(
            [d[2] for d in doors],
            [0x11, 0x18, 0x19, 0x1A, 0x1B, 0x1C],  # Academy, Dorm, Inn, House1, Shop, House2
        )
        for x, y, destination in doors:
            with self.subTest(destination=hex(destination)):
                self.assertEqual(piata.collision.type_at(x, y + 1), 0x1)
                self.assertEqual(piata.collision.name_at(x, y + 1), "map_change")

        found = {(x, y) for y in range(piata.collision.height)
                 for x in range(piata.collision.width)
                 if piata.collision.type_at(x, y) == 0x1}
        # The seventh is the Academy's second doorway cell: its transition
        # rectangle is two cells wide (XYRange_XPlus20_YPlus10).
        self.assertEqual(len(found), 7)
        self.assertIn((0x1F, 0x07), found)
        self.assertIn((0x20, 0x07), found)

    def test_the_item_shop_counter_is_where_the_shop_table_says(self):
        # loc_68394's PiataItemShop entry is at pixel (0x210, 0x1D0). One cell
        # below it -- the same 16-pixel probe offset the doorways use -- is a
        # type $C cell, the one TileColl_Shop handles.
        shop = self.maps["piata_item_shop"]
        x, y = PIATA_ITEM_SHOP_COUNTER
        self.assertEqual(shop.collision.type_at(x, y + 1), 0xC)
        self.assertEqual(shop.collision.name_at(x, y + 1), "shop")
        self.assertTrue(is_blocking(0xC))
        entry = self.data[0x0683F4:0x0683FC]
        self.assertEqual(entry, bytes.fromhex("001b021001d00100"))

    def test_collision_bit_never_reaches_the_vdp_word(self):
        for decoded in self.maps.values():
            for words in decoded.chunks.words:
                for word in words:
                    self.assertEqual(vdp_word(word) >> COLLISION_BIT & 1, 0)

    def test_chunk_tiles_only_use_the_two_reachable_palette_lines(self):
        # Bit 14 is collision, so no chunk word can ask for CRAM line 2 or 3.
        for decoded in self.maps.values():
            for words in decoded.chunks.words:
                for word in words:
                    self.assertIn(palette_line(vdp_word(word)), (0, 1))

    def test_layouts_only_name_chunks_the_map_loads(self):
        for label, decoded in self.maps.items():
            with self.subTest(map=label):
                for layout in (decoded.fg, decoded.bg):
                    self.assertLess(max(layout.cells), len(decoded.chunks))

    def test_piata_composes_without_a_single_unloaded_pattern(self):
        piata = self.maps["piata"]
        width, height, pixels, missing = compose_layout(piata.chunks, piata.bg, piata.patterns)
        self.assertEqual((width, height), (1024, 1024))
        self.assertEqual(missing, set())
        _, _, composed, missing_fg = compose_layout(
            piata.chunks, piata.fg, piata.patterns, pixels
        )
        self.assertEqual(missing_fg, set())
        self.assertEqual(
            hashlib.sha256(bytes(composed)).hexdigest(),
            "40a29262b475bd5bd6268e0ec4a0ef2bc3622e33a541c60b0935f12e9c6aef4a",
        )
        # A town is mostly not one flat colour. If the tile lookup or the chunk
        # stride were wrong this would collapse.
        self.assertGreater(len(set(composed)), 24)

    def test_piata_palette_is_a_map_palette(self):
        colours = decode_map_palette(self.data, MAP_PIATA["spec"].palette)
        self.assertEqual(len(colours), MAP_PALETTE_BYTES // 2)
        self.assertFalse(any(c.get("unused_bits_set") for c in colours))
        reachable = chunk_palette(self.data, MAP_PIATA["spec"].palette)
        self.assertEqual(len(reachable), 32)
        self.assertEqual(reachable[0], (0, 0, 0))

    def test_renderers_produce_pngs_at_map_resolution(self):
        piata = self.maps["piata"]
        layout_png = render_layout(
            piata.chunks, piata.bg, piata.patterns,
            chunk_palette(self.data, MAP_PIATA["spec"].palette), overlay=piata.fg,
        )
        self.assertEqual(png_size(layout_png), (1024, 1024))
        collision_png = render_collision(piata.collision)
        self.assertEqual(png_size(collision_png), (1024, 1024))

    def test_mismatched_overlay_dimensions_are_rejected(self):
        piata = self.maps["piata"]
        cave = self.maps["island_cave"]
        with self.assertRaises(LayoutError):
            render_layout(piata.chunks, piata.bg, piata.patterns, overlay=cave.fg)

    def test_retail_code_assembles_the_collision_nibble_the_way_we_do(self):
        # The strongest available proof that this is a transcription: the exact
        # instruction bytes at loc_45B0A in the cartridge.
        self.assertEqual(
            self.data[COLLISION_ASSEMBLY_OPCODE:
                      COLLISION_ASSEMBLY_OPCODE + len(COLLISION_ASSEMBLY_BYTES)],
            COLLISION_ASSEMBLY_BYTES,
        )

    def test_retail_code_masks_the_collision_bit_before_the_vdp_sees_it(self):
        for offset in COLLISION_MASK_OPCODES:
            with self.subTest(offset=hex(offset)):
                # andi.w #$BFFF,d3
                self.assertEqual(self.data[offset:offset + 4], bytes.fromhex("0243bfff"))
        self.assertEqual(VDP_WORD_MASK, 0xBFFF)

    def test_retail_code_picks_the_collision_plane_from_ec24(self):
        # loc_45AD0: tst.b ($FFFFEC24).w / bne / addi.l #Map_Layout_FG,d3 /
        # bra / addi.l #Map_Layout_BG,d3. The two RAM bases are 0x1000 apart,
        # which is the size this module gives a layout region.
        chunk = self.data[0x045ACC:0x045AE0]
        self.assertEqual(
            chunk,
            bytes.fromhex("4a38ec24" "6608" "0683ffffa000" "6006" "0683ffffb000"),
        )
        self.assertEqual(LAYOUT_RAM_BYTES, 0xB000 - 0xA000)

    def test_the_layout_stride_branch_is_crossed_in_retail_too(self):
        # A real cartridge quirk, not a fork edit: GetChunkAndCollision takes
        # the *other* plane's row size as the layout stride. The bounds check
        # eleven instructions earlier gets it right, and SetupChunksFG uses
        # Map_Row_Size_FG for the FG plane, so the two disagree. It is dormant
        # only because every map this project has walked gives both planes the
        # same width.
        # GetChunkAndCollision: EC24 clear picks Map_Layout_FG (proven two
        # tests up) but loads d4 from $FFFFEC66, which is Map_Row_Size_BG.
        self.assertEqual(
            self.data[0x045AB8:0x045AC6],
            bytes.fromhex("1838ec64" "4a38ec24" "6604" "1838ec66"),
        )
        # SetupChunksFG and SetupChunksBG, the routines that actually draw,
        # pair each plane with its own row size. The three cannot all be right.
        self.assertEqual(
            self.data[0x054356:0x054366],
            bytes.fromhex("1838ec64" "5244" "c6c4" "d645" "0683ffffa000"),
        )
        self.assertEqual(
            self.data[0x0543A6:0x0543B6],
            bytes.fromhex("1838ec66" "5244" "c6c4" "d645" "0683ffffb000"),
        )
        for entry in HAND_WALKED:
            spec = entry["spec"]
            with self.subTest(map=spec.label):
                self.assertEqual(spec.width_chunks_fg, spec.width_chunks_bg)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    REFERENCE.exists(),
    f"Disassembly clone not present at {REFERENCE}; "
    "git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm",
)
class DisassemblyOracleTest(unittest.TestCase):
    """Cross-checks against the public disassembly.

    The clone is configured as a Grand Cross hack build, so nothing here is
    trusted on its own -- these assertions only confirm that the labels this
    module was transcribed from say what the retail bytes already proved.
    """

    @classmethod
    def setUpClass(cls):
        cls.constants = (REFERENCE / "ps4.constants.asm").read_text(errors="replace")
        cls.source = (REFERENCE / "ps4.asm").read_text(errors="replace")

    def test_ram_map_gives_a_layout_region_per_plane(self):
        self.assertIn("Map_Layout_FG = ramaddr($FFFFA000)", self.constants)
        self.assertIn("Map_Layout_BG = ramaddr($FFFFB000)", self.constants)

    def test_chunk_table_comment_states_the_definition_size(self):
        line = next(
            l for l in self.constants.splitlines() if l.startswith("Chunk_Table =")
        )
        self.assertIn("$FFFF6000", line)
        self.assertIn("32 bytes (16 words) per definition", line)
        self.assertIn("bit 6 is the collision flag", line)

    def test_chunk_blobs_are_kosinski(self):
        # Map_LoadChunks: longs until $FFFF, each one KosDecomp'd into a1,
        # which is loaded once before the loop.
        body = self.source.split("Map_LoadChunks:")[1].split("loc_51AFC:")[0]
        self.assertIn("lea\t(Chunk_Table).l, a1", body)
        self.assertIn("jsr\t(KosDecomp).l", body)
        self.assertNotIn("NemDecomp", body)
        self.assertNotIn("Enigma", body)

    def test_layout_blobs_are_kosinski_and_skipped_on_the_overworlds(self):
        for label, plane in (("loc_539E2:", "Map_Layout_FG"), ("loc_53A04:", "Map_Layout_BG")):
            body = self.source.split(label)[1][:400]
            self.assertIn("andi.w\t#$FFFE, d0", body)
            self.assertIn(f"lea\t({plane}).w, a1", body)
            self.assertIn("jsr\t(KosDecomp).l", body)

    def test_tilesets_are_kosinski_staged_through_the_chunk_table(self):
        body = self.source.split("loc_519D2:")[1].split("loc_51A14:")[0]
        self.assertIn("lea\t(Chunk_Table).l, a1", body)
        self.assertIn("jsr\t(KosDecomp).l", body)
        self.assertIn("lea\t$6(a0), a0", body)  # six-byte entries

    def test_collision_bit_is_masked_with_a_named_comment(self):
        self.assertIn("andi.w\t#$BFFF, d3\t; remove collision bit", self.source)

    def test_collision_type_names_come_from_the_disassembly(self):
        body = self.source.split("GetChunkAndCollision:")[1].split("RunMapTransitions:")[0]
        self.assertIn("1 = map change", body)
        self.assertIn("2 = recovery", body)
        self.assertIn("8 = solid; 9 = water; $A = sand; $B = ice block; $C = shop", body)
        for value, name in COLLISION_TYPE_NAMES.items():
            if value in (0, 1, 2):
                continue
            self.assertIn(name.replace("_", " "), body)

    def test_blocking_types_match_the_jump_table(self):
        table = self.source.split("TileCollNormalPtrs:")[1].split("; =")[0]
        entries = []
        for line in table.splitlines():
            line = line.strip()
            if line.startswith("dc.l"):
                entries.append(line.split()[1].rstrip(";").strip())
        self.assertEqual(len(entries), 16)
        blocking = {i for i, name in enumerate(entries)
                    if name in ("TileColl_Solid", "TileColl_Shop")}
        self.assertEqual(blocking, set(BLOCKING_COLLISION_TYPES))

    def test_the_four_dimension_bytes_are_read_in_record_order(self):
        body = self.source.split("GameMode_LoadFieldMap:")[1].split("Map_LoadChunks).l")[0]
        order = [
            line.split(", ")[1].split(")")[0].lstrip("(")
            for line in body.splitlines()
            if "move.b\t(a0)+, (Map_" in line
        ]
        # The first of these is Map_General_Var, read before the tileset loop.
        self.assertEqual(order[0], "Map_General_Var")
        self.assertEqual(
            order[1:],
            ["Map_Row_Size_FG", "Map_Column_Size_FG", "Map_Row_Size_BG", "Map_Column_Size_BG"],
        )


if __name__ == "__main__":
    unittest.main()
