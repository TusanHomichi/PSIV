"""Mega Drive plane composition: art + Enigma plane mapping + palette.

`psiv_tools.gfx` decodes art into 8x8 patterns but deliberately stops there,
because a pattern carries no colour and no position. Both live in the plane
mapping: a run of pattern-name words, one per screen cell, Enigma-compressed
(`psiv_tools.enigma`). This module puts the three together and produces the
picture the VDP would put on screen.

## The pattern-name word

    %P CC V H TTTTTTTTTTT

    P    priority (plane B over plane A when set)
    CC   which of the four 16-colour CRAM lines the cell is drawn with
    V/H  flip the pattern vertically / horizontally
    T    11-bit VRAM tile number

## How a tile number reaches the art

A mapping's tile numbers are **VRAM** tile numbers, not indices into the art
blob. The two are related by where the loader put the art:

    art_index = word_tile - art_vram_tile

`EniDecomp` takes the base in `d0` and adds it to every word it emits, and in
every case in this ROM the low 11 bits of that base are exactly the VRAM tile
the matching art was decompressed to. The battle backgrounds pass `d0 = 0` and
load their art at VRAM address `$0000` (`move.l #$40000000, (a6)` in
`loc_6C3C`); the title portraits pass `$0114`, `$2236`, `$4304` and `$6401`
against a `Title_CharPortraitsArtList` that loads the same four art blobs at
VRAM tiles `$114`, `$236`, `$304` and `$401`. That is proven here rather than
assumed: `compose` refuses any cell whose tile falls outside the art blob, and
all 35 located mappings span their blob's tile range exactly.

The upper bits of the base carry the palette line and priority for the whole
mapping, which is why four title portraits sharing one four-line palette can
each be drawn with a different line without a single flag bit in the stream.

## Colour

The composed pixel buffer is indexed with a 64-entry palette: the whole CRAM
image, four lines of sixteen, so a mapping that mixes palette lines needs no
special handling. A cell's pixel `i` becomes `line * 16 + i`.

Pixel index 0 is the exception. On the VDP it is transparent in *every* line,
and a plane rendered on its own shows the backdrop colour (VDP register
`$8700`, CRAM entry 0) through it. Composed buffers therefore write global
index 0 for it, which is also what the battle loader intends: `loc_6C3C`
clears CRAM entry 0 before copying the background's 13 colours in at index 1.

## Plane geometry

The VDP is set to a 64x32-cell plane (`$9001`) in 40-cell mode, so a plane
buffer is `$1000` bytes and only 40x28 cells of it are on screen. Two shapes
of consumer follow from that:

- Battle backgrounds decompress **straight into `Plane_B_Buffer`**, so the
  mapping is the plane image itself and its row stride is the plane's 64.
- Everything else decompresses to scratch RAM and is blitted by
  `PlaneMapToRAM`, whose `d1`/`d2` give the mapping its own width and height.
  Those two numbers are recorded per mapping below and are checked against the
  decoded word count, which is the dimension proof: 40x11 is 440 words or the
  decode is wrong.

## Retail vs the fork

Nine of the call sites sit inside `if revision=0 ... else ... endif`. The
`reference/` clone builds the `revision=0` side; the cartridge runs the other
one. This is settled by the cartridge three separate ways -- the immediates
themselves (`move.w #$60E3, d0` is present at ROM `0x042596`, `#$60DC` appears
nowhere), the decoded word counts (`MapEni_PressStartButton` decodes 18 words,
which is the `else` branch's 18x1 and not the `revision=0` branch's 17x1), and
the VRAM layout (`$60DC` would point the title background mapping at tiles
220..633 while `ArtNem_TitleBackground` occupies 227..640). Every base tile and
every dimension below is the retail one.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any, NamedTuple, Sequence

from . import png
from .enigma import decompress as enigma_decompress
from .enigma import read_header as enigma_header
from .enigma import words_to_bytes
from .gfx import (
    BATTLE_BG_ART_SYMBOLS,
    BATTLE_BG_PALETTE_COLORS,
    BATTLE_BG_PALETTE_SYMBOLS,
    BATTLE_BG_TABLE,
    COLORS_PER_LINE,
    GRAYSCALE_RAMP,
    NEMESIS_ART,
    PALETTE_LINE_SIZE,
    PALETTES,
    TILE_HEIGHT,
    TILE_WIDTH,
    decode_palette,
    decode_tiles,
    decompress_art,
    palette_rgb,
    read_long,
    _gap_bounds,
    _slice,
)
from .kosinski import decompress as kosinski_decompress

RGB = tuple[int, int, int]

# VDP register $9001 selects a 64x32-cell plane; $8C81 selects 40-cell mode.
PLANE_WIDTH_CELLS = 64
PLANE_HEIGHT_CELLS = 32
PLANE_BUFFER_WORDS = PLANE_WIDTH_CELLS * PLANE_HEIGHT_CELLS
SCREEN_WIDTH_CELLS = 40
SCREEN_HEIGHT_CELLS = 28

CRAM_LINES = 4
CRAM_COLORS = CRAM_LINES * COLORS_PER_LINE


class PlaneError(ValueError):
    pass


# ---------------------------------------------------------------------------
# Pattern-name words
# ---------------------------------------------------------------------------
class PlaneCell(NamedTuple):
    """One decoded pattern-name word."""

    tile: int
    h_flip: bool
    v_flip: bool
    palette_line: int
    priority: bool
    word: int


def decode_cell(word: int) -> PlaneCell:
    if not 0 <= word <= 0xFFFF:
        raise PlaneError(f"0x{word:X} is not a 16-bit pattern-name word")
    return PlaneCell(
        tile=word & 0x07FF,
        h_flip=bool(word & 0x0800),
        v_flip=bool(word & 0x1000),
        palette_line=(word >> 13) & 3,
        priority=bool(word & 0x8000),
        word=word,
    )


def decode_cells(words: Sequence[int]) -> list[PlaneCell]:
    return [decode_cell(w) for w in words]


# ---------------------------------------------------------------------------
# Composition
# ---------------------------------------------------------------------------
def _oriented(tile: bytes, h_flip: bool, v_flip: bool) -> bytes:
    if not h_flip and not v_flip:
        return tile
    rows = [tile[y * TILE_WIDTH:(y + 1) * TILE_WIDTH] for y in range(TILE_HEIGHT)]
    if h_flip:
        rows = [row[::-1] for row in rows]
    if v_flip:
        rows = rows[::-1]
    return b"".join(rows)


def compose(
    cells: Sequence[PlaneCell],
    columns: int,
    tiles: Sequence[bytes],
    art_vram_tile: int = 0,
) -> tuple[int, int, bytes]:
    """Draw `cells` row-major, `columns` across. Returns (w, h, CRAM indices).

    Every cell must resolve to a pattern inside `tiles`; a mapping that reaches
    outside the art blob it was paired with means the base tile or the art is
    wrong, and that is worth failing on rather than drawing garbage.
    """
    if columns <= 0:
        raise PlaneError(f"A plane needs at least one column, got {columns}")
    if not cells:
        raise PlaneError("Cannot compose a plane from zero cells")
    if len(cells) % columns:
        raise PlaneError(
            f"{len(cells)} cells is not a whole number of {columns}-cell rows"
        )

    rows = len(cells) // columns
    width = columns * TILE_WIDTH
    height = rows * TILE_HEIGHT
    pixels = bytearray(width * height)

    for index, cell in enumerate(cells):
        art_index = cell.tile - art_vram_tile
        if not 0 <= art_index < len(tiles):
            raise PlaneError(
                f"Cell {index} references VRAM tile 0x{cell.tile:03X}, which is "
                f"art index {art_index} of a {len(tiles)}-pattern blob loaded at "
                f"VRAM tile 0x{art_vram_tile:03X}"
            )
        tile = _oriented(tiles[art_index], cell.h_flip, cell.v_flip)
        base = cell.palette_line * COLORS_PER_LINE
        ox = (index % columns) * TILE_WIDTH
        oy = (index // columns) * TILE_HEIGHT
        for y in range(TILE_HEIGHT):
            start = (oy + y) * width + ox
            row = tile[y * TILE_WIDTH:(y + 1) * TILE_WIDTH]
            # Index 0 is transparent in every CRAM line, so it shows the
            # backdrop colour (CRAM entry 0) rather than colour 0 of the line.
            pixels[start:start + TILE_WIDTH] = bytes(
                base + p if p else 0 for p in row
            )
    return width, height, bytes(pixels)


def render(
    cells: Sequence[PlaneCell],
    columns: int,
    tiles: Sequence[bytes],
    cram: Sequence[RGB],
    art_vram_tile: int = 0,
    transparent_backdrop: bool = False,
) -> bytes:
    """Compose and encode as an indexed PNG against a whole 64-colour CRAM."""
    width, height, pixels = compose(cells, columns, tiles, art_vram_tile)
    palette = list(cram)
    if len(palette) != CRAM_COLORS:
        raise PlaneError(f"A CRAM image is {CRAM_COLORS} colours, got {len(palette)}")
    return png.encode_indexed(
        width, height, pixels, palette, (0,) if transparent_backdrop else ()
    )


# ---------------------------------------------------------------------------
# CRAM images
# ---------------------------------------------------------------------------
def grayscale_cram() -> list[RGB]:
    """Four copies of the index ramp, for art whose palette is not pinned."""
    return list(GRAYSCALE_RAMP) * CRAM_LINES


def named_cram(data: bytes, label: str) -> list[RGB]:
    """The four CRAM lines of one palette in `gfx.PALETTES`."""
    spec = next((p for p in PALETTES if p["label"] == label), None)
    if spec is None:
        raise PlaneError(f"Unknown palette {label!r}")
    if spec["line_count"] < CRAM_LINES:
        raise PlaneError(
            f"{label} holds {spec['line_count']} lines; a CRAM image needs {CRAM_LINES}"
        )
    raw = _slice(data, spec["rom_offset"], CRAM_LINES * PALETTE_LINE_SIZE, label)
    return palette_rgb(decode_palette(raw))


def battle_cram(colors: Sequence[dict[str, Any]]) -> list[RGB]:
    """`loc_6C3C`'s CRAM image: entry 0 forced black, 13 colours after it.

    Battle backgrounds only ever use line 0 (asserted by the extractor), so the
    other three lines are whatever the previous screen left behind; they are
    filled with black here rather than guessed at.
    """
    line = [(0, 0, 0)] + palette_rgb(colors)
    if len(line) > COLORS_PER_LINE:
        raise PlaneError(f"A CRAM line holds {COLORS_PER_LINE} colours, got {len(line)}")
    line += [(0, 0, 0)] * (COLORS_PER_LINE - len(line))
    return line + [(0, 0, 0)] * (COLORS_PER_LINE * (CRAM_LINES - 1))


# ---------------------------------------------------------------------------
# Art the mappings are paired with
# ---------------------------------------------------------------------------
# The four title portraits are Kosinski-compressed, not Nemesis, and are
# reached through `Title_CharPortraitsArtList` (ps4.asm:87318), which pairs
# each art pointer with the VRAM tile it is loaded at. The offsets were located
# by matching the disassembly's `graphics/title/*Kosinski.bin` payloads against
# the retail image, the same way `gfx.NEMESIS_ART` was, and each blob's
# decompressor must stop before the next one starts.
KOSINSKI_ART: list[dict[str, Any]] = [
    {"label": "ArtKos_RuneTitlePortrait", "rom_offset": 0x2F114E, "tile_count": 290},
    {"label": "ArtKos_RikaTitlePortrait", "rom_offset": 0x2F1E6E, "tile_count": 206},
    {"label": "ArtKos_WrenTitlePortrait", "rom_offset": 0x2F2A9E, "tile_count": 253},
    {"label": "ArtKos_ChazTitlePortrait", "rom_offset": 0x2F3AAE, "tile_count": 215},
]


def art_tiles(data: bytes, label: str) -> tuple[list[bytes], dict[str, Any]]:
    """Decode one named art blob to patterns, with its provenance record."""
    nemesis = next((s for s in NEMESIS_ART if s["label"] == label), None)
    if nemesis is not None:
        decompressed, record = decompress_art(
            data, nemesis["rom_offset"], label, compressed_size=nemesis["compressed_size"]
        )
        return decode_tiles(decompressed), record

    kos = next((s for s in KOSINSKI_ART if s["label"] == label), None)
    if kos is None:
        raise PlaneError(f"No art blob named {label!r}")
    offset = kos["rom_offset"]
    decompressed, consumed = kosinski_decompress(data, offset)
    tiles = decode_tiles(decompressed)
    if len(tiles) != kos["tile_count"]:
        raise PlaneError(
            f"{label} at 0x{offset:06X} decoded {len(tiles)} patterns, expected "
            f"{kos['tile_count']}"
        )
    record = {
        "label": label,
        "rom_offset": f"0x{offset:06X}",
        "compression": "kosinski",
        "compressed_size": consumed,
        "rom_end_exclusive": f"0x{offset + consumed:06X}",
        "tile_count": len(tiles),
        "decompressed_size": len(decompressed),
        "decompressed_sha256": hashlib.sha256(decompressed).hexdigest(),
        "compressed_sha256": hashlib.sha256(
            _slice(data, offset, consumed, label)
        ).hexdigest(),
    }
    return tiles, record


# ---------------------------------------------------------------------------
# The located mappings.
#
# `base_tile` is the caller's `d0`; `columns`/`rows` are `PlaneMapToRAM`'s
# `d1`/`d2` at the same call site (both are the register's value, since the
# routine's `subq.w #1` is undone by `dbf` running one extra time). Battle
# backgrounds are not here: they come out of the pointer table at runtime.
# ---------------------------------------------------------------------------
MAPPINGS: list[dict[str, Any]] = [
    {
        "label": "MapEni_SegaLogo",
        "rom_offset": 0x0415BC,
        "compressed_size": 32,
        "base_tile": 0x0010,
        "columns": 0x11,
        "rows": 5,
        "art": "ArtNem_SegaLogo",
        "art_vram_tile": 0x010,
        "palette": None,
        "source": "GameMode_Sega, ps4.asm:84268 (revision != 0 branch)",
        "note": (
            "the loader fills all 64 CRAM entries with $0E00 before fading in, "
            "so this screen has no per-line palette to render against"
        ),
    },
    {
        "label": "MapEni_TitlePSTitle",
        "rom_offset": 0x1D20F2,
        "compressed_size": 52,
        "base_tile": 0x0010,
        "columns": 0x11,
        "rows": 0x0D,
        "art": "ArtNem_TitlePSTitle",
        "art_vram_tile": 0x010,
        "palette": "Pal_TitleScreen",
        "source": "TitleRoutine_FadingText, ps4.asm:86495",
    },
    {
        "label": "MapEni_TitleTheEndOfTheMillennium",
        "rom_offset": 0x1D2126,
        "compressed_size": 44,
        "base_tile": 0x0092,
        "columns": 0x1D,
        "rows": 3,
        "art": "ArtNem_TitleTheEndOfTheMillennium",
        "art_vram_tile": 0x092,
        "palette": "Pal_TitleScreen",
        "source": "TitleRoutine_FadingText, ps4.asm:86504 (revision != 0 branch)",
    },
    {
        "label": "MapEni_TitleCopyrightText",
        "rom_offset": 0x1D2152,
        "compressed_size": 14,
        "base_tile": 0x20CA,
        "columns": 0x11,
        "rows": 1,
        "art": "ArtNem_TitleCopyrightText",
        "art_vram_tile": 0x0CA,
        "palette": "Pal_TitleScreen",
        "source": "loc_4295C, ps4.asm:86621 (revision != 0 branch)",
    },
    {
        "label": "MapEni_PressStartButton",
        "rom_offset": 0x1D2160,
        "compressed_size": 18,
        "base_tile": 0x40D8,
        "columns": 0x12,
        "rows": 1,
        "art": "ArtNem_PressStartButton",
        "art_vram_tile": 0x0D8,
        "palette": "Pal_TitleScreen",
        "source": "loc_4295C, ps4.asm:86612 (revision != 0 branch)",
    },
    {
        "label": "MapEni_TitleBarBGTopPart",
        "rom_offset": 0x1D2172,
        "compressed_size": 204,
        "base_tile": 0x60E3,
        "columns": 8,
        "rows": 0x1C,
        "art": "ArtNem_TitleBackground",
        "art_vram_tile": 0x0E3,
        "palette": "Pal_TitleScreen",
        "source": "TitleRoutine_FadingText, ps4.asm:86291 (revision != 0 branch)",
    },
    {
        "label": "MapEni_TitleBarBGBottomPart",
        "rom_offset": 0x1D223E,
        "compressed_size": 140,
        "base_tile": 0x60E3,
        "columns": 8,
        "rows": 0x1C,
        "art": "ArtNem_TitleBackground",
        "art_vram_tile": 0x0E3,
        "palette": "Pal_TitleScreen",
        "source": "TitleRoutine_FadingText, ps4.asm:86295 (revision != 0 branch)",
    },
    {
        "label": "MapEni_TitleBGLeftPart",
        "rom_offset": 0x1D22CA,
        "compressed_size": 162,
        "base_tile": 0x60E3,
        "columns": 6,
        "rows": 0x1C,
        "art": "ArtNem_TitleBackground",
        "art_vram_tile": 0x0E3,
        "palette": "Pal_TitleScreen",
        "source": "TitleRoutine_FadingText, ps4.asm:86299 (revision != 0 branch)",
    },
    {
        "label": "MapEni_TitleBGRightPart",
        "rom_offset": 0x1D236C,
        "compressed_size": 594,
        "base_tile": 0x60E3,
        "columns": 0x1A,
        "rows": 0x1C,
        "art": "ArtNem_TitleBackground",
        "art_vram_tile": 0x0E3,
        "palette": "Pal_TitleScreen",
        "source": "TitleRoutine_FadingText, ps4.asm:86303 (revision != 0 branch)",
    },
    {
        "label": "MapEni_GameStartMotaBG",
        "rom_offset": 0x1D25BE,
        "compressed_size": 338,
        "base_tile": 0x2010,
        "columns": 0x28,
        "rows": 0x10,
        "art": "ArtNem_GameStartMotaBG",
        "art_vram_tile": 0x010,
        "palette": None,
        "source": (
            "ROM 0x073C4E; the label is unreferenced in the disassembly's source, "
            "so the base tile and dimensions come from the cartridge's own code"
        ),
    },
    {
        "label": "MapEni_TitleScrollingTextBG",
        "rom_offset": 0x1D2710,
        "compressed_size": 336,
        "base_tile": 0x0010,
        "columns": 0x28,
        "rows": 0x10,
        "art": "ArtNem_TitleScrollingTextBG",
        "art_vram_tile": 0x010,
        "palette": "Pal_TitleScrollingText",
        "source": "TitleRoutine_ScrollingText, ps4.asm:87195",
        "note": (
            "the disassembly's binclude is 684 bytes; only the first 336 are the "
            "Enigma stream, the rest is sprite-mapping data"
        ),
    },
    {
        "label": "MapEni_RuneTitlePortrait",
        "rom_offset": 0x2F478E,
        "compressed_size": 142,
        "base_tile": 0x0114,
        "columns": 0x28,
        "rows": 0x0B,
        "art": "ArtKos_RuneTitlePortrait",
        "art_vram_tile": 0x114,
        "palette": "Pal_TitleCharPortraits",
        "source": "TitleRoutine_CharPortraits, ps4.asm:87097",
    },
    {
        "label": "MapEni_RikaTitlePortrait",
        "rom_offset": 0x2F481C,
        "compressed_size": 84,
        "base_tile": 0x2236,
        "columns": 0x28,
        "rows": 0x0B,
        "art": "ArtKos_RikaTitlePortrait",
        "art_vram_tile": 0x236,
        "palette": "Pal_TitleCharPortraits",
        "source": "TitleRoutine_CharPortraits, ps4.asm:87127",
    },
    {
        "label": "MapEni_WrenTitlePortrait",
        "rom_offset": 0x2F4870,
        "compressed_size": 134,
        "base_tile": 0x4304,
        "columns": 0x0E,
        "rows": 0x1C,
        "art": "ArtKos_WrenTitlePortrait",
        "art_vram_tile": 0x304,
        "palette": "Pal_TitleCharPortraits",
        "source": "TitleRoutine_CharPortraits, ps4.asm:87112",
    },
    {
        "label": "MapEni_ChazTitlePortrait",
        "rom_offset": 0x2F48F6,
        "compressed_size": 158,
        "base_tile": 0x6401,
        "columns": 0x0E,
        "rows": 0x1C,
        "art": "ArtKos_ChazTitlePortrait",
        "art_vram_tile": 0x401,
        "palette": "Pal_TitleCharPortraits",
        "source": "TitleRoutine_CharPortraits, ps4.asm:87143",
    },
]

# Battle backgrounds decompress straight into Plane_B_Buffer, so their row
# stride is the plane's and their height is whatever the stream produces --
# 24 rows for every one of the twenty, leaving the bottom of the plane for the
# battle window.
BATTLE_BG_COLUMNS = PLANE_WIDTH_CELLS
BATTLE_BG_ROWS = 24
BATTLE_BG_BASE_TILE = 0  # moveq #0, d0 in loc_6C3C
BATTLE_BG_ART_VRAM_TILE = 0  # move.l #$40000000, (a6) -- VRAM address 0

# Seventeen of the twenty mappings are followed by another pointer from
# BattleBGArtPtrs, which bounds them without leaving the cartridge. The other
# three end the pointer table's coverage, so their ends are named here. Each is
# a label the disassembly puts immediately after the mapping, and `loc_27AE00`
# is additionally a live pointer: it is the first entry of `loc_748A`, the
# vehicle-battle art list (ps4.asm:10478).
BATTLE_BG_EXTRA_LANDMARKS: dict[int, str] = {
    0x237A60: "loc_237A60",
    0x23A226: "loc_23A226",
    0x27AE00: "loc_27AE00 (first entry of the vehicle-battle art list loc_748A)",
}


# ---------------------------------------------------------------------------
# Decoding one mapping
# ---------------------------------------------------------------------------
def decode_mapping(
    data: bytes,
    offset: int,
    label: str,
    base_tile: int = 0,
    compressed_size: int | None = None,
    max_compressed_size: int | None = None,
    expected_words: int | None = None,
    max_words: int | None = PLANE_BUFFER_WORDS,
) -> tuple[list[int], dict[str, Any]]:
    """Decompress one Enigma plane mapping and check everything known about it.

    `compressed_size` is an exact documented length and must be hit exactly --
    unlike Nemesis, `EniDecomp` rewinds over its lookahead before reporting,
    so there is no off-by-one to allow for.
    """
    header = enigma_header(data, offset, base_tile)
    words, consumed = enigma_decompress(data, offset, base_tile, max_words=max_words)

    if compressed_size is not None and consumed != compressed_size:
        raise PlaneError(
            f"{label} at 0x{offset:06X}: decompressor consumed {consumed} bytes, "
            f"but the stream is documented as {compressed_size}"
        )
    if max_compressed_size is not None and consumed > max_compressed_size:
        raise PlaneError(
            f"{label} at 0x{offset:06X}: decompressor consumed {consumed} bytes and "
            f"ran into the next structure, which starts {max_compressed_size} bytes in"
        )
    if expected_words is not None and len(words) != expected_words:
        raise PlaneError(
            f"{label} at 0x{offset:06X}: decoded {len(words)} cells, but its "
            f"consumer copies {expected_words}"
        )

    size = compressed_size if compressed_size is not None else consumed
    cells = decode_cells(words)
    tiles = [c.tile for c in cells]
    record: dict[str, Any] = {
        "label": label,
        "rom_offset": f"0x{offset:06X}",
        "compression": "enigma",
        "compressed_size": size,
        "rom_end_exclusive": f"0x{offset + size:06X}",
        "compressed_sha256": hashlib.sha256(_slice(data, offset, size, label)).hexdigest(),
        "base_tile": f"0x{base_tile:04X}",
        "inline_bits": header.inline_bits,
        "flag_bits": {
            "raw": f"0x{header.flags:02X}",
            "v_flip": header.uses_v_flip,
            "h_flip": header.uses_h_flip,
        },
        "incremental_word": f"0x{header.incremental_word:04X}",
        "literal_word": f"0x{header.literal_word:04X}",
        "cell_count": len(words),
        "distinct_words": len(set(words)),
        "tile_range": [min(tiles), max(tiles)],
        "palette_lines": sorted({c.palette_line for c in cells}),
        "cells_h_flipped": sum(1 for c in cells if c.h_flip),
        "cells_v_flipped": sum(1 for c in cells if c.v_flip),
        "cells_high_priority": sum(1 for c in cells if c.priority),
        "decoded_sha256": hashlib.sha256(words_to_bytes(words)).hexdigest(),
    }
    return words, record


# ---------------------------------------------------------------------------
# Extraction
# ---------------------------------------------------------------------------
def _battle_pointers(data: bytes) -> tuple[list[int], list[int], list[int]]:
    spec = BATTLE_BG_TABLE
    base, count, stride = spec["rom_offset"], spec["entry_count"], spec["entry_size"]
    art = [read_long(data, base + i * stride) for i in range(count)]
    mapping = [read_long(data, base + i * stride + 4) for i in range(count)]
    palette = [read_long(data, base + i * stride + 8) for i in range(count)]
    return art, mapping, palette


def extract_battle_background_planes(data: bytes) -> dict[str, Any]:
    """Decode all 20 distinct battle-background plane mappings.

    The mappings sit in the same adjacency chain as the art: each background's
    Enigma mapping follows its Nemesis art immediately, and each mapping is
    followed by the next background's art. Requiring the decoder to consume
    exactly that gap turns the pointer table into an exact length oracle for
    both halves, with nothing outside the cartridge involved.
    """
    art_ptrs, map_ptrs, pal_ptrs = _battle_pointers(data)
    landmarks = art_ptrs + map_ptrs + list(BATTLE_BG_EXTRA_LANDMARKS)
    map_bounds = _gap_bounds(landmarks, len(data), map_ptrs)
    art_bounds = _gap_bounds(landmarks, len(data), art_ptrs)

    mappings: dict[int, dict[str, Any]] = {}
    arts: dict[int, dict[str, Any]] = {}
    entries = []
    for index in range(BATTLE_BG_TABLE["entry_count"]):
        art_ptr, map_ptr, pal_ptr = art_ptrs[index], map_ptrs[index], pal_ptrs[index]
        art_symbol = BATTLE_BG_ART_SYMBOLS[index]
        symbol = BATTLE_BG_PALETTE_SYMBOLS[index]
        if art_ptr not in arts:
            _, arts[art_ptr] = decompress_art(
                data, art_ptr, f"ArtNem_{art_symbol}BattleBG",
                compressed_size=art_bounds[art_ptr],
            )
        if map_ptr not in mappings:
            _, record = decode_mapping(
                data,
                map_ptr,
                f"MapEni_{art_symbol}BattleBG",
                base_tile=BATTLE_BG_BASE_TILE,
                compressed_size=map_bounds[map_ptr],
                expected_words=BATTLE_BG_COLUMNS * BATTLE_BG_ROWS,
            )
            art = arts[art_ptr]
            low, high = record["tile_range"]
            if high >= art["tile_count"]:
                raise PlaneError(
                    f"{record['label']} references VRAM tile 0x{high:03X}, past the "
                    f"end of its {art['tile_count']}-pattern art blob"
                )
            record["end_oracle"] = BATTLE_BG_EXTRA_LANDMARKS.get(
                map_ptr + record["compressed_size"], "next pointer in BattleBGArtPtrs"
            )
            record["columns"] = BATTLE_BG_COLUMNS
            record["rows"] = BATTLE_BG_ROWS
            record["art_vram_tile"] = f"0x{BATTLE_BG_ART_VRAM_TILE:03X}"
            record["art_label"] = art["label"]
            record["art_tile_count"] = art["tile_count"]
            record["spans_art_exactly"] = high == art["tile_count"] - 1
            mappings[map_ptr] = record
        raw = _slice(data, pal_ptr, BATTLE_BG_PALETTE_COLORS * 2, f"battle palette {index:02X}")
        entries.append({
            "index": index,
            "symbol": symbol,
            "art_symbol": art_symbol,
            "art": arts[art_ptr],
            "plane_mapping": mappings[map_ptr],
            "palette": {
                "label": f"Pal_{symbol}BattleBG",
                "rom_offset": f"0x{pal_ptr:06X}",
                "colors": decode_palette(raw),
            },
        })

    return {
        "label": BATTLE_BG_TABLE["label"],
        "table_rom_offset": f"0x{BATTLE_BG_TABLE['rom_offset']:06X}",
        "entry_count": BATTLE_BG_TABLE["entry_count"],
        "distinct_mappings": len(mappings),
        "plane_size_cells": [BATTLE_BG_COLUMNS, BATTLE_BG_ROWS],
        "destination": "Plane_B_Buffer ($FFFF9000), written directly by EniDecomp",
        "base_tile": f"0x{BATTLE_BG_BASE_TILE:04X}",
        "entries": entries,
    }


def extract_named_planes(data: bytes) -> list[dict[str, Any]]:
    """Decode the Sega, title-screen and title-portrait mappings."""
    records = []
    for spec in MAPPINGS:
        _, record = decode_mapping(
            data,
            spec["rom_offset"],
            spec["label"],
            base_tile=spec["base_tile"],
            compressed_size=spec["compressed_size"],
            expected_words=spec["columns"] * spec["rows"],
        )
        tiles, art = art_tiles(data, spec["art"])
        low, high = record["tile_range"]
        first = spec["art_vram_tile"]
        if low < first or high >= first + len(tiles):
            raise PlaneError(
                f"{spec['label']} references VRAM tiles 0x{low:03X}..0x{high:03X}, "
                f"outside the {len(tiles)} patterns {art['label']} loads at "
                f"0x{first:03X}"
            )
        record["columns"] = spec["columns"]
        record["rows"] = spec["rows"]
        record["art_label"] = art["label"]
        record["art_vram_tile"] = f"0x{first:03X}"
        record["art_tile_count"] = len(tiles)
        record["spans_art_exactly"] = (low, high) == (first, first + len(tiles) - 1)
        record["palette"] = spec["palette"] or "grayscale index ramp"
        record["source"] = spec["source"]
        if "note" in spec:
            record["note"] = spec["note"]
        records.append(record)
    return records


def extract_planes(data: bytes) -> dict[str, Any]:
    """Decode every located plane mapping; return metadata only, no pixels."""
    named = extract_named_planes(data)
    battle = extract_battle_background_planes(data)
    distinct_battle = {
        e["plane_mapping"]["rom_offset"]: e["plane_mapping"]["cell_count"]
        for e in battle["entries"]
    }
    return {
        "pattern_name_word": "%PCCVHTTTTTTTTTTT (priority, palette line, V/H flip, VRAM tile)",
        "plane_size_cells": [PLANE_WIDTH_CELLS, PLANE_HEIGHT_CELLS],
        "screen_size_cells": [SCREEN_WIDTH_CELLS, SCREEN_HEIGHT_CELLS],
        "tile_index_semantics": (
            "mapping tile numbers are VRAM tile numbers; art index = tile - "
            "art_vram_tile, and the low 11 bits of a mapping's base tile are the "
            "VRAM tile its art was loaded at"
        ),
        "named": named,
        "battle_backgrounds": battle,
        "distinct_mappings": len(named) + len(distinct_battle),
        "total_cells_decoded": (
            sum(r["cell_count"] for r in named) + sum(distinct_battle.values())
        ),
    }


# ---------------------------------------------------------------------------
# PNG export (never committed; write under generated/ or a temp directory)
# ---------------------------------------------------------------------------
def _safe_name(label: str) -> str:
    return "".join(c if c.isalnum() or c in "-_" else "_" for c in label)


def export_plane_pngs(data: bytes, out_dir: str | Path) -> list[dict[str, Any]]:
    """Compose every located plane mapping to a PNG. Returns what was written.

    The output directory holds decoded Sega artwork and must stay out of
    version control.
    """
    directory = Path(out_dir)
    directory.mkdir(parents=True, exist_ok=True)
    written: list[dict[str, Any]] = []

    def emit(label: str, cells, columns, tiles, cram, art_vram_tile, palette_name):
        image = render(cells, columns, tiles, cram, art_vram_tile)
        path = directory / f"{_safe_name(label)}.png"
        path.write_bytes(image)
        written.append({
            "label": label,
            "path": str(path),
            "cells": len(cells),
            "size": [columns * TILE_WIDTH, (len(cells) // columns) * TILE_HEIGHT],
            "palette": palette_name,
            "sha256": hashlib.sha256(image).hexdigest(),
        })

    for spec in MAPPINGS:
        words, _ = decode_mapping(
            data,
            spec["rom_offset"],
            spec["label"],
            base_tile=spec["base_tile"],
            compressed_size=spec["compressed_size"],
            expected_words=spec["columns"] * spec["rows"],
        )
        tiles, _ = art_tiles(data, spec["art"])
        cram = named_cram(data, spec["palette"]) if spec["palette"] else grayscale_cram()
        emit(
            spec["label"], decode_cells(words), spec["columns"], tiles, cram,
            spec["art_vram_tile"], spec["palette"] or "grayscale index ramp",
        )

    art_ptrs, map_ptrs, pal_ptrs = _battle_pointers(data)
    landmarks = art_ptrs + map_ptrs + list(BATTLE_BG_EXTRA_LANDMARKS)
    map_bounds = _gap_bounds(landmarks, len(data), map_ptrs)
    art_bounds = _gap_bounds(landmarks, len(data), art_ptrs)
    for index in range(BATTLE_BG_TABLE["entry_count"]):
        art_ptr, map_ptr, pal_ptr = art_ptrs[index], map_ptrs[index], pal_ptrs[index]
        art_symbol = BATTLE_BG_ART_SYMBOLS[index]
        symbol = BATTLE_BG_PALETTE_SYMBOLS[index]
        label = f"MapEni_{art_symbol}BattleBG_{symbol}"
        decompressed, _ = decompress_art(
            data, art_ptr, f"ArtNem_{art_symbol}BattleBG",
            compressed_size=art_bounds[art_ptr],
        )
        words, _ = decode_mapping(
            data, map_ptr, f"MapEni_{art_symbol}BattleBG",
            base_tile=BATTLE_BG_BASE_TILE,
            compressed_size=map_bounds[map_ptr],
            expected_words=BATTLE_BG_COLUMNS * BATTLE_BG_ROWS,
        )
        raw = _slice(data, pal_ptr, BATTLE_BG_PALETTE_COLORS * 2, label)
        emit(
            label, decode_cells(words), BATTLE_BG_COLUMNS, decode_tiles(decompressed),
            battle_cram(decode_palette(raw)), BATTLE_BG_ART_VRAM_TILE,
            f"Pal_{symbol}BattleBG",
        )

    return written
