import collections
import hashlib
import re
import unittest
from pathlib import Path

from psiv_tools.core import read_rom
from psiv_tools.gfx import PALETTES, PALETTE_LINE_SIZE, decode_palette, palette_rgb
from psiv_tools.sprites import (
    CHAR_FIELD_ART_PTRS,
    CHAR_SPRITE_MAPPINGS_PTRS,
    FACINGS,
    FIELD_OBJECTS_JMP_TBL,
    FIELD_OBJECT_COUNT,
    FIELD_OBJ_ANIMATE,
    HFLIP_BIT,
    FIELD_OBJECTS_CLEAR_SIGNATURE,
    FIELD_OBJECTS_CLEAR_SITE,
    FIELD_OBJECTS_MEMORY_CLEAR_LONGS,
    FIELD_OBJ_DO_OBJ_COLLISION,
    INTERACTION_BTST_SIGNATURE,
    INTERACTION_CHK_OBJECTS,
    INTERACTION_LOOP_SIGNATURE,
    MOVEMENTS_TBL,
    MOVEMENT_SELECTOR_MASK,
    RENDER_FLAG_INTERACTABLE,
    MOVEMENT_SELECTOR_MASK_SITE,
    PAL_INIT_LINE_3,
    PAL_INIT_LINE_3_CRAM_LINE,
    PARTY_ART_BYTES,
    PARTY_ART_TILES,
    PARTY_SYMBOLS,
    SPRITE_TILE_PROPS_LINES,
    Box,
    Frame,
    Mapping,
    Piece,
    AnimationSequence,
    SequenceFrame,
    Sheet,
    SpriteError,
    TileSource,
    build_sheet,
    compose_frame,
    decode_mapping,
    decode_sequence,
    effective_tile_word,
    facing_table_extents,
    mapping_box,
    pal_init_line_3,
    party_sprites,
    scan_field_objects,
    step_timing,
    union_box,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
DISASM = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm" / "ps4.asm"

#: `FieldObj_Chaz` and `FieldObj_None`, the first two jump-table entries.
CHAZ_ROUTINE = 0x0466A0
NONE_ROUTINE = 0x04669E

#: `CharFieldArtPtrs`: eleven 2,304-byte blobs, back to back.
FIRST_PARTY_ART = 0x290000
PARTY_ART_STRIDE = 0x900

#: `SprMapsPtrs_Chaz` and the block of twelve shared `Mappings_Char*` records.
CHAZ_MAPPINGS_TABLE = 0x047E44
CHAR_MAPPINGS_BLOCK = 0x0482A4


# ---------------------------------------------------------------------------
# Record formats. These need no ROM.
# ---------------------------------------------------------------------------
class TestMappingRecords(unittest.TestCase):
    def test_the_count_byte_is_one_less_than_the_pieces(self):
        # `Field_FillSpriteAttributes` loads byte 0 into d1 and runs `dbf`, so a
        # record of one piece stores zero. Reading it as a plain count silently
        # drops the last piece of every mapping in the cartridge.
        data = bytes([0x00, 0x00]) + bytes([0xFE, 0x07, 0x00, 0x00, 0x00, 0xF0])
        mapping = decode_mapping(data, 0)
        self.assertEqual(len(mapping.pieces), 1)

    def test_a_two_piece_record(self):
        data = (
            bytes([0x01, 0x00])
            + bytes([0x00, 0x0F, 0x00, 0x20, 0xFC, 0xE4])
            + bytes([0x00, 0x03, 0x00, 0x30, 0x1C, 0xDC])
        )
        mapping = decode_mapping(data, 0)
        self.assertEqual(len(mapping.pieces), 2)
        first, second = mapping.pieces
        self.assertEqual((first.y, first.x, first.x_mirrored), (0, -4, -28))
        self.assertEqual((first.width_tiles, first.height_tiles), (4, 4))
        self.assertEqual(second.tile_word, 0x0030)
        self.assertEqual((second.width_tiles, second.height_tiles), (1, 4))
        self.assertEqual(mapping.size_bytes, 14)

    def test_the_size_byte_is_the_mega_drive_sprite_size(self):
        # %0000wwhh: bits 3-2 are tiles across minus one, bits 1-0 tiles down.
        for size, expected in ((0x00, (1, 1)), (0x07, (2, 4)), (0x0F, (4, 4)), (0x0C, (4, 1))):
            with self.subTest(size=size):
                piece = Piece(y=0, size_byte=size, tile_word=0, x=0, x_mirrored=0)
                self.assertEqual((piece.width_tiles, piece.height_tiles), expected)
                self.assertEqual(piece.tile_count, expected[0] * expected[1])

    def test_offsets_are_signed_bytes(self):
        data = bytes([0x00, 0x00]) + bytes([0xFE, 0x00, 0x00, 0x00, 0x80, 0x7F])
        piece = decode_mapping(data, 0).pieces[0]
        self.assertEqual((piece.y, piece.x, piece.x_mirrored), (-2, -128, 127))

    def test_the_header_byte_is_preserved(self):
        data = bytes([0x00, 0x5A]) + bytes(6)
        self.assertEqual(decode_mapping(data, 0).header_byte, 0x5A)


class TestSequenceRecords(unittest.TestCase):
    def test_the_shared_duration_form(self):
        data = bytes([0x04, 0x0A]) + b"".join(
            n.to_bytes(4, "big") for n in (0x1000, 0x1008, 0x1000, 0x1010)
        )
        sequence = decode_sequence(data, 0)
        self.assertFalse(sequence.per_frame_durations)
        self.assertEqual(
            [f.mapping_offset for f in sequence.frames], [0x1000, 0x1008, 0x1000, 0x1010]
        )
        self.assertEqual([f.duration_byte for f in sequence.frames], [10] * 4)

    def test_the_count_byte_is_exact_here(self):
        # The other direction from the mapping record: `FieldObj_Animate` does
        # `cmp.b (a1),d0 / bcs`, so index < count. The two records disagree on
        # this and mixing them up costs a frame at one end or the other.
        data = bytes([0x02, 0x00]) + (0x1000).to_bytes(4, "big") * 2
        self.assertEqual(len(decode_sequence(data, 0).frames), 2)

    def test_the_per_frame_duration_form_with_an_odd_count(self):
        # count 3 -> durations at 1..3, pointers from offset 4 (loc_4477E forces
        # its index odd and reads from 1(a1,d0.w)).
        data = bytes([0x83, 0x01, 0x02, 0x03]) + b"".join(
            n.to_bytes(4, "big") for n in (0x2000, 0x2008, 0x2010)
        )
        sequence = decode_sequence(data, 0)
        self.assertTrue(sequence.per_frame_durations)
        self.assertEqual([f.duration_byte for f in sequence.frames], [1, 2, 3])
        self.assertEqual(
            [f.mapping_offset for f in sequence.frames], [0x2000, 0x2008, 0x2010]
        )

    def test_the_per_frame_duration_form_with_an_even_count(self):
        # count 2 -> durations at 1..2, then one pad byte before the pointers.
        data = bytes([0x82, 0x05, 0x06, 0xFF]) + b"".join(
            n.to_bytes(4, "big") for n in (0x3000, 0x3008)
        )
        sequence = decode_sequence(data, 0)
        self.assertEqual([f.duration_byte for f in sequence.frames], [5, 6])
        self.assertEqual([f.mapping_offset for f in sequence.frames], [0x3000, 0x3008])

    def test_a_frame_is_displayed_for_duration_plus_one(self):
        # `subq.b #1, mappings_duration(a4) / bpl` only advances once the
        # counter goes negative, so a stored 0 is one frame, not none.
        self.assertEqual(SequenceFrame(0, 0).ticks, 1)
        self.assertEqual(SequenceFrame(0, 10).ticks, 11)

    def test_a_sequence_of_no_frames_is_rejected(self):
        with self.assertRaises(SpriteError):
            decode_sequence(bytes([0x00, 0x00, 0, 0, 0, 0]), 0)
        with self.assertRaises(SpriteError):
            decode_sequence(bytes([0x80, 0x00, 0, 0, 0, 0]), 0)


class TestPatternArithmetic(unittest.TestCase):
    def test_the_pattern_word_is_added_to_the_art_tile(self):
        self.assertEqual(effective_tile_word(0x0300, 0x0018, 0x00), 0x0318)

    def test_the_tile_properties_are_ored_into_the_high_byte(self):
        # $60 is CRAM line 3; $40 line 2. The palette bits are 6-5 of the high
        # byte, which is bits 14-13 of the pattern word.
        self.assertEqual(effective_tile_word(0x0300, 0x0018, 0x60), 0x6318)
        self.assertEqual(effective_tile_word(0x0000, 0x0800, 0x40) & HFLIP_BIT, HFLIP_BIT)

    def test_the_low_byte_carry_reaches_the_high_byte(self):
        # The routine writes the high byte first and pushes the carry back into
        # it with `addq.b #1,-$1(a2)`, so this is a real 16-bit add and not two
        # independent ones.
        self.assertEqual(effective_tile_word(0x00F0, 0x0020, 0x00), 0x0110)

    def test_every_retail_tile_property_value_names_a_cram_line(self):
        self.assertEqual(sorted(SPRITE_TILE_PROPS_LINES), [0x00, 0x20, 0x40, 0x60])
        self.assertEqual(list(SPRITE_TILE_PROPS_LINES.values()), [0, 1, 2, 3])


class TestComposition(unittest.TestCase):
    def _source(self):
        source = TileSource()
        source.add(0, b"".join(bytes([n + 1]) * 32 for n in range(8)))
        return source

    def test_pieces_run_down_each_column_before_moving_right(self):
        # Mega Drive sprite patterns are column-major. A row-major reading puts
        # a character's shoulders where their hip should be.
        source = TileSource()
        for index in range(4):
            # 0x11, 0x22, ... so both nybbles of every byte are the same value.
            source.add(index, bytes([(index + 1) * 0x11] * 32))
        piece = Piece(y=0, size_byte=0x05, tile_word=0, x=0, x_mirrored=0)  # 2x2
        mapping = Mapping(0, 0, (piece,))
        frame, missing = compose_frame(mapping, source, mapping_box(mapping))
        self.assertEqual(missing, [])
        # tile 0 top-left, tile 1 below it, tile 2 top-right, tile 3 below that.
        self.assertEqual(frame.pixels[0], 1)
        self.assertEqual(frame.pixels[8 * 16], 2)
        self.assertEqual(frame.pixels[8], 3)
        self.assertEqual(frame.pixels[8 * 16 + 8], 4)

    def test_hflip_mirrors_the_whole_piece(self):
        source = TileSource()
        source.add(0, bytes([1] * 32) + bytes([2] * 32))
        piece = Piece(y=0, size_byte=0x04, tile_word=HFLIP_BIT, x=0, x_mirrored=0)  # 2x1
        mapping = Mapping(0, 0, (piece,))
        frame, _ = compose_frame(mapping, source, mapping_box(mapping))
        # Unflipped the left half is tile 0; flipped it is tile 1.
        self.assertEqual(frame.pixels[0], 2)
        self.assertEqual(frame.pixels[8], 1)

    def test_hflip_reverses_pixels_inside_a_tile_too(self):
        source = TileSource()
        source.add(0, bytes([0x12, 0x34, 0x56, 0x78] * 8))
        piece = Piece(y=0, size_byte=0x00, tile_word=HFLIP_BIT, x=0, x_mirrored=0)
        mapping = Mapping(0, 0, (piece,))
        frame, _ = compose_frame(mapping, source, mapping_box(mapping))
        plain = TileSource()
        plain.add(0, bytes([0x12, 0x34, 0x56, 0x78] * 8))
        unflipped, _ = compose_frame(
            Mapping(0, 0, (Piece(0, 0x00, 0, 0, 0),)), plain, mapping_box(mapping)
        )
        self.assertEqual(frame.pixels[:8], unflipped.pixels[:8][::-1])

    def test_colour_zero_is_transparent_between_pieces(self):
        source = TileSource()
        source.add(0, bytes([0x00] * 32))          # tile 0: all colour 0
        source.add(1, bytes([0x55] * 32))          # tile 1: colour 5 everywhere
        pieces = (
            Piece(y=0, size_byte=0x00, tile_word=1, x=0, x_mirrored=0),
            Piece(y=0, size_byte=0x00, tile_word=0, x=0, x_mirrored=0),
        )
        mapping = Mapping(0, 0, pieces)
        frame, _ = compose_frame(mapping, source, mapping_box(mapping))
        self.assertEqual(set(frame.pixels), {5})

    def test_a_missing_pattern_is_reported_not_drawn(self):
        piece = Piece(y=0, size_byte=0x00, tile_word=0x100, x=0, x_mirrored=0)
        mapping = Mapping(0, 0, (piece,))
        frame, missing = compose_frame(mapping, TileSource(), mapping_box(mapping))
        self.assertEqual(missing, [0x100])
        self.assertEqual(set(frame.pixels), {0})

    def test_the_box_is_the_union_of_the_pieces(self):
        mapping = Mapping(0, 0, (
            Piece(y=-2, size_byte=0x07, tile_word=0, x=0, x_mirrored=0),
            Piece(y=8, size_byte=0x00, tile_word=0, x=-8, x_mirrored=0),
        ))
        self.assertEqual(mapping_box(mapping), Box(-8, -2, 16, 30))
        self.assertEqual(union_box([Box(0, 0, 4, 4), Box(-2, 1, 3, 9)]), Box(-2, 0, 4, 9))

    def test_the_origin_is_where_the_object_stands_inside_the_frame(self):
        mapping = Mapping(0, 0, (Piece(y=-2, size_byte=0x07, tile_word=0, x=0, x_mirrored=0),))
        source = TileSource()
        source.add(0, bytes(32 * 8))
        frame, _ = compose_frame(mapping, source, mapping_box(mapping))
        self.assertEqual((frame.width, frame.height), (16, 32))
        self.assertEqual((frame.origin_x, frame.origin_y), (0, 2))


class TestSheets(unittest.TestCase):
    def _frame(self, box, fill):
        return Frame(box.width, box.height, 0, 0, bytes([fill]) * (box.width * box.height))

    def test_repeated_frames_collapse_to_one_image(self):
        box = Box(0, 0, 8, 8)
        idle, step = self._frame(box, 1), self._frame(box, 2)
        sequence = AnimationSequence(0x100, False, tuple(SequenceFrame(0, 3) for _ in range(4)))
        sheet = build_sheet(
            [("down", 0, sequence, [idle, step, idle, step])], box, [(0, 0, 0)] * 16, 3
        )
        self.assertEqual(sheet.frame_count, 2)
        self.assertEqual(sheet.sequences["walk_down"].frames, (0, 1, 0, 1))
        self.assertEqual(sheet.sequences["idle_down"].frames, (0,))
        self.assertEqual(sheet.sequences["walk_down"].durations, (4, 4, 4, 4))

    def test_a_mirrored_frame_is_recorded_as_one(self):
        box = Box(0, 0, 2, 1)
        left = Frame(2, 1, 0, 0, bytes([1, 2]))
        right = Frame(2, 1, 0, 0, bytes([2, 1]))
        sequence = AnimationSequence(0x100, False, (SequenceFrame(0, 0),))
        sheet = build_sheet(
            [("left", 0xC, sequence, [left]), ("right", 8, sequence, [right])],
            box, [(0, 0, 0)] * 16, 3,
        )
        self.assertEqual(sheet.frame_count, 2)
        self.assertEqual(sheet.mirrors, {1: 0})

    def test_the_strip_lays_frames_out_left_to_right(self):
        box = Box(0, 0, 2, 2)
        sheet = Sheet(
            frames=(Frame(2, 2, 0, 0, bytes([1, 2, 3, 4])), Frame(2, 2, 0, 0, bytes([5, 6, 7, 8]))),
            sequences={}, box=box, palette=((0, 0, 0),) * 16, palette_line=0, mirrors={},
        )
        width, height, pixels = sheet.strip()
        self.assertEqual((width, height), (4, 2))
        self.assertEqual(list(pixels), [1, 2, 5, 6, 3, 4, 7, 8])

    def test_identity_separates_sheets_that_differ_only_in_palette(self):
        box = Box(0, 0, 1, 1)
        frames = (Frame(1, 1, 0, 0, bytes([1])),)
        first = Sheet(frames, {}, box, ((0, 0, 0),) * 16, 1, {})
        second = Sheet(frames, {}, box, ((255, 0, 0),) + ((0, 0, 0),) * 15, 1, {})
        self.assertNotEqual(first.identity(), second.identity())


# ---------------------------------------------------------------------------
# Against the cartridge.
# ---------------------------------------------------------------------------
@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestFieldObjectTable(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.routines = scan_field_objects(cls.data)

    def test_the_table_is_222_branches_and_stops(self):
        # The clone's table is the same 222 entries, and the seven-entry
        # TileAnimJmpTbl follows it: 222 * 4 lands exactly on loc_4AE00's
        # neighbourhood, which is how the length was pinned without trusting
        # the clone's own assembly.
        self.assertEqual(len(self.routines), FIELD_OBJECT_COUNT)
        for index in range(FIELD_OBJECT_COUNT):
            offset = FIELD_OBJECTS_JMP_TBL + index * 4
            self.assertEqual(self.data[offset:offset + 2], b"\x60\x00")

    def test_entry_zero_is_a_bare_rts_and_entry_one_is_chaz(self):
        self.assertEqual(self.routines[0].symbol, "None")
        self.assertEqual(self.routines[0].rom_offset, NONE_ROUTINE)
        self.assertEqual(self.data[NONE_ROUTINE:NONE_ROUTINE + 2], b"\x4e\x75")
        self.assertEqual(self.routines[1].symbol, "Chaz")
        self.assertEqual(self.routines[1].rom_offset, CHAZ_ROUTINE)
        self.assertEqual(self.routines[1].object_id, 4)

    def test_field_obj_animate_is_the_bit_five_setter(self):
        # The whole streamed/staged split hangs off this one instruction:
        # `bset #5, render_flags(a4)`, then fall through into Animate2.
        self.assertEqual(
            self.data[FIELD_OBJ_ANIMATE:FIELD_OBJ_ANIMATE + 6],
            bytes.fromhex("08ec00050002"),
        )

    def test_every_routine_but_none_has_an_init_block(self):
        without = [r.symbol for r in self.routines if r.init_end == r.init_start]
        self.assertEqual(without, ["None"])

    def test_field_objects_only_ever_select_four_palette_lines(self):
        # This is the whole palette question: $13 is the only thing in the
        # chain that sets the pattern word's palette bits, and retail stores
        # exactly these four values.
        values = {r.sprite_tile_props for r in self.routines if r.sprite_tile_props is not None}
        self.assertEqual(sorted(values), [0x00, 0x20, 0x40, 0x60])

    def test_the_party_routines_all_draw_on_cram_line_two(self):
        for symbol in PARTY_SYMBOLS:
            routine = next(r for r in self.routines if r.symbol == symbol)
            with self.subTest(symbol=symbol):
                self.assertEqual(routine.sprite_tile_props, 0x40)
                self.assertEqual(routine.palette_line, PAL_INIT_LINE_3_CRAM_LINE)

    def test_the_streaming_flag_matches_the_art_pointer(self):
        # An object streams its own art exactly when it reaches
        # FieldObj_Animate, which is the routine that sets render_flags bit 5.
        # One routine reaches it without storing an art pointer, and it is
        # never placed by a map record.
        odd = [
            r.symbol for r in self.routines if r.streams_art != (r.art_ptr is not None)
        ]
        self.assertEqual(odd, ["loc_483DC"])

    def test_only_three_routines_never_build_sprites(self):
        artless = [r.symbol for r in self.routines if not r.builds_sprites]
        self.assertEqual(artless, ["None", "InvisibleBlock", "KingRappyFlyingAway"])

    def test_objects_that_start_hidden_but_can_appear(self):
        # `bset #1, render_flags` at load is not the same as art-less: the Zio
        # Fort barrier beams `bchg` the bit to blink.
        hidden = sorted(r.symbol for r in self.routines if r.starts_hidden)
        self.assertEqual(
            hidden,
            ["BarrierBeam2", "BarrierBeam4", "GyLaguiah", "InvisibleBlock",
             "KingRappyFlyingAway", "LandaleBeam"],
        )
        for symbol in ("BarrierBeam2", "BarrierBeam4", "GyLaguiah", "LandaleBeam"):
            routine = next(r for r in self.routines if r.symbol == symbol)
            self.assertTrue(routine.builds_sprites)

    def test_the_two_routines_that_choose_tile_properties_at_run_time(self):
        alternates = {
            r.symbol: r.alternate_tile_props
            for r in self.routines if r.alternate_tile_props
        }
        self.assertEqual(alternates, {"TreasureChest": (0x20,), "Elevator": (0x20,)})

    def test_facing_tables_are_bounded_by_their_neighbours(self):
        extents = facing_table_extents(self.routines)
        chaz = next(r for r in self.routines if r.symbol == "Chaz")
        self.assertEqual(extents[chaz.mappings_addr], 4)
        # `FieldObj_Elevator` shares a region with its neighbours and owns two
        # longs; reading four would walk into `loc_4D2D0`'s table.
        elevator = next(r for r in self.routines if r.symbol == "Elevator")
        self.assertEqual(extents[elevator.mappings_addr], 2)
        self.assertTrue(all(1 <= value <= 4 for value in extents.values()))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestPartySprites(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.sprites = party_sprites(cls.data)
        cls.by_symbol = {s.symbol: s for s in cls.sprites}

    def test_the_eleven_art_blobs_are_contiguous(self):
        self.assertEqual([s.symbol for s in self.sprites], list(PARTY_SYMBOLS))
        for slot, sprite in enumerate(self.sprites):
            with self.subTest(symbol=sprite.symbol):
                self.assertEqual(sprite.art_offset, FIRST_PARTY_ART + slot * PARTY_ART_STRIDE)
                self.assertEqual(sprite.slot, slot)
        self.assertEqual(PARTY_ART_BYTES, PARTY_ART_STRIDE)
        self.assertEqual(PARTY_ART_TILES, 72)

    def test_the_pointer_tables_agree_with_the_routines(self):
        for slot, sprite in enumerate(self.sprites):
            art = int.from_bytes(
                self.data[CHAR_FIELD_ART_PTRS + slot * 4:CHAR_FIELD_ART_PTRS + slot * 4 + 4], "big"
            )
            table = int.from_bytes(
                self.data[CHAR_SPRITE_MAPPINGS_PTRS + slot * 4:CHAR_SPRITE_MAPPINGS_PTRS + slot * 4 + 4],
                "big",
            )
            self.assertEqual((art, table), (sprite.art_offset, sprite.mappings_addr))

    def test_chaz_art_is_pinned(self):
        chaz = self.by_symbol["Chaz"]
        self.assertEqual(chaz.art_offset, 0x290000)
        self.assertEqual(chaz.mappings_addr, CHAZ_MAPPINGS_TABLE)
        self.assertEqual(
            chaz.art_sha256,
            hashlib.sha256(
                self.data[0x290000:0x290000 + PARTY_ART_BYTES]
            ).hexdigest(),
        )
        self.assertEqual(
            chaz.art_sha256,
            "f70183b358de30338eb13ff6749a834f78f9499bcaaefabd3723072ca03072b1",
        )

    def test_every_sheet_is_twelve_sixteen_by_thirty_two_frames(self):
        for sprite in self.sprites:
            with self.subTest(symbol=sprite.symbol):
                sheet = sprite.sheet
                self.assertEqual(sheet.box, Box(0, -2, 16, 30))
                self.assertEqual(sheet.frame_count, 12)
                self.assertEqual(
                    sorted(sheet.sequences),
                    sorted(
                        f"{kind}_{name}" for kind in ("idle", "walk") for _, name in FACINGS
                    ),
                )
                self.assertEqual(sheet.palette_line, PAL_INIT_LINE_3_CRAM_LINE)

    def test_the_walk_cycle_is_idle_step_idle_step(self):
        # `SprMapsData_ChazDown` is four longs: idle, walk 1, idle, walk 2. The
        # idle frame is the same image twice, so the sheet holds three per
        # direction and the sequence names one of them twice.
        chaz = self.by_symbol["Chaz"].sheet
        self.assertEqual(chaz.sequences["walk_down"].frames, (0, 1, 0, 2))
        self.assertEqual(chaz.sequences["idle_down"].frames, (0,))
        self.assertEqual(chaz.sequences["walk_up"].frames, (3, 4, 3, 5))
        self.assertEqual(chaz.sequences["walk_right"].frames, (6, 7, 6, 8))
        self.assertEqual(chaz.sequences["walk_left"].frames, (9, 10, 9, 11))

    def test_left_and_right_are_the_same_art_h_flipped(self):
        # `Mappings_CharIdleRight` is `Mappings_CharIdleLeft`'s tile with bit 11
        # set, so the composed right frames are exact mirrors of the left ones.
        chaz = self.by_symbol["Chaz"].sheet
        self.assertEqual(chaz.mirrors, {9: 6, 10: 7, 11: 8})
        left = decode_mapping(self.data, CHAR_MAPPINGS_BLOCK + 3 * 8)
        right = decode_mapping(self.data, CHAR_MAPPINGS_BLOCK + 9 * 8)
        self.assertEqual(left.pieces[0].tile_word | HFLIP_BIT, right.pieces[0].tile_word)

    def test_frame_durations_are_per_character(self):
        # Byte 1 of `SprMapsData_*`, plus one for the `bpl` that spends it.
        durations = {
            s.symbol: s.sheet.sequences["walk_down"].durations[0] for s in self.sprites
        }
        self.assertEqual(durations["Chaz"], 11)
        self.assertEqual(durations["Alys"], 9)
        self.assertEqual(durations["Hahn"], 8)
        self.assertEqual(durations["Rune"], 12)
        self.assertEqual(durations["Demi"], 5)
        self.assertEqual(durations["Raja"], 13)

    def test_chaz_frame_pixels_are_pinned(self):
        width, height, pixels = self.by_symbol["Chaz"].sheet.strip()
        self.assertEqual((width, height), (192, 32))
        self.assertEqual(
            hashlib.sha256(pixels).hexdigest(),
            "d5f1964a9b1e93160623de8e47916fa6fa6040daeeecd59f3cd603a40c3cc4a4",
        )


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestSpritePalettes(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)

    def test_cram_line_two_is_pal_init_line_3(self):
        # `loc_53F14` copies 16 longs of the map's own blob, then eight longs of
        # `Pal_Init_Line_3`, then eight more of the blob: CRAM lines 0 and 1 and
        # 3 come from the map, line 2 never does. The party's $40 selects line
        # 2, which is why Chaz is the same colours in every town in the game.
        spec = next(p for p in PALETTES if p["label"] == "Pal_Init_Line_3")
        self.assertEqual(spec["rom_offset"], PAL_INIT_LINE_3)
        raw = self.data[PAL_INIT_LINE_3:PAL_INIT_LINE_3 + PALETTE_LINE_SIZE]
        self.assertEqual(pal_init_line_3(self.data), palette_rgb(decode_palette(raw)))

    def test_the_field_sprite_line_holds_skin_and_hair(self):
        # A cheap sanity check that this is a character palette and not, say,
        # the window frame: indices 4-6 are a skin-tone ramp and 15 is white.
        colours = pal_init_line_3(self.data)
        self.assertEqual(colours[4], (238, 170, 98))
        self.assertEqual(colours[15], (238, 238, 238))
        self.assertEqual(colours[0], (0, 0, 0))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestWalkTiming(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)

    def test_the_three_speed_blocks(self):
        # `FieldObj_Step_Offset` picks a block: 0 slow, 1 normal, 2 fast. One
        # step is always one 16-pixel collision cell; only the pixels per frame
        # change, so a step is 16, 8 or 4 frames.
        timings = step_timing(self.data)
        self.assertEqual([t.frames_per_cell for t in timings], [16, 8, 4])
        self.assertEqual([t.pixels_per_frame for t in timings], [1, 2, 4])
        self.assertEqual([t.step_word for t in timings], [0x0100, 0x0200, 0x0400])

    def test_the_movements_table_is_where_it_is_expected(self):
        # Entry $00 is the standing-still record -- no durations, no step, and
        # facing $FF for "leave the direction alone". Entry $02 walks down: a
        # $10 Y duration, a +$0100 Y step, facing 0.
        self.assertEqual(
            self.data[MOVEMENTS_TBL:MOVEMENTS_TBL + 8],
            bytes.fromhex("0000" "0000" "0000" "FF00"),
        )
        self.assertEqual(
            self.data[MOVEMENTS_TBL + 16:MOVEMENTS_TBL + 24],
            bytes.fromhex("0010" "0000" "0100" "0000"),
        )


# ---------------------------------------------------------------------------
# Against the public disassembly.
# ---------------------------------------------------------------------------
def parse_labelled_bytes(text: str, label: str) -> bytes:
    """The `dc.b` payload under one label, stopping at the next label.

    Only `dc.b` is accepted: a `dc.l` or a macro under the label means the
    listing is not the plain byte table this oracle assumes, and pretending
    otherwise would compare the wrong thing.
    """
    match = re.search(rf"^{re.escape(label)}:\s*$", text, re.MULTILINE)
    if match is None:
        raise AssertionError(f"{label} is not in the disassembly")
    out = bytearray()
    for line in text[match.end():].splitlines()[1:]:
        stripped = line.split(";")[0].strip()
        if not stripped:
            continue
        if not stripped.startswith("dc.b"):
            break
        for token in stripped[4:].split(","):
            out.append(int(token.strip().lstrip("$"), 16))
    return bytes(out)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    DISASM.exists(),
    "reference/ps4disasm is not checked out; clone it to run the oracle tests",
)
class TestAgainstTheDisassembly(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.text = DISASM.read_text(errors="replace")
        cls.sprites = {s.symbol: s for s in party_sprites(cls.data)}

    def test_the_twelve_shared_character_mappings_match_byte_for_byte(self):
        # The clone stores these as plain `dc.b` records, so the whole 96-byte
        # block is an exact oracle for the mapping format: if the piece layout
        # here were wrong, the offsets our sequences walk to would not land on
        # these bytes.
        labels = [
            "Mappings_CharIdleDown", "Mappings_CharWalkDown_1", "Mappings_CharWalkDown_2",
            "Mappings_CharIdleLeft", "Mappings_CharWalkLeft_1", "Mappings_CharWalkLeft_2",
            "Mappings_CharIdleUp", "Mappings_CharWalkUp_1", "Mappings_CharWalkUp_2",
            "Mappings_CharIdleRight", "Mappings_CharWalkRight_1", "Mappings_CharWalkRight_2",
        ]
        expected = b"".join(parse_labelled_bytes(self.text, label) for label in labels)
        self.assertEqual(len(expected), 12 * 8)
        self.assertEqual(
            self.data[CHAR_MAPPINGS_BLOCK:CHAR_MAPPINGS_BLOCK + len(expected)], expected
        )

    def test_every_party_sequence_header_matches_the_listing(self):
        # `SprMapsData_ChazDown: dc.b $04, $0A` -- frame count and duration.
        for symbol, sprite in self.sprites.items():
            for facing, name in FACINGS:
                label = f"SprMapsData_{symbol}{name.capitalize()}"
                expected = parse_labelled_bytes(self.text, label)
                offset = int.from_bytes(
                    self.data[sprite.mappings_addr + facing:sprite.mappings_addr + facing + 4],
                    "big",
                )
                with self.subTest(label=label):
                    self.assertEqual(self.data[offset:offset + 2], expected)

    def test_the_movements_table_matches_the_clone_but_the_selector_does_not(self):
        # The three speed blocks agree byte for byte, which is worth pinning
        # because the clone is a Grand Cross build and does not have to. What
        # does differ is the selector: `FieldObj_Move` masks
        # `FieldObj_Step_Offset` with `#3` in the cartridge and `#7` in the
        # clone, so the hack can reach blocks retail cannot. Retail's own mask
        # already reaches one block past the labelled table, and nothing in
        # retail ever stores a 3, so that is dormant rather than a bug.
        match = re.search(r"^FieldObj_MovementsTbl:\s*$", self.text, re.MULTILINE)
        self.assertIsNotNone(match)
        body = self.text[match.end():]
        words = re.findall(r"dc\.w\s+\$([0-9A-F]{4}), \$([0-9A-F]{4})", body)
        # Entry $02 of each block walks down: block N at index N * 16 + 2.
        self.assertEqual(
            [words[block * 16 + 2][1] for block in range(3)], ["0100", "0200", "0400"]
        )
        self.assertEqual(
            [t.step_word for t in step_timing(self.data)], [0x0100, 0x0200, 0x0400]
        )
        self.assertIn("andi.w\t#7, d1", self.text)
        self.assertEqual(
            self.data[MOVEMENT_SELECTOR_MASK_SITE:MOVEMENT_SELECTOR_MASK_SITE + 4],
            bytes.fromhex("0241") + MOVEMENT_SELECTOR_MASK.to_bytes(2, "big"),
        )


# ---------------------------------------------------------------------------
# render_flags bit 3: the talk probe and object collision.
# ---------------------------------------------------------------------------
#: Every 68000 encoding that can write a byte at `(d16, A4)`. The ea half is
#: fixed to mode 5 register 4 (0x2C), so only the operation half varies. The
#: point of the list is that most of it is *absent* from the cartridge.
def byte_write_opcodes() -> dict[int, str]:
    forms = {
        0x086C: "bchg #imm", 0x08AC: "bclr #imm", 0x08EC: "bset #imm",
        0x002C: "ori.b #imm", 0x022C: "andi.b #imm", 0x0A2C: "eori.b #imm",
        0x422C: "clr.b", 0x462C: "not.b", 0x197C: "move.b #imm",
    }
    for n in range(8):
        forms[0x016C | (n << 9)] = f"bchg d{n}"
        forms[0x01AC | (n << 9)] = f"bclr d{n}"
        forms[0x01EC | (n << 9)] = f"bset d{n}"
        forms[0x1940 | n] = f"move.b d{n}"
        forms[0x812C | (n << 9)] = f"or.b d{n}"
        forms[0xC12C | (n << 9)] = f"and.b d{n}"
        forms[0xB12C | (n << 9)] = f"eor.b d{n}"
    return forms


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestInteractableBit(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.routines = scan_field_objects(cls.data)
        cls.by_symbol = {r.symbol: r for r in cls.routines}

    def test_both_readers_of_the_bit_are_where_we_say(self):
        # `Interaction_ChkObjects` is the talk probe; `FieldObj_DoObjCollision`
        # is whether the object blocks the walker. Both open with the same
        # `lea / moveq #$33 / tst.w (a3) / beq / btst #3,$2(a3) / beq`, so one
        # bit governs both and an object nobody can talk to is also walkable
        # through.
        for offset in (INTERACTION_CHK_OBJECTS, FIELD_OBJ_DO_OBJ_COLLISION):
            with self.subTest(offset=hex(offset)):
                self.assertEqual(self.data[offset:offset + 4], bytes.fromhex("47f8c300"))
                head = offset + 4
                self.assertEqual(
                    self.data[head:head + len(INTERACTION_LOOP_SIGNATURE)],
                    INTERACTION_LOOP_SIGNATURE,
                )
                btst = head + 8
                self.assertEqual(
                    self.data[btst:btst + len(INTERACTION_BTST_SIGNATURE)],
                    INTERACTION_BTST_SIGNATURE,
                )
        self.assertEqual(RENDER_FLAG_INTERACTABLE, 3)

    def test_only_two_instruction_forms_ever_write_the_flags_byte(self):
        # The claim the extractor rests on: retail writes `render_flags` with
        # static-immediate bit instructions and nothing else. If a future read
        # of this region ever finds an `ori.b` or a register-operand `bset`,
        # the scanner is blind to it and this test says so.
        starts = sorted({r.rom_offset for r in self.routines})
        lo, hi = starts[0], starts[-1] + 0x200
        forms = byte_write_opcodes()
        seen = set()
        offset = lo
        while offset < hi - 6:
            word = int.from_bytes(self.data[offset:offset + 2], "big")
            name = forms.get(word)
            if name is not None:
                immediate = "#imm" in name
                displacement = offset + (4 if immediate else 2)
                if int.from_bytes(self.data[displacement:displacement + 2], "big") == 0x0002:
                    bit = (
                        int.from_bytes(self.data[offset + 2:offset + 4], "big")
                        if immediate else None
                    )
                    seen.add((name, bit))
            offset += 2
        operations = {name.split()[0] for name, _ in seen}
        self.assertEqual(operations, {"bset", "bclr", "bchg"})
        # Bit 3 specifically is only ever set or cleared, never toggled.
        self.assertEqual(
            {name for name, bit in seen if bit == RENDER_FLAG_INTERACTABLE},
            {"bset #imm", "bclr #imm"},
        )

    def test_an_unwritten_bit_is_clear_because_the_loader_zeroes_the_slots(self):
        # This is what makes "the routine never mentions bit 3" a decided
        # answer rather than "whatever the previous map left in that slot".
        self.assertEqual(
            self.data[FIELD_OBJECTS_CLEAR_SITE:
                      FIELD_OBJECTS_CLEAR_SITE + len(FIELD_OBJECTS_CLEAR_SIGNATURE)],
            FIELD_OBJECTS_CLEAR_SIGNATURE,
        )
        # `trap #0` is `moveq #0,d0 / move.l d0,(a0)+ / dbf d7`, so d7+1 longs.
        declared = int.from_bytes(
            self.data[FIELD_OBJECTS_CLEAR_SITE + 2:FIELD_OBJECTS_CLEAR_SITE + 4], "big"
        ) + 1
        self.assertEqual(declared, FIELD_OBJECTS_MEMORY_CLEAR_LONGS)
        # Field_Obj_Secondary is $FFFFC300 and Field_LoadObject hands out slots
        # up to $FFFFCFC0, so the clear covers every slot it can return.
        self.assertGreaterEqual(0xC000 + declared * 4, 0xCFC0 + 0x40)

    def test_the_split_across_all_222_routines(self):
        interactable = [r for r in self.routines if r.interactable]
        self.assertEqual(len(interactable), 104)
        self.assertEqual(len(self.routines) - len(interactable), 118)
        sources = collections.Counter(r.interactable_source for r in self.routines)
        self.assertEqual(sources["bset #3, $2(a4) in the init block"], 104)
        self.assertEqual(sources["bclr #3, $2(a4) in the init block"], 74)
        self.assertEqual(sources["routine has no init block"], 1)
        self.assertEqual(sum(sources.values()), len(self.routines))
        # The 43 that write neither are not interactable by the loader's clear.
        silent = [r for r in self.routines if r.interactable_source.startswith("no write")]
        self.assertEqual(len(silent), 43)
        self.assertTrue(all(not r.interactable for r in silent))

    def test_the_one_routine_that_changes_the_bit_while_it_runs(self):
        # `FieldObj_FellowPenguin` sets bit 3 in its init, then clears it once
        # `EventFlag_Penguin` is set -- once you have talked to it, it stops
        # answering and stops blocking. Reported, not flattened.
        changing = [r for r in self.routines if r.interactable_changes_at_runtime]
        self.assertEqual([r.symbol for r in changing], ["FellowPenguin"])
        self.assertTrue(changing[0].interactable)

    def test_the_academy_basement_monsters_are_not_talkable(self):
        # The live bug this extraction exists for: both bosses carry
        # `bclr #3`, so retail's probe skips them and never reaches their map's
        # dialogue tree.
        for symbol in ("Xanafalgue", "Igglanova"):
            with self.subTest(symbol=symbol):
                routine = self.by_symbol[symbol]
                self.assertFalse(routine.interactable)
                self.assertIn("bclr", routine.interactable_source)
        # An invisible block draws nothing and is still both a talk trigger and
        # a wall, so art-less does not mean interaction-less.
        block = self.by_symbol["InvisibleBlock"]
        self.assertTrue(block.interactable)
        self.assertFalse(block.builds_sprites)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    DISASM.exists(),
    "reference/ps4disasm is not checked out; clone it to run the oracle tests",
)
class TestInteractableAgainstTheDisassembly(unittest.TestCase):
    """The clone's init blocks agree about bit 3 for every routine it names."""

    def test_every_matched_init_block_agrees(self):
        data = read_rom(ROM)
        text = DISASM.read_text(errors="replace")
        # The clone spells the field both ways, `$2(a4)` and `render_flags(a4)`.
        setter = re.compile(r"^\tbset\t#3, (?:\$2|render_flags)\(a4\)", re.M)
        clearer = re.compile(r"^\tbclr\t#3, (?:\$2|render_flags)\(a4\)", re.M)
        bodies = {}
        pattern = re.compile(
            r"^(FieldObj_[A-Za-z0-9_]+|loc_[0-9A-F]+):\s*\n\tbset\t#7, \(a4\)\n"
            r"\tbne\.s\s+([.\w+]+)\s*\n",
            re.M,
        )
        for match in pattern.finditer(text):
            label, target = match.group(1), match.group(2)
            rest = text[match.end():]
            stop = (
                re.search(r"^\+", rest, re.M) if target.startswith("+")
                else re.search(rf"^{re.escape(target)}\b", rest, re.M)
            )
            bodies[label] = rest[:stop.start()] if stop else rest[:2000]

        checked = 0
        for routine in scan_field_objects(data):
            body = bodies.get(f"FieldObj_{routine.symbol}") or bodies.get(routine.symbol)
            if body is None:
                continue
            checked += 1
            with self.subTest(symbol=routine.symbol):
                clone_sets = bool(setter.search(body))
                clone_clears = bool(clearer.search(body))
                self.assertEqual(clone_sets, routine.interactable)
                if not clone_sets:
                    self.assertEqual(
                        clone_clears,
                        "bclr" in routine.interactable_source,
                    )
        # Seven routines the clone labels in a form this parser does not reach;
        # the rest is a full cross-check.
        self.assertEqual(checked, 215)


if __name__ == "__main__":
    unittest.main()
