"""How a chunk's pattern-name words become pixels.

The geometry and the word fields come from the retail load path that
`psiv_tools.layouts` describes (`SetupChunksFG`, `ChunkTilesToBuffer`); they
live here so that the two plane draws -- `layouts.compose_layout`, the base render,
and `render.priority_overlay`, the above-sprites layer -- share one drawing
routine instead of two per-pixel loops. Import these names from here.

A layout repeats a few hundred chunks across thousands of cells (#130), so a
chunk is drawn once into a 32x32 block and the block is copied after that.
"""

from __future__ import annotations

import re
from typing import TYPE_CHECKING, Iterable, NamedTuple

if TYPE_CHECKING:
    from .layouts import TilePatterns

# ---------------------------------------------------------------------------
# Geometry, read off the load path rather than assumed.
# ---------------------------------------------------------------------------

#: A chunk is 16 big-endian words. `SetupChunksFG` does `lsl.w #5,d3` on the
#: chunk id to reach it, and `ChunkTilesToBuffer` is called with d1 = d2 = 3,
#: i.e. four cells across and four rows down.
CHUNK_TILES_X = 4
CHUNK_TILES_Y = 4
CHUNK_WORDS = CHUNK_TILES_X * CHUNK_TILES_Y
CHUNK_BYTES = CHUNK_WORDS * 2

TILE_PIXELS = 8
CHUNK_PIXELS_X = CHUNK_TILES_X * TILE_PIXELS
CHUNK_PIXELS_Y = CHUNK_TILES_Y * TILE_PIXELS

#: Mega Drive pattern-name word fields. Bit 14 would be the high palette bit;
#: PSIV steals it for collision and masks it off on the way to the VDP.
TILE_INDEX_MASK = 0x07FF
H_FLIP_BIT = 11
V_FLIP_BIT = 12
PALETTE_BIT = 13
COLLISION_BIT = 14
PRIORITY_BIT = 15
VDP_WORD_MASK = 0xBFFF  # ChunkTilesToBuffer / ChunkTilesToVRAM
VRAM_TILES = 0x800      # 11-bit pattern index


# ---------------------------------------------------------------------------
# Pattern-name word accessors
# ---------------------------------------------------------------------------
def tile_index(word: int) -> int:
    return word & TILE_INDEX_MASK


def h_flip(word: int) -> bool:
    return bool(word >> H_FLIP_BIT & 1)


def v_flip(word: int) -> bool:
    return bool(word >> V_FLIP_BIT & 1)


def palette_line(word: int) -> int:
    """The CRAM line the VDP will use.

    Only one palette bit survives: bit 14 is the collision flag and is masked
    off by `ChunkTilesToBuffer`, so chunk tiles can only ever draw from CRAM
    lines 0 and 1.
    """
    return word >> PALETTE_BIT & 1


def priority(word: int) -> bool:
    return bool(word >> PRIORITY_BIT & 1)


def collision_flag(word: int) -> int:
    """Bit 14, the bit `btst #6,0(a1)` tests and `andi.w #$BFFF` removes."""
    return word >> COLLISION_BIT & 1


def vdp_word(word: int) -> int:
    """The word as the VDP receives it, collision bit stripped."""
    return word & VDP_WORD_MASK


# ---------------------------------------------------------------------------
# Drawing
# ---------------------------------------------------------------------------
class PreparedChunk(NamedTuple):
    """One chunk's tiles drawn into a 32x32 block, ready to copy into a plane.

    `rows` are the block's pixel rows as an opaque draw leaves them: every
    pixel of a loaded tile at `palette_line * 16 + colour_index`, colour 0
    included, and 0 where no tile was drawn. `spans` are the `(y, x, pixels)`
    stretches of non-zero colour, which are all a compositing draw writes:
    colour 0 is transparent there. `missing` is the pattern indices the chunk
    referenced that the map never loaded; nothing is drawn for them.
    """

    rows: tuple[bytes, ...]
    spans: tuple[tuple[int, int, bytes], ...]
    missing: frozenset[int]


#: `bytes.translate` tables adding a palette line's base (0 or 16) to a pixel.
_LINE_SHIFT = {
    shift: bytes(min(value + shift, 0xFF) for value in range(0x100)) for shift in (0, 16)
}
_OPAQUE = re.compile(rb"[^\x00]+")


def prepare_chunk(
    entries: Iterable[tuple[int, int, int]], patterns: TilePatterns
) -> PreparedChunk:
    """Draw `(tile_x, tile_y, word)` entries the way the VDP draws each word."""
    pixels = bytearray(CHUNK_PIXELS_X * CHUNK_PIXELS_Y)
    opaque = bytearray(len(pixels))
    missing: set[int] = set()
    for tile_x, tile_y, raw in entries:
        word = vdp_word(raw)
        index = tile_index(word)
        if index not in patterns.loaded:
            missing.add(index)
            continue
        tile = patterns.tile(index)
        shift = palette_line(word) * 16
        if tile and max(tile) + shift > 0xFF:
            # What writing `value + shift` into a bytearray raised.
            raise ValueError("byte must be in range(0, 256)")
        shifted = tile.translate(_LINE_SHIFT[shift])
        flip_x, flip_y = h_flip(word), v_flip(word)
        for y in range(TILE_PIXELS):
            source_y = TILE_PIXELS - 1 - y if flip_y else y
            row = tile[source_y * TILE_PIXELS:(source_y + 1) * TILE_PIXELS]
            drawn = shifted[source_y * TILE_PIXELS:(source_y + 1) * TILE_PIXELS]
            if flip_x:
                row, drawn = row[::-1], drawn[::-1]
            start = (tile_y * TILE_PIXELS + y) * CHUNK_PIXELS_X + tile_x * TILE_PIXELS
            pixels[start:start + TILE_PIXELS] = drawn
            opaque[start:start + TILE_PIXELS] = row
    rows: list[bytes] = []
    spans: list[tuple[int, int, bytes]] = []
    for y in range(CHUNK_PIXELS_Y):
        row = bytes(pixels[y * CHUNK_PIXELS_X:(y + 1) * CHUNK_PIXELS_X])
        rows.append(row)
        mask = opaque[y * CHUNK_PIXELS_X:(y + 1) * CHUNK_PIXELS_X]
        for run in _OPAQUE.finditer(mask):
            spans.append((y, run.start(), row[run.start():run.end()]))
    return PreparedChunk(tuple(rows), tuple(spans), frozenset(missing))


def overlay_chunk(
    pixels: bytearray, width: int, chunk_x: int, chunk_y: int, chunk: PreparedChunk
) -> None:
    """Composite `chunk` into the plane cell (chunk_x, chunk_y); colour 0 shows through."""
    origin = chunk_y * CHUNK_PIXELS_Y * width + chunk_x * CHUNK_PIXELS_X
    for y, x, run in chunk.spans:
        start = origin + y * width + x
        pixels[start:start + len(run)] = run
