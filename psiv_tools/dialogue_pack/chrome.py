"""The box the text sits in: `ArtNem_WindowTiles` and `WinGroup_Dialogue`.

`loc_68704` draws every window in the game out of three tiles and their flip
bits. The role map here is that routine, and the records are the cartridge's
own window table.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any, Sequence

from .. import png
from ..gfx import compose_sheet, decode_tiles, decompress_art
from .common import (
    DIALOGUE_FORMAT_VERSION,
    WINDOW_PNG_NAME,
    DialoguePackError,
    rom_slice,
)
from .window import (
    BACKGROUND_COLOR_INDEX,
    CHARS_PER_LINE,
    DIALOGUE_CRAM_LINE,
    GLYPH_HEIGHT,
    GLYPH_WIDTH,
    LINES_PER_WINDOW,
    PAL_INIT_LINE_3,
    TILE_PIXELS,
    WINDOW_TILE_HEIGHT,
    WINDOW_TILE_WIDTH,
    WINDOW_TILE_X,
    WINDOW_TILE_Y,
)

# ---------------------------------------------------------------------------
# Window chrome
# ---------------------------------------------------------------------------
#: `ArtNem_WindowTiles`, and the VRAM tile `Title_ArtPtrs` loads it at. The
#: frame's pattern words are absolute VRAM tiles, so this base is what turns
#: them back into indices into the blob.
WINDOW_ART_ROM_OFFSET = 0x2A2B7E
WINDOW_ART_COMPRESSED_SIZE = 1212
WINDOW_ART_TILES = 128
WINDOW_ART_VRAM_TILE = 0x680

#: `ArtNem_Font` is loaded at $681 by the same table, over the window blob.
#: The frame is untouched only because it lives at either end of the 128.
MENU_FONT_VRAM_TILE = 0x681
MENU_FONT_TILES = 87

#: The nine words `loc_68704` writes, in the order it writes them. A pattern
#: word is `priority | palette << 13 | vflip << 12 | hflip << 11 | tile`, so
#: the nine roles are three tiles plus the flip bits, and the frame is one
#: cell thick with no shadow: this list is the whole vocabulary.
WINDOW_FRAME_WORDS: tuple[tuple[str, int], ...] = (
    ("corner_top_left", 0xC6E9),
    ("edge_top", 0xC6EA),
    ("corner_top_right", 0xCEE9),
    ("edge_left", 0xC6F3),
    ("fill", 0xC680),
    ("edge_right", 0xCEF3),
    ("corner_bottom_left", 0xD6E9),
    ("edge_bottom", 0xD6EA),
    ("corner_bottom_right", 0xDEE9),
)

WINDOW_BORDER_CELLS = 1

#: `WinGroup_Dialogue`: four records of `width-1, height-1, x, y, pointer`.
#: `LoadWindowGroup` picks the group from `Game_Mode_Routine`; the records are
#: read out of the cartridge rather than repeated here, and only their names
#: are this module's.
WIN_GROUP_DIALOGUE = 0x069380
WIN_GROUP_RECORD_SIZE = 8
WIN_GROUP_DIALOGUE_NAMES = ("controls", "dialogue", "portrait", "yes_no")
#: The window `Window_Create` is called with for the message box itself.
DIALOGUE_WINDOW_INDEX = 1
PORTRAIT_WINDOW_INDEX = 2

#: `loc_68690`: the frame is redrawn from the centre outwards, two cells wider
#: each step, one DMA per step, unless `Window_Render_Mode` bit 0 is set.
WINDOW_OPEN_STEP_CELLS = 2

# ---------------------------------------------------------------------------
# Window chrome
# ---------------------------------------------------------------------------
def pattern_word(word: int) -> dict[str, Any]:
    """One VDP pattern word: priority, palette line, both flips and the tile."""
    return {
        "word": f"0x{word:04X}",
        "priority": bool(word & 0x8000),
        "cram_line": (word >> 13) & 3,
        "flip_v": bool(word & 0x1000),
        "flip_h": bool(word & 0x0800),
        "vram_tile": word & 0x07FF,
    }


def frame_roles() -> dict[str, dict[str, Any]]:
    """The nine words `loc_68704` writes, as tile indices and flips.

    A role's `tile` is an index into `window.png`, so a renderer draws a
    corner by blitting cell 105 and setting the flips the word asks for. The
    three distinct tiles are the whole frame: the right edge is the left one
    mirrored, the bottom row is the top one flipped, and there is no shadow.
    """
    roles = {}
    for name, word in WINDOW_FRAME_WORDS:
        decoded = pattern_word(word)
        tile = decoded["vram_tile"] - WINDOW_ART_VRAM_TILE
        if not 0 <= tile < WINDOW_ART_TILES:
            raise DialoguePackError(
                f"window frame role {name} names VRAM tile "
                f"0x{decoded['vram_tile']:03X}, outside the {WINDOW_ART_TILES} "
                f"patterns loaded at 0x{WINDOW_ART_VRAM_TILE:03X}"
            )
        if decoded["cram_line"] != DIALOGUE_CRAM_LINE:
            raise DialoguePackError(
                f"window frame role {name} draws in CRAM line "
                f"{decoded['cram_line']}, not the dialogue line {DIALOGUE_CRAM_LINE}"
            )
        roles[name] = {
            "tile": tile,
            "x": tile * TILE_PIXELS,
            "y": 0,
            "width": TILE_PIXELS,
            "height": TILE_PIXELS,
            **decoded,
        }
    return roles


def window_records(rom: bytes) -> list[dict[str, Any]]:
    """`WinGroup_Dialogue`, read from the cartridge.

    The stored width and height are one less than the window's, which
    `Window_Draw` restores with two `addq.w #1`s before it draws anything.
    `x` and `y` are screen tiles, the same space `TextBufferToPlane` uses.
    """
    records = []
    for index, name in enumerate(WIN_GROUP_DIALOGUE_NAMES):
        offset = WIN_GROUP_DIALOGUE + index * WIN_GROUP_RECORD_SIZE
        raw = rom_slice(rom, offset, WIN_GROUP_RECORD_SIZE, "WinGroup_Dialogue")
        width, height, x, y = raw[0] + 1, raw[1] + 1, raw[2], raw[3]
        if width <= 2 or height <= 2:
            raise DialoguePackError(
                f"window {index} ({name}) is {width}x{height} cells, which has no "
                "room for a border and an interior"
            )
        records.append({
            "index": index,
            "name": name,
            "record_offset": f"0x{offset:06X}",
            "record_hex": raw.hex(),
            "width_cells": width,
            "height_cells": height,
            "x_cell": x,
            "y_cell": y,
            "rect": {
                "x": x * TILE_PIXELS, "y": y * TILE_PIXELS,
                "width": width * TILE_PIXELS, "height": height * TILE_PIXELS,
            },
            "interior": {
                "x_cell": x + WINDOW_BORDER_CELLS,
                "y_cell": y + WINDOW_BORDER_CELLS,
                "width_cells": width - 2 * WINDOW_BORDER_CELLS,
                "height_cells": height - 2 * WINDOW_BORDER_CELLS,
            },
            "content_pointer": f"0x{int.from_bytes(raw[4:], 'big'):06X}",
        })
    return records


def check_window_fits_the_text(records: Sequence[dict[str, Any]]) -> dict[str, Any]:
    """The message box's interior must be exactly the text area.

    `WinGroup_Dialogue` record 1 and `TextBufferToPlane`'s immediates are two
    unrelated pieces of the cartridge -- a data table and a pair of `move.w`s
    -- and they describe the same rectangle. If they ever disagree, one of the
    two offsets this module reads is wrong, so this is a check and not a note.
    """
    window = records[DIALOGUE_WINDOW_INDEX]
    interior = window["interior"]
    expected = {
        "x_cell": WINDOW_TILE_X, "y_cell": WINDOW_TILE_Y,
        "width_cells": WINDOW_TILE_WIDTH, "height_cells": WINDOW_TILE_HEIGHT,
    }
    if interior != expected:
        raise DialoguePackError(
            f"WinGroup_Dialogue record {DIALOGUE_WINDOW_INDEX} at "
            f"{window['record_offset']} has interior {interior}, but "
            f"TextBufferToPlane writes {expected}"
        )
    return window


def window_json_document(
    rom: bytes,
    records: Sequence[dict[str, Any]],
    palette: Sequence[tuple[int, int, int]],
    art: dict[str, Any],
    image: bytes,
) -> dict[str, Any]:
    box = check_window_fits_the_text(records)
    return {
        "format_version": DIALOGUE_FORMAT_VERSION,
        "kind": "dialogue_window",
        "png": WINDOW_PNG_NAME,
        "png_sha256": hashlib.sha256(image).hexdigest(),
        "source": {
            "label": "ArtNem_WindowTiles",
            "rom_offset": f"0x{WINDOW_ART_ROM_OFFSET:06X}",
            "compression": "nemesis",
            "compressed_size": art["compressed_size"],
            "tile_count": art["tile_count"],
            "decompressed_sha256": art["decompressed_sha256"],
            "vram_tile": f"0x{WINDOW_ART_VRAM_TILE:03X}",
            "loader": "Title_ArtPtrs",
            "note": (
                "Title_ArtPtrs loads ArtNem_Font at $681 as well, so the menu "
                f"font's {MENU_FONT_TILES} patterns cover indices 1..{MENU_FONT_TILES} "
                "of this blob for the whole game. The frame is untouched because "
                "it lives at either end: the fill at 0 and the border at 105..115."
            ),
        },
        "tile": {
            "width": TILE_PIXELS, "height": TILE_PIXELS,
            "count": art["tile_count"],
            "note": "cell N of the strip is blob tile N, so x = N * 8",
        },
        "palette": {
            "source": "Pal_Init_Line_3",
            "rom_offset": f"0x{PAL_INIT_LINE_3:06X}",
            "cram_line": DIALOGUE_CRAM_LINE,
            "colors": [list(colour) for colour in palette],
            "fill_index": BACKGROUND_COLOR_INDEX,
            "transparent_index": None,
            "note": (
                "No pixel of the window art is index 0, so nothing in it is "
                "transparent; the interior is a solid tile of index $E, the same "
                "colour FillTextBackground paints the text area with."
            ),
        },
        "roles": frame_roles(),
        "geometry": {
            "border_cells": WINDOW_BORDER_CELLS,
            "cell_pixels": TILE_PIXELS,
            "shadow": False,
            "rule": (
                "A window of W x H cells is: corner_top_left, W-2 edge_top, "
                "corner_top_right; then H-2 rows of edge_left, W-2 fill, "
                "edge_right; then corner_bottom_left, W-2 edge_bottom, "
                "corner_bottom_right. Nothing else is drawn."
            ),
            "record_encoding": (
                "WinGroup records store width-1 and height-1; Window_Draw adds "
                "one to each before drawing."
            ),
            "open_animation": {
                "routine": "loc_68690",
                "axis": "horizontal",
                "from": "center",
                "step_cells": WINDOW_OPEN_STEP_CELLS,
                "frames_per_step": 1,
                "note": (
                    "The frame is redrawn two cells wider from the centre each "
                    "step, with one DMA between steps, unless Window_Render_Mode "
                    "bit 0 is set -- TextCtrlCode_Portrait sets it, so portrait "
                    "windows appear instantly and the message box animates."
                ),
            },
        },
        "group": {
            "label": "WinGroup_Dialogue",
            "rom_offset": f"0x{WIN_GROUP_DIALOGUE:06X}",
            "record_size": WIN_GROUP_RECORD_SIZE,
            "selector": "LoadWindowGroup, by Game_Mode_Routine",
        },
        "windows": list(records),
        "text_window": {
            "window": DIALOGUE_WINDOW_INDEX,
            "rect": box["rect"],
            "interior": box["interior"],
            "chars_per_line": CHARS_PER_LINE,
            "lines_per_window": LINES_PER_WINDOW,
            "glyph_width": GLYPH_WIDTH,
            "glyph_height": GLYPH_HEIGHT,
            "note": (
                "The interior is the text area exactly: 32 glyphs of 8 pixels "
                "across, two lines of 16 pixels down."
            ),
        },
        "portrait_window": {
            "window": PORTRAIT_WINDOW_INDEX,
            "rect": records[PORTRAIT_WINDOW_INDEX]["rect"],
            "note": (
                "The portrait's 6x6 plane map is drawn at the same cell as this "
                "window and is the same size, so the art covers the frame "
                "completely -- portrait PNGs carry their own border. The window "
                "is created for the backup and restore Window_Destroy needs, not "
                "for the picture."
            ),
        },
    }


def emit_window(
    rom: bytes, root: Path, palette: Sequence[tuple[int, int, int]]
) -> tuple[dict[str, Any], bytes]:
    """Write `window.png` and build `window.json`."""
    decompressed, art = decompress_art(
        rom, WINDOW_ART_ROM_OFFSET, "ArtNem_WindowTiles",
        compressed_size=WINDOW_ART_COMPRESSED_SIZE,
    )
    tiles = decode_tiles(decompressed)
    if len(tiles) != WINDOW_ART_TILES:
        raise DialoguePackError(
            f"ArtNem_WindowTiles decodes to {len(tiles)} patterns, not "
            f"{WINDOW_ART_TILES}"
        )
    width, height, pixels = compose_sheet(tiles, len(tiles))
    image = png.encode_indexed(width, height, pixels, list(palette))
    (root / WINDOW_PNG_NAME).write_bytes(image)
    document = window_json_document(rom, window_records(rom), palette, art, image)
    return document, image

