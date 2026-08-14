"""Sprite mapping and animation records, byte for byte.

The two record types `Field_FillSpriteAttributes` and `FieldObj_Animate`
read, plus the pattern-word arithmetic that joins a mapping to a map's art.
The package docstring is the transcription and says where each rule came
from; this module is just the reader.
"""

from __future__ import annotations

import struct
from dataclasses import dataclass
from typing import Any

from ..gfx import TILE_HEIGHT, TILE_WIDTH

# ---------------------------------------------------------------------------
# Constants the record grammar owns.
# ---------------------------------------------------------------------------
#: `facing_dir` is added to `mappings_addr` as a byte offset, so it is already
#: four times the table index.
FACINGS: tuple[tuple[int, str], ...] = ((0, "down"), (4, "up"), (8, "right"), (0xC, "left"))
FACING_NAMES = {value: name for value, name in FACINGS}

#: `$13(a4)` -> CRAM line. Bits 6-5 of a pattern word's high byte are the
#: palette select; retail field objects only ever store these four values.
SPRITE_TILE_PROPS_LINES = {0x00: 0, 0x20: 1, 0x40: 2, 0x60: 3}

#: Pattern word fields.
TILE_INDEX_MASK = 0x07FF
HFLIP_BIT = 1 << 11
VFLIP_BIT = 1 << 12
PALETTE_SHIFT = 13
PRIORITY_BIT = 1 << 15

#: Work RAM is `$FFxxxxxx`; `NemDecomp_ToRAM` destinations are
#: `$FFFF0000 | tile << 5`.
RAM_BASE = 0xFFFF0000
ROM_LIMIT = 0x400000


class SpriteError(ValueError):
    pass


# ---------------------------------------------------------------------------
# Small readers
# ---------------------------------------------------------------------------
def _u8(rom: bytes, offset: int) -> int:
    _bounds(rom, offset, 1)
    return rom[offset]


def _s8(rom: bytes, offset: int) -> int:
    _bounds(rom, offset, 1)
    return struct.unpack_from(">b", rom, offset)[0]


def _u16(rom: bytes, offset: int) -> int:
    _bounds(rom, offset, 2)
    return struct.unpack_from(">H", rom, offset)[0]


def _s16(rom: bytes, offset: int) -> int:
    _bounds(rom, offset, 2)
    return struct.unpack_from(">h", rom, offset)[0]


def _u32(rom: bytes, offset: int) -> int:
    _bounds(rom, offset, 4)
    return struct.unpack_from(">I", rom, offset)[0]


def _bounds(rom: bytes, offset: int, size: int) -> None:
    if offset < 0 or offset + size > len(rom):
        raise SpriteError(f"read of {size} bytes at 0x{offset:06X} runs past the ROM")


def _hex(value: int) -> str:
    return f"0x{value:06X}"


# ---------------------------------------------------------------------------
# Sprite mappings
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Piece:
    """One six-byte sprite piece of a mapping record."""

    y: int
    size_byte: int
    tile_word: int
    x: int
    x_mirrored: int

    @property
    def width_tiles(self) -> int:
        return ((self.size_byte >> 2) & 3) + 1

    @property
    def height_tiles(self) -> int:
        return (self.size_byte & 3) + 1

    @property
    def width(self) -> int:
        return self.width_tiles * TILE_WIDTH

    @property
    def height(self) -> int:
        return self.height_tiles * TILE_HEIGHT

    @property
    def tile_count(self) -> int:
        return self.width_tiles * self.height_tiles

    def to_json(self) -> dict[str, Any]:
        return {
            "x": self.x,
            "y": self.y,
            "x_mirrored": self.x_mirrored,
            "size_byte": self.size_byte,
            "width_tiles": self.width_tiles,
            "height_tiles": self.height_tiles,
            "tile_word": f"0x{self.tile_word:04X}",
        }


PIECE_SIZE = 6


@dataclass(frozen=True)
class Mapping:
    """A `Mappings_*` record: one drawn frame."""

    rom_offset: int
    header_byte: int
    pieces: tuple[Piece, ...]

    @property
    def size_bytes(self) -> int:
        return 2 + PIECE_SIZE * len(self.pieces)


def decode_mapping(rom: bytes, offset: int) -> Mapping:
    """One sprite-mapping record, as `Field_FillSpriteAttributes` reads it.

    Byte 0 is the piece count *minus one*: the routine loads it into `d1` and
    runs `dbf`, so a record of one piece stores zero. (The animation sequence
    record two levels up stores its count exactly; the two conventions differ
    and mixing them silently truncates.)
    """
    count = _u8(rom, offset) + 1
    header = _u8(rom, offset + 1)
    pieces = []
    for index in range(count):
        base = offset + 2 + index * PIECE_SIZE
        _bounds(rom, base, PIECE_SIZE)
        pieces.append(
            Piece(
                y=_s8(rom, base),
                size_byte=_u8(rom, base + 1),
                tile_word=_u16(rom, base + 2),
                x=_s8(rom, base + 4),
                x_mirrored=_s8(rom, base + 5),
            )
        )
    return Mapping(rom_offset=offset, header_byte=header, pieces=tuple(pieces))


# ---------------------------------------------------------------------------
# Animation sequences
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class SequenceFrame:
    mapping_offset: int
    duration_byte: int

    @property
    def ticks(self) -> int:
        """Game frames the frame is displayed for.

        `subq.b #1, mappings_duration(a4) / bpl` only advances once the counter
        goes negative, so a stored 0 is one frame, not none.
        """
        return self.duration_byte + 1


@dataclass(frozen=True)
class AnimationSequence:
    """A `SprMapsData_*` record: the frame list for one facing."""

    rom_offset: int
    per_frame_durations: bool
    frames: tuple[SequenceFrame, ...]

    @property
    def cycle_ticks(self) -> int:
        return sum(frame.ticks for frame in self.frames)


SEQUENCE_PER_FRAME_FLAG = 0x80


def decode_sequence(rom: bytes, offset: int) -> AnimationSequence:
    """One animation sequence, in whichever of its two forms it is stored.

    The shared form is `count, duration, count longs`. The per-frame form sets
    bit 7 of the count byte and stores one duration byte per frame; `loc_4477E`
    reaches the pointers with `d0 = idx*4 + count`, forced odd, read from
    `1(a1,d0.w)`, which is the same as saying the pointer table starts at the
    first even offset at or after `1 + count`.
    """
    head = _u8(rom, offset)
    if head & SEQUENCE_PER_FRAME_FLAG:
        count = head & 0x7F
        if count == 0:
            raise SpriteError(f"sequence at {_hex(offset)} declares no frames")
        durations = [_u8(rom, offset + 1 + i) for i in range(count)]
        table = offset + ((count + 2) & ~1)
        per_frame = True
    else:
        count = head
        if count == 0:
            raise SpriteError(f"sequence at {_hex(offset)} declares no frames")
        durations = [_u8(rom, offset + 1)] * count
        table = offset + 2
        per_frame = False
    frames = tuple(
        SequenceFrame(mapping_offset=_u32(rom, table + 4 * i), duration_byte=durations[i])
        for i in range(count)
    )
    return AnimationSequence(rom_offset=offset, per_frame_durations=per_frame, frames=frames)
