"""The text window: how big it is, where it sits, and what colour it is.

Every constant here is read off one of the opcodes `signatures.py` pins, and
the module docstring of the package walks the proof end to end.
"""

from __future__ import annotations

from typing import Any

from ..gfx import PALETTE_LINE_SIZE, decode_palette, palette_rgb
from .common import (
    DIALOGUE_FORMAT_VERSION,
    WINDOW_JSON_NAME,
    DialoguePackError,
    rom_slice,
)

# ---------------------------------------------------------------------------
# Window and font metrics, and the retail opcodes each one is read from.
# ---------------------------------------------------------------------------
#: `addq.w #1,d1 / andi.w #$1F,d1` in `RunText_CharacterLoop`.
CHARS_PER_LINE = 32
#: `andi.w #1,d2` in the same loop, three instructions later.
LINES_PER_WINDOW = 2

#: `GetFontGraphics`: `andi.w #$7F,d1 / lsl.w #4,d1`.
GLYPH_WIDTH = 8
GLYPH_HEIGHT = 16
GLYPH_BYTES = 16
GLYPH_INDEX_MASK = 0x7F

FONT_ROM_OFFSET = 0x2A3542
FONT_SIZE = 1280
GLYPH_COUNT = FONT_SIZE // GLYPH_BYTES  # 80

#: `ParseText`: `move.w #$F,d0` (set bits) and `move.w #$E,d1` (clear bits).
TEXT_COLOR_INDEX = 0xF
BACKGROUND_COLOR_INDEX = 0xE

#: `TextBufferPlaneMaps` and the portrait's plane map are both `$C...`: palette
#: bits 14-13 = %10. `loc_53F14` fills that line from `Pal_Init_Line_3`.
DIALOGUE_CRAM_LINE = 2
PAL_INIT_LINE_3 = 0x296300
PAL_INIT = 0x09F2BC
PAL_INIT_MIRRORED_LINE = 2

#: `TextBufferToPlane`: `move.w #4,d3 / move.w #$15,d4`, then a 32x4 plane map.
#: `GetPlaneAOffset` adds the camera's tile position, so these are screen tiles.
WINDOW_TILE_X = 4
WINDOW_TILE_Y = 0x15
WINDOW_TILE_WIDTH = 32
WINDOW_TILE_HEIGHT = 4
WINDOW_VRAM_TILE = 0x580
WINDOW_VRAM_ADDRESS = 0xB000
TILE_PIXELS = 8


#: Page endings, in the order a renderer meets them.
PAGE_ENDINGS = ("full", "wait", "close", "choice", "end")


#: `Text_Scroll_Arrow` is a sprite at ($188, $14A); VDP sprite coordinates are
#: offset by 128, and the routine subtracts the camera's sub-tile scroll so the
#: arrow stays locked to the window.
SCROLL_ARROW_SPRITE_X = 0x188
SCROLL_ARROW_SPRITE_Y = 0x14A
SPRITE_ORIGIN = 128

def dialogue_palette(rom: bytes) -> list[tuple[int, int, int]]:
    """`Pal_Init_Line_3`, the CRAM line the whole dialogue window renders in.

    Re-proves the mirror `psiv_tools.gfx` documents -- the line is stored twice,
    once inside `Pal_Init` and once on its own -- because a palette read from
    the wrong offset would still decode to sixteen plausible colours.
    """
    mirror = rom_slice(rom, PAL_INIT_LINE_3, PALETTE_LINE_SIZE, "Pal_Init_Line_3")
    inside = rom_slice(
        rom,
        PAL_INIT + PAL_INIT_MIRRORED_LINE * PALETTE_LINE_SIZE,
        PALETTE_LINE_SIZE,
        "Pal_Init",
    )
    if mirror != inside:
        raise DialoguePackError(
            f"Pal_Init_Line_3 at 0x{PAL_INIT_LINE_3:06X} is not a copy of line "
            f"{PAL_INIT_MIRRORED_LINE} of Pal_Init at 0x{PAL_INIT:06X}"
        )
    return palette_rgb(decode_palette(mirror))


def window_json(scroll_arrow: dict[str, Any] | None = None) -> dict[str, Any]:
    """Everything a renderer needs to lay a dialogue window out.

    ``scroll_arrow`` is the emitted art/provenance record.  Keeping the
    position here means the text-side JSON and the chrome JSON share one
    screen-space answer, while the optional argument keeps this helper useful
    for geometry-only callers.
    """
    arrow = {
        "screen_x": SCROLL_ARROW_SPRITE_X - SPRITE_ORIGIN,
        "screen_y": SCROLL_ARROW_SPRITE_Y - SPRITE_ORIGIN,
        "note": (
            "A sprite, so its stored position is VDP sprite space; the "
            "routine also subtracts the camera's sub-tile scroll so it "
            "stays locked to the window."
        ),
    }
    if scroll_arrow is not None:
        arrow.update(scroll_arrow)
    return {
        "chars_per_line": CHARS_PER_LINE,
        "lines_per_window": LINES_PER_WINDOW,
        "glyph_width": GLYPH_WIDTH,
        "glyph_height": GLYPH_HEIGHT,
        "tile_width": WINDOW_TILE_WIDTH,
        "tile_height": WINDOW_TILE_HEIGHT,
        "screen_tile_x": WINDOW_TILE_X,
        "screen_tile_y": WINDOW_TILE_Y,
        "rect": {
            "x": WINDOW_TILE_X * TILE_PIXELS,
            "y": WINDOW_TILE_Y * TILE_PIXELS,
            "width": WINDOW_TILE_WIDTH * TILE_PIXELS,
            "height": WINDOW_TILE_HEIGHT * TILE_PIXELS,
        },
        "vram_tile": f"0x{WINDOW_VRAM_TILE:03X}",
        "vram_address": f"0x{WINDOW_VRAM_ADDRESS:04X}",
        "cram_line": DIALOGUE_CRAM_LINE,
        "palette": "Pal_Init_Line_3",
        "text_color_index": TEXT_COLOR_INDEX,
        "background_color_index": BACKGROUND_COLOR_INDEX,
        "scroll_arrow": arrow,
        "page_endings": list(PAGE_ENDINGS),
        "chrome": WINDOW_JSON_NAME,
        "note": (
            "A glyph is 8x16 and the window holds 32 of them on each of two "
            "lines. The column counter wraps at 32 (andi.w #$1F,d1), which "
            "swallows an explicit $FC landing exactly there, and the line "
            "counter is masked to one bit (andi.w #1,d2), which is what makes "
            "a full second line wait for input. $FC itself is not masked: a "
            "third line would be written past the window's tiles."
        ),
    }


def interaction_json() -> dict[str, Any]:
    return {
        "grammar": "entry := $FA* ( $F6 event | $F3? text... )",
        "routine": "Interaction_ProcessDialogueTree",
        "rom_offset": "0x05887A",
        "note": (
            "The interaction code walks the preamble before RunText sees the "
            "entry: $FA follows an event flag to another entry by id, $F6 "
            "fires an event instead of showing the message, and $F3 suppresses "
            "Interaction_UpdateObj so the NPC keeps its facing. $F6's operand "
            "is read as a word here, which is what fixes its length; "
            "TextCtrlCode_Event skips only one byte and no retail data reaches "
            "it. The retail scanner has no $FB branch."
        ),
        "entry_addressing": (
            "GetDialogueByID counts $FF bytes from the tree's start, so entry "
            "ids are dense and include the empty entries."
        ),
    }
