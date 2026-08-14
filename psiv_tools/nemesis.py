"""Nemesis decompression, transcribed from the game's own decompressor.

The reference implementation is `NemDecomp` / `NemDecomp_Main` in the public
disassembly (`ps4.asm`). Every branch below mirrors one branch of that 68000
routine so that the Python decoder cannot quietly disagree with the cartridge:

    NemDecomp_Main:             header word -> pattern count, XOR flag in the
                                bit shifted out by `lsl.w #1`; `lsl.w #2` then
                                turns it into a count of 8-pixel rows
    Nem_BuildCodeTable:         prefix-free code table, expanded so every
                                8-bit lookahead value has an entry
    Nem_ProcessCompressedData:  read the next 8 bits; six leading 1s mean
                                inline data, otherwise it is a table code
    Nem_PCD_InlineData:         6 marker bits + 7 payload bits (3-bit repeat
                                count, 4-bit palette index)
    Nem_PCD_WritePixel:         nybbles are shifted into a longword, high
                                nybble first, and flushed one row at a time
    Nem_PCD_WriteRowToRAM/_XOR: normal mode writes the row; XOR mode writes a
                                running XOR of every row so far

Three details were taken from the routine rather than assumed:

- The output length is fixed by the header alone: `(count & $7FFF) * 8` rows of
  one longword each. The routine stops the instant that many rows have been
  written, even if it is in the middle of a repeat run, so a decoder that
  instead decodes until the input is exhausted will overrun on real data.
- The bit window `d5` is 16 bits wide and `d6` counts unconsumed bits. A byte
  is fetched only when `d6` would drop below 9, and that fetch happens
  *before* the pixels of the element that consumed those bits are written.
  The routine therefore reads one byte of lookahead it may never use, so
  `consumed` is the stream's length or its length plus one.
- XOR mode accumulates: `d2` is never reset between rows, so row N of the
  output is the XOR of decoded rows 0..N.
"""

from __future__ import annotations

from typing import NamedTuple

TILE_SIZE = 32  # 8 rows x one longword
ROWS_PER_TILE = 8
INLINE_MARKER = 0xFC  # cmpi.b #$FC,d1 -- six leading 1 bits


class NemesisError(ValueError):
    pass


class NemesisHeader(NamedTuple):
    """The header word, decoded the way `NemDecomp_Main` decodes it."""

    tile_count: int
    xor_mode: bool
    row_count: int
    raw: int

    @property
    def decompressed_size(self) -> int:
        return self.row_count * 4


def read_header(data: bytes, offset: int = 0) -> NemesisHeader:
    if offset < 0 or offset + 2 > len(data):
        raise NemesisError(f"Header at 0x{offset:X} runs past end of input")
    raw = (data[offset] << 8) | data[offset + 1]
    # lsl.w #1,d2 / bcc -> the sign bit is the XOR flag and is shifted out.
    xor_mode = bool(raw & 0x8000)
    tile_count = raw & 0x7FFF
    # lsl.w #2,d2 / movea.w d2,a5 -- the row counter is a 16-bit register.
    row_count = (tile_count << 3) & 0xFFFF
    return NemesisHeader(tile_count, xor_mode, row_count, raw)


class _Reader:
    """`a0` plus the `d5`/`d6` bit window from Nem_ProcessCompressedData."""

    __slots__ = ("data", "pos", "window", "bits")

    def __init__(self, data: bytes, pos: int) -> None:
        self.data = data
        self.pos = pos
        self.window = 0
        self.bits = 0

    def byte(self) -> int:
        if self.pos >= len(self.data):
            raise NemesisError(f"Read past end of input at 0x{self.pos:X}")
        value = self.data[self.pos]
        self.pos += 1
        return value

    def prime(self) -> None:
        # move.b (a0)+,d5 / asl.w #8,d5 / move.b (a0)+,d5 / move.w #$10,d6
        self.window = (self.byte() << 8) | self.byte()
        self.bits = 0x10

    def peek8(self) -> int:
        """d1 = d5 >> (d6-8); the low byte is the next eight code bits."""
        return (self.window >> (self.bits - 8)) & 0xFF

    def take(self, count: int) -> None:
        """subq/sub.w on d6 followed by the `cmpi.w #9,d6` refill check."""
        self.bits -= count
        if self.bits < 9:
            self.bits += 8
            # asl.w #8,d5 discards the already-consumed high byte.
            self.window = ((self.window << 8) | self.byte()) & 0xFFFF


def build_code_table(data: bytes, offset: int) -> tuple[dict[int, tuple[int, int, int]], int]:
    """Nem_BuildCodeTable: expand the prefix code table to 256 lookahead slots.

    Returns `({lookahead_byte: (code_length, repeat_count, palette_index)},
    position of the first byte of compressed data)`.

    The game stores one word per 8-bit lookahead value in `Nem_Code_Table`, so
    a code shorter than 8 bits is written to every index whose high bits match
    it. Reproducing that expansion is what lets the decode loop mask nothing.
    """
    pos = offset
    table: dict[int, tuple[int, int, int]] = {}

    def read() -> int:
        nonlocal pos
        if pos >= len(data):
            raise NemesisError(f"Code table at 0x{offset:X} runs past end of input")
        value = data[pos]
        pos += 1
        return value

    marker = read()
    while True:
        # Nem_BCT_ChkEnd
        if marker == 0xFF:
            return table, pos
        # Nem_BCT_NewPalIndex: the marker's low nybble is the palette index;
        # the routine masks with andi.w #$F, so its high bit only serves to
        # distinguish a marker from a code descriptor.
        palette_index = marker & 0x0F
        while True:
            descriptor = read()
            # cmpi.b #$80 / bcc -- a set sign bit is the next palette marker.
            if descriptor >= 0x80:
                marker = descriptor
                break
            length = descriptor & 0x0F
            repeat = (descriptor & 0x70) >> 4
            code = read()
            shift = 8 - length
            if shift < 0:
                raise NemesisError(
                    f"Code table at 0x{offset:X}: code length {length} exceeds 8 bits"
                )
            base = code << shift
            span = 1 << shift
            if base + span > 0x100:
                # On hardware this would run off the end of Nem_Code_Table and
                # corrupt neighbouring RAM, so it can only mean bad input.
                raise NemesisError(
                    f"Code table at 0x{offset:X}: {length}-bit code 0x{code:02X} "
                    f"expands past the 256-entry table"
                )
            entry = (length, repeat, palette_index)
            for i in range(span):
                table[base + i] = entry


def decompress(data: bytes, offset: int = 0) -> tuple[bytes, int]:
    """Decompress the Nemesis stream at `offset`.

    Returns `(decompressed, consumed)` where `consumed` is the number of bytes
    the 68000 routine would have read. Because the routine refills its bit
    window before writing the pixels that emptied it, `consumed` is either the
    exact stream length or one byte more; it is never short, so it still bounds
    every blob's extent in the ROM.
    """
    if offset < 0 or offset > len(data):
        raise NemesisError(f"Offset 0x{offset:X} is outside the input")

    header = read_header(data, offset)
    table, pos = build_code_table(data, offset + 2)

    out = bytearray()
    if header.row_count == 0:
        return bytes(out), pos - offset

    reader = _Reader(data, pos)
    reader.prime()

    rows_left = header.row_count  # a5
    row = 0  # d4, the longword being filled
    nybbles_left = ROWS_PER_TILE  # d3, 8 pixels per row
    previous = 0  # d2, the XOR accumulator

    while True:
        lookahead = reader.peek8()
        if lookahead >= INLINE_MARKER:
            # Nem_PCD_InlineData: 6 marker bits, then 7 bits of payload. The
            # asm subtracts them in two steps with a refill check between, so
            # the payload can straddle a byte boundary.
            reader.take(6)
            reader.bits -= 7
            payload = (reader.window >> reader.bits) & 0xFFFF
            palette_index = payload & 0x0F
            repeat = (payload & 0x70) >> 4
            if reader.bits < 9:
                reader.bits += 8
                reader.window = ((reader.window << 8) | reader.byte()) & 0xFFFF
        else:
            entry = table.get(lookahead)
            if entry is None:
                raise NemesisError(
                    f"Undefined code 0x{lookahead:02X} at 0x{reader.pos:X}; the "
                    "code table does not cover this bit pattern"
                )
            length, repeat, palette_index = entry
            reader.take(length)

        # Nem_PCD_WritePixel / Nem_PCD_WritePixel_Loop: dbf runs repeat+1 times.
        for _ in range(repeat + 1):
            row = ((row << 4) | palette_index) & 0xFFFFFFFF
            nybbles_left -= 1
            if nybbles_left:
                continue
            if header.xor_mode:
                # Nem_PCD_WriteRowToRAM_XOR: d2 is never reset.
                previous ^= row
                out += previous.to_bytes(4, "big")
            else:
                out += row.to_bytes(4, "big")
            rows_left -= 1
            if rows_left == 0:
                return bytes(out), reader.pos - offset
            # Nem_PCD_NewRow
            row = 0
            nybbles_left = ROWS_PER_TILE
