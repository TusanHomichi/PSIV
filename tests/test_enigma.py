import hashlib
import struct
import tempfile
import unittest
from pathlib import Path

from psiv_tools.core import read_rom
from psiv_tools.enigma import (
    FLAG_H_FLIP,
    FLAG_V_FLIP,
    MASKS,
    WORD_H_FLIP,
    WORD_V_FLIP,
    EnigmaError,
    decompress,
    read_header,
    words_to_bytes,
)
from psiv_tools.gfx import (
    BATTLE_BG_ART_SYMBOLS,
    BATTLE_BG_PALETTE_COLORS,
    BATTLE_BG_TABLE,
    COLORS_PER_LINE,
    decode_palette,
    decode_tiles,
    decompress_art,
    _gap_bounds,
    _slice,
)
from psiv_tools.planes import (
    BATTLE_BG_ART_VRAM_TILE,
    BATTLE_BG_BASE_TILE,
    BATTLE_BG_COLUMNS,
    BATTLE_BG_EXTRA_LANDMARKS,
    BATTLE_BG_ROWS,
    CRAM_COLORS,
    KOSINSKI_ART,
    MAPPINGS,
    PLANE_BUFFER_WORDS,
    PLANE_WIDTH_CELLS,
    PlaneError,
    art_tiles,
    battle_cram,
    compose,
    decode_cell,
    decode_cells,
    decode_mapping,
    export_plane_pngs,
    extract_planes,
    named_cram,
    render,
    _battle_pointers,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"
MAPPING_DIR = REFERENCE / "plane mappings"
GRAPHICS_DIR = REFERENCE / "graphics"

# The disassembly stores every Enigma stream as an already-compressed
# `binclude` payload, so each file is the retail stream byte for byte and its
# length is the compressed length. That makes all 83 of them a length oracle
# for the decoder, ROM or no ROM.
#
# One is not a clean stream: `MapEni_TitleScrollingTextBG`'s file is 684 bytes
# but only its first 336 are Enigma. The exception is pinned here with its
# exact shape so a future clone update fails loudly instead of quietly
# widening the exception set.
MAPPING_FILE_EXCEPTIONS = {
    "title/Scrolling Text Background Enigma.bin": (684, 336),
}

# The four Kosinski title-portrait art blobs, matched by content the same way
# gfx.NEMESIS_ART's offsets were established.
KOSINSKI_ORACLES = {
    "ArtKos_RuneTitlePortrait": "title/Rune Portrait Kosinski.bin",
    "ArtKos_RikaTitlePortrait": "title/Rika Portrait Kosinski.bin",
    "ArtKos_WrenTitlePortrait": "title/Wren Portrait Kosinski.bin",
    "ArtKos_ChazTitlePortrait": "title/Chaz Portrait Kosinski.bin",
}

# sha256 of every decoded mapping's words, big-endian, in order.
DECODED_MAPPING_SHA256 = {
    "MapEni_SegaLogo": "04f16b453e1cabab53fb9fe28e704254672decedfa0d04a2b7043e88c37f871e",
    "MapEni_TitlePSTitle": "d9c0f7fbed4c2a60058b324c6de8369d28e7f928d41ed37e14b6d7d08726d020",
    "MapEni_TitleTheEndOfTheMillennium": "5009e1db85b740ce7abd74a713ad049025321ea95efd5885510cbe3b48ea54dc",
    "MapEni_TitleCopyrightText": "449d867d7f37baa6d4d4eadd6ec6bfc75787984ea2175df5cf37f9f12f98c342",
    "MapEni_PressStartButton": "233cc0bca3a73cf8ad5a43dfb2d044e0e8dea45cf85683fccc7d5d8f1913b196",
    "MapEni_TitleBarBGTopPart": "0e9c60177a40f007d44120335932df0502dba798a4a709c98907cdad272ea9df",
    "MapEni_TitleBarBGBottomPart": "f681bfe23ed576df675e30fb591c5e5420440fcb6353c39d8c5a6dedf0a28221",
    "MapEni_TitleBGLeftPart": "779a029b227003f2ed62331f222aa0e8662519abe0df096d79fc987c9552a362",
    "MapEni_TitleBGRightPart": "3a6006c941f29bc8a18eebe9395bda0ce95ef409fd1cad29468369a4a45f2d0e",
    "MapEni_GameStartMotaBG": "da40f7674ed35b71aa2d405f9f84e3acc306b8cf0441980267e78f7dd816037e",
    "MapEni_TitleScrollingTextBG": "f50168d7fdd3da1f421efb5163a61017f27c0d5424424603905c62497a82ed06",
    "MapEni_RuneTitlePortrait": "a5e045e638c9f65027a68f8e80ebe2c436f6ceb53647601d1634c4c7208de26a",
    "MapEni_RikaTitlePortrait": "8a373fe44c9a14cc4fddd0c41c993328ecf834b61445f141b8c97e259ae755ec",
    "MapEni_WrenTitlePortrait": "021ef0b971d668f0587b678d127b1cd309476272a4e79db612c0e9e8052200cc",
    "MapEni_ChazTitlePortrait": "863de16b18a0d1f1d5c733cc957735282303d3236b9cf85c39c408f1944aba9c",
    "MapEni_MotaDesertBattleBG": "a99f92d65bb430387ca47dace2554c54c9c2b08f54ee6dc98f48667e61ac0fa4",
    "MapEni_MotaBeachBattleBG": "77d1f29ac68b64f9fdb0a4cf098dd2ec66380b20f8e6e43d50e9b9322b7f1f8a",
    "MapEni_MotaGrassBattleBG": "04506fac19325461c0bfe58c52e04476379887f9f6206537ec0ba523382fed32",
    "MapEni_MotaSeaBattleBG": "60739669e0d75f2e3583864adc275930a690a7955ebe20ced2f081c733ae0b5f",
    "MapEni_DezoBattleBG": "ac2a41aa9c80cf37663b6a4c0e28baf43e237c245e8b1610890aa4b34f1b4d2c",
    "MapEni_PowerPlantsBattleBG": "1f9350dc18209986a0aeaea74818d7318fc35cde6f0647baf773576ccaee1acd",
    "MapEni_AcademyBasementBattleBG": "1e1de1ff5e2031e595902b2df3df3479d9465129b4e6de0649a287a6c1f28a19",
    "MapEni_LadeaTowerBattleBG": "4944af70bac00479ea23eb43f465d3f901c4a95cdeff965a60ccbe7ae990b653",
    "MapEni_ZioFortAirCastleBattleBG": "cd152d52e8c856b4cbae8b37ac6e79e3cbe567935aef02f16677285ff00b6fe2",
    "MapEni_GaruberkTwBattleBG": "e19a8137cfac64aa8cf2dfc171fd1be23b5c57f45d291094e7bd8aa6369fe951",
    "MapEni_TonoeBasementBattleBG": "8ba7159a133bb2e0392c9d55d714214973b058008a6094edd6ccfc48281035d8",
    "MapEni_MonsenCavePassagewayBattleBG": "594e6c4fcb2a572ab577144e7986d9249646ec53b9efcd0ecd094045bec0bfc8",
    "MapEni_IslandAndElsydeonCaveBattleBG": "dd7a0a9d89744f99a554e3ee0ede240f57e91f52f3501d8399656e134dc270df",
    "MapEni_RykrosBattleBG": "aeea759df5b84fa2585165dcc36dcaa32577abab17e1a5cf78b3958bcabcd5ff",
    "MapEni_ZemaBattleBG": "9907465767d02faf4389040e8165133cae2f7a4ecf0adf3038982e5bb2a55093",
    "MapEni_ReshelBattleBG": "b740253a84408de358da6029f0700eec94818c9fdef1882538b196992d37e841",
    "MapEni_RykrosTowersBattleBG": "527fb5f60dc467c385f3b2d96cc561c4e2c6d5b43183a6061d0ac10f6f596508",
    "MapEni_TheEdgeBattleBG": "ca310734d00467cbc1284f1e118666987c2aa44bdbfbd4ac39106661292e46b1",
    "MapEni_CarnivorousTreesBattleBG": "088b0770e714bb02716f65ea44bacdb01ad399ceea8fb2eef34eeef1dc7ab87d",
    "MapEni_DarkForce1BattleBG": "04666439330c7767050214cff8d00e623e481ec47332744d3cdf20ef6e9f44c3",
}

# Composed pixel buffers: CRAM indices, row-major, before any palette is
# applied. A wrong tile order, a missed flip or a dropped palette line all
# change these.
COMPOSED_SHA256 = {
    "MapEni_ChazTitlePortrait": (112, 224, "2a03fadf0277ef52b545da81a1fb3fd56e9b2697711ddb6ec12c4933e104babb"),
    "MapEni_TitlePSTitle": (136, 104, "64dd2669b4dd038960b2f31665f79c8cae5f848be0c2d70ed916a339ec7eec20"),
}
COMPOSED_BATTLE_SHA256 = {
    0x00: (512, 192, "f0b86a200e089b41c37867012db651d1b9822a3d22c8ce710107ce32f1f961c3"),
    0x02: (512, 192, "7a667d4a93abc7a3ada35ade5dc7bb431b02d829d2a4fefc0827021399e8ad10"),
}
MOTA_DESERT_PNG_SHA256 = "3f0d7eca21ddb181f10b743181b7e514a4f9f6073e93465eccfa438d5d7a63ba"

# Six of the call sites sit inside `if revision=0 ... else ... endif` in the
# fork, which assembles the `revision=0` side. These are the base tiles that
# branch would pass; none of them may appear in the cartridge next to its
# mapping's address.
REVISION_ZERO_BASE_TILES = {
    "MapEni_TitleTheEndOfTheMillennium": 0x0090,
    "MapEni_TitleCopyrightText": 0x20C8,
    "MapEni_PressStartButton": 0x40D1,
    "MapEni_TitleBarBGTopPart": 0x60DC,
    "MapEni_TitleBarBGBottomPart": 0x60DC,
    "MapEni_TitleBGLeftPart": 0x60DC,
    "MapEni_TitleBGRightPart": 0x60DC,
}


def call_site(mapping_offset: int, base_tile: int) -> bytes:
    """`lea (MapEni_x).l, a0` followed by `move.w #base, d0`.

    Every one of the fifteen named mappings is set up by exactly this pair of
    instructions, so searching for it recovers both the mapping's ROM offset
    and its base tile from the cartridge's own machine code -- no disassembly,
    no revision guessing.
    """
    return (
        b"\x41\xf9" + struct.pack(">I", mapping_offset)
        + b"\x30\x3c" + struct.pack(">H", base_tile)
    )


class EnigmaWriter:
    """Minimal Enigma encoder, used only to build test fixtures.

    It writes the six header bytes and then packs format entries and inline
    values most significant bit first, which is the order `EniDecomp` unpacks
    them in: its 16-bit window is only a sliding view of one continuous
    MSB-first bitstream. Format entries are two mode bits plus a four-bit count
    for the copy-word modes and three plus four for the inline modes, and an
    inline value is the declared flag bits (V then H) followed by
    `inline_bits` value bits.

    `finish()` writes the `111 1111` terminator, pads the last byte with zero
    bits and rounds to an even length -- which is exactly the length
    `EniDecomp_Done` reports, so fixtures can be concatenated.
    """

    def __init__(self, inline_bits: int, flags: int = 0, incremental: int = 0, literal: int = 0) -> None:
        assert 1 <= inline_bits <= 16
        assert flags & ~(FLAG_V_FLIP | FLAG_H_FLIP) == 0
        self.inline_bits = inline_bits
        self.flags = flags
        self.incremental = incremental
        self.literal = literal
        self.bits: list[int] = []

    def _put(self, value: int, count: int) -> None:
        assert 0 <= value < (1 << count)
        for shift in range(count - 1, -1, -1):
            self.bits.append((value >> shift) & 1)

    def _value(self, value: int, v_flip: bool = False, h_flip: bool = False) -> None:
        if self.flags & FLAG_V_FLIP:
            self._put(int(v_flip), 1)
        if self.flags & FLAG_H_FLIP:
            self._put(int(h_flip), 1)
        self._put(value & MASKS[self.inline_bits - 1], self.inline_bits)

    def incremental_run(self, words: int) -> "EnigmaWriter":
        """Mode 00: copy the incremental word `words` times, incrementing it."""
        self._put(0b00, 2)
        self._put(words - 1, 4)
        return self

    def literal_run(self, words: int) -> "EnigmaWriter":
        """Mode 01: copy the literal word `words` times."""
        self._put(0b01, 2)
        self._put(words - 1, 4)
        return self

    def inline_run(self, words: int, value: int, mode: int = 0b100, **flags) -> "EnigmaWriter":
        """Modes 100/101/110: one inline value, copied / incremented / decremented."""
        assert mode in (0b100, 0b101, 0b110)
        self._put(mode, 3)
        self._put(words - 1, 4)
        self._value(value, **flags)
        return self

    def inline_values(self, values, **flags) -> "EnigmaWriter":
        """Mode 111: a fresh inline value per word. A count of $F would end the
        stream, so at most fifteen values fit in one entry."""
        assert 1 <= len(values) <= 0xF
        self._put(0b111, 3)
        self._put(len(values) - 1, 4)
        for value in values:
            self._value(value, **flags)
        return self

    def finish(self) -> bytes:
        self._put(0b111, 3)
        self._put(0x0F, 4)
        bits = list(self.bits)
        while len(bits) % 8:
            bits.append(0)
        out = bytearray(
            [self.inline_bits, self.flags]
        )
        out += self.incremental.to_bytes(2, "big") + self.literal.to_bytes(2, "big")
        for i in range(0, len(bits), 8):
            byte = 0
            for bit in bits[i:i + 8]:
                byte = (byte << 1) | bit
            out.append(byte)
        if len(out) % 2:
            out.append(0)
        return bytes(out)


def padded(stream: bytes) -> bytes:
    """`EniDecomp` reads up to two bytes of lookahead it then rewinds over, so
    a stream decoded on its own needs somewhere for those reads to land. In the
    ROM the next structure provides them."""
    return stream + b"\x00" * 8


def parse_png_chunks(data: bytes):
    pos = 8
    while pos < len(data):
        length = struct.unpack(">I", data[pos:pos + 4])[0]
        kind = data[pos + 4:pos + 8]
        yield kind, data[pos + 8:pos + 8 + length]
        pos += 12 + length


class TestEnigmaHeader(unittest.TestCase):
    def test_header_fields(self):
        stream = EnigmaWriter(9, FLAG_V_FLIP | FLAG_H_FLIP, 0x0123, 0x4567).finish()
        header = read_header(stream, 0)
        self.assertEqual(header.inline_bits, 9)
        self.assertEqual(header.incremental_word, 0x0123)
        self.assertEqual(header.literal_word, 0x4567)
        self.assertTrue(header.uses_v_flip)
        self.assertTrue(header.uses_h_flip)
        self.assertEqual(header.flag_bits_per_value, 2)

    def test_base_tile_offsets_both_copy_words(self):
        """`adda.w a3, a2` and `adda.w a3, a4` -- the base applies to the
        header words too, not only to inline values."""
        stream = EnigmaWriter(4, 0, 0x0010, 0x0020).finish()
        header = read_header(stream, 0, base_tile=0x6400)
        self.assertEqual(header.incremental_word, 0x6410)
        self.assertEqual(header.literal_word, 0x6420)

    def test_copy_words_wrap_at_16_bits(self):
        stream = EnigmaWriter(4, 0, 0xFFF0, 0).finish()
        self.assertEqual(read_header(stream, 0, base_tile=0x0020).incremental_word, 0x0010)

    def test_unsupported_flag_bits_are_rejected(self):
        """EniDecomp tests only the V and H bits. A stream declaring P or CC
        would leave those bits in the stream and desynchronise everything after
        the first inline value, so it cannot be decoded, not merely rendered
        oddly."""
        stream = bytearray(EnigmaWriter(4, FLAG_H_FLIP).finish())
        for bit in (0x04, 0x08, 0x10):
            stream[1] = FLAG_H_FLIP | bit
            with self.subTest(flag=bit):
                with self.assertRaises(EnigmaError):
                    read_header(bytes(stream), 0)

    def test_inline_bit_count_must_index_the_mask_table(self):
        stream = bytearray(EnigmaWriter(4, 0).finish())
        for bits in (0, 17, 0x80, 0xFF):
            stream[0] = bits
            with self.subTest(bits=bits):
                with self.assertRaises(EnigmaError):
                    read_header(bytes(stream), 0)

    def test_truncated_header(self):
        with self.assertRaises(EnigmaError):
            read_header(b"\x04\x00\x00", 0)


class TestEnigmaDecoder(unittest.TestCase):
    def decode(self, writer: EnigmaWriter, base_tile: int = 0):
        stream = writer.finish()
        words, consumed = decompress(padded(stream), 0, base_tile)
        self.assertEqual(
            consumed, len(stream),
            "EniDecomp_Done must rewind to exactly the end of the stream",
        )
        return words

    def test_incremental_copy_word_increments(self):
        writer = EnigmaWriter(4, 0, incremental=0x0005)
        writer.incremental_run(3)
        self.assertEqual(self.decode(writer), [5, 6, 7])

    def test_literal_copy_word_does_not(self):
        writer = EnigmaWriter(4, 0, literal=0x1234)
        writer.literal_run(4)
        self.assertEqual(self.decode(writer), [0x1234] * 4)

    def test_incremental_word_carries_across_entries(self):
        """`a2` lives in the header, not in the entry, so two separate runs
        continue one sequence."""
        writer = EnigmaWriter(4, 0, incremental=0x0100)
        writer.incremental_run(2).literal_run(1).incremental_run(2)
        self.assertEqual(self.decode(writer), [0x100, 0x101, 0, 0x102, 0x103])

    def test_inline_modes(self):
        writer = EnigmaWriter(8, 0)
        writer.inline_run(2, 0x42, mode=0b100)
        writer.inline_run(3, 0x50, mode=0b101)
        writer.inline_run(2, 0x60, mode=0b110)
        writer.inline_values([1, 2, 3])
        self.assertEqual(
            self.decode(writer),
            [0x42, 0x42, 0x50, 0x51, 0x52, 0x60, 0x5F, 1, 2, 3],
        )

    def test_repeat_count_is_a_dbf_count(self):
        """A count field of `n` means `n + 1` words -- the `dbf` runs once more
        than the value it counts down."""
        writer = EnigmaWriter(4, 0, literal=7)
        writer.literal_run(16)  # count field $F, the maximum for mode 01
        self.assertEqual(self.decode(writer), [7] * 16)

    def test_mode_111_count_f_is_the_terminator_not_sixteen_values(self):
        writer = EnigmaWriter(4, 0)
        writer.inline_values(list(range(1, 16)))  # 15 is the most that fits
        self.assertEqual(self.decode(writer), list(range(1, 16)))
        with self.assertRaises(AssertionError):
            EnigmaWriter(4, 0).inline_values(list(range(16)))

    def test_six_bit_entries_do_not_eat_the_seventh_bit(self):
        """The routine always reads 7 bits and only then decides the entry was
        6, giving the seventh back. A long chain of 6-bit entries desynchronises
        immediately if that bit is consumed."""
        writer = EnigmaWriter(4, 0, incremental=1, literal=0xFF)
        expected = []
        value = 1
        for i in range(12):
            if i % 2:
                writer.literal_run(1)
                expected.append(0xFF)
            else:
                writer.incremental_run(2)
                expected += [value, value + 1]
                value += 2
        self.assertEqual(self.decode(writer), expected)

    def test_flag_bits_set_the_flip_bits(self):
        writer = EnigmaWriter(4, FLAG_V_FLIP | FLAG_H_FLIP)
        writer.inline_run(1, 1, v_flip=False, h_flip=False)
        writer.inline_run(1, 2, v_flip=True, h_flip=False)
        writer.inline_run(1, 3, v_flip=False, h_flip=True)
        writer.inline_run(1, 4, v_flip=True, h_flip=True)
        self.assertEqual(
            self.decode(writer),
            [1, WORD_V_FLIP | 2, WORD_H_FLIP | 3, WORD_V_FLIP | WORD_H_FLIP | 4],
        )

    def test_only_declared_flags_consume_bits(self):
        """With only the H flag declared, a single bit precedes each value; a
        decoder that reads two would return garbage from here on."""
        writer = EnigmaWriter(4, FLAG_H_FLIP)
        writer.inline_run(1, 1, h_flip=True)
        writer.inline_run(1, 2, h_flip=False)
        writer.inline_values([3, 4], h_flip=True)
        self.assertEqual(
            self.decode(writer),
            [WORD_H_FLIP | 1, 2, WORD_H_FLIP | 3, WORD_H_FLIP | 4],
        )

    def test_base_tile_is_added_to_inline_values(self):
        writer = EnigmaWriter(8, FLAG_H_FLIP, incremental=0x10, literal=0x20)
        writer.incremental_run(1).literal_run(1).inline_run(1, 0x30, h_flip=True)
        self.assertEqual(
            self.decode(writer, base_tile=0x6400),
            [0x6410, 0x6420, 0x6400 | WORD_H_FLIP | 0x30],
        )

    def test_inline_values_straddle_the_window(self):
        """With 13 value bits and 2 flag bits, values very quickly stop fitting
        in what is left of the 16-bit window, which is the path that borrows
        from the next byte and then reloads the window wholesale."""
        values = [(0x0100 + i * 137) & MASKS[12] for i in range(15)]
        writer = EnigmaWriter(13, FLAG_V_FLIP | FLAG_H_FLIP)
        writer.inline_values(values, v_flip=True)
        self.assertEqual(self.decode(writer), [WORD_V_FLIP + v for v in values])

    def test_every_inline_width_round_trips(self):
        """Widths 1..15 with a flag bit declared, and 16 without one. A 16-bit
        value plus flag bits can drive the shift counter below zero, which is a
        stream `EniDecomp` cannot decode; the ROM's widest is 9."""
        for bits in range(1, 17):
            with self.subTest(bits=bits):
                mask = MASKS[bits - 1]
                values = [(0x5A5A * i) & mask for i in range(1, 12)]
                flags = FLAG_H_FLIP if bits < 16 else 0
                writer = EnigmaWriter(bits, flags)
                writer.inline_values(values, h_flip=bool(flags))
                flip = WORD_H_FLIP if flags else 0
                self.assertEqual(
                    self.decode(writer), [(flip + v) & 0xFFFF for v in values]
                )

    def test_the_base_and_flags_are_added_to_the_value_not_or_ed(self):
        """`add.w d3, d1`, not `or.w`. With the ROM's 4..9 inline bits the two
        fields never overlap, but the addition is what the routine does and a
        value wide enough to reach bit 11 carries into the flip bits."""
        writer = EnigmaWriter(12, FLAG_H_FLIP)
        writer.inline_values([0x800, 0x001], h_flip=True)
        self.assertEqual(self.decode(writer), [WORD_H_FLIP + 0x800, WORD_H_FLIP + 1])

        writer = EnigmaWriter(8, 0, incremental=0x00F0)
        writer.incremental_run(1).inline_run(1, 0x20)
        self.assertEqual(self.decode(writer, base_tile=0x0030), [0x0120, 0x0050])

    def test_sixteen_bit_values_with_flags_are_rejected(self):
        writer = EnigmaWriter(16, FLAG_V_FLIP | FLAG_H_FLIP)
        writer.inline_values([0xFFFF] * 4, v_flip=True, h_flip=True)
        with self.assertRaises(EnigmaError):
            decompress(padded(writer.finish()), 0)

    def test_streams_can_be_chained(self):
        """The consumed length is what lets one mapping follow another with no
        pointer in between, which is how the ROM stores them."""
        first = EnigmaWriter(6, FLAG_H_FLIP, incremental=3)
        first.incremental_run(5).inline_values([1, 2], h_flip=True)
        second = EnigmaWriter(9, 0, literal=0x777)
        second.literal_run(2).inline_run(3, 0x123, mode=0b101)
        blob = first.finish() + second.finish()

        words, consumed = decompress(padded(blob), 0)
        self.assertEqual(words, [3, 4, 5, 6, 7, WORD_H_FLIP | 1, WORD_H_FLIP | 2])
        self.assertEqual(consumed, len(first.finish()))
        rest, _ = decompress(padded(blob), consumed)
        self.assertEqual(rest, [0x777, 0x777, 0x123, 0x124, 0x125])

    def test_consumed_length_is_always_even(self):
        for words in range(1, 40):
            writer = EnigmaWriter(7, 0, literal=1)
            writer.literal_run(min(words, 16))
            stream = writer.finish()
            _, consumed = decompress(padded(stream), 0)
            with self.subTest(words=words):
                self.assertEqual(consumed % 2, 0)
                self.assertEqual(consumed, len(stream))

    def test_max_words_is_enforced(self):
        writer = EnigmaWriter(4, 0, literal=1)
        writer.literal_run(16).literal_run(16)
        with self.assertRaises(EnigmaError):
            decompress(padded(writer.finish()), 0, max_words=20)

    def test_truncated_stream_raises(self):
        writer = EnigmaWriter(8, 0, literal=1)
        writer.literal_run(16)
        stream = writer.finish()
        with self.assertRaises(EnigmaError):
            decompress(stream[:-2], 0)

    def test_offset_outside_input(self):
        with self.assertRaises(EnigmaError):
            decompress(b"\x04\x00\x00\x00\x00\x00\x00\x00", 99)

    def test_words_to_bytes_rejects_non_words(self):
        self.assertEqual(words_to_bytes([0x1234, 0x00FF]), b"\x12\x34\x00\xff")
        with self.assertRaises(EnigmaError):
            words_to_bytes([0x10000])


class TestPatternNameWords(unittest.TestCase):
    def test_field_layout(self):
        cell = decode_cell(0xE7FF)
        self.assertEqual(cell.tile, 0x7FF)
        self.assertEqual(cell.palette_line, 3)
        self.assertTrue(cell.priority)
        self.assertFalse(cell.v_flip)
        self.assertFalse(cell.h_flip)

        cell = decode_cell(0x1800)
        self.assertEqual(cell.tile, 0)
        self.assertTrue(cell.v_flip)
        self.assertTrue(cell.h_flip)
        self.assertEqual(cell.palette_line, 0)
        self.assertFalse(cell.priority)

    def test_rejects_non_words(self):
        with self.assertRaises(PlaneError):
            decode_cell(0x10000)


class TestComposition(unittest.TestCase):
    """Composition against hand-built patterns, no ROM involved."""

    def setUp(self):
        # Tile 0: a top-left corner marker. Tile 1: a solid fill of index 2.
        corner = bytearray(64)
        corner[0] = 1
        corner[7] = 3
        self.tiles = [bytes(corner), bytes([2] * 64)]

    def test_cells_are_placed_row_major(self):
        cells = decode_cells([0, 1, 1, 0])
        width, height, pixels = compose(cells, 2, self.tiles)
        self.assertEqual((width, height), (16, 16))
        self.assertEqual(pixels[0], 1)  # tile 0 top-left
        self.assertEqual(pixels[8], 2)  # tile 1 begins at x=8
        self.assertEqual(pixels[8 * 16], 2)  # second row starts with tile 1
        self.assertEqual(pixels[8 * 16 + 8], 1)  # and ends with tile 0

    def test_h_flip_mirrors_within_the_cell(self):
        _, _, plain = compose(decode_cells([0]), 1, self.tiles)
        _, _, flipped = compose(decode_cells([WORD_H_FLIP]), 1, self.tiles)
        self.assertEqual(plain[:8], bytes([1, 0, 0, 0, 0, 0, 0, 3]))
        self.assertEqual(flipped[:8], bytes([3, 0, 0, 0, 0, 0, 0, 1]))

    def test_v_flip_reverses_the_rows(self):
        _, _, flipped = compose(decode_cells([WORD_V_FLIP]), 1, self.tiles)
        self.assertEqual(flipped[:8], bytes(8))
        self.assertEqual(flipped[56:64], bytes([1, 0, 0, 0, 0, 0, 0, 3]))

    def test_palette_line_offsets_the_index_but_not_index_zero(self):
        """Index 0 is transparent in every CRAM line, so it always resolves to
        the backdrop colour rather than to colour 0 of the cell's line."""
        _, _, pixels = compose(decode_cells([0x6000]), 1, self.tiles)
        self.assertEqual(pixels[0], 3 * COLORS_PER_LINE + 1)
        self.assertEqual(pixels[1], 0)
        self.assertEqual(pixels[7], 3 * COLORS_PER_LINE + 3)

    def test_art_vram_tile_shifts_the_lookup(self):
        cells = decode_cells([0x101, 0x100])
        _, _, pixels = compose(cells, 2, self.tiles, art_vram_tile=0x100)
        self.assertEqual(pixels[0], 2)
        self.assertEqual(pixels[8], 1)

    def test_a_cell_outside_the_art_blob_is_an_error(self):
        with self.assertRaises(PlaneError):
            compose(decode_cells([2]), 1, self.tiles)
        with self.assertRaises(PlaneError):
            compose(decode_cells([0]), 1, self.tiles, art_vram_tile=1)

    def test_cell_count_must_fill_whole_rows(self):
        with self.assertRaises(PlaneError):
            compose(decode_cells([0, 0, 0]), 2, self.tiles)

    def test_render_uses_the_whole_cram(self):
        cram = [(i, i, i) for i in range(CRAM_COLORS)]
        image = render(decode_cells([0, 0x6000]), 2, self.tiles, cram)
        chunks = dict(parse_png_chunks(image))
        self.assertEqual(struct.unpack(">II", chunks[b"IHDR"][:8]), (16, 8))
        self.assertEqual(len(chunks[b"PLTE"]), CRAM_COLORS * 3)
        self.assertNotIn(b"tRNS", chunks)

    def test_render_rejects_a_partial_cram(self):
        with self.assertRaises(PlaneError):
            render(decode_cells([0]), 1, self.tiles, [(0, 0, 0)] * COLORS_PER_LINE)


@unittest.skipUnless(
    MAPPING_DIR.is_dir(), f"disassembly oracle not present at {REFERENCE}"
)
class TestEnigmaAgainstDisassembly(unittest.TestCase):
    """Every Enigma `binclude` payload in the disassembly, decoded.

    No decompressed Enigma source exists to compare words against, but the file
    lengths are the compressed lengths, and `EniDecomp_Done` reports a length
    of its own. Requiring the two to agree on 82 real streams -- across every
    inline width from 4 to 9 bits and every flag combination -- is the
    acceptance criterion for the decoder.
    """

    def test_every_payload_consumes_exactly_its_own_length(self):
        files = sorted(MAPPING_DIR.glob("*/*.bin"))
        self.assertGreaterEqual(len(files), 80)
        checked = 0
        for path in files:
            name = f"{path.parent.name}/{path.name}"
            blob = path.read_bytes()
            expected = MAPPING_FILE_EXCEPTIONS.get(name, (len(blob), len(blob)))[1]
            with self.subTest(name):
                words, consumed = decompress(padded(blob), 0)
                self.assertEqual(consumed, expected)
                self.assertGreater(len(words), 0)
            checked += 1
        self.assertEqual(checked, len(files))

    def test_the_scrolling_text_payload_bundles_unrelated_data(self):
        """`MapEni_TitleScrollingTextBG`'s binclude is 684 bytes, but the
        Enigma stream ends after 336 and the remaining 348 bytes are sprite
        records. Pinned so a clone update that splits the file fails loudly."""
        blob = (MAPPING_DIR / "title" / "Scrolling Text Background Enigma.bin").read_bytes()
        self.assertEqual(len(blob), 684)
        words, consumed = decompress(padded(blob), 0)
        self.assertEqual(consumed, 336)
        self.assertEqual(len(words), 640)
        with self.assertRaises(EnigmaError):
            read_header(blob, consumed)

    def test_the_character_battle_sprite_mappings_decode_too(self):
        """43 more Enigma streams this slice does not otherwise touch: the
        in-battle character poses. They are decoded here purely as decoder
        coverage; binding them to their art is a later slice."""
        files = sorted((MAPPING_DIR / "characters").glob("*.bin"))
        self.assertEqual(len(files), 43)
        for path in files:
            with self.subTest(path.name):
                words, consumed = decompress(padded(path.read_bytes()), 0)
                self.assertEqual(consumed, path.stat().st_size)
                self.assertIn(len(words), (36, 42))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestPlanesFromRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.result = extract_planes(cls.data)
        cls.named = {r["label"]: r for r in cls.result["named"]}
        cls.battle = {}
        for entry in cls.result["battle_backgrounds"]["entries"]:
            cls.battle.setdefault(entry["plane_mapping"]["label"], entry["plane_mapping"])

    def test_thirty_five_mappings_are_located(self):
        self.assertEqual(len(self.named), len(MAPPINGS))
        self.assertEqual(len(self.battle), 20)
        self.assertEqual(self.result["distinct_mappings"], 35)
        self.assertEqual(self.result["total_cells_decoded"], 35436)

    def test_named_mappings_consume_their_documented_length(self):
        for spec in MAPPINGS:
            with self.subTest(spec["label"]):
                record = self.named[spec["label"]]
                self.assertEqual(record["compressed_size"], spec["compressed_size"])
                self.assertEqual(
                    record["rom_end_exclusive"],
                    f"0x{spec['rom_offset'] + spec['compressed_size']:06X}",
                )

    def test_named_mapping_cell_counts_match_their_consumers(self):
        """`PlaneMapToRAM`'s d1/d2 at the call site predict the cell count
        exactly, which is what fixes each mapping's width and height."""
        for spec in MAPPINGS:
            with self.subTest(spec["label"]):
                record = self.named[spec["label"]]
                self.assertEqual(record["cell_count"], spec["columns"] * spec["rows"])

    def test_every_cell_resolves_inside_its_art_blob(self):
        for label, record in {**self.named, **self.battle}.items():
            with self.subTest(label):
                first = int(record["art_vram_tile"], 16)
                low, high = record["tile_range"]
                self.assertGreaterEqual(low, first)
                self.assertLess(high, first + record["art_tile_count"])

    def test_a_mapping_spans_the_art_blob_it_is_paired_with(self):
        """Thirty-one of the thirty-five reach the blob's very last pattern,
        which is what proves the base tile: shift it by one and the range walks
        off the end. The four exceptions are the title-background parts, four
        mappings that carve up one 414-pattern blob between them."""
        shared = {
            "MapEni_TitleBarBGTopPart",
            "MapEni_TitleBarBGBottomPart",
            "MapEni_TitleBGLeftPart",
        }
        for label, record in {**self.named, **self.battle}.items():
            with self.subTest(label):
                self.assertEqual(record["spans_art_exactly"], label not in shared)

    def test_battle_mappings_fill_the_top_of_plane_b(self):
        for label, record in self.battle.items():
            with self.subTest(label):
                self.assertEqual(record["cell_count"], BATTLE_BG_COLUMNS * BATTLE_BG_ROWS)
                self.assertEqual(record["columns"], PLANE_WIDTH_CELLS)
                self.assertLess(record["cell_count"], PLANE_BUFFER_WORDS)
                self.assertEqual(record["base_tile"], "0x0000")
                self.assertEqual(record["palette_lines"], [0])
                self.assertEqual(record["cells_high_priority"], 0)

    def test_battle_mappings_are_bounded_by_the_pointer_table(self):
        """Each background's mapping sits between its own art and the next
        background's art, so the pointer table bounds both halves without
        anything outside the cartridge. Three mappings end the table's
        coverage; those ends are named landmarks instead."""
        art_ptrs, map_ptrs, _ = _battle_pointers(self.data)
        landmarks = sorted(set(art_ptrs + map_ptrs + list(BATTLE_BG_EXTRA_LANDMARKS)))
        internal = 0
        for record in self.battle.values():
            end = int(record["rom_end_exclusive"], 16)
            with self.subTest(record["label"]):
                self.assertIn(end, landmarks)
                if end in art_ptrs:
                    internal += 1
        self.assertEqual(internal, 17)
        self.assertEqual(len(BATTLE_BG_EXTRA_LANDMARKS), 3)

    def test_battle_art_immediately_precedes_its_mapping(self):
        art_ptrs, map_ptrs, _ = _battle_pointers(self.data)
        landmarks = art_ptrs + map_ptrs + list(BATTLE_BG_EXTRA_LANDMARKS)
        bounds = _gap_bounds(landmarks, len(self.data), art_ptrs)
        for index in range(BATTLE_BG_TABLE["entry_count"]):
            with self.subTest(index=index):
                self.assertEqual(art_ptrs[index] + bounds[art_ptrs[index]], map_ptrs[index])

    def test_decoded_mappings_are_pinned(self):
        for label, digest in DECODED_MAPPING_SHA256.items():
            with self.subTest(label):
                record = self.named.get(label) or self.battle[label]
                self.assertEqual(record["decoded_sha256"], digest)
        self.assertEqual(len(DECODED_MAPPING_SHA256), 35)

    def test_flag_bits_declared_match_the_flips_used(self):
        """A stream that declares neither flag must produce no flipped cells,
        and one that declares a flag generally uses it. Both directions catch a
        misread header byte."""
        for label, record in {**self.named, **self.battle}.items():
            with self.subTest(label):
                flags = record["flag_bits"]
                if not flags["v_flip"]:
                    self.assertEqual(record["cells_v_flipped"], 0)
                if not flags["h_flip"]:
                    self.assertEqual(record["cells_h_flipped"], 0)

    def test_palette_lines_come_from_the_base_tile(self):
        """No mapping in the ROM carries per-cell palette bits -- EniDecomp
        cannot decode them -- so every cell's line is the base tile's."""
        for spec in MAPPINGS:
            with self.subTest(spec["label"]):
                record = self.named[spec["label"]]
                self.assertEqual(record["palette_lines"], [(spec["base_tile"] >> 13) & 3])

    def test_base_tile_low_bits_are_the_art_vram_tile(self):
        for spec in MAPPINGS:
            with self.subTest(spec["label"]):
                self.assertEqual(spec["base_tile"] & 0x7FF, spec["art_vram_tile"])

    def test_the_cartridges_own_code_pins_every_offset_and_base_tile(self):
        """Each mapping is set up by `lea (mapping).l, a0` / `move.w #base, d0`,
        and that exact eight-byte pair occurs once in the 3 MiB image. Nothing
        in `MAPPINGS` outside the dimensions is taken on the disassembly's
        word."""
        for spec in MAPPINGS:
            with self.subTest(spec["label"]):
                pattern = call_site(spec["rom_offset"], spec["base_tile"])
                self.assertEqual(self.data.count(pattern), 1)

    def test_the_cartridge_runs_the_forks_non_zero_revision_branch(self):
        """`reference/` is configured as a Grand Cross build and assembles the
        `revision=0` side of these call sites. The cartridge disagrees: the
        base tile that branch would pass appears nowhere next to the mapping."""
        for label, base_tile in REVISION_ZERO_BASE_TILES.items():
            spec = next(s for s in MAPPINGS if s["label"] == label)
            with self.subTest(label):
                self.assertNotEqual(spec["base_tile"], base_tile)
                self.assertEqual(
                    self.data.count(call_site(spec["rom_offset"], base_tile)), 0
                )

    def test_the_revision_branch_is_confirmed_by_the_decoded_cell_counts(self):
        """Independently of the immediates: the `revision=0` branch copies
        17x1 cells for Press Start and 12x1 for the copyright text, and the
        streams decode to 18 and 17 cells."""
        self.assertEqual(self.named["MapEni_PressStartButton"]["cell_count"], 18)
        self.assertEqual(self.named["MapEni_TitleCopyrightText"]["cell_count"], 17)

    def test_the_revision_branch_is_confirmed_by_the_vram_layout(self):
        """And a third way: the `revision=0` base tile $60DC would point the
        title background mappings seven tiles below where the art is loaded."""
        record = self.named["MapEni_TitleBGRightPart"]
        first = int(record["art_vram_tile"], 16)
        self.assertEqual(first, 0x0E3)
        self.assertEqual(record["tile_range"], [first, first + record["art_tile_count"] - 1])
        words, wrong = decode_mapping(
            self.data, 0x1D236C, "MapEni_TitleBGRightPart", base_tile=0x60DC,
            compressed_size=594,
        )
        self.assertEqual(wrong["tile_range"], [0x0DC, 0x0DC + record["art_tile_count"] - 1])
        tiles, _ = art_tiles(self.data, "ArtNem_TitleBackground")
        with self.assertRaises(PlaneError):
            compose(decode_cells(words), 26, tiles, art_vram_tile=first)

    def test_kosinski_title_portrait_art(self):
        for spec in KOSINSKI_ART:
            with self.subTest(spec["label"]):
                tiles, record = art_tiles(self.data, spec["label"])
                self.assertEqual(len(tiles), spec["tile_count"])
                self.assertEqual(record["compression"], "kosinski")
                end = int(record["rom_end_exclusive"], 16)
                self.assertLessEqual(end, 0x2F478E)  # the first mapping that follows

    def test_composed_pixel_buffers_are_pinned(self):
        for label, (width, height, digest) in COMPOSED_SHA256.items():
            spec = next(s for s in MAPPINGS if s["label"] == label)
            with self.subTest(label):
                words, _ = decode_mapping(
                    self.data, spec["rom_offset"], label, base_tile=spec["base_tile"],
                    compressed_size=spec["compressed_size"],
                )
                tiles, _ = art_tiles(self.data, spec["art"])
                w, h, pixels = compose(
                    decode_cells(words), spec["columns"], tiles, spec["art_vram_tile"]
                )
                self.assertEqual((w, h), (width, height))
                self.assertEqual(hashlib.sha256(pixels).hexdigest(), digest)

    def test_composed_battle_backgrounds_are_pinned(self):
        art_ptrs, map_ptrs, pal_ptrs = _battle_pointers(self.data)
        landmarks = art_ptrs + map_ptrs + list(BATTLE_BG_EXTRA_LANDMARKS)
        map_bounds = _gap_bounds(landmarks, len(self.data), map_ptrs)
        art_bounds = _gap_bounds(landmarks, len(self.data), art_ptrs)
        for index, (width, height, digest) in COMPOSED_BATTLE_SHA256.items():
            with self.subTest(BATTLE_BG_ART_SYMBOLS[index]):
                art, _ = decompress_art(
                    self.data, art_ptrs[index], "art",
                    compressed_size=art_bounds[art_ptrs[index]],
                )
                words, _ = decode_mapping(
                    self.data, map_ptrs[index], "map",
                    base_tile=BATTLE_BG_BASE_TILE,
                    compressed_size=map_bounds[map_ptrs[index]],
                )
                w, h, pixels = compose(
                    decode_cells(words), BATTLE_BG_COLUMNS, decode_tiles(art),
                    BATTLE_BG_ART_VRAM_TILE,
                )
                self.assertEqual((w, h), (width, height))
                self.assertEqual(hashlib.sha256(pixels).hexdigest(), digest)
                self.assertLess(max(pixels), COLORS_PER_LINE)

    def test_the_mota_desert_png_is_pinned(self):
        """The end-to-end acceptance criterion: pointer table, Nemesis art,
        Enigma mapping, flips and the battle palette, all the way to pixels."""
        art_ptrs, map_ptrs, pal_ptrs = _battle_pointers(self.data)
        landmarks = art_ptrs + map_ptrs + list(BATTLE_BG_EXTRA_LANDMARKS)
        map_bounds = _gap_bounds(landmarks, len(self.data), map_ptrs)
        art_bounds = _gap_bounds(landmarks, len(self.data), art_ptrs)
        art, _ = decompress_art(
            self.data, art_ptrs[0], "art", compressed_size=art_bounds[art_ptrs[0]]
        )
        words, _ = decode_mapping(
            self.data, map_ptrs[0], "map", base_tile=BATTLE_BG_BASE_TILE,
            compressed_size=map_bounds[map_ptrs[0]],
        )
        raw = _slice(self.data, pal_ptrs[0], BATTLE_BG_PALETTE_COLORS * 2, "palette")
        cram = battle_cram(decode_palette(raw))
        self.assertEqual(cram[0], (0, 0, 0))
        self.assertEqual(len(cram), CRAM_COLORS)
        image = render(
            decode_cells(words), BATTLE_BG_COLUMNS, decode_tiles(art), cram,
            BATTLE_BG_ART_VRAM_TILE,
        )
        self.assertEqual(hashlib.sha256(image).hexdigest(), MOTA_DESERT_PNG_SHA256)

    def test_named_palettes_are_whole_cram_images(self):
        for label in ("Pal_TitleScreen", "Pal_TitleScrollingText", "Pal_TitleCharPortraits"):
            with self.subTest(label):
                self.assertEqual(len(named_cram(self.data, label)), CRAM_COLORS)

    def test_export_writes_one_png_per_mapping(self):
        with tempfile.TemporaryDirectory() as directory:
            written = export_plane_pngs(self.data, directory)
            self.assertEqual(len(written), len(MAPPINGS) + BATTLE_BG_TABLE["entry_count"])
            by_label = {w["label"]: w for w in written}
            chaz = by_label["MapEni_ChazTitlePortrait"]
            self.assertEqual(chaz["size"], [112, 224])
            chunks = dict(parse_png_chunks(Path(chaz["path"]).read_bytes()))
            self.assertEqual(struct.unpack(">II", chunks[b"IHDR"][:8]), (112, 224))
            desert = by_label["MapEni_MotaDesertBattleBG_MotaDesert"]
            self.assertEqual(desert["size"], [512, 192])
            self.assertEqual(desert["sha256"], MOTA_DESERT_PNG_SHA256)

    def test_extract_returns_no_pixels(self):
        """Same rule as gfx: metadata is committable, Sega's artwork is not."""
        blob = repr(self.result)
        self.assertNotIn("pixels", blob)
        for record in self.result["named"]:
            self.assertNotIn("words", record)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    MAPPING_DIR.is_dir() and GRAPHICS_DIR.is_dir(),
    f"disassembly oracle not present at {REFERENCE}",
)
class TestPlanesAgainstDisassembly(unittest.TestCase):
    """The offsets in `planes.MAPPINGS` come from matching the disassembly's
    payloads against the retail image; this is that match, re-run."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)

    def test_mapping_payloads_occur_exactly_once_at_the_documented_offset(self):
        names = {
            "MapEni_SegaLogo": "sega/Sega Enigma.bin",
            "MapEni_TitlePSTitle": "title/Phantasy Star Title Enigma.bin",
            "MapEni_TitleTheEndOfTheMillennium": "title/The End of the Millennium Enigma.bin",
            "MapEni_TitleCopyrightText": "title/Copyright Text Enigma.bin",
            "MapEni_PressStartButton": "title/Press Start Button Enigma.bin",
            "MapEni_TitleBarBGTopPart": "title/Bar Background Top Part Enigma.bin",
            "MapEni_TitleBarBGBottomPart": "title/Bar Background Bottom Part Enigma.bin",
            "MapEni_TitleBGLeftPart": "title/Background Left Part Enigma.bin",
            "MapEni_TitleBGRightPart": "title/Background Right Part Enigma.bin",
            "MapEni_GameStartMotaBG": "events/Game Start Mota BG Enigma.bin",
            "MapEni_RuneTitlePortrait": "title/Rune Portrait Enigma.bin",
            "MapEni_RikaTitlePortrait": "title/Rika Portrait Enigma.bin",
            "MapEni_WrenTitlePortrait": "title/Wren Portrait Enigma.bin",
            "MapEni_ChazTitlePortrait": "title/Chaz Portrait Enigma.bin",
        }
        specs = {s["label"]: s for s in MAPPINGS}
        for label, filename in names.items():
            with self.subTest(label):
                blob = (MAPPING_DIR / filename).read_bytes()
                spec = specs[label]
                self.assertEqual(len(blob), spec["compressed_size"])
                self.assertEqual(self.data.count(blob), 1)
                self.assertEqual(self.data.find(blob), spec["rom_offset"])

    def test_the_scrolling_text_mapping_is_the_head_of_its_payload(self):
        spec = next(s for s in MAPPINGS if s["label"] == "MapEni_TitleScrollingTextBG")
        blob = (MAPPING_DIR / "title" / "Scrolling Text Background Enigma.bin").read_bytes()
        self.assertEqual(self.data.find(blob), spec["rom_offset"])
        self.assertEqual(
            self.data[spec["rom_offset"]:spec["rom_offset"] + spec["compressed_size"]],
            blob[:spec["compressed_size"]],
        )

    def test_battle_mapping_payloads_sit_where_the_pointer_table_says(self):
        art_ptrs, map_ptrs, _ = _battle_pointers(self.data)
        located = 0
        for path in sorted((MAPPING_DIR / "battle").glob("*.bin")):
            blob = path.read_bytes()
            with self.subTest(path.name):
                self.assertEqual(self.data.count(blob), 1)
                self.assertIn(self.data.find(blob), map_ptrs)
            located += 1
        self.assertEqual(located, 20)
        self.assertEqual(len(set(map_ptrs)), 20)

    def test_kosinski_portrait_payloads_occur_exactly_once(self):
        specs = {s["label"]: s for s in KOSINSKI_ART}
        for label, filename in KOSINSKI_ORACLES.items():
            with self.subTest(label):
                blob = (GRAPHICS_DIR / filename).read_bytes()
                self.assertEqual(self.data.count(blob), 1)
                self.assertEqual(self.data.find(blob), specs[label]["rom_offset"])


if __name__ == "__main__":
    unittest.main()
