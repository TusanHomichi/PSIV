"""Enigma decompression, transcribed from the game's own decompressor.

The reference implementation is `EniDecomp` in the public disassembly
(`ps4.asm:85028-85258`), the routine every `jsr (EniDecomp).l` in the game
calls. Enigma is Sega's *tilemap* compressor: it does not produce pixels, it
produces a run of Mega Drive pattern-name words to be written into a plane
buffer. Every branch below mirrors one branch of the 68000 routine so that the
Python decoder cannot quietly disagree with the cartridge:

    EniDecomp:                  five header bytes, then the first 16-bit
                                window of the format bitstream
    EniDecompLoop:              read a 7-bit format entry; if its top bit is
                                clear only 6 of those bits were really there
    EniDecomp_00:               copy the incremental word, incrementing it
    EniDecomp_01:               copy the literal word unchanged
    EniDecomp_100/101/110:      fetch one inline value, then copy it / copy
                                and increment / copy and decrement
    EniDecomp_111:              count $F ends the stream; otherwise fetch a
                                fresh inline value for every repetition
    EniDecomp_FetchInlineValue: optional V and H flag bits, then `inline_bits`
                                bits of tile index, plus the base art tile
    EniDecomp_FetchByte:        the 8-bit refill, run whenever fewer than 9
                                bits are left in the window
    EniDecomp_Done:             rewind over the unread lookahead and round the
                                source pointer up to an even address

## The output words

A pattern-name word is `%PCCVHTTT TTTTTTTT`: priority, two palette-line bits,
vertical flip, horizontal flip, and an 11-bit tile index. Enigma stores those
words relative to a base value the caller passes in `d0` ("starting art tile"),
which the routine adds to every word it emits -- the two header words as well
as every inline value. The base is not part of the stream, so a mapping is only
meaningful together with the `move.w #$xxxx, d0` at its call site. Battle
backgrounds pass 0; the title portraits pass values like `$6401`, which carry
the palette line *and* the VRAM tile the art was loaded at.

## Six details taken from the routine rather than assumed

- **This decompressor implements only two of Enigma's five flags.** The
  standard format's header bitfield is `PCCVH`; `EniDecomp_FetchInlineValue`
  tests exactly two bits (the `ror.l #1` / `ror.w #1` in the header parks bit 1
  of the byte at bit 15 of the low word and bit 0 at bit 31, and the two
  `swap`/`bpl` pairs test those two) and sets `$1000` (V flip) and `$800`
  (H flip). Bits 2-4 -- the P and CC flags -- are never tested, so their bits
  would never be consumed and a stream that declared them would desynchronise
  the whole bitstream. `read_header` therefore rejects them: a flag byte
  outside 0..3 means the offset is wrong, not that the game renders it oddly.

- **The format entry is read as 7 bits and then partly given back.** The
  routine always shifts 7 bits out of the window, and only afterwards decides
  (`cmpi.w #$40`) that a leading 0 makes it a 6-bit entry, subtracting 6 from
  the bit counter and taking the repeat count from bits 4..1 of the 7-bit read
  rather than bits 3..0. Reading 6 bits up front and then a 7th gets the same
  mode but the wrong count.

- **The repeat count is a `dbf` count**, so `n` means `n + 1` words. Mode 111
  is the exception: count `$F` is the end marker, not sixteen inline values.

- **An inline value that straddles the window reloads the window entirely.**
  The plain path (`EniDecomp_FetchByte`) shifts in one byte and keeps 9..16
  bits live, but when the window holds fewer bits than the value needs, the
  routine takes the top bits of the *next* byte without advancing past it
  (`move.b (a0), d5` -- no post-increment), then falls into `loc_41B92`, which
  reloads a fresh 16-bit window from that same byte and the one after it. The
  same reload happens when the value consumes the window exactly. A decoder
  that keeps a running bit reservoir instead of reproducing this reload
  desynchronises on real data. That path also bounds the usable inline width:
  the shift counter can be as low as 7 when a value is fetched, so a 16-bit
  value that also declares flag bits drives it negative and cannot be decoded
  at all. Every stream in this ROM declares 4 to 9 bits.

- **The base and the flags are *added* to the value, not or-ed in.**
  `add.w d3, d1` -- and `d3` is the base tile with `$1000`/`$800` already
  or-ed into it. With this ROM's 4-to-9-bit inline values the tile field and
  the flag field never overlap, so the distinction is invisible; a wider value
  carries into the flip bits, and reproducing that is free while guessing at
  it is not. The same is true of the two header copy words, which are offset
  by `adda.w a3` rather than being combined with it.

- **The flag bits are taken without a refill check.** `subq.w #1, d6` /
  `btst d6, d5` never calls `EniDecomp_FetchByte`, so the counter legitimately
  drops to 7 before the inline value is fetched; the fetch's straddle path is
  what covers it.

- **The consumed length is rounded, not exact.** `EniDecomp_Done` rewinds one
  byte, rewinds a second if the window was freshly reloaded (`d6 == $10`), and
  then rounds the *absolute* source address up to an even boundary. Enigma
  streams therefore always occupy an even number of bytes when they start at an
  even address, which every one in this ROM does.
"""

from __future__ import annotations

from typing import NamedTuple

WINDOW_BITS = 0x10  # moveq #$10, d6

# The two flags this decompressor supports, as they appear in the header byte
# and in the pattern-name word each one sets.
FLAG_V_FLIP = 0x02  # header bit 1 -> ori.w #$1000
FLAG_H_FLIP = 0x01  # header bit 0 -> ori.w #$800
SUPPORTED_FLAGS = FLAG_V_FLIP | FLAG_H_FLIP

WORD_V_FLIP = 0x1000
WORD_H_FLIP = 0x0800

# EniDecomp_Masks: dc.w 1, 3, 7, ... $FFFF, indexed as `Masks-2(pc,d0.w)` with
# d0 = 2 * bit count, so a count of 0 would read the word before the table.
MASKS: tuple[int, ...] = tuple((1 << n) - 1 for n in range(1, 17))

MAX_INLINE_BITS = len(MASKS)


class EnigmaError(ValueError):
    pass


class EnigmaHeader(NamedTuple):
    """The five header bytes, decoded the way `EniDecomp` decodes them."""

    inline_bits: int  # a5
    flags: int  # the PCCVH byte, before the ror.l/ror.w shuffle
    incremental_word: int  # a2, already offset by the base tile
    literal_word: int  # a4, already offset by the base tile
    base_tile: int  # a3, supplied by the caller in d0

    @property
    def uses_v_flip(self) -> bool:
        return bool(self.flags & FLAG_V_FLIP)

    @property
    def uses_h_flip(self) -> bool:
        return bool(self.flags & FLAG_H_FLIP)

    @property
    def flag_bits_per_value(self) -> int:
        return int(self.uses_v_flip) + int(self.uses_h_flip)

    @property
    def size(self) -> int:
        return 6  # 1 + 1 + 2 + 2, before the first window word


def _byte(data: bytes, pos: int) -> int:
    if pos < 0 or pos >= len(data):
        raise EnigmaError(f"Read past end of input at 0x{pos:X}")
    return data[pos]


def read_header(data: bytes, offset: int, base_tile: int = 0) -> EnigmaHeader:
    """Decode the six header bytes at `offset` (the two 16-bit copy words
    included). The first window word follows immediately."""
    if offset < 0 or offset + 6 > len(data):
        raise EnigmaError(f"Header at 0x{offset:X} runs past end of input")
    if not 0 <= base_tile <= 0xFFFF:
        raise EnigmaError(f"Base tile 0x{base_tile:X} is not a 16-bit value")

    # move.b (a0)+, d0 / ext.w d0 / movea.w d0, a5
    inline_bits = data[offset]
    if not 1 <= inline_bits <= MAX_INLINE_BITS:
        # ext.w sign-extends, and EniDecomp_Masks has no entry for 0 or >16, so
        # either would index outside the table on hardware.
        raise EnigmaError(
            f"Enigma stream at 0x{offset:X} declares {inline_bits} inline bits; "
            f"EniDecomp_Masks only covers 1..{MAX_INLINE_BITS}"
        )

    flags = data[offset + 1]
    if flags & ~SUPPORTED_FLAGS & 0xFF:
        raise EnigmaError(
            f"Enigma stream at 0x{offset:X} declares flag byte 0x{flags:02X}; "
            "EniDecomp only implements the V ($02) and H ($01) bits, so any "
            "other bit would leave its per-value bits unconsumed"
        )

    incremental = int.from_bytes(data[offset + 2:offset + 4], "big")
    literal = int.from_bytes(data[offset + 4:offset + 6], "big")
    return EnigmaHeader(
        inline_bits=inline_bits,
        flags=flags,
        incremental_word=(incremental + base_tile) & 0xFFFF,
        literal_word=(literal + base_tile) & 0xFFFF,
        base_tile=base_tile,
    )


class _Stream:
    """`a0`, the 16-bit window `d5` and the shift counter `d6`."""

    __slots__ = ("data", "pos", "window", "shift")

    def __init__(self, data: bytes, pos: int) -> None:
        self.data = data
        self.pos = pos
        # move.b (a0)+, d5 / asl.w #8, d5 / move.b (a0)+, d5
        self.window = (self.next_byte() << 8) | self.next_byte()
        self.shift = WINDOW_BITS  # moveq #$10, d6

    def next_byte(self) -> int:
        value = _byte(self.data, self.pos)
        self.pos += 1
        return value

    def fetch_byte(self, used: int) -> None:
        """EniDecomp_FetchByte: drop `used` bits, refill if fewer than 9 left."""
        self.shift -= used
        if self.shift < 9:
            self.shift += 8
            self.window = ((self.window << 8) | self.next_byte()) & 0xFFFF

    def reload_window(self) -> None:
        """loc_41B92's tail: a whole fresh 16-bit window from the next two bytes."""
        self.window = (self.next_byte() << 8) | self.next_byte()

    def flag_bit(self) -> int:
        """subq.w #1, d6 / btst d6, d5 -- no refill check, deliberately."""
        self.shift -= 1
        if self.shift < 0:
            raise EnigmaError(
                f"Bit counter underflowed at 0x{self.pos:X}; the stream is not Enigma"
            )
        return (self.window >> self.shift) & 1


def _rol8(value: int, count: int) -> int:
    count &= 7
    return ((value << count) | (value >> (8 - count))) & 0xFF if count else value & 0xFF


def _fetch_inline_value(stream: _Stream, header: EnigmaHeader) -> int:
    """EniDecomp_FetchInlineValue: flags, then `inline_bits` bits, plus base."""
    value_base = header.base_tile  # move.w a3, d3
    # swap d4 / bpl -- the V flag was parked at bit 15 of d4's low word.
    if header.uses_v_flip and stream.flag_bit():
        value_base |= WORD_V_FLIP
    # swap d4 / bpl -- the H flag was parked at bit 31.
    if header.uses_h_flip and stream.flag_bit():
        value_base |= WORD_H_FLIP

    bits = header.inline_bits
    mask = MASKS[bits - 1]
    value = stream.window  # move.w d5, d1
    remainder = stream.shift - bits  # sub.w a5, d7

    if remainder < 0:
        # The value straddles the window. Take what is left, then borrow the
        # top `-remainder` bits of the next byte *without* consuming it.
        stream.shift = remainder + WINDOW_BITS
        short_by = -remainder
        value = (value << short_by) & 0xFFFF
        # move.b (a0), d5 replaces only the low byte of the window; rol.b then
        # rotates that byte alone, and the and.w that follows is 16-bit wide.
        borrowed = (stream.window & 0xFF00) | _rol8(_byte(stream.data, stream.pos), short_by)
        borrowed &= MASKS[short_by - 1]
        value = (value + borrowed) & 0xFFFF
        value = ((value & mask) + value_base) & 0xFFFF
        stream.reload_window()
        return value

    if remainder == 0:
        # The value consumes the window exactly; d6 is set to 16 and the window
        # is reloaded wholesale rather than shifted.
        stream.shift = WINDOW_BITS
        value = ((value & mask) + value_base) & 0xFFFF
        stream.reload_window()
        return value

    # loc_41BA4: the value sits inside the window.
    value = (value >> remainder) & 0xFFFF
    value = ((value & mask) + value_base) & 0xFFFF
    stream.fetch_byte(bits)
    return value


def decompress(
    data: bytes,
    offset: int = 0,
    base_tile: int = 0,
    max_words: int | None = None,
) -> tuple[list[int], int]:
    """Decompress the Enigma stream at `offset`.

    Returns `(words, consumed)`: the pattern-name words the routine would have
    written to `a1`, and the number of source bytes `EniDecomp` would report
    having read (rewound past the unread lookahead and rounded up to an even
    address, exactly as `EniDecomp_Done` does).

    `base_tile` is the caller's `d0`; it is added to every emitted word, so
    passing the wrong one shifts the whole mapping through VRAM. `max_words`
    bounds the output the way the destination buffer bounds it on hardware --
    a plane buffer is 2,048 words -- and is a decode error when exceeded rather
    than a silent truncation.
    """
    if offset < 0 or offset > len(data):
        raise EnigmaError(f"Offset 0x{offset:X} is outside the input")

    header = read_header(data, offset, base_tile)
    stream = _Stream(data, offset + header.size)
    out: list[int] = []
    incremental = header.incremental_word  # a2

    def emit(word: int) -> None:
        if max_words is not None and len(out) >= max_words:
            raise EnigmaError(
                f"Enigma stream at 0x{offset:X} produced more than {max_words} "
                "words; it does not fit the destination it was decoded for"
            )
        out.append(word & 0xFFFF)

    while True:
        # EniDecompLoop: always read 7 bits, then decide whether it was 6.
        if stream.shift < 7:
            raise EnigmaError(
                f"Only {stream.shift} bits left in the window at 0x{stream.pos:X}; "
                "the stream is not Enigma"
            )
        entry = (stream.window >> (stream.shift - 7)) & 0x7F
        count = entry
        if entry >= 0x40:
            used = 7
        else:
            used = 6
            count >>= 1  # lsr.w #1, d2 -- the 7th bit belongs to the next entry
        stream.fetch_byte(used)
        count &= 0x0F  # andi.w #$F, d2
        mode = entry >> 4  # jmp EniDecom_JumpTbl(pc,d1.w) after add.w d1,d1

        if mode < 2:
            # EniDecomp_00: copy the incremental word, incrementing it.
            for _ in range(count + 1):
                emit(incremental)
                incremental = (incremental + 1) & 0xFFFF
        elif mode < 4:
            # EniDecomp_01: copy the literal word, unchanged.
            for _ in range(count + 1):
                emit(header.literal_word)
        elif mode == 4:
            # EniDecomp_100
            value = _fetch_inline_value(stream, header)
            for _ in range(count + 1):
                emit(value)
        elif mode == 5:
            # EniDecomp_101
            value = _fetch_inline_value(stream, header)
            for _ in range(count + 1):
                emit(value)
                value = (value + 1) & 0xFFFF
        elif mode == 6:
            # EniDecomp_110
            value = _fetch_inline_value(stream, header)
            for _ in range(count + 1):
                emit(value)
                value = (value - 1) & 0xFFFF
        else:
            # EniDecomp_111
            if count == 0x0F:
                break
            for _ in range(count + 1):
                emit(_fetch_inline_value(stream, header))

    # EniDecomp_Done
    end = stream.pos - 1
    if stream.shift == WINDOW_BITS:
        end -= 1
    if end & 1:
        end += 1
    return out, end - offset


def words_to_bytes(words: list[int] | tuple[int, ...]) -> bytes:
    """The words as the 68000 wrote them: big-endian, in order."""
    out = bytearray()
    for word in words:
        if not 0 <= word <= 0xFFFF:
            raise EnigmaError(f"0x{word:X} is not a 16-bit pattern-name word")
        out += word.to_bytes(2, "big")
    return bytes(out)
