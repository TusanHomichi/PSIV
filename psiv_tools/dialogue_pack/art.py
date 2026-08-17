"""The dialogue font, waiting sprite and portraits: pixels and their maps.

All three render against `Pal_Init_Line_3`, which is `window.py`'s business;
what is here is the glyph format, the arrow's two font tiles, the strip layout
and the portrait table.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any, Sequence

from .. import png
from ..gfx import (
    COLORS_PER_LINE,
    NEMESIS_ART,
    PORTRAIT_COLUMNS,
    PORTRAIT_TILES,
    compose_sheet,
    decode_tiles,
    decompress_art,
    extract_dialogue_portraits,
)
from ..text import DIALOGUE_CHARSET
from .common import (
    DIALOGUE_FORMAT_VERSION,
    FONT_PNG_NAME,
    PORTRAITS_DIRECTORY,
    SCROLL_ARROW_PNG_NAME,
    DialoguePackError,
    rom_slice,
    safe_name,
)
from .window import (
    BACKGROUND_COLOR_INDEX,
    DIALOGUE_CRAM_LINE,
    FONT_ROM_OFFSET,
    FONT_SIZE,
    GLYPH_BYTES,
    GLYPH_COUNT,
    GLYPH_HEIGHT,
    GLYPH_INDEX_MASK,
    GLYPH_WIDTH,
    PAL_INIT_LINE_3,
    TEXT_COLOR_INDEX,
    TILE_PIXELS,
)

#: `TextCtrlCode_Portrait`: `move.w #5,d3 / move.w #$D,d4`, plus
#: `mulu.w #$C,d5` for the Talk command's second operand.
PORTRAIT_TILE_X = 5
PORTRAIT_TILE_Y = 0xD
PORTRAIT_TALK_SLOT_TILES = 12
#: The cutscene placement, taken when `Game_Mode_Routine` is $C, `Event_Index`
#: bit 7 is set and `Render_Sprites_In_Cutscenes` is on.
PORTRAIT_CUTSCENE_TILE_X = 3
PORTRAIT_CUTSCENE_TILE_Y = 0xE
PORTRAIT_PLANE_MAP = 0x2A2B36
PORTRAIT_VRAM_TILE = 0x55C
PORTRAIT_TRANSPARENT_INDEX = 0

# `FieldObj_ScrollTextArrow` selects the two adjacent patterns beginning at
# `$7F6`. They are the retail menu-font blob at `$7C0`, not the 1bpp dialogue
# font and not a triangle reconstructed from its position.
SCROLL_ARROW_ART_LABEL = "ArtNem_Font"
SCROLL_ARROW_ART_ROM_OFFSET = 0x2A303A
SCROLL_ARROW_ART_COMPRESSED_SIZE = 790
SCROLL_ARROW_FONT_VRAM_TILE = 0x7C0
SCROLL_ARROW_VRAM_TILE = 0x7F6
SCROLL_ARROW_TILE_INDEX = SCROLL_ARROW_VRAM_TILE - SCROLL_ARROW_FONT_VRAM_TILE
SCROLL_ARROW_TILE_COUNT = 2
SCROLL_ARROW_MAPPING_ROM_OFFSET = 0x046532
SCROLL_ARROW_MAPPING_BYTES = bytes.fromhex("00000004000000F0")
SCROLL_ARROW_WIDTH = TILE_PIXELS * SCROLL_ARROW_TILE_COUNT
SCROLL_ARROW_HEIGHT = TILE_PIXELS
SCROLL_ARROW_TRANSPARENT_INDEX = 0

# ---------------------------------------------------------------------------
# Font
# ---------------------------------------------------------------------------
def glyph_bitmaps(rom: bytes) -> list[list[str]]:
    """The 80 glyphs as rows of `'0'`/`'1'`, straight out of `Art_DialogueFont`."""
    raw = rom_slice(rom, FONT_ROM_OFFSET, FONT_SIZE, "Art_DialogueFont")
    glyphs = []
    for index in range(GLYPH_COUNT):
        rows = raw[index * GLYPH_BYTES:(index + 1) * GLYPH_BYTES]
        glyphs.append([f"{byte:08b}" for byte in rows])
    return glyphs


def font_strip(glyphs: Sequence[Sequence[str]]) -> tuple[int, int, bytes]:
    """One row of glyphs, left to right, as palette indices.

    A set bit is `$F` and a clear bit is `$E`, which is what `ParseText` builds
    out of the 1bpp source before the DMA. The strip is the glyph *index*
    space, not the charset: cell N is byte N, so a consumer needs no table to
    find a glyph it already has the byte for.
    """
    width = len(glyphs) * GLYPH_WIDTH
    pixels = bytearray(width * GLYPH_HEIGHT)
    for index, rows in enumerate(glyphs):
        left = index * GLYPH_WIDTH
        for y, row in enumerate(rows):
            start = y * width + left
            for x, bit in enumerate(row):
                pixels[start + x] = (
                    TEXT_COLOR_INDEX if bit == "1" else BACKGROUND_COLOR_INDEX
                )
    return width, GLYPH_HEIGHT, bytes(pixels)


def font_json(
    rom: bytes,
    glyphs: Sequence[Sequence[str]],
    palette: Sequence[tuple[int, int, int]],
    image: bytes,
) -> dict[str, Any]:
    raw = rom_slice(rom, FONT_ROM_OFFSET, FONT_SIZE, "Art_DialogueFont")
    by_char = {char: byte for byte, char in DIALOGUE_CHARSET.items()}
    entries = []
    for index, rows in enumerate(glyphs):
        entries.append({
            "byte": index,
            "byte_hex": f"0x{index:02X}",
            "char": DIALOGUE_CHARSET.get(index),
            "x": index * GLYPH_WIDTH,
            "y": 0,
            "width": GLYPH_WIDTH,
            "height": GLYPH_HEIGHT,
            "blank": all(row == "0" * GLYPH_WIDTH for row in rows),
        })
    return {
        "format_version": DIALOGUE_FORMAT_VERSION,
        "kind": "dialogue_font",
        "png": FONT_PNG_NAME,
        "png_sha256": hashlib.sha256(image).hexdigest(),
        "charset": "dialogue",
        "charset_source": "script/charset.asm",
        "source": {
            "label": "Art_DialogueFont",
            "rom_offset": f"0x{FONT_ROM_OFFSET:06X}",
            "rom_end_exclusive": f"0x{FONT_ROM_OFFSET + FONT_SIZE:06X}",
            "compression": None,
            "size_bytes": FONT_SIZE,
            "sha256": hashlib.sha256(raw).hexdigest(),
        },
        "glyph": {
            "width": GLYPH_WIDTH,
            "height": GLYPH_HEIGHT,
            "bits_per_pixel": 1,
            "bytes_per_glyph": GLYPH_BYTES,
            "count": GLYPH_COUNT,
            "index_mask": f"0x{GLYPH_INDEX_MASK:02X}",
            "vram_patterns_per_glyph": GLYPH_HEIGHT // TILE_PIXELS,
            "note": (
                "GetFontGraphics indexes the font by the text byte itself "
                "(masked with $7F); the strip is laid out so cell N is byte N. "
                "ParseText expands each 1bpp row into eight 4bpp pixels, which "
                "is the only place the two 8x8 VRAM patterns exist."
            ),
        },
        "palette": {
            "source": "Pal_Init_Line_3",
            "rom_offset": f"0x{PAL_INIT_LINE_3:06X}",
            "cram_line": DIALOGUE_CRAM_LINE,
            "text_index": TEXT_COLOR_INDEX,
            "background_index": BACKGROUND_COLOR_INDEX,
            "transparent_index": BACKGROUND_COLOR_INDEX,
            "colors": [list(colour) for colour in palette],
            "note": (
                "The glyph cell's background is index $E, which "
                "FillTextBackground paints across the whole window; the PNG "
                "marks it transparent so glyphs composite over a window fill, "
                "and filling with index $E underneath reproduces the cartridge."
            ),
        },
        "glyphs": entries,
        "by_char": {char: by_char[char] for char in sorted(by_char)},
        "unmapped_glyphs": [
            index for index in range(GLYPH_COUNT) if index not in DIALOGUE_CHARSET
        ],
    }


# ---------------------------------------------------------------------------
# Waiting arrow
# ---------------------------------------------------------------------------
def emit_scroll_arrow(
    rom: bytes, root: Path, palette: Sequence[tuple[int, int, int]]
) -> dict[str, Any]:
    """Emit the two-tile hardware sprite used while retail text waits.

    The mapping is a 2x1 sprite, so `$7F6` and `$7F7` are both part of the
    image.  The first pattern is the byte-verified `ArtNem_Font` payload's
    tile `$36`; the second is its adjacent `$37`.  VDP colour zero is
    transparent for the sprite, while the two drawn indices use the dialogue
    CRAM line selected by the decoded sprite attribute.
    """
    spec = next(
        (candidate for candidate in NEMESIS_ART
         if candidate["label"] == SCROLL_ARROW_ART_LABEL),
        None,
    )
    if spec is None or int(spec["rom_offset"]) != SCROLL_ARROW_ART_ROM_OFFSET:
        raise DialoguePackError("ArtNem_Font spec drifted away from the retail arrow source")
    if int(spec["compressed_size"]) != SCROLL_ARROW_ART_COMPRESSED_SIZE:
        raise DialoguePackError("ArtNem_Font compressed size drifted away from the retail arrow")

    decompressed, art = decompress_art(
        rom,
        SCROLL_ARROW_ART_ROM_OFFSET,
        SCROLL_ARROW_ART_LABEL,
        compressed_size=SCROLL_ARROW_ART_COMPRESSED_SIZE,
    )
    tiles = decode_tiles(decompressed)
    end = SCROLL_ARROW_TILE_INDEX + SCROLL_ARROW_TILE_COUNT
    if len(tiles) != 87 or end > len(tiles):
        raise DialoguePackError(
            f"{SCROLL_ARROW_ART_LABEL} decodes to {len(tiles)} tiles; "
            f"arrow needs tile range {SCROLL_ARROW_TILE_INDEX}:{end}"
        )

    mapping = rom_slice(
        rom,
        SCROLL_ARROW_MAPPING_ROM_OFFSET,
        len(SCROLL_ARROW_MAPPING_BYTES),
        "Mappings_ScrollTextArrow",
    )
    if mapping != SCROLL_ARROW_MAPPING_BYTES:
        raise DialoguePackError(
            "Mappings_ScrollTextArrow bytes drifted: expected "
            f"{SCROLL_ARROW_MAPPING_BYTES.hex()}, got {mapping.hex()}"
        )

    # A 2x1 VDP sprite is two side-by-side cells: compose row-interleaved
    # (left tile's row r, then right tile's row r), not tile-concatenated —
    # the concatenated form smeared tile 0 across the top four rows of the
    # 16-wide image. Receipt: the frame-7250 oracle SAT/VRAM decode shows
    # the clean 13-wide triangle this now reproduces.
    left = tiles[SCROLL_ARROW_TILE_INDEX]
    right = tiles[SCROLL_ARROW_TILE_INDEX + 1]
    pixels = b"".join(
        left[row * TILE_PIXELS : (row + 1) * TILE_PIXELS]
        + right[row * TILE_PIXELS : (row + 1) * TILE_PIXELS]
        for row in range(TILE_PIXELS)
    )
    image = png.encode_indexed(
        SCROLL_ARROW_WIDTH,
        SCROLL_ARROW_HEIGHT,
        pixels,
        list(palette),
        (SCROLL_ARROW_TRANSPARENT_INDEX,),
    )
    (root / SCROLL_ARROW_PNG_NAME).write_bytes(image)
    return {
        "png": SCROLL_ARROW_PNG_NAME,
        "png_sha256": hashlib.sha256(image).hexdigest(),
        "width": SCROLL_ARROW_WIDTH,
        "height": SCROLL_ARROW_HEIGHT,
        "screen_x": 0x188 - 128,
        "screen_y": 0x14A - 128,
        "note": (
            "A sprite, so its stored position is VDP sprite space; the "
            "routine also subtracts the camera's sub-tile scroll so it "
            "stays locked to the window."
        ),
        "source": {
            "label": SCROLL_ARROW_ART_LABEL,
            "rom_offset": f"0x{SCROLL_ARROW_ART_ROM_OFFSET:06X}",
            "rom_end_exclusive": f"0x{SCROLL_ARROW_ART_ROM_OFFSET + SCROLL_ARROW_ART_COMPRESSED_SIZE:06X}",
            "compression": "nemesis",
            "compressed_size": art["compressed_size"],
            "compressed_sha256": art["compressed_sha256"],
            "decompressed_sha256": art["decompressed_sha256"],
            "font_vram_tile": f"0x{SCROLL_ARROW_FONT_VRAM_TILE:03X}",
            "sprite_vram_tile": f"0x{SCROLL_ARROW_VRAM_TILE:03X}",
            "tile_indices": [
                f"0x{SCROLL_ARROW_TILE_INDEX + index:02X}"
                for index in range(SCROLL_ARROW_TILE_COUNT)
            ],
            "tile_count": SCROLL_ARROW_TILE_COUNT,
            "loader": "FieldObj_ScrollTextArrow",
            "mapping_rom_offset": f"0x{SCROLL_ARROW_MAPPING_ROM_OFFSET:06X}",
            "mapping_hex": mapping.hex(),
            "mapping_width_cells": SCROLL_ARROW_TILE_COUNT,
            "mapping_height_cells": 1,
            "palette": {
                "source": "Pal_Init_Line_3",
                "cram_line": DIALOGUE_CRAM_LINE,
                "transparent_index": SCROLL_ARROW_TRANSPARENT_INDEX,
            },
            "note": (
                "FieldObj_ScrollTextArrow writes art tile $7F6 and the retail "
                "mapping is a 2x1 sprite; the indexed PNG preserves both tiles."
            ),
        },
    }


# ---------------------------------------------------------------------------
# Portraits
# ---------------------------------------------------------------------------
def portrait_png(
    rom: bytes, offset: int, label: str, palette: Sequence[tuple[int, int, int]]
) -> bytes:
    """One 48x48 portrait, composed six tiles across as its plane map orders it."""
    decompressed, _ = decompress_art(rom, offset, label)
    tiles = decode_tiles(decompressed)
    if len(tiles) != PORTRAIT_TILES:
        raise DialoguePackError(
            f"{label} at 0x{offset:06X} decodes to {len(tiles)} patterns, not the "
            f"{PORTRAIT_TILES} the 6x6 plane map at 0x{PORTRAIT_PLANE_MAP:06X} names"
        )
    width, height, pixels = compose_sheet(tiles, PORTRAIT_COLUMNS)
    colours = list(palette)
    if len(colours) != COLORS_PER_LINE:
        raise DialoguePackError(
            f"portrait palette holds {len(colours)} colours, not {COLORS_PER_LINE}"
        )
    return png.encode_indexed(
        width, height, pixels, colours, (PORTRAIT_TRANSPARENT_INDEX,)
    )


def emit_portraits(
    rom: bytes, root: Path, palette: Sequence[tuple[int, int, int]]
) -> tuple[dict[str, Any], int]:
    """Write every portrait the retail table names; return its index and bytes.

    One file per *id*, because `$F4` names an id and that is how a runtime asks
    for one. Six of the ids share three art blobs (two Espers, two Esper
    Chiefs, two Xe A Thouls); those files are byte-identical and each says
    which id it duplicates.
    """
    directory = root / PORTRAITS_DIRECTORY
    directory.mkdir(parents=True, exist_ok=True)
    table = extract_dialogue_portraits(rom)

    first_by_offset: dict[str, int] = {}
    entries: list[dict[str, Any]] = []
    total = 0
    for entry in table["entries"]:
        art = entry["art"]
        index = entry["index"]
        if art is None:
            continue
        offset = art["rom_offset"]
        name = f"{index:02X}_{safe_name(entry['symbol'] or f'Portrait{index:02X}')}.png"
        image = portrait_png(rom, int(offset, 16), art["label"], palette)
        (directory / name).write_bytes(image)
        total += len(image)
        entries.append({
            "id": index,
            "id_hex": f"0x{index:02X}",
            "symbol": entry["symbol"],
            "png": f"{PORTRAITS_DIRECTORY}/{name}",
            "png_sha256": hashlib.sha256(image).hexdigest(),
            "duplicate_of": first_by_offset.get(offset),
            "art": {
                "label": art["label"],
                "rom_offset": offset,
                "compression": art["compression"],
                "compressed_size": art["compressed_size"],
                "tile_count": art["tile_count"],
                "decompressed_sha256": art["decompressed_sha256"],
            },
        })
        first_by_offset.setdefault(offset, index)

    index_json = {
        "format_version": DIALOGUE_FORMAT_VERSION,
        "kind": "dialogue_portraits",
        "table": {
            "label": table["label"],
            "rom_offset": table["table_rom_offset"],
            "entry_count": table["entry_count"],
            "null_entry": 0,
            "note": (
                "Index 0 is a null pointer and $F4 id 0 means hide the portrait "
                "window. The disassembly's seven shopkeeper entries at $28-$2E "
                "are not in the retail table; those portraits are reached "
                "through the shop tables near 0x068000."
            ),
        },
        "geometry": {
            "width": PORTRAIT_COLUMNS * TILE_PIXELS,
            "height": PORTRAIT_COLUMNS * TILE_PIXELS,
            "tile_columns": PORTRAIT_COLUMNS,
            "tile_rows": PORTRAIT_TILES // PORTRAIT_COLUMNS,
            "tile_count": PORTRAIT_TILES,
            "vram_tile": f"0x{PORTRAIT_VRAM_TILE:03X}",
            "plane_map_rom_offset": f"0x{PORTRAIT_PLANE_MAP:06X}",
            "screen_tile_x": PORTRAIT_TILE_X,
            "screen_tile_y": PORTRAIT_TILE_Y,
            "screen_x": PORTRAIT_TILE_X * TILE_PIXELS,
            "screen_y": PORTRAIT_TILE_Y * TILE_PIXELS,
            "talk_slot_stride_tiles": PORTRAIT_TALK_SLOT_TILES,
            "cutscene_screen_tile_x": PORTRAIT_CUTSCENE_TILE_X,
            "cutscene_screen_tile_y": PORTRAIT_CUTSCENE_TILE_Y,
            "note": (
                "The plane map at 0x2A2B36 lays the 36 patterns out row-major "
                "six across from tile $55C, priority set, palette line 2. In "
                "Talk mode (Game_Mode_Routine 4) $F4's second operand shifts "
                "the portrait right by 12 tiles per slot."
            ),
        },
        "palette": {
            "source": "Pal_Init_Line_3",
            "rom_offset": f"0x{PAL_INIT_LINE_3:06X}",
            "cram_line": DIALOGUE_CRAM_LINE,
            "transparent_index": PORTRAIT_TRANSPARENT_INDEX,
            "colors": [list(colour) for colour in palette],
        },
        "count": len(entries),
        "distinct_art": table["distinct_art_blobs"],
        "portraits": entries,
    }
    return index_json, total
