"""Field-map chunks, plane layouts and tile collision.

This module decodes the *inside* of a field map's layout section: what a chunk
is, how the two plane layouts index chunks, and where per-tile collision lives.
It deliberately does not walk the 417-entry map table or parse the rest of a map
record -- it takes explicit ROM offsets and dimensions so that a map-record
walker can feed it every map without this module growing a second grammar.

Everything below is transcribed from the retail load path in the public
disassembly (`ps4.asm`), not from a description of the format:

    GameMode_LoadFieldMap   reads four dimension bytes into Map_Row_Size_FG /
                            Map_Column_Size_FG / Map_Row_Size_BG /
                            Map_Column_Size_BG, then calls the routines below
    loc_519D2               tileset loop: (word VRAM tile, long art pointer)
                            repeated until a negative word; each art blob is
                            *Kosinski*-compressed raw 4bpp patterns, staged in
                            Chunk_Table and DMAed to VRAM at `tile << 5`
    loc_51AB2               six-or-more scroll bytes; its first byte is the
                            flag at $FFFFEC24 that selects which plane the
                            collision reader uses
    Map_LoadChunks          long chunk-blob pointers until a $FFFF word; every
                            blob is Kosinski-decompressed back to back into
                            Chunk_Table, so the blobs of one map concatenate
                            into a single chunk array
    loc_539E2 / loc_53A04   one long each: the FG and BG layout blobs, also
                            Kosinski, decompressed into Map_Layout_FG ($FFFFA000)
                            and Map_Layout_BG ($FFFFB000)
    SetupChunksFG / ...BG   chunk id = layout[row * (row_size + 1) + column]
    ChunkTilesToBuffer      `andi.w #$BFFF,d3 ; remove collision bit` -- the
                            word the VDP sees, with bit 14 stripped
    GetChunkAndCollision    assembles the 4-bit collision type out of bit 14 of
                            four neighbouring chunk words
    TileCollNormalPtrs      which of those 16 types actually block movement

Three formats are in play across this project; the entire map-layout path uses
only one of them. Chunk definitions, plane layouts and field tile art are all
Kosinski. Nemesis carries field *sprite* art, and Enigma carries plane mappings
for battle backgrounds. Nothing here needs `psiv_tools.enigma`.
"""

from __future__ import annotations

import hashlib
import struct
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence

from . import png
from .gfx import GRAYSCALE_RAMP, RGB, decode_palette, decode_tile, palette_rgb
from .kosinski import decompress as kos_decompress

# ---------------------------------------------------------------------------
# Geometry, all of it read off the load path rather than assumed.
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

#: `GetChunkAndCollision` picks a collision quadrant with bit 1 of the tile
#: coordinate, so one collision cell covers 2x2 tiles.
COLLISION_CELL_TILES = 2
COLLISION_CELL_PIXELS = COLLISION_CELL_TILES * TILE_PIXELS

#: Layout cells are single bytes (`move.b (a1),d3`), so a map can name at most
#: 256 chunks, and Map_Layout_FG..Map_Layout_BG is 0x1000 bytes apart.
MAX_CHUNKS = 256
LAYOUT_RAM_BYTES = 0x1000

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

#: Names as the disassembly annotates them at `GetChunkAndCollision`. They are
#: labels for a 4-bit code, not a bitmask: the code is assembled from the
#: collision flags of the cell's four tiles, in the order top-left (1),
#: top-right (2), bottom-left (4), bottom-right (8).
COLLISION_TYPE_NAMES: dict[int, str] = {
    0x0: "normal",
    0x1: "map_change",
    0x2: "recovery",
    0x8: "solid",
    0x9: "water",
    0xA: "sand",
    0xB: "ice_block",
    0xC: "shop",
}

#: Which types stop the walker, taken from `TileCollNormalPtrs`: types 8, 9, $A
#: and $B route to `TileColl_Solid` and $C to `TileColl_Shop`, and both of those
#: are `moveq #1,d2 / rts`. Every other type routes to `TileColl_Empty`.
#: Type 1 does not block -- it walks onto the cell and fires a map transition.
BLOCKING_COLLISION_TYPES = frozenset({0x8, 0x9, 0xA, 0xB, 0xC})

#: `GetChunkAndCollision`: EC24 zero reads Map_Layout_FG, non-zero reads
#: Map_Layout_BG. The byte comes from the map record via `loc_51AB2`.
PLANE_FG = "fg"
PLANE_BG = "bg"
PLANES = (PLANE_FG, PLANE_BG)


class LayoutError(ValueError):
    pass


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


def collision_type_name(value: int) -> str:
    if not 0 <= value <= 0xF:
        raise LayoutError(f"Collision type {value} is not a nibble")
    return COLLISION_TYPE_NAMES.get(value, f"unnamed_{value:X}")


def is_blocking(value: int) -> bool:
    return value in BLOCKING_COLLISION_TYPES


def is_walkable(value: int) -> bool:
    return not is_blocking(value)


def dimension_from_header(size_byte: int) -> int:
    """The record stores size-1; every reader does `addq.w #1`."""
    if not 0 <= size_byte <= 0xFF:
        raise LayoutError(f"Dimension byte 0x{size_byte:X} is not a byte")
    return size_byte + 1


# ---------------------------------------------------------------------------
# Blobs
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Blob:
    """One Kosinski stream, with the provenance a decompressed record lacks."""

    rom_offset: int
    compressed_length: int
    decompressed_length: int
    sha256: str

    @property
    def rom_end(self) -> int:
        return self.rom_offset + self.compressed_length

    def to_json(self) -> dict[str, Any]:
        return {
            "rom_offset": f"0x{self.rom_offset:06X}",
            "rom_end": f"0x{self.rom_end:06X}",
            "compressed_length": self.compressed_length,
            "decompressed_length": self.decompressed_length,
            "sha256": self.sha256,
        }


def _decompress_blob(rom: bytes, offset: int, what: str) -> tuple[bytes, Blob]:
    if not 0 <= offset < len(rom):
        raise LayoutError(f"{what} offset 0x{offset:X} is outside the ROM")
    data, consumed = kos_decompress(rom, offset)
    blob = Blob(
        rom_offset=offset,
        compressed_length=consumed,
        decompressed_length=len(data),
        sha256=hashlib.sha256(rom[offset:offset + consumed]).hexdigest(),
    )
    return data, blob


# ---------------------------------------------------------------------------
# Chunks
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class ChunkTable:
    """The concatenated contents of Chunk_Table for one map.

    `Map_LoadChunks` loads `a1` once and lets each `KosDecomp` leave it past the
    bytes it wrote, so a map's chunk blobs are one array, not several.
    """

    words: tuple[tuple[int, ...], ...]
    blobs: tuple[Blob, ...]

    def __len__(self) -> int:
        return len(self.words)

    def __getitem__(self, chunk_id: int) -> tuple[int, ...]:
        if not 0 <= chunk_id < len(self.words):
            raise LayoutError(
                f"Chunk id 0x{chunk_id:02X} is outside the {len(self.words)} "
                f"chunks this map loads"
            )
        return self.words[chunk_id]

    def to_json(self) -> dict[str, Any]:
        return {
            "chunk_count": len(self.words),
            "chunk_bytes": CHUNK_BYTES,
            "tiles_per_chunk": f"{CHUNK_TILES_X}x{CHUNK_TILES_Y}",
            "pixels_per_chunk": f"{CHUNK_PIXELS_X}x{CHUNK_PIXELS_Y}",
            "blobs": [b.to_json() for b in self.blobs],
        }


def decode_chunks(rom: bytes, offsets: Sequence[int]) -> ChunkTable:
    """Decode one map's chunk definitions from its Kosinski blob pointers.

    `offsets` is the list `Map_LoadChunks` walks, in record order.
    """
    if not offsets:
        raise LayoutError("A map needs at least one chunk blob")

    raw = bytearray()
    blobs: list[Blob] = []
    for offset in offsets:
        data, blob = _decompress_blob(rom, offset, "Chunk blob")
        raw += data
        blobs.append(blob)

    if len(raw) % CHUNK_BYTES:
        raise LayoutError(
            f"Chunk data is {len(raw)} bytes, which is not a whole number of "
            f"{CHUNK_BYTES}-byte definitions"
        )
    count = len(raw) // CHUNK_BYTES
    if count > MAX_CHUNKS:
        raise LayoutError(
            f"{count} chunk definitions, but layout cells are single bytes so "
            f"only {MAX_CHUNKS} are reachable"
        )

    words = tuple(
        struct.unpack_from(f">{CHUNK_WORDS}H", raw, i * CHUNK_BYTES)
        for i in range(count)
    )
    return ChunkTable(words=words, blobs=tuple(blobs))


# ---------------------------------------------------------------------------
# Layouts
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Layout:
    """One plane's chunk grid, as decompressed into Map_Layout_FG/BG."""

    plane: str
    width_chunks: int
    height_chunks: int
    cells: bytes
    blob: Blob
    #: Present when the blob's length disagreed with the grid in a way the
    #: loader tolerates: "padded" (zero surplus no reader can reach) or
    #: "short" (cells the cartridge leaves holding stale RAM; zero-filled
    #: here, which is what a cold boot shows). See `decode_layout`.
    anomaly: dict | None = None

    def __post_init__(self) -> None:
        if self.plane not in PLANES:
            raise LayoutError(f"Plane must be one of {PLANES}, got {self.plane!r}")

    def chunk_at(self, chunk_x: int, chunk_y: int) -> int:
        if not 0 <= chunk_x < self.width_chunks or not 0 <= chunk_y < self.height_chunks:
            raise LayoutError(
                f"Chunk ({chunk_x}, {chunk_y}) is outside the "
                f"{self.width_chunks}x{self.height_chunks} layout"
            )
        # SetupChunksFG: mulu.w (row_size + 1), d3 / add.w d5, d3
        return self.cells[chunk_y * self.width_chunks + chunk_x]

    @property
    def width_pixels(self) -> int:
        return self.width_chunks * CHUNK_PIXELS_X

    @property
    def height_pixels(self) -> int:
        return self.height_chunks * CHUNK_PIXELS_Y

    @property
    def distinct_chunks(self) -> tuple[int, ...]:
        return tuple(sorted(set(self.cells)))

    def to_json(self) -> dict[str, Any]:
        return {
            "plane": self.plane,
            "width_chunks": self.width_chunks,
            "height_chunks": self.height_chunks,
            "width_pixels": self.width_pixels,
            "height_pixels": self.height_pixels,
            "distinct_chunks": len(self.distinct_chunks),
            "highest_chunk_id": max(self.cells),
            "blob": self.blob.to_json(),
            "sha256": hashlib.sha256(self.cells).hexdigest(),
        }


def decode_layout(
    rom: bytes,
    offset: int,
    width_chunks: int,
    height_chunks: int,
    plane: str = PLANE_FG,
) -> Layout:
    """Decode the layout blob a map record points at.

    `width_chunks`/`height_chunks` are counts, not the record's stored size-1
    bytes; pass them through `dimension_from_header` first.

    The two overworlds (map index 0 and 1) do not use this path at all --
    `loc_539E2`/`loc_53A04` return without consuming a pointer when
    `Field_Map_Index & $FFFE` is zero, and stream their layout from a paged
    table instead. Feeding an overworld record's bytes in here is a mistake this
    function cannot detect, so the caller has to keep the two apart.
    """
    if width_chunks <= 0 or height_chunks <= 0:
        raise LayoutError(
            f"Layout dimensions must be positive, got {width_chunks}x{height_chunks}"
        )
    expected = width_chunks * height_chunks
    if expected > LAYOUT_RAM_BYTES:
        raise LayoutError(
            f"A {width_chunks}x{height_chunks} layout needs {expected} bytes, more "
            f"than the {LAYOUT_RAM_BYTES}-byte Map_Layout region holds"
        )

    data, blob = _decompress_blob(rom, offset, "Layout blob")
    # The loader's actual length rule: `loc_539E2`/`loc_53A04` hand the pointer
    # to KosDecomp and never look at how much came back -- the grid's extent
    # comes from the record's dimension bytes alone. Fourteen retail planes
    # disagree with their grid: seven Academy maps store zero-padded 32x32
    # buffers for 32x16 grids, and ClimCenter_F2's BG pointer is the next
    # map's all-zero 32x32 buffer for a 48x48 grid (a cartridge defect; the
    # short cells hold stale RAM on hardware, zeros on a cold boot). A surplus
    # that is not all zero still fails closed: that means the extent is being
    # read wrong, not that the data is slack.
    anomaly: dict | None = None
    if len(data) > expected:
        if set(data[expected:]) - {0}:
            raise LayoutError(
                f"Layout at 0x{offset:06X} decompresses to {len(data)} bytes for "
                f"a {width_chunks}x{height_chunks} grid and the surplus is not "
                "zero padding; the grid extent is being read wrong"
            )
        anomaly = {
            "plane": plane,
            "kind": "padded",
            "blob_offset": f"0x{offset:06X}",
            "blob_bytes": len(data),
            "grid_bytes": expected,
            "effect": "surplus is zero padding no reader can reach",
        }
        cells = data[:expected]
    elif len(data) < expected:
        anomaly = {
            "plane": plane,
            "kind": "short",
            "blob_offset": f"0x{offset:06X}",
            "blob_bytes": len(data),
            "grid_bytes": expected,
            "effect": (
                f"{expected - len(data)} cells are left holding whatever the "
                "previously loaded map wrote to Map_Layout; zero-filled here"
            ),
        }
        cells = data + bytes(expected - len(data))
    else:
        cells = data
    return Layout(
        plane=plane,
        width_chunks=width_chunks,
        height_chunks=height_chunks,
        cells=cells,
        blob=blob,
        anomaly=anomaly,
    )


# ---------------------------------------------------------------------------
# Collision
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class CollisionGrid:
    """A 4-bit collision type per 16x16-pixel cell."""

    width: int
    height: int
    types: bytes

    def type_at(self, cell_x: int, cell_y: int) -> int:
        if not 0 <= cell_x < self.width or not 0 <= cell_y < self.height:
            raise LayoutError(
                f"Cell ({cell_x}, {cell_y}) is outside the {self.width}x{self.height} grid"
            )
        return self.types[cell_y * self.width + cell_x]

    def name_at(self, cell_x: int, cell_y: int) -> str:
        return collision_type_name(self.type_at(cell_x, cell_y))

    def walkable_at(self, cell_x: int, cell_y: int) -> bool:
        return is_walkable(self.type_at(cell_x, cell_y))

    def histogram(self) -> dict[int, int]:
        counts: dict[int, int] = {}
        for value in self.types:
            counts[value] = counts.get(value, 0) + 1
        return dict(sorted(counts.items()))

    def to_json(self) -> dict[str, Any]:
        return {
            "width_cells": self.width,
            "height_cells": self.height,
            "cell_pixels": COLLISION_CELL_PIXELS,
            "types": {
                f"0x{value:X}": {"name": collision_type_name(value),
                                 "blocking": is_blocking(value),
                                 "cells": count}
                for value, count in self.histogram().items()
            },
            "walkable_cells": sum(1 for v in self.types if is_walkable(v)),
            "sha256": hashlib.sha256(self.types).hexdigest(),
        }


#: `GetChunkAndCollision` reads bit 6 of the bytes at +0, +2, +8 and +$A from
#: the quadrant base -- the high bytes of the words 0, 1, 4 and 5 words along,
#: i.e. the cell's four tiles -- and adds 1, 2, 4 and 8 for them in that order.
_COLLISION_WORD_OFFSETS = (0, 1, CHUNK_TILES_X, CHUNK_TILES_X + 1)


def collision_at(chunks: ChunkTable, layout: Layout, cell_x: int, cell_y: int) -> int:
    """The collision type of one 16x16 cell, straight out of the asm's arithmetic."""
    chunk_id = layout.chunk_at(cell_x // COLLISION_CELL_TILES, cell_y // COLLISION_CELL_TILES)
    words = chunks[chunk_id]
    # andi.w #2,d0 -> +4 bytes (2 words); andi.w #2,d1 -> +$10 bytes (8 words).
    base = (cell_y & 1) * (CHUNK_TILES_X * COLLISION_CELL_TILES) + (cell_x & 1) * COLLISION_CELL_TILES
    value = 0
    for bit, step in enumerate(_COLLISION_WORD_OFFSETS):
        if collision_flag(words[base + step]):
            value |= 1 << bit
    return value


def decode_collision(chunks: ChunkTable, layout: Layout) -> CollisionGrid:
    """Expand a layout into one collision type per 16x16-pixel cell.

    Which layout to pass is not this function's decision: `GetChunkAndCollision`
    reads Map_Layout_BG when the map record's `$FFFFEC24` byte is non-zero and
    Map_Layout_FG when it is zero. `collision_plane` on `MapLayoutSpec` carries
    that byte.
    """
    width = layout.width_chunks * COLLISION_CELL_TILES
    height = layout.height_chunks * COLLISION_CELL_TILES
    out = bytearray(width * height)
    for cell_y in range(height):
        row = cell_y * width
        for cell_x in range(width):
            out[row + cell_x] = collision_at(chunks, layout, cell_x, cell_y)
    return CollisionGrid(width=width, height=height, types=bytes(out))


# ---------------------------------------------------------------------------
# Tile art
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Tileset:
    """One `loc_519D2` entry: Kosinski patterns landing at a fixed VRAM tile."""

    first_tile: int
    tile_count: int
    blob: Blob

    @property
    def end_tile(self) -> int:
        return self.first_tile + self.tile_count

    def to_json(self) -> dict[str, Any]:
        return {
            "first_tile": f"0x{self.first_tile:03X}",
            "end_tile": f"0x{self.end_tile:03X}",
            "tile_count": self.tile_count,
            "blob": self.blob.to_json(),
        }


@dataclass(frozen=True)
class TilePatterns:
    """A VRAM image: 8x8 palette-index tiles, indexed by pattern number."""

    tiles: tuple[bytes, ...]
    loaded: frozenset[int]
    tilesets: tuple[Tileset, ...]

    def tile(self, index: int) -> bytes:
        if not 0 <= index < len(self.tiles):
            raise LayoutError(f"Pattern index 0x{index:X} is outside VRAM")
        return self.tiles[index]

    def to_json(self) -> dict[str, Any]:
        return {
            "loaded_tiles": len(self.loaded),
            "tilesets": [t.to_json() for t in self.tilesets],
        }


BLANK_TILE = bytes(TILE_PIXELS * TILE_PIXELS)


def decode_tilesets(rom: bytes, specs: Sequence[tuple[int, int]]) -> TilePatterns:
    """Rebuild the VRAM tile image a map's tileset list produces.

    `specs` is the `loc_519D2` list as `(first_vram_tile, rom_offset)` pairs.
    Tiles the map never loads stay blank; `loaded` says which are real, so a
    renderer can report a reference to VRAM the map did not fill instead of
    quietly drawing whatever this module happens to hold there.
    """
    tiles: list[bytes] = [BLANK_TILE] * VRAM_TILES
    loaded: set[int] = set()
    sets: list[Tileset] = []

    for first_tile, offset in specs:
        if not 0 <= first_tile < VRAM_TILES:
            raise LayoutError(f"Tileset VRAM tile 0x{first_tile:X} is outside VRAM")
        data, blob = _decompress_blob(rom, offset, "Tileset")
        if len(data) % 32:
            raise LayoutError(
                f"Tileset at 0x{offset:06X} decompresses to {len(data)} bytes, "
                f"which is not a whole number of 32-byte patterns"
            )
        count = len(data) // 32
        if first_tile + count > VRAM_TILES:
            raise LayoutError(
                f"Tileset at 0x{offset:06X} would run past VRAM: "
                f"0x{first_tile:X} + {count} tiles"
            )
        for i in range(count):
            tiles[first_tile + i] = decode_tile(data[i * 32:(i + 1) * 32])
            loaded.add(first_tile + i)
        sets.append(Tileset(first_tile=first_tile, tile_count=count, blob=blob))

    return TilePatterns(tiles=tuple(tiles), loaded=frozenset(loaded), tilesets=tuple(sets))


#: `loc_53F14` copies 16 longs into palette lines 0 and 1, then a fixed line
#: from Pal_Init_Line_3 into line 2, then 8 more longs from the map's blob into
#: line 3. So a map palette is 96 bytes covering CRAM lines 0, 1 and 3.
MAP_PALETTE_BYTES = 96
MAP_PALETTE_LINES = (0, 1, 3)


def decode_map_palette(rom: bytes, offset: int) -> list[dict[str, Any]]:
    """Decode the 48 CRAM words a map's palette pointer supplies."""
    if offset < 0 or offset + MAP_PALETTE_BYTES > len(rom):
        raise LayoutError(f"Map palette at 0x{offset:06X} runs past the end of the ROM")
    return decode_palette(rom[offset:offset + MAP_PALETTE_BYTES])


def chunk_palette(rom: bytes, offset: int) -> list[RGB]:
    """The 32 colours a chunk word can actually select.

    Chunk tiles reach CRAM lines 0 and 1 only, because the bit that would
    select lines 2 and 3 is the collision flag and never reaches the VDP.
    """
    return palette_rgb(decode_map_palette(rom, offset))[:32]


# ---------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------
def compose_layout(
    chunks: ChunkTable,
    layout: Layout,
    patterns: TilePatterns,
    base: bytes | bytearray | None = None,
) -> tuple[int, int, bytearray, set[int]]:
    """Draw one plane's tiles into an index buffer.

    Returns `(width, height, pixels, missing)`, where `missing` collects pattern
    indices the layout referenced but the map's tilesets never loaded.

    Pixel values are `palette_line * 16 + colour_index`, matching a 32-colour
    palette built from CRAM lines 0 and 1.

    With `base` given, this composites on top of it the way the VDP overlays
    plane A on plane B: colour index 0 is transparent, everything else wins.
    """
    width, height = layout.width_pixels, layout.height_pixels
    if base is None:
        pixels = bytearray(width * height)
        overlay = False
    else:
        if len(base) != width * height:
            raise LayoutError(
                f"Base image is {len(base)} bytes, expected {width * height} for a "
                f"{width}x{height} plane"
            )
        pixels = bytearray(base)
        overlay = True

    missing: set[int] = set()
    for chunk_y in range(layout.height_chunks):
        for chunk_x in range(layout.width_chunks):
            words = chunks[layout.chunk_at(chunk_x, chunk_y)]
            for tile_y in range(CHUNK_TILES_Y):
                for tile_x in range(CHUNK_TILES_X):
                    word = vdp_word(words[tile_y * CHUNK_TILES_X + tile_x])
                    index = tile_index(word)
                    if index not in patterns.loaded:
                        missing.add(index)
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
                            if overlay and value == 0:
                                continue
                            pixels[start + x] = value + shift
    return width, height, pixels, missing


def render_layout(
    chunks: ChunkTable,
    layout: Layout,
    patterns: TilePatterns,
    palette: Sequence[RGB] | None = None,
    overlay: Layout | None = None,
) -> bytes:
    """Render one plane -- optionally with a second plane composited over it --
    to an indexed PNG.

    `overlay` is drawn on top with colour 0 transparent, which is how plane A
    sits over plane B. Pass the FG layout as `overlay` and the BG layout as
    `layout` for a map whose ground is on plane B.

    Without a palette the tiles render against a grayscale ramp repeated for
    both CRAM lines, so an unproven palette binding stays visibly unproven.
    """
    width, height, pixels, missing = compose_layout(chunks, layout, patterns)
    if overlay is not None:
        if (overlay.width_chunks, overlay.height_chunks) != (
            layout.width_chunks,
            layout.height_chunks,
        ):
            raise LayoutError(
                f"Overlay plane is {overlay.width_chunks}x{overlay.height_chunks} "
                f"chunks but the base plane is {layout.width_chunks}x{layout.height_chunks}"
            )
        _, _, pixels, extra = compose_layout(chunks, overlay, patterns, pixels)
        missing |= extra

    colours = list(palette) if palette is not None else list(GRAYSCALE_RAMP) * 2
    if len(colours) < 32:
        colours += [(0, 0, 0)] * (32 - len(colours))
    return png.encode_indexed(width, height, bytes(pixels), colours[:32])


#: Walkable, blocking, and the two non-blocking types that still do something:
#: a map change and anything else the disassembly names.
COLLISION_RENDER_PALETTE: tuple[RGB, ...] = (
    (255, 255, 255),  # 0 walkable
    (0, 0, 0),        # 1 blocking
    (208, 32, 48),    # 2 map change
    (48, 96, 208),    # 3 named but non-blocking (recovery and friends)
)


def collision_class(value: int) -> int:
    if value == 0x1:
        return 2
    if is_blocking(value):
        return 1
    if value != 0 and value in COLLISION_TYPE_NAMES:
        return 3
    return 0


def render_collision(grid: CollisionGrid, scale: int = COLLISION_CELL_PIXELS) -> bytes:
    """Render a collision grid to an indexed PNG.

    The default scale is one pixel per map pixel, so the image lines up cell for
    cell with `render_layout`'s output for the same map.
    """
    if scale <= 0:
        raise LayoutError(f"Scale must be positive, got {scale}")
    width, height = grid.width * scale, grid.height * scale
    pixels = bytearray(width * height)
    for cell_y in range(grid.height):
        row = bytearray()
        for cell_x in range(grid.width):
            row += bytes([collision_class(grid.type_at(cell_x, cell_y))]) * scale
        for y in range(scale):
            start = (cell_y * scale + y) * width
            pixels[start:start + width] = row
    return png.encode_indexed(width, height, bytes(pixels), COLLISION_RENDER_PALETTE)


# ---------------------------------------------------------------------------
# One map's layout section, end to end
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class MapLayoutSpec:
    """Everything the layout section needs, as a map-record walker finds it.

    Field for field, this is what `GameMode_LoadFieldMap` reads:

    * `tilesets`    -- the `loc_519D2` list, `(first VRAM tile, ROM offset)`
    * `collision_plane` -- the `$FFFFEC24` byte from `loc_51AB2`; 0 means
      `GetChunkAndCollision` reads the FG layout, non-zero the BG layout
    * `width_chunks` / `height_chunks` -- the record's four dimension bytes plus
      one each, already passed through `dimension_from_header`
    * `chunk_blobs` -- the `Map_LoadChunks` pointer list
    * `layout_fg` / `layout_bg` -- the `loc_539E2` and `loc_53A04` pointers
    * `palette` -- the `Map_Palettes_Addr` pointer, optional
    """

    chunk_blobs: tuple[int, ...]
    layout_fg: int
    layout_bg: int
    width_chunks_fg: int
    height_chunks_fg: int
    width_chunks_bg: int
    height_chunks_bg: int
    collision_plane: int = 0
    tilesets: tuple[tuple[int, int], ...] = ()
    palette: int | None = None
    label: str | None = None

    @classmethod
    def from_header(
        cls,
        chunk_blobs: Iterable[int],
        layout_fg: int,
        layout_bg: int,
        dimension_bytes: Sequence[int],
        collision_plane: int = 0,
        tilesets: Iterable[tuple[int, int]] = (),
        palette: int | None = None,
        label: str | None = None,
    ) -> "MapLayoutSpec":
        """Build a spec from the record's four raw dimension bytes.

        They appear in record order: FG row size, FG column size, BG row size,
        BG column size, each one less than the count.
        """
        if len(dimension_bytes) != 4:
            raise LayoutError(
                f"A map record carries four dimension bytes, got {len(dimension_bytes)}"
            )
        row_fg, column_fg, row_bg, column_bg = dimension_bytes
        return cls(
            chunk_blobs=tuple(chunk_blobs),
            layout_fg=layout_fg,
            layout_bg=layout_bg,
            width_chunks_fg=dimension_from_header(row_fg),
            height_chunks_fg=dimension_from_header(column_fg),
            width_chunks_bg=dimension_from_header(row_bg),
            height_chunks_bg=dimension_from_header(column_bg),
            collision_plane=collision_plane,
            tilesets=tuple(tilesets),
            palette=palette,
            label=label,
        )

    @property
    def collision_plane_name(self) -> str:
        return PLANE_BG if self.collision_plane else PLANE_FG


@dataclass(frozen=True)
class MapLayout:
    """The decoded layout section of one field map."""

    spec: MapLayoutSpec
    chunks: ChunkTable
    fg: Layout
    bg: Layout
    collision: CollisionGrid
    patterns: TilePatterns | None = None

    @property
    def collision_layout(self) -> Layout:
        return self.bg if self.spec.collision_plane else self.fg

    def to_json(self) -> dict[str, Any]:
        out: dict[str, Any] = {
            "label": self.spec.label,
            "collision_plane": self.spec.collision_plane_name,
            "collision_plane_byte": self.spec.collision_plane,
            "chunks": self.chunks.to_json(),
            "layout_fg": self.fg.to_json(),
            "layout_bg": self.bg.to_json(),
            "collision": self.collision.to_json(),
        }
        if self.patterns is not None:
            out["tilesets"] = self.patterns.to_json()
        if self.spec.palette is not None:
            out["palette_rom_offset"] = f"0x{self.spec.palette:06X}"
        return out


def decode_map_layout(
    rom: bytes, spec: MapLayoutSpec, *, with_tiles: bool = False
) -> MapLayout:
    """Decode a whole layout section from one spec.

    This is the entry point a map-record walker should use: fill in a
    `MapLayoutSpec` from the record and hand it over. Set `with_tiles` to also
    rebuild the VRAM tile image, which is only needed for rendering.
    """
    chunks = decode_chunks(rom, spec.chunk_blobs)
    fg = decode_layout(
        rom, spec.layout_fg, spec.width_chunks_fg, spec.height_chunks_fg, PLANE_FG
    )
    bg = decode_layout(
        rom, spec.layout_bg, spec.width_chunks_bg, spec.height_chunks_bg, PLANE_BG
    )
    for layout in (fg, bg):
        highest = max(layout.cells)
        if highest >= len(chunks):
            raise LayoutError(
                f"{layout.plane.upper()} layout names chunk 0x{highest:02X} but the "
                f"map loads only {len(chunks)} chunks"
            )
    collision = decode_collision(chunks, bg if spec.collision_plane else fg)
    patterns = decode_tilesets(rom, spec.tilesets) if with_tiles else None
    return MapLayout(spec=spec, chunks=chunks, fg=fg, bg=bg,
                     collision=collision, patterns=patterns)


def export_map_pngs(
    rom: bytes, spec: MapLayoutSpec, out_dir: str | Path, name: str | None = None
) -> dict[str, Any]:
    """Render one map's composited planes and its collision grid to PNGs.

    Pixel output never enters committed files; this writes under whatever
    directory it is given, which for this project is `generated/`.
    """
    decoded = decode_map_layout(rom, spec, with_tiles=True)
    if decoded.patterns is None or not spec.tilesets:
        raise LayoutError("Rendering needs the map's tileset list")

    palette = chunk_palette(rom, spec.palette) if spec.palette is not None else None
    # Plane A is always in front of plane B, whichever of the two carries the
    # collision data, so the compositing order never depends on EC24.
    base, over = decoded.bg, decoded.fg

    directory = Path(out_dir)
    directory.mkdir(parents=True, exist_ok=True)
    stem = name or spec.label or "map"

    layout_png = directory / f"{stem}.png"
    layout_png.write_bytes(
        render_layout(decoded.chunks, base, decoded.patterns, palette, overlay=over)
    )
    collision_png = directory / f"{stem}_collision.png"
    collision_png.write_bytes(render_collision(decoded.collision))

    result = decoded.to_json()
    result["png"] = {"layout": str(layout_png), "collision": str(collision_png)}
    result["composited"] = {"base_plane": base.plane, "overlay_plane": over.plane}
    return result
