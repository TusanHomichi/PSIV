"""Kosinski decompression, transcribed from the game's own decompressor.

The reference implementation is `KosDecomp` in the public disassembly
(`ps4.asm`). Every branch below mirrors one branch of that 68000 routine so
that the Python decoder cannot quietly disagree with the cartridge:

    KosDecomp:              read the first 16-bit description field
    KosDecomp_Loop:         next bit set  -> copy one literal byte
    KosDecomp_Match:        next bit clear -> inline match (2-bit count,
                            1-byte displacement, -256..-1)
    KosDecomp_FullMatch:    13-bit displacement (-8192..-1) plus a 3-bit
                            count; a zero count means an extra count byte
    KosDecomp_FullMatch2:   extra byte 0 ends the stream, 1 resumes the loop,
                            anything else is the extended repeat count

Description fields are little-endian 16-bit words and their bits are consumed
LSB-first. The asm reloads a field only *after* consuming its sixteenth bit
(the `dbf d4` runs after the `lsr.w`/`move sr,d6` pair), so a stream that
terminates on that sixteenth bit still pays for the next field. `consumed`
below reproduces that exactly, which is what makes it a usable length oracle.
"""

from __future__ import annotations


class KosinskiError(ValueError):
    pass


class _BitReader:
    """The `d5`/`d4` description-field pair from KosDecomp."""

    __slots__ = ("data", "pos", "field", "remaining")

    def __init__(self, data: bytes, offset: int) -> None:
        self.data = data
        self.pos = offset
        self.field = 0
        self.remaining = 0
        self._reload()

    def _reload(self) -> None:
        if self.pos + 2 > len(self.data):
            raise KosinskiError(
                f"Description field at 0x{self.pos:X} runs past end of input"
            )
        # move.b (a0)+,$1(sp) / move.b (a0)+,(sp) / move.w (sp),d5
        low = self.data[self.pos]
        high = self.data[self.pos + 1]
        self.pos += 2
        self.field = (high << 8) | low
        self.remaining = 16

    def bit(self) -> int:
        # lsr.w #1,d5 ; move sr,d6 ; dbf d4,... -- the bit is taken from the
        # current field, and only then may the field be replaced.
        value = self.field & 1
        self.field >>= 1
        self.remaining -= 1
        if self.remaining == 0:
            self._reload()
        return value

    def byte(self) -> int:
        if self.pos >= len(self.data):
            raise KosinskiError(f"Read past end of input at 0x{self.pos:X}")
        value = self.data[self.pos]
        self.pos += 1
        return value


def _copy_match(out: bytearray, displacement: int, count: int) -> None:
    """KosDecomp_MatchLoop: byte-by-byte, so overlapping runs self-extend."""
    start = len(out) + displacement
    if start < 0:
        raise KosinskiError(
            f"Match displacement {displacement} points before start of output"
        )
    for i in range(count):
        out.append(out[start + i])


def decompress(data: bytes, offset: int = 0) -> tuple[bytes, int]:
    """Decompress the Kosinski stream at `offset`.

    Returns `(decompressed, consumed)` where `consumed` is the number of
    compressed bytes the 68000 routine would have read, i.e. the length of the
    stream including its terminator.
    """
    if offset < 0 or offset > len(data):
        raise KosinskiError(f"Offset 0x{offset:X} is outside the input")

    reader = _BitReader(data, offset)
    out = bytearray()

    while True:
        if reader.bit():
            # KosDecomp_ChkBit: bit set, copy byte as-is.
            out.append(reader.byte())
            continue

        if not reader.bit():
            # KosDecomp_Match: inline match, high count bit then low count bit.
            count = (reader.bit() << 1) | reader.bit()
            displacement = reader.byte() - 0x100
            _copy_match(out, displacement, count + 2)
            continue

        # KosDecomp_FullMatch: d2 = $E000 | ((byte1 & $F8) << 5) | byte0,
        # sign-extended from 16 bits, so -8192..-1.
        low = reader.byte()
        high = reader.byte()
        displacement = (0xE000 | ((high & 0xF8) << 5) | low) - 0x10000
        count = high & 7
        if count:
            _copy_match(out, displacement, count + 2)
            continue

        # KosDecomp_FullMatch2
        extended = reader.byte()
        if extended == 0:
            break
        if extended == 1:
            # Not a match at all: fall back into the main loop. Note this is
            # *not* an unconditional "read a new description field" -- the
            # field is only refilled when its bits run out.
            continue
        _copy_match(out, displacement, extended + 1)

    return bytes(out), reader.pos - offset
