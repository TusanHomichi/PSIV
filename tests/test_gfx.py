import hashlib
import struct
import unittest
import zlib
from pathlib import Path

from psiv_tools import png
from psiv_tools.core import read_rom
from psiv_tools.gfx import (
    BATTLE_BG_ART_SYMBOLS,
    BATTLE_BG_PALETTE_COLORS,
    BATTLE_BG_TABLE,
    COLORS_PER_LINE,
    DIALOGUE_PORTRAIT_SYMBOLS,
    DIALOGUE_PORTRAIT_TABLE,
    NEMESIS_ART,
    PALETTES,
    PORTRAIT_COLUMNS,
    PORTRAIT_TILES,
    RAW_ART,
    GraphicsError,
    GPGX_RGB565_RAMP,
    compose_sheet,
    decode_color,
    decode_palette,
    decode_tile,
    decode_tiles,
    decompress_art,
    expand_channel,
    export_art_pngs,
    extract_graphics,
    read_long,
    render_sheet,
)
from psiv_tools.nemesis import NemesisError, build_code_table, decompress, read_header

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"
GRAPHICS_DIR = REFERENCE / "graphics"
PALETTE_DIR = REFERENCE / "palettes"

# The disassembly stores each of these art blobs as an already-compressed
# `binclude` payload, so the file is the retail stream byte for byte. Finding
# it exactly once in the ROM is what fixes the offset; feeding the retail bytes
# back through the decoder then has to consume exactly that many bytes.
BINCLUDE_ORACLES = {
    "ArtNem_SegaLogo": "sega/Sega Nemesis.bin",
    "ArtNem_TitlePSTitle": "title/Phantasy Star Title Nemesis.bin",
    "ArtNem_TitleTheEndOfTheMillennium": "title/The End of the Millennium Nemesis.bin",
    "ArtNem_PressStartButton": "title/Intro Press Start Button text Nemesis.bin",
    "ArtNem_TitleCopyrightText": "title/Copyright Text Nemesis.bin",
    "ArtNem_TitleBackground": "title/Background Nemesis.bin",
    "ArtNem_GameStartMotaBG": "events/Game Start Mota BG Nemesis.bin",
    "ArtNem_TitleScrollingTextBG": "title/Scrolling Text Background Nemesis.bin",
    "ArtNem_SaveBlockGlowingCursor": "title/Save Block Glowing Cursor Nemesis.bin",
    "ArtNem_WindowTiles": "windows/Tiles Nemesis.bin",
    "ArtNem_Font": "font/Font Nemesis.bin",
    "ArtNem_CreditFont": "font/Credits Nemesis.bin",
}

RAW_ART_ORACLES = {
    "Art_DialogueFont": "font/Dialogue Font.bin",
}


class NemesisWriter:
    """Minimal Nemesis encoder used only to build test fixtures.

    It writes the code table the way `Nem_BuildCodeTable` reads it -- a
    palette-index marker with its sign bit set, then one descriptor plus one
    code byte per entry, then `$FF` -- and packs code bits most significant
    first, which is the order `Nem_ProcessCompressedData` unpacks them in.

    Codes must be prefix-free and must not begin with six 1 bits, since that
    pattern is reserved for inline data. `finish()` pads the last byte with 0
    bits, matching what a real encoder has to do.
    """

    def __init__(self, tile_count: int, xor_mode: bool = False) -> None:
        self.tile_count = tile_count
        self.xor_mode = xor_mode
        self.entries: list[tuple[int, int, int, int]] = []  # pal, repeat, code, length
        self.codes: dict[tuple[int, int], str] = {}
        self.bits: list[int] = []

    def define(self, palette_index: int, repeat: int, code: str) -> "NemesisWriter":
        assert 0 <= palette_index <= 0xF and 0 <= repeat <= 7
        assert 1 <= len(code) <= 8 and set(code) <= {"0", "1"}
        assert not code.startswith("111111"), "six leading 1 bits mean inline data"
        self.entries.append((palette_index, repeat, int(code, 2), len(code)))
        self.codes[(palette_index, repeat)] = code
        return self

    def emit(self, palette_index: int, repeat: int) -> "NemesisWriter":
        """Emit a previously defined code."""
        self._push(self.codes[(palette_index, repeat)])
        return self

    def emit_inline(self, palette_index: int, repeat: int) -> "NemesisWriter":
        """Emit the 6-bit inline marker plus its 7-bit payload."""
        assert 0 <= palette_index <= 0xF and 0 <= repeat <= 7
        self._push("111111")
        self._push(format((repeat << 4) | palette_index, "07b"))
        return self

    def _push(self, code: str) -> None:
        self.bits.extend(int(b) for b in code)

    def finish(self) -> bytes:
        out = bytearray()
        header = self.tile_count | (0x8000 if self.xor_mode else 0)
        out += struct.pack(">H", header)
        for palette_index, repeat, code, length in self.entries:
            out.append(0x80 | palette_index)
            out.append((repeat << 4) | length)
            out.append(code)
        out.append(0xFF)
        bits = self.bits + [0] * (-len(self.bits) % 8)
        for i in range(0, len(bits), 8):
            out.append(int("".join(str(b) for b in bits[i:i + 8]), 2))
        # The decoder primes a 16-bit window and may read one byte of lookahead
        # past the last it needs, so a real stream is always followed by
        # something. Pad so a fixture can be decoded standalone.
        out += b"\x00\x00\x00"
        return bytes(out)


def parse_png_chunks(data: bytes) -> list[tuple[bytes, bytes]]:
    """Re-parse a PNG, verifying every chunk CRC."""
    if data[:8] != png.PNG_SIGNATURE:
        raise AssertionError("missing PNG signature")
    chunks = []
    pos = 8
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        payload = data[pos + 8:pos + 8 + length]
        (crc,) = struct.unpack(">I", data[pos + 8 + length:pos + 12 + length])
        expected = zlib.crc32(kind + payload) & 0xFFFFFFFF
        if crc != expected:
            raise AssertionError(f"bad CRC on {kind!r}: {crc:08X} != {expected:08X}")
        chunks.append((kind, payload))
        pos += 12 + length
    return chunks


class TestNemesisDecoder(unittest.TestCase):
    """Decoder unit tests. These need no ROM and no disassembly."""

    def test_header_splits_count_and_xor_flag(self):
        self.assertEqual(read_header(b"\x00\x24")[:3], (0x24, False, 0x24 * 8))
        self.assertEqual(read_header(b"\x80\x24")[:3], (0x24, True, 0x24 * 8))
        self.assertEqual(read_header(b"\x00\x01").decompressed_size, 32)

    def test_code_table_expands_short_codes(self):
        # One 2-bit code 01 covers every lookahead byte of the form 01xxxxxx.
        stream = NemesisWriter(1).define(5, 0, "01").finish()
        table, pos = build_code_table(stream, 2)
        self.assertEqual(pos, 2 + 3 + 1)
        self.assertEqual({table[i] for i in range(0x40, 0x80)}, {(2, 0, 5)})
        self.assertNotIn(0x3F, table)
        self.assertNotIn(0x80, table)

    def test_code_table_keeps_repeat_and_palette_apart(self):
        stream = NemesisWriter(1).define(0xB, 6, "0011").finish()
        table, _ = build_code_table(stream, 2)
        self.assertEqual(table[0b00110000], (4, 6, 0xB))

    def test_single_tile_of_one_colour(self):
        writer = NemesisWriter(1).define(7, 7, "0")
        for _ in range(8):  # eight runs of eight pixels
            writer.emit(7, 7)
        decoded, _ = decompress(writer.finish())
        self.assertEqual(decoded, b"\x77" * 32)

    def test_repeat_count_spans_row_boundaries(self):
        """A run keeps going into the next row; only the row flush resets."""
        writer = NemesisWriter(1).define(3, 7, "0").define(0, 7, "10")
        writer.emit(3, 7).emit(3, 7).emit(3, 7).emit(3, 7)
        for _ in range(4):
            writer.emit(0, 7)
        decoded, _ = decompress(writer.finish())
        self.assertEqual(decoded, b"\x33" * 16 + b"\x00" * 16)

    def test_pixels_fill_a_row_high_nybble_first(self):
        """Nem_PCD_WritePixel shifts up by a nybble, so pixel 0 is the high one."""
        writer = NemesisWriter(1)
        for index in range(8):
            writer.define(index, 0, format(index, "03b"))
        for _ in range(8):  # eight identical rows
            for index in range(8):
                writer.emit(index, 0)
        decoded, _ = decompress(writer.finish())
        self.assertEqual(decoded[:4], b"\x01\x23\x45\x67")
        self.assertEqual(decode_tile(decoded)[:8], bytes(range(8)))

    def test_inline_data_is_six_ones_then_seven_bits(self):
        writer = NemesisWriter(1).define(0, 0, "0")
        for _ in range(8):
            writer.emit_inline(0xD, 7)
        decoded, _ = decompress(writer.finish())
        self.assertEqual(decoded, b"\xDD" * 32)

    def test_inline_and_table_codes_interleave(self):
        writer = NemesisWriter(1).define(1, 3, "0")
        for _ in range(8):
            writer.emit(1, 3).emit_inline(2, 3)
        decoded, _ = decompress(writer.finish())
        self.assertEqual(decoded, b"\x11\x11\x22\x22" * 8)

    def test_xor_mode_accumulates_across_every_row(self):
        """d2 is never reset, so row N of the output is the XOR of rows 0..N."""
        plain = NemesisWriter(2).define(1, 7, "0").define(0, 7, "10")
        xored = NemesisWriter(2, xor_mode=True).define(1, 7, "0").define(0, 7, "10")
        for writer in (plain, xored):
            for row in range(16):
                writer.emit(1, 7) if row % 2 == 0 else writer.emit(0, 7)
        decoded, _ = decompress(xored.finish())
        raw, _ = decompress(plain.finish())
        expected = bytearray()
        running = 0
        for i in range(0, len(raw), 4):
            running ^= int.from_bytes(raw[i:i + 4], "big")
            expected += running.to_bytes(4, "big")
        self.assertEqual(decoded, bytes(expected))
        # Every other source row is zero, so the accumulator alternates.
        self.assertEqual(decoded[:4], b"\x11\x11\x11\x11")
        self.assertEqual(decoded[4:8], b"\x11\x11\x11\x11")

    def test_output_length_comes_from_the_header_alone(self):
        writer = NemesisWriter(3).define(4, 7, "0")
        for _ in range(64):  # far more runs than three tiles need
            writer.emit(4, 7)
        decoded, consumed = decompress(writer.finish())
        self.assertEqual(len(decoded), 3 * 32)
        # It stopped early, so it did not read the whole fixture.
        self.assertLess(consumed, len(writer.finish()))

    def test_zero_tile_header_decodes_to_nothing(self):
        decoded, consumed = decompress(NemesisWriter(0).define(0, 0, "0").finish())
        self.assertEqual(decoded, b"")
        self.assertEqual(consumed, 2 + 3 + 1)

    def test_offset_is_honoured(self):
        writer = NemesisWriter(1).define(9, 7, "0")
        for _ in range(8):
            writer.emit(9, 7)
        stream = writer.finish()
        decoded, consumed = decompress(b"\xDE\xAD" + stream, 2)
        self.assertEqual(decoded, b"\x99" * 32)
        self.assertEqual(decompress(stream)[1], consumed)

    def test_undefined_code_raises(self):
        writer = NemesisWriter(1).define(1, 7, "0")
        writer._push("10")  # never defined
        with self.assertRaises(NemesisError):
            decompress(writer.finish())

    def test_truncated_stream_raises(self):
        writer = NemesisWriter(4).define(1, 7, "0")
        writer.emit(1, 7)
        with self.assertRaises(NemesisError):
            decompress(writer.finish())

    def test_unterminated_code_table_raises(self):
        with self.assertRaises(NemesisError):
            decompress(b"\x00\x01\x80\x01\x00")


class TestTileAndPaletteDecoding(unittest.TestCase):
    def test_channel_expansion_matches_the_receipt_backed_ramp(self):
        self.assertEqual(tuple(expand_channel(v) for v in range(8)), GPGX_RGB565_RAMP["r"])
        self.assertEqual(
            tuple(expand_channel(v, "g") for v in range(8)), GPGX_RGB565_RAMP["g"]
        )
        self.assertEqual(expand_channel(0), 0)
        self.assertEqual(expand_channel(7), 238)

    def test_cram_word_layout(self):
        self.assertEqual(decode_color(0x0000)["rgb"], [0, 0, 0])
        self.assertEqual(decode_color(0x0EEE)["rgb"], [238, 238, 238])
        self.assertEqual(decode_color(0x000E)["rgb"], [238, 0, 0])
        self.assertEqual(decode_color(0x00E0)["rgb"], [0, 238, 0])
        self.assertEqual(decode_color(0x0E00)["rgb"], [0, 0, 238])
        self.assertEqual(decode_color(0x0246)["hex"], "#624420")
        self.assertEqual(decode_color(0x0246)["levels"], {"r": 3, "g": 2, "b": 1})

    def test_unused_cram_bits_are_flagged_not_dropped(self):
        self.assertNotIn("unused_bits_set", decode_color(0x0EEE))
        self.assertTrue(decode_color(0xF111)["unused_bits_set"])

    def test_palette_needs_whole_words(self):
        self.assertEqual(len(decode_palette(b"\x00\x00\x0E\xEE")), 2)
        with self.assertRaises(GraphicsError):
            decode_palette(b"\x00")

    def test_tile_unpacks_two_pixels_per_byte(self):
        tile = decode_tile(bytes([0x12, 0x34, 0x56, 0x78] + [0] * 28))
        self.assertEqual(tile[:8], bytes([1, 2, 3, 4, 5, 6, 7, 8]))
        self.assertEqual(len(tile), 64)

    def test_bad_art_length_raises(self):
        with self.assertRaises(GraphicsError):
            decode_tiles(b"\x00" * 33)

    def test_sheet_layout_is_row_major(self):
        tiles = [bytes([n]) * 64 for n in range(5)]
        width, height, pixels = compose_sheet(tiles, columns=2)
        self.assertEqual((width, height), (16, 24))
        self.assertEqual(pixels[0], 0)
        self.assertEqual(pixels[8], 1)  # second column of the first row
        self.assertEqual(pixels[16 * 8], 2)  # first column of the second row
        self.assertEqual(pixels[16 * 16], 4)
        self.assertEqual(pixels[16 * 16 + 8], 0)  # unused cell stays index 0


class TestPngWriter(unittest.TestCase):
    def test_indexed_png_round_trips(self):
        palette = [(0, 0, 0), (255, 0, 0), (0, 255, 0), (0, 0, 255)]
        pixels = bytes([0, 1, 2, 3, 3, 2, 1, 0])
        data = png.encode_indexed(4, 2, pixels, palette)
        chunks = parse_png_chunks(data)
        self.assertEqual([kind for kind, _ in chunks], [b"IHDR", b"PLTE", b"IDAT", b"IEND"])

        width, height, depth, color_type, comp, filt, interlace = struct.unpack(
            ">IIBBBBB", chunks[0][1]
        )
        self.assertEqual((width, height, depth, color_type), (4, 2, 8, png.COLOR_TYPE_INDEXED))
        self.assertEqual((comp, filt, interlace), (0, 0, 0))
        self.assertEqual(chunks[1][1], b"\x00\x00\x00\xff\x00\x00\x00\xff\x00\x00\x00\xff")

        raw = zlib.decompress(chunks[2][1])
        self.assertEqual(raw, b"\x00" + pixels[:4] + b"\x00" + pixels[4:])

    def test_transparency_chunk_is_truncated_after_the_last_entry(self):
        palette = [(0, 0, 0)] * 4
        data = png.encode_indexed(2, 1, b"\x00\x01", palette, transparent=(1,))
        chunks = dict(parse_png_chunks(data))
        self.assertEqual(chunks[b"tRNS"], b"\xff\x00")

    def test_rgb_png_round_trips(self):
        pixels = bytes([255, 0, 0, 0, 255, 0, 0, 0, 255, 9, 9, 9])
        data = png.encode_rgb(2, 2, pixels)
        chunks = dict(parse_png_chunks(data))
        self.assertEqual(struct.unpack(">IIB", chunks[b"IHDR"][:9]), (2, 2, 8))
        self.assertEqual(chunks[b"IHDR"][9], png.COLOR_TYPE_RGB)
        raw = zlib.decompress(chunks[b"IDAT"])
        self.assertEqual(raw, b"\x00" + pixels[:6] + b"\x00" + pixels[6:])

    def test_pixel_index_outside_the_palette_raises(self):
        with self.assertRaises(png.PngError):
            png.encode_indexed(1, 1, b"\x05", [(0, 0, 0)])

    def test_wrong_sized_pixel_buffer_raises(self):
        with self.assertRaises(png.PngError):
            png.encode_indexed(4, 2, b"\x00" * 7, [(0, 0, 0)])

    def test_rendered_tile_sheet_is_a_valid_png(self):
        tiles = [bytes(range(16)) * 4 for _ in range(4)]
        chunks = dict(parse_png_chunks(render_sheet(tiles, columns=2)))
        self.assertEqual(struct.unpack(">II", chunks[b"IHDR"][:8]), (16, 16))
        self.assertEqual(len(chunks[b"PLTE"]), COLORS_PER_LINE * 3)
        self.assertEqual(chunks[b"tRNS"], b"\x00")


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestGraphicsFromRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.result = extract_graphics(cls.data)
        cls.art_by_label = {a["label"]: a for a in cls.result["nemesis_art"]}

    def test_every_named_blob_ends_where_the_map_says(self):
        """Decode must stop inside the documented stream, give or take the one
        lookahead byte NemDecomp reads before it notices it is finished."""
        for spec in NEMESIS_ART:
            with self.subTest(spec["label"]):
                record = self.art_by_label[spec["label"]]
                self.assertEqual(record["compressed_size"], spec["compressed_size"])
                self.assertIn(record["consumed"] - spec["compressed_size"], (0, 1))

    def test_tile_count_predicts_the_decompressed_size(self):
        for record in self.result["nemesis_art"]:
            with self.subTest(record["label"]):
                self.assertEqual(record["decompressed_size"], record["tile_count"] * 32)

    def test_pinned_blob_metadata(self):
        font = self.art_by_label["ArtNem_Font"]
        self.assertEqual(font["rom_offset"], "0x2A303A")
        self.assertEqual(font["tile_count"], 87)
        self.assertFalse(font["xor_mode"])
        self.assertEqual(font["header_word"], "0x0057")
        self.assertEqual(
            font["decompressed_sha256"],
            "ed9fe69a6a21a45ff08bf7ad74f3e390c941c548f33998004f706ae8c9bddf1c",
        )

        sega = self.art_by_label["ArtNem_SegaLogo"]
        self.assertEqual(sega["rom_offset"], "0x0411D0")
        self.assertEqual(sega["tile_count"], 70)
        self.assertTrue(sega["xor_mode"])
        self.assertEqual(sega["header_word"], "0x8046")
        self.assertEqual(
            sega["decompressed_sha256"],
            "bb477e197ab6b8c5e790050a90fc946cb37876ff66e56849a329954f42fe7f00",
        )

    def test_both_compression_modes_are_exercised(self):
        modes = {a["xor_mode"] for a in self.result["nemesis_art"]}
        self.assertEqual(modes, {True, False})

    def test_decoded_font_starts_with_a_capital_a(self):
        """The strongest cheap check that this really is art and not noise: the
        first pattern of ArtNem_Font is a legible letter A, and the sheet that
        follows is the Latin alphabet in order.

        The glyphs use colour index $F on a background of index $E, which is
        why this thresholds on $F rather than on "not transparent".
        """
        decompressed, _ = decompress_art(
            self.data, 0x2A303A, "ArtNem_Font", compressed_size=790
        )
        tiles = decode_tiles(decompressed)
        self.assertEqual(set(tiles[0]), {0xE, 0xF})
        rows = ["".join("#" if p == 0xF else "." for p in tiles[0][y * 8:y * 8 + 8])
                for y in range(8)]
        self.assertEqual(rows, [
            "...#....",
            "..###...",
            ".##.##..",
            "##...##.",
            "##...##.",
            "#######.",
            "##...##.",
            "........",
        ])
        # 'I' is narrow, 'O' is a closed ring: both distinguish real glyphs
        # from a plausible-looking decode.
        letter_i = tiles[ord("I") - ord("A")]
        self.assertEqual(
            "".join("#" if p == 0xF else "." for p in letter_i[:8]), "..####.."
        )

    def test_dialogue_portrait_table(self):
        portraits = self.result["dialogue_portraits"]
        self.assertEqual(portraits["table_rom_offset"], "0x06A4B0")
        self.assertEqual(portraits["entry_count"], 0x28)
        self.assertEqual(len(portraits["entries"]), 0x28)
        self.assertIsNone(portraits["entries"][0]["art"])
        chaz = portraits["entries"][1]
        self.assertEqual(chaz["symbol"], "Chaz")
        self.assertEqual(chaz["art"]["label"], "ArtNem_ChazDialPortrait")
        self.assertEqual(chaz["art"]["rom_offset"], "0x29BE5C")
        self.assertEqual(chaz["art"]["tile_count"], PORTRAIT_TILES)
        self.assertTrue(chaz["art"]["xor_mode"])
        self.assertEqual(
            chaz["art"]["decompressed_sha256"],
            "a87e9a934b75920b4ff6f1211305aa56e50494ad4b05969fb484d901a649212a",
        )
        # Every portrait is a 6x6 picture.
        for entry in portraits["entries"][1:]:
            self.assertEqual(entry["art"]["tile_count"], PORTRAIT_TILES)
        self.assertEqual(PORTRAIT_TILES, PORTRAIT_COLUMNS * PORTRAIT_COLUMNS)

    def test_portrait_table_stops_before_the_shopkeepers(self):
        """The disassembly appends seven shopkeeper entries at $28-$2E. The
        retail table ends at $27, and the longword after it is not a pointer."""
        base = DIALOGUE_PORTRAIT_TABLE["rom_offset"]
        count = DIALOGUE_PORTRAIT_TABLE["entry_count"]
        self.assertEqual(len(DIALOGUE_PORTRAIT_SYMBOLS), count)
        last = read_long(self.data, base + 4 * (count - 1))
        self.assertEqual(last, 0x2A28DA)  # Sekreas, index $27

        # The disassembly's next entry would be ArtNem_ShopkeeperDialPortrait1.
        # The retail longword there is 0x1A003E, which is not that address and
        # is not the start of a Nemesis stream at all.
        beyond = read_long(self.data, base + 4 * count)
        self.assertNotEqual(beyond, 0x29DBBE)
        with self.assertRaises(NemesisError):
            decompress_art(self.data, beyond, "not a portrait")

        # The seven shopkeeper portraits do exist, between Seth and Saya, but
        # nothing in this table points at them; the shop tables near 0x068000
        # do instead.
        pointers = {read_long(self.data, base + 4 * i) for i in range(count)}
        for shopkeeper in (0x29DBBE, 0x29DE1E, 0x29E0A4, 0x29E340, 0x29E576, 0x29E7A6, 0x29EA5C):
            self.assertNotIn(shopkeeper, pointers)
            reference = self.data.find(shopkeeper.to_bytes(4, "big"))
            self.assertTrue(0x068000 <= reference < 0x069000, hex(reference))
            art, _ = decompress_art(self.data, shopkeeper, "shopkeeper")
            self.assertEqual(len(art), PORTRAIT_TILES * 32)

    def test_battle_background_table(self):
        battle = self.result["battle_backgrounds"]
        self.assertEqual(battle["table_rom_offset"], "0x006ED4")
        self.assertEqual(len(battle["entries"]), 0x20)
        self.assertEqual(battle["distinct_art_blobs"], 20)
        first = battle["entries"][0]
        self.assertEqual(first["symbol"], "MotaDesert")
        self.assertEqual(first["art"]["label"], "ArtNem_MotaDesertBattleBG")
        self.assertEqual(first["art"]["rom_offset"], "0x2609C0")
        self.assertEqual(first["art"]["tile_count"], 348)
        self.assertEqual(first["palette"]["rom_offset"], "0x007054")
        self.assertEqual(len(first["palette"]["colors"]), BATTLE_BG_PALETTE_COLORS)
        self.assertEqual(first["palette"]["colors"][0]["hex"], "#ACAA8B")

        # Seven consecutive entries share one art blob under seven palettes.
        power = [e for e in battle["entries"] if e["art_symbol"] == "PowerPlants"]
        self.assertEqual([e["index"] for e in power], list(range(6, 13)))
        self.assertEqual(len({e["art"]["rom_offset"] for e in power}), 1)
        self.assertEqual(len({e["palette"]["rom_offset"] for e in power}), 7)
        self.assertEqual(len(BATTLE_BG_ART_SYMBOLS), BATTLE_BG_TABLE["entry_count"])

    def test_battle_art_ends_exactly_where_its_plane_mapping_begins(self):
        """A length oracle that needs nothing but the cartridge.

        `BattleBGArtPtrs` names both the art and the Enigma plane mapping, and
        the mapping is stored immediately after the art it belongs to, so the
        two pointers bracket the compressed stream. All twenty distinct blobs
        decode to exactly that length.
        """
        base = BATTLE_BG_TABLE["rom_offset"]
        stride = BATTLE_BG_TABLE["entry_size"]
        for index, entry in enumerate(self.result["battle_backgrounds"]["entries"]):
            with self.subTest(entry["symbol"]):
                art_offset = read_long(self.data, base + index * stride)
                mapping_offset = read_long(self.data, base + index * stride + 4)
                self.assertEqual(
                    entry["art"]["compressed_size"], mapping_offset - art_offset
                )
                self.assertEqual(entry["art"]["rom_end_exclusive"], f"0x{mapping_offset:06X}")

    def test_battle_palettes_are_a_contiguous_table(self):
        offsets = sorted(
            {int(e["palette"]["rom_offset"], 16) for e in self.result["battle_backgrounds"]["entries"]}
        )
        self.assertEqual(offsets[0], 0x7054)
        self.assertEqual(len(offsets), 31)
        for a, b in zip(offsets, offsets[1:]):
            self.assertEqual(b - a, BATTLE_BG_PALETTE_COLORS * 2)

    def test_named_palettes(self):
        palettes = {p["label"]: p for p in self.result["palettes"]["palettes"]}
        self.assertEqual(len(palettes), len(PALETTES))
        init = palettes["Pal_Init"]
        self.assertEqual(init["rom_offset"], "0x09F2BC")
        self.assertEqual(init["line_count"], 4)
        # Only line index 2 carries colours; the other three lines are black.
        for line in (0, 1, 3):
            self.assertTrue(all(c["raw"] == "0x0000" for c in init["lines"][line]["colors"]))
        self.assertEqual(
            [c["hex"] for c in init["lines"][2]["colors"][:6]],
            ["#000000", "#000000", "#ACAAAC", "#626562", "#EEAA62", "#AC6520"],
        )
        self.assertEqual(init["lines"][2]["colors"][15]["hex"], "#EEEEEE")
        self.assertEqual(
            palettes["Pal_Init_Line_3"]["lines"][0]["colors"],
            init["lines"][2]["colors"],
        )
        self.assertEqual(palettes["Pal_TitleScreen"]["rom_offset"], "0x1D29BC")
        self.assertEqual(palettes["Pal_TitleCharPortraits"]["rom_offset"], "0x2F4994")

    def test_no_named_palette_sets_bits_the_vdp_ignores(self):
        for palette in self.result["palettes"]["palettes"]:
            for line in palette["lines"]:
                for colour in line["colors"]:
                    self.assertNotIn("unused_bits_set", colour, palette["label"])

    def test_raw_art_matches_its_documented_length(self):
        raw = {a["label"]: a for a in self.result["raw_art"]}
        font = raw["Art_DialogueFont"]
        self.assertEqual(font["rom_offset"], "0x2A3542")
        self.assertEqual(font["tile_count"], 40)
        self.assertIsNone(font["compression"])
        # It ends exactly where the mirrored level-table block begins.
        self.assertEqual(font["rom_end_exclusive"], "0x2A3A42")

    def test_all_decompressed_output_is_pinned(self):
        """One digest over the digests of all 68 distinct blobs.

        Individually pinning every blob would be noise; this catches any change
        to any of them, and the per-blob pins above say which ones were checked
        against the rendered image by eye.
        """
        digests = [a["decompressed_sha256"] for a in self.result["nemesis_art"]]
        digests += [
            e["art"]["decompressed_sha256"]
            for e in self.result["dialogue_portraits"]["entries"] if e["art"]
        ]
        digests += [
            e["art"]["decompressed_sha256"]
            for e in self.result["battle_backgrounds"]["entries"]
        ]
        distinct = sorted(set(digests))
        self.assertEqual(len(distinct), 68)
        self.assertEqual(
            hashlib.sha256("\n".join(distinct).encode()).hexdigest(),
            "e3d8797053723adb2d2c8e5fd1afea3006d5469b83089b39fdecda8f1c8dff3c",
        )

    def test_totals(self):
        self.assertEqual(len(self.result["nemesis_art"]), len(NEMESIS_ART))
        self.assertEqual(len(self.result["raw_art"]), len(RAW_ART))
        self.assertEqual(self.result["dialogue_portraits"]["distinct_art_blobs"], 36)
        self.assertEqual(self.result["total_tiles_decoded"], 10434)

    def test_bad_offset_is_rejected_rather_than_decoded(self):
        with self.assertRaises(GraphicsError):
            decompress_art(self.data, 0x2A303A, "ArtNem_Font", compressed_size=100)

    def test_png_export_writes_readable_images(self):
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            written = export_art_pngs(self.data, tmp)
            self.assertEqual(len(written), 81)
            by_label = {w["label"]: w for w in written}
            font = by_label["ArtNem_Font"]
            self.assertEqual(font["palette"], "Pal_Init_Line_3")
            chunks = dict(parse_png_chunks(Path(font["path"]).read_bytes()))
            # 87 tiles at 16 across is six rows.
            self.assertEqual(struct.unpack(">II", chunks[b"IHDR"][:8]), (128, 48))
            portrait = by_label["ArtNem_ChazDialPortrait"]
            self.assertEqual(portrait["columns"], PORTRAIT_COLUMNS)
            chunks = dict(parse_png_chunks(Path(portrait["path"]).read_bytes()))
            self.assertEqual(struct.unpack(">II", chunks[b"IHDR"][:8]), (48, 48))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    GRAPHICS_DIR.is_dir() and PALETTE_DIR.is_dir(),
    f"disassembly oracle not present at {REFERENCE}",
)
class TestGraphicsAgainstDisassembly(unittest.TestCase):
    """Oracle checks against the public disassembly's own asset files."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)

    def test_binclude_payloads_occur_exactly_once_at_the_documented_offset(self):
        """Each `binclude` file is the retail compressed stream byte for byte.

        This is what fixes every offset in NEMESIS_ART: the payload appears
        once in the whole 3 MiB image, so the match cannot be a coincidence.
        """
        specs = {s["label"]: s for s in NEMESIS_ART}
        for label, filename in BINCLUDE_ORACLES.items():
            with self.subTest(label):
                blob = (GRAPHICS_DIR / filename).read_bytes()
                spec = specs[label]
                self.assertEqual(len(blob), spec["compressed_size"])
                offset = spec["rom_offset"]
                self.assertEqual(self.data[offset:offset + len(blob)], blob)
                self.assertEqual(self.data.count(blob), 1)

    def test_raw_art_payloads_match(self):
        specs = {s["label"]: s for s in RAW_ART}
        for label, filename in RAW_ART_ORACLES.items():
            with self.subTest(label):
                blob = (GRAPHICS_DIR / filename).read_bytes()
                spec = specs[label]
                self.assertEqual(len(blob), spec["size"])
                offset = spec["rom_offset"]
                self.assertEqual(self.data[offset:offset + len(blob)], blob)
                self.assertEqual(self.data.count(blob), 1)

    def test_battle_palette_payloads_match_the_pointer_table(self):
        """The palette pointers read out of BattleBGArtPtrs must land on the
        palette files the disassembly names for those same indices."""
        result = extract_graphics(self.data)
        for entry in result["battle_backgrounds"]["entries"]:
            with self.subTest(entry["index"]):
                name = entry["palette"]["label"]
                self.assertIsNotNone(name)
                candidates = [
                    p for p in (PALETTE_DIR / "battle").iterdir()
                    if p.stem.replace(" ", "").lower() in _palette_stems(name)
                ]
                self.assertEqual(len(candidates), 1, f"{name}: {candidates}")
                self.assertEqual(
                    bytes.fromhex(entry["palette"]["raw_hex"]), candidates[0].read_bytes()
                )

    def _match_binclude(self, subdirectory: str, offset: int) -> bytes:
        """Find the one `binclude` payload that sits at `offset` in the ROM."""
        matches = [
            path for path in sorted((GRAPHICS_DIR / subdirectory).iterdir())
            if self.data[offset:offset + path.stat().st_size] == path.read_bytes()
        ]
        self.assertEqual(len(matches), 1, f"0x{offset:06X}: {[p.name for p in matches]}")
        return matches[0].read_bytes()

    def test_every_portrait_stream_is_the_documented_length(self):
        """Match each portrait blob to its `binclude` payload by content, so no
        label-to-filename guess is involved, then require the decoder to stop
        at that length."""
        portraits = extract_graphics(self.data)["dialogue_portraits"]
        seen = set()
        for entry in portraits["entries"]:
            art = entry["art"]
            if art is None or art["rom_offset"] in seen:
                continue
            seen.add(art["rom_offset"])
            with self.subTest(art["label"]):
                offset = int(art["rom_offset"], 16)
                blob = self._match_binclude("portraits", offset)
                self.assertIn(art["consumed"] - len(blob), (0, 1))
                self.assertEqual(art["tile_count"], PORTRAIT_TILES)
        self.assertEqual(len(seen), 36)

    def test_every_battle_background_stream_is_the_documented_length(self):
        """The ROM-derived compressed sizes must equal the disassembly's."""
        battle = extract_graphics(self.data)["battle_backgrounds"]
        seen = set()
        for entry in battle["entries"]:
            art = entry["art"]
            if art["rom_offset"] in seen:
                continue
            seen.add(art["rom_offset"])
            with self.subTest(art["label"]):
                blob = self._match_binclude("battle", int(art["rom_offset"], 16))
                self.assertEqual(art["compressed_size"], len(blob))
        self.assertEqual(len(seen), 20)

    def test_decompressed_output_is_stable(self):
        """Pinned digests for the two blobs proven by eye: the font renders as
        the Latin alphabet and the Sega logo renders as the Sega logo."""
        for offset, size, digest in (
            (0x2A303A, 790, "ed9fe69a6a21a45ff08bf7ad74f3e390c941c548f33998004f706ae8c9bddf1c"),
            (0x0411D0, 1004, "bb477e197ab6b8c5e790050a90fc946cb37876ff66e56849a329954f42fe7f00"),
        ):
            with self.subTest(hex(offset)):
                decompressed, _ = decompress_art(self.data, offset, "blob", compressed_size=size)
                self.assertEqual(hashlib.sha256(decompressed).hexdigest(), digest)


# Palette file names in the disassembly do not map to labels mechanically
# (`Pal_DezoBattleBG1` is "Dezolis Background 1.bin"), so the oracle test above
# accepts any of a few spellings rather than inventing a rule.
_PALETTE_NAME_OVERRIDES = {
    "Pal_Dezo1BattleBG": {"dezolisbackground1"},
    "Pal_Dezo2BattleBG": {"dezolisbackground2"},
    "Pal_MotaDesertBattleBG": {"motaviadesertbackground"},
    "Pal_MotaBeachBattleBG": {"motaviabeachbackground"},
    "Pal_MotaGrassBattleBG": {"motaviagrassbackground"},
    "Pal_MotaSeaBattleBG": {"motaviaseabackground"},
    "Pal_BioPlantBattleBG": {"bioplantbackground"},
    "Pal_WreckageBattleBG": {"wreckagebackground"},
    "Pal_PlateSysBattleBG": {"platesystembackground"},
    "Pal_ClimCenterBattleBG": {"climatecenterbackground"},
    "Pal_WeaponPlantBattleBG": {"weaponplantbackground"},
    "Pal_VahalFortBattleBG": {"vahalfortbackground"},
    "Pal_PlateSysF1BattleBG": {"platesystemf1background"},
    "Pal_AcademyBasementBattleBG": {"piataacademybasementbackground"},
    "Pal_LadeaTowerBattleBG": {"ladeatowerbackground"},
    "Pal_ZioFortBattleBG": {"ziofortbackground"},
    "Pal_AirCastleBattleBG": {"aircastlebackground"},
    "Pal_GaruberkTwBattleBG": {"garuberktowerbackground"},
    "Pal_TonoeBasementBattleBG": {"tonoebasementbackground"},
    "Pal_MonsenAndIslandCaveBattleBG": {"monsenandislandcavebackground"},
    "Pal_PassagewayBattleBG": {"passagewaybackground"},
    "Pal_ElsydeonCaveBattleBG": {"elsydeoncavebackground"},
    "Pal_RykrosBattleBG": {"rykrosbackground"},
    "Pal_ZemaBattleBG": {"zemabackground"},
    "Pal_ReshelBattleBG": {"reshelbackground"},
    "Pal_CourageTwBattleBG": {"couragetowerbackground"},
    "Pal_StrengthTwBattleBG": {"strengthtowerbackground"},
    "Pal_AngerTwBattleBG": {"angertowerbackground"},
    "Pal_TheEdgeBattleBG": {"theedgebackground"},
    "Pal_CarnivorousTreesBattleBG": {"carnivoroustreesbackground"},
    "Pal_DarkForce1BattleBG": {"darkforce1background"},
}


def _palette_stems(label: str) -> set[str]:
    return _PALETTE_NAME_OVERRIDES[label]


if __name__ == "__main__":
    unittest.main()
