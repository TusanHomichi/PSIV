"""The priority overlay: the pixels the VDP draws *above* sprites.

Bit 15 of a pattern-name word is the VDP's priority bit, and unlike bit 14 it
is real video data: `ChunkTilesToBuffer` masks with `#$BFFF`, which clears the
collision flag and leaves priority alone. The Mega Drive resolves a pixel in
this order:

    high-priority sprites
    high-priority plane A
    high-priority plane B
    low-priority sprites
    low-priority plane A
    low-priority plane B
    backdrop

so a map tile with the bit set draws over an ordinary sprite. That is how an
archway's keystone and a palm tree's crown sit in front of Chaz instead of
behind him. A renderer that draws the base image, then the party, and stops,
puts the party through the archway -- which is the bug this overlay fixes.

The overlay is the base render with everything but the priority tiles removed:
same dimensions, same 32-colour palette, same compositing order (plane B first,
plane A over it with colour 0 transparent). A renderer draws base, then
sprites, then this, and gets the hardware's answer for every pixel field mode
can produce -- including a high-priority plane B pixel beating a low-priority
plane A pixel, which the base render has already covered over.

Colour index 0 stays transparent on a priority tile too, because that is what
transparent means to the VDP; it is why a sprite still shows through the empty
corners of an arch. The composer never writes a colour-0 pixel, so a nonzero
byte means "the cartridge draws this above sprites" and a zero byte means
nothing at all -- but the tRNS chunk marks both lines' colour 0, because the
palette is the base render's palette and in it both are the transparent colour.

One caveat for the runtime, read off `Field_FillSpriteAttributes`: a field
object whose flag byte has bit 4 set gets `bset #7,d0` on its pattern word's
high byte, i.e. it becomes a *high-priority* sprite and belongs above this
overlay rather than below it. The party is never one of those -- all eleven
party routines store $40 at `$13(a4)`, which leaves bit 7 clear -- so the
overlay is unconditionally correct for the case that motivated it. Whether any
NPC sets the bit is a question about `FieldObjectsJmpTbl` routines rather than
about map data, and this module does not answer it.
"""

from __future__ import annotations

from typing import Any, Sequence

from . import png
from .layouts import (
    CHUNK_PIXELS_X,
    CHUNK_PIXELS_Y,
    CHUNK_TILES_X,
    PLANE_BG,
    PLANE_FG,
    TILE_PIXELS,
    ChunkTable,
    Layout,
    h_flip,
    palette_line,
    priority,
    tile_index,
    v_flip,
    vdp_word,
)
from .warps import PackError

#: The overlay's transparent palette indices. Pixel values are
#: `palette_line * 16 + colour_index`, so colour 0 of CRAM lines 0 and 1 are
#: indices 0 and 16.
OVERLAY_TRANSPARENT_INDICES = (0, 16)

#: `plane_byte` values, the same 0/1 the collision section uses for its plane.
PLANE_BYTES = {PLANE_FG: 0, PLANE_BG: 1}


def priority_tiles(chunks: ChunkTable) -> dict[int, tuple[tuple[int, int, int], ...]]:
    """Each chunk definition's priority tiles, as `(tile x, tile y, word)`.

    Chunks without one are absent rather than empty, so the compositor can skip
    a layout cell with a single dictionary miss. Most cells are such a miss.
    """
    found: dict[int, tuple[tuple[int, int, int], ...]] = {}
    for chunk_id, words in enumerate(chunks.words):
        tiles = tuple(
            (index % CHUNK_TILES_X, index // CHUNK_TILES_X, word)
            for index, word in enumerate(words)
            if priority(word)
        )
        if tiles:
            found[chunk_id] = tiles
    return found


def _draw_priority_plane(
    layout: Layout,
    tiles_by_chunk: dict[int, tuple[tuple[int, int, int], ...]],
    patterns,
    pixels: bytearray,
    width: int,
) -> int:
    """Draw one plane's priority tiles, returning how many it placed.

    The inner loop is `compose_layout`'s, minus the branch for a base image:
    this always composites, because a plane that is not drawing a priority tile
    is not drawing anything.
    """
    placed = 0
    for chunk_y in range(layout.height_chunks):
        for chunk_x in range(layout.width_chunks):
            tiles = tiles_by_chunk.get(layout.chunk_at(chunk_x, chunk_y))
            if tiles is None:
                continue
            for tile_x, tile_y, raw in tiles:
                placed += 1
                word = vdp_word(raw)
                index = tile_index(word)
                if index not in patterns.loaded:
                    # Already surfaced by `unloaded_patterns`; drawing nothing
                    # is what the base render does with it too.
                    continue
                tile = patterns.tile(index)
                shift = palette_line(word) * 16
                flip_x, flip_y = h_flip(word), v_flip(word)
                ox = chunk_x * CHUNK_PIXELS_X + tile_x * TILE_PIXELS
                oy = chunk_y * CHUNK_PIXELS_Y + tile_y * TILE_PIXELS
                for y in range(TILE_PIXELS):
                    source_y = TILE_PIXELS - 1 - y if flip_y else y
                    row = tile[source_y * TILE_PIXELS:(source_y + 1) * TILE_PIXELS]
                    if flip_x:
                        row = row[::-1]
                    start = (oy + y) * width + ox
                    for x in range(TILE_PIXELS):
                        value = row[x]
                        if value:
                            pixels[start + x] = value + shift
    return placed


def priority_overlay(
    decoded, palette: Sequence[tuple[int, int, int]]
) -> tuple[bytes | None, dict[str, Any]]:
    """The above-sprites layer of one map, as a PNG, plus what went into it.

    Returns `(None, counts)` when the map has no priority pixels at all: an
    entirely transparent file is a file every consumer has to load to learn
    nothing, so the pack says `png_over: null` instead of writing one.
    """
    width, height = decoded.bg.width_pixels, decoded.bg.height_pixels
    if (decoded.fg.width_pixels, decoded.fg.height_pixels) != (width, height):
        raise PackError(
            f"map planes are {decoded.fg.width_pixels}x{decoded.fg.height_pixels} and "
            f"{width}x{height}; an overlay has to line up with the base render"
        )

    tiles_by_chunk = priority_tiles(decoded.chunks)
    pixels = bytearray(width * height)
    counts = {"tiles": {}, "opaque_pixels": 0}
    # Plane B first, plane A over it: the same order the base render uses, and
    # the order the VDP resolves two high-priority pixels in.
    for layout in (decoded.bg, decoded.fg):
        counts["tiles"][layout.plane] = _draw_priority_plane(
            layout, tiles_by_chunk, decoded.patterns, pixels, width
        )
    counts["opaque_pixels"] = sum(1 for value in pixels if value)
    counts["chunks_with_priority_tiles"] = len(tiles_by_chunk)
    if not counts["opaque_pixels"]:
        return None, counts
    colours = list(palette)
    return png.encode_indexed(
        width, height, bytes(pixels), colours[:32], OVERLAY_TRANSPARENT_INDICES
    ), counts
