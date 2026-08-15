"""The dialogue half of the runtime pack: text, font and portraits.

`psiv_tools.pack` emits what field mode needs to *walk*; this module emits what
it needs to *talk*. Three files and one directory, all under `dialogue/`:

    dialogue/trees.json          43 trees, every entry as renderer-ready segments
    dialogue/font.png            the 80 dialogue glyphs as an 8x16 strip
    dialogue/font.json           byte -> glyph rect, plus the window's palette
    dialogue/window.png          ArtNem_WindowTiles, the box's chrome
    dialogue/window.json         which tile is which corner, and the box's size
    dialogue/portraits.json      the 39 portrait ids and their provenance
    dialogue/portraits/*.png     48x48 composed portraits

Nothing here re-decodes anything. The trees come from `psiv_tools.text`, the
font and the portraits from `psiv_tools.gfx`. What lives here and nowhere else
is the *window*: how many characters fit on a line, how many lines fit in the
window, which colours the glyphs are drawn in, and where the box sits on the
screen. All of it is transcribed from `RunText` and checked against the retail
opcodes before anything is emitted (`SIGNATURES`).

The window
----------

`RunText_CharacterLoop` prints one glyph per iteration and keeps two counters:
`d1` is the column, `d2` is the line.

* `addq.w #1,d1 / andi.w #$1F,d1` (0x06A0CE) -- **32 characters per line**. The
  Japanese build masks with `$F` instead; the retail English image masks with
  `$1F`, which is why the count is read out of the cartridge rather than out of
  the disassembly's `revision` switch.
* On the wrap, `cmpi.b #$FC,(a0)` swallows an explicit newline that lands
  exactly at the end of a full line, then `andi.w #1,d2` (0x06A0F0) -- **two
  lines per window**. When that mask comes out zero the loop jumps into
  `TextCtrlCode_Interrupt`, which shows the scroll arrow, waits for a button,
  clears the window and starts again at line 0.
* `GetFontGraphics` (0x06A8F8) is `andi.w #$7F,d1 / lsl.w #4,d1`, so a glyph is
  **16 bytes**, and `ParseText` (0x06A938) expands it as sixteen rows of eight
  bits: an **8x16 1bpp glyph**, painted with palette index `$F` where the bit is
  set and `$E` where it is not. That is the whole font format. The two 8x8
  patterns a glyph becomes exist only in VRAM.
* `TextBufferToPlane` (0x06A7D8) writes a 32x4-tile map at screen tile (4, 21)
  -- `GetPlaneAOffset` adds the camera's tile position, so those are screen
  coordinates -- and every word of `TextBufferPlaneMaps` (0x06A7F8) carries
  palette line 2 and priority. `loc_53F14` copies `Pal_Init_Line_3` into CRAM
  line 2 on every map load, which is why the dialogue window's colours never
  change. The portrait's own plane map (0x2A2B36) is the same: line 2, priority,
  tiles $55C..$57F row-major, six across.

So the window is 256x32 pixels at (32, 168), two lines of 32 8x16 glyphs, drawn
in colour $F on colour $E of `Pal_Init_Line_3`.

The box around it
-----------------

The text area is the *interior* of a window `Window_Create` draws first.
`WinGroup_Dialogue` (0x069380) is four 8-byte records of `width-1, height-1,
x, y` in screen tiles, and record 1 is 34x6 tiles at (3, 20) -- exactly one
cell of border around the 32x4 text area at (4, 21). The two facts are
independent (one is a data table, the other a pair of `move.w` immediates) and
they agree to the cell, which is the strongest check either of them gets. The
pack asserts it rather than assuming it.

`Window_Draw` -> `loc_68704` builds the frame out of **three tiles and their
flips**, and the words it writes carry the palette line the same way the text
does: `$C6E9` top-left corner, `$C6EA` top edge, `$C6F3` left edge, `$C680`
interior fill, and `$CEE9`/`$CEF3`/`$D6E9`/`$D6EA`/`$DEE9` for the right,
bottom and remaining corners, which are the same three tiles with the H and V
flip bits set. Every one is priority 1, palette line 2. Masked to 11 bits those
are VRAM tiles $6E9, $6EA, $6F3 and $680, and `Title_ArtPtrs` (0x0429CE) loads
`ArtNem_WindowTiles` at $680, so the roles are blob indices 105, 106, 115 and 0.

That table also loads `ArtNem_Font` at $681, on top of the window blob: the
menu font's 87 patterns occupy $681..$6D7 for the whole game. The frame tiles
survive because they sit at either end of the 128-tile blob -- the fill at
index 0, the frame at 105..115 -- which is why the window art looks mostly
empty when it is dumped whole.

Border thickness is one cell, there are no shadow tiles (the routine writes
nine words and no more), and the interior is one tile repeated. A window opens
with an animation unless `Window_Render_Mode` bit 0 is set: `loc_68690` draws
the frame repeatedly from the centre outwards, two cells wider each step, one
DMA per step. `TextCtrlCode_Portrait` sets bit 0, so portrait windows appear
instantly; the dialogue box animates.

The retail corpus agrees with both numbers independently: no line of any of the
2,736 entries exceeds 32 characters, 335 page lines are exactly 32, and no entry ever
writes a third line into a window. (It could: `TextCtrlCode_Newline` increments
`d2` without masking it, and a third line would land in VRAM past the window's
tiles. The script never does it.)

What the interaction code eats before `RunText` sees it
------------------------------------------------------

`Interaction_ProcessDialogueTree` (0x05887A in retail) walks an entry's
*preamble* before any text is drawn:

    entry := $FA*  ( $F6 event_hi event_lo | $F3? text... )

`$FA` tests an event flag and, when set, jumps to another entry by id; `$F6`
fires an event instead of showing the message; `$F3` suppresses
`Interaction_UpdateObj`, i.e. the NPC does *not* turn to face the player. All
69 `$F6` and all 33 `$F3` in the retail script sit in exactly that position, and
`GetEventFromDialogue` reads `$F6`'s operand as a word -- which is what settles
the operand length, since `TextCtrlCode_Event` itself skips only one byte and
is unreachable for retail data.

Two codes the disassembly documents are not retail codes at all. `$FB`
(extended event flag) reaches `TextCtrlCode_Null` in the retail jump table
(0x06A162 -> 0x06A176, an `rts`) and the retail preamble scanner has no `$FB`
branch; it is a Grand Cross addition. And `TextCtrlCode_Null` being an `rts`
means `$F0`, `$F1`, `$F3`, `$F8` and `$FB` all *end* the message if `RunText`
ever meets one. None of them occurs mid-text in the retail script.

Determinism
-----------

Same ROM, same bytes: JSON is written sorted with a fixed indent, PNGs come out
of `psiv_tools.png` with a fixed zlib level, and portrait file names come from
the pointer table's own index. Nothing here reads the clock or the filesystem.

The output holds Sega-derived pixels and text and is never committed.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Iterable, Sequence

from . import png
from .gfx import (
    COLORS_PER_LINE,
    PALETTE_LINE_SIZE,
    PORTRAIT_COLUMNS,
    PORTRAIT_TILES,
    compose_sheet,
    decode_palette,
    decode_tiles,
    decompress_art,
    extract_dialogue_portraits,
    palette_rgb,
)
from .text import (
    DIALOGUE_CHARSET,
    TEXT_ACTIONS,
    extract_dialogue,
)

#: Tracks `psiv_tools.pack.PACK_FORMAT_VERSION`. It is repeated rather than
#: imported because `pack` imports this module, not the other way round; the
#: test suite asserts the two are equal.
DIALOGUE_FORMAT_VERSION = 1

DIALOGUE_DIRECTORY = "dialogue"
PORTRAITS_DIRECTORY = f"{DIALOGUE_DIRECTORY}/portraits"
TREES_NAME = f"{DIALOGUE_DIRECTORY}/trees.json"
FONT_JSON_NAME = f"{DIALOGUE_DIRECTORY}/font.json"
FONT_PNG_NAME = f"{DIALOGUE_DIRECTORY}/font.png"
WINDOW_JSON_NAME = f"{DIALOGUE_DIRECTORY}/window.json"
WINDOW_PNG_NAME = f"{DIALOGUE_DIRECTORY}/window.png"
PORTRAITS_NAME = f"{DIALOGUE_DIRECTORY}/portraits.json"


class DialoguePackError(ValueError):
    pass


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

#: `Text_Scroll_Arrow` is a sprite at ($188, $14A); VDP sprite coordinates are
#: offset by 128, and the routine subtracts the camera's sub-tile scroll so the
#: arrow stays locked to the window.
SCROLL_ARROW_SPRITE_X = 0x188
SCROLL_ARROW_SPRITE_Y = 0x14A
SPRITE_ORIGIN = 128

#: Retail opcode bytes at fixed offsets. Every metric above is read off one of
#: these, so the pack refuses to emit against an image whose code differs --
#: the `reference/` clone is a Grand Cross build and its `revision` and
#: `large_dialog_window` switches change all of these numbers.
SIGNATURES: dict[str, tuple[int, str, str]] = {
    "chars_per_line": (
        0x06A0CE, "52410241001f6622",
        "RunText_CharacterLoop: addq.w #1,d1 / andi.w #$1F,d1 / bne",
    ),
    "lines_per_window": (
        0x06A0E6, "0c1000fc660441e80001024200016700",
        "RunText_CharacterLoop: swallow a following $FC, then andi.w #1,d2 / beq",
    ),
    "glyph_stride": (
        0x06A8F8, "48e7c0000c0000e06502101832000241007fe94943f9002a3542",
        "GetFontGraphics: andi.w #$7F,d1 / lsl.w #4,d1 / lea (Art_DialogueFont).l,a1",
    ),
    "glyph_colors": (
        0x06A938, "303c000f323c000e3e3c000f",
        "ParseText: move.w #$F,d0 (text) / #$E,d1 (background) / #$F,d7 (16 rows)",
    ),
    "window_position": (
        0x06A7D8, "363c0004383c0015",
        "TextBufferToPlane: move.w #4,d3 / move.w #$15,d4",
    ),
    "window_size": (
        0x06A7EA, "323c0020343c0004",
        "TextBufferToPlane: move.w #$20,d1 / move.w #4,d2",
    ),
    "window_plane_map": (
        0x06A7F8, "c580c582c584c586c588c58ac58cc58e",
        "TextBufferPlaneMaps: priority, palette line 2, base tile $580",
    ),
    "window_fill": (
        0x06A7A8, "2cbc70000002303ceeee3e3c07ff",
        "FillTextBackground: VRAM $B000, $EEEE, $800 words",
    ),
    "scroll_arrow": (
        0x06A400, "38bc0030397c0188002c397c014a002e",
        "TextCtrlCode_Interrupt: Text_Scroll_Arrow at ($188, $14A)",
    ),
    "null_is_rts": (
        0x06A162, "60000012",
        "TextCtrlCodesJmpTbl entry $FB branches to TextCtrlCode_Null at 0x06A176",
    ),
    "window_frame_top": (
        0x068714,
        "363cc6e936c3610000943a0053456b0c363cc6ea36c36100008460f0363ccee936c3",
        "loc_68704: the top row, $C6E9 corner / $C6EA edge / $CEE9 flipped corner",
    ),
    "window_frame_middle": (
        0x068746,
        "363cc6f336c3610000623a0053456b0c363cc68036c36100005260f0363ccef3",
        "loc_68704: a middle row, $C6F3 edge / $C680 fill / $CEF3 flipped edge",
    ),
    "window_frame_bottom": (
        0x068778,
        "363cd6e936c3610000303a0053456b0c363cd6ea36c36100002060f0363cdee936c3",
        "loc_68704: the bottom row, the top row's three tiles with V-flip set",
    ),
    "window_art_vram_tile": (
        0x0429CE, "0680002a2b7e07c0002a303a0681002a303affff",
        "Title_ArtPtrs: ArtNem_WindowTiles at $680, ArtNem_Font at $7C0 and $681",
    ),
    "window_records": (
        0x069380,
        "19040715000697e421050314000697e40505050d000697e406041b0e000697e4",
        "WinGroup_Dialogue: four width/height/x/y records, dialogue is 34x6 at (3, 20)",
    ),
    "portrait_table": (
        0x06A1BA, "47f90006a4b0",
        "TextCtrlCode_Portrait: lea (DialoguePortraitArtPtrs).l,a3",
    ),
    "portrait_vram_tile": (
        0x06A1CC, "303c055c",
        "TextCtrlCode_Portrait: move.w #$55C,d0 before NemDecomp",
    ),
    "portrait_position": (
        0x06A238, "363c0005383c000d",
        "TextCtrlCode_Portrait: move.w #5,d3 / move.w #$D,d4",
    ),
    "portrait_talk_slot": (
        0x06A26A, "ec9dcafc000cd6454eb9",
        "TextCtrlCode_Portrait: mulu.w #$C,d5 / add.w d5,d3 in Talk mode",
    ),
    "portrait_plane_map": (
        0x06A27A, "41f9002a2b36323c0006343c",
        "TextCtrlCode_Portrait: lea (0x2A2B36).l,a0 / 6 x 6 tiles",
    ),
    "interaction_preamble": (
        0x058B9A, "0c1000fa661c41e80001700010186100ea7a67081010",
        "Interaction_ProcessDialogueTree: the $FA loop, with no $FB branch",
    ),
    "interaction_event": (
        0x058BBC, "0c1000f66604600000d8",
        "Interaction_ProcessDialogueTree: $F6 hands off to GetEventFromDialogue",
    ),
    "interaction_facing": (
        0x058BC6, "0c1800f367086100008841e8ffff",
        "Interaction_ProcessDialogueTree: $F3 skips Interaction_UpdateObj",
    ),
}

#: Retail names for the sixteen control codes, as the emitted segments call
#: them. `$FB` is `null` here and `extended_event_flag_check` in
#: `psiv_tools.text`: the disassembly's handler is a Grand Cross addition and
#: the retail jump table sends `$FB` to `TextCtrlCode_Null`.
CTRL_NAMES: dict[int, str] = {
    0xF0: "null", 0xF1: "null", 0xF2: "action", 0xF3: "keep_npc_facing",
    0xF4: "portrait", 0xF5: "yes_no", 0xF6: "event", 0xF7: "close",
    0xF8: "null", 0xF9: "delay", 0xFA: "flag_check", 0xFB: "null",
    0xFC: "newline", 0xFD: "wait", 0xFE: "terminate", 0xFF: "terminate",
}

#: What each retail code does, for the consumer that has only the pack.
CTRL_NOTES: dict[int, str] = {
    0xF0: "TextCtrlCode_Null is an rts: it ends the message",
    0xF1: "TextCtrlCode_Null is an rts: it ends the message",
    0xF2: "TextActionsOffs sub-dispatch; see `action`",
    0xF3: "preamble only: the NPC does not turn to face the player",
    0xF4: "load portrait `id` into VRAM $55C and show it; id 0 hides it",
    0xF5: "yes/no window; the chosen operand is an entry id in this tree",
    0xF6: "preamble only: fire event `id` instead of showing the message",
    0xF7: "close the window and yield; the caller resumes after this byte",
    0xF8: "TextCtrlCode_Null is an rts: it ends the message",
    0xF9: "wait `frames` frames",
    0xFA: "if event flag `flag` is set, continue at entry `then_entry`",
    0xFB: "not a retail code: the jump table sends it to TextCtrlCode_Null",
    0xFC: "newline; does not check the two-line limit",
    0xFD: "show the scroll arrow, wait, clear the window, start at line 0",
    0xFE: "end the message",
    0xFF: "end the message; also the entry delimiter",
}

#: Page endings, in the order a renderer meets them.
PAGE_ENDINGS = ("full", "wait", "close", "choice", "end")


# ---------------------------------------------------------------------------
# Fail-closed checks
# ---------------------------------------------------------------------------
def _slice(rom: bytes, offset: int, size: int, what: str) -> bytes:
    if offset < 0 or offset + size > len(rom):
        raise DialoguePackError(
            f"{what} at 0x{offset:06X} (+{size}) runs past the end of the ROM"
        )
    return rom[offset:offset + size]


def check_signatures(rom: bytes) -> list[dict[str, Any]]:
    """Assert every routine this module transcribes is the retail one."""
    checked = []
    for name, (offset, expected, note) in sorted(SIGNATURES.items()):
        raw = bytes.fromhex(expected)
        actual = _slice(rom, offset, len(raw), name)
        if actual != raw:
            raise DialoguePackError(
                f"{name}: expected {expected} at 0x{offset:06X} ({note}) but the "
                f"image holds {actual.hex()}; this is not the supported retail build"
            )
        checked.append({
            "name": name,
            "rom_offset": f"0x{offset:06X}",
            "opcodes": expected,
            "proves": note,
        })
    return checked


def dialogue_palette(rom: bytes) -> list[tuple[int, int, int]]:
    """`Pal_Init_Line_3`, the CRAM line the whole dialogue window renders in.

    Re-proves the mirror `psiv_tools.gfx` documents -- the line is stored twice,
    once inside `Pal_Init` and once on its own -- because a palette read from
    the wrong offset would still decode to sixteen plausible colours.
    """
    mirror = _slice(rom, PAL_INIT_LINE_3, PALETTE_LINE_SIZE, "Pal_Init_Line_3")
    inside = _slice(
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


# ---------------------------------------------------------------------------
# Segments
# ---------------------------------------------------------------------------
def _action_segment(out: dict[str, Any], seg: dict[str, Any]) -> None:
    operands = out["operands"]
    out["action"] = seg["action"]
    out["action_id"] = seg["action_id"]
    if seg["action"] == "load_panel":
        out["panel"] = (operands[0] << 8) | operands[1]
    elif seg["action"] in ("load_sound", "load_sound_2"):
        out["sound"] = operands[0]
    elif seg["action"] == "set_event_flag":
        out["flag"] = operands[0]


def control_segment(seg: dict[str, Any]) -> dict[str, Any]:
    """One `psiv_tools.text` control record as a typed renderer segment.

    Every operand byte survives in `operands`, so the typed fields are a
    convenience over the raw bytes rather than a replacement for them.
    """
    code = int(seg["ctrl"], 16)
    if code == 0xFB:
        raise DialoguePackError(
            "control code $FB occurs in the script, but the retail jump table "
            "sends $FB to TextCtrlCode_Null (an rts) and the retail interaction "
            "preamble has no $FB branch; psiv_tools.text decodes it with the "
            "Grand Cross handler's three operand bytes, which would be wrong here"
        )
    operands = list(seg["operands"])
    out: dict[str, Any] = {
        "ctrl": CTRL_NAMES[code],
        "code": seg["ctrl"],
        "operands": operands,
    }
    if code == 0xF2:
        _action_segment(out, seg)
    elif code == 0xF4:
        out["id"] = seg["portrait_id"]
        out["position"] = seg.get("portrait_location")
    elif code == 0xF5:
        out["yes_entry"], out["no_entry"] = operands[0], operands[1]
    elif code == 0xF6:
        out["id"] = (operands[0] << 8) | operands[1]
    elif code == 0xF9:
        # `subq.b #1,d7` then `dbf`, so the operand is the frame count and a
        # zero operand would wrap to 256 rather than not waiting at all.
        out["frames"] = operands[0] or 0x100
    elif code == 0xFA:
        out["scope"] = "event_flag"
        out["flag"], out["then_entry"] = operands[0], operands[1]
    return out


def entry_segments(segments: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        dict(seg) if "text" in seg else control_segment(seg)
        for seg in segments
    ]


# ---------------------------------------------------------------------------
# Pagination
# ---------------------------------------------------------------------------
def _ctrl(segment: dict[str, Any] | None) -> str | None:
    if segment is None or "text" in segment:
        return None
    return segment["ctrl"]


def split_preamble(segments: Sequence[dict[str, Any]]) -> tuple[int, bool]:
    """How much of an entry the interaction code eats, and whether it is an event.

    `entry := $FA* ( $F6 event | $F3? text... )`. The `$FA` run is walked with
    the flags unknown, so what is skipped here is the not-set branch: the entry
    as it reads when no flag sends the text somewhere else.
    """
    index = 0
    while index < len(segments) and _ctrl(segments[index]) == "0xFA":
        index += 1
    if index < len(segments) and _ctrl(segments[index]) == "0xF6":
        return index + 1, True
    if index < len(segments) and _ctrl(segments[index]) == "0xF3":
        index += 1
    return index, False


def paginate(segments: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    """Split one entry into windows, exactly as `RunText_CharacterLoop` does.

    Returns one record per window: the lines it holds and how it ended --
    `full` (two full lines, the arrow appears), `wait` (`$FD`), `close`
    (`$F7`), `choice` (`$F5`) or `end` (the entry ran out).

    Pagination starts at line 0 column 0, which is where every entry the
    interaction code opens begins, and after the preamble the interaction code
    has already consumed. An entry reached by a `$FA` jump does *not* reset the
    counters -- `GetOffsetByID` only moves the text pointer -- so for those the
    pages are what the entry would look like from a fresh window. A `$FA` met
    mid-message is paginated as its not-set branch, for the same reason: which
    way it goes is a runtime fact, not a cartridge one.

    An entry whose preamble ends in `$F6` shows nothing at all -- the event
    fires instead -- and paginates to no windows.

    The input is `psiv_tools.text`'s segment list, which is exactly the byte
    stream regrouped, so "the next byte is `$FC`" is "the next segment is the
    `$FC` control": `decode_string` flushes its run at every control code. The
    entry's own `$FF` terminator is not in the list, so running off the end is
    the same lookahead as reading it.
    """
    pages: list[dict[str, Any]] = []
    lines = [""]
    line = 0
    column = 0
    index, is_event = split_preamble(segments)
    if is_event:
        return []

    def close(end: str) -> None:
        nonlocal lines, line, column
        pages.append({"lines": lines, "end": end})
        lines, line, column = [""], 0, 0

    def peek(offset: int = 0) -> str | None:
        position = index + offset
        return _ctrl(segments[position]) if position < len(segments) else None

    def interrupt(waited: str) -> bool:
        """`TextCtrlCode_Interrupt`, reached from `$FD` and from a full window.

        Returns whether the message ended here. The routine looks at the byte
        it stopped on before it shows anything: `$FF` and `$F7` terminate
        without an arrow, and a `$FD` is swallowed so the window does not wait
        twice for one page.
        """
        nonlocal index
        if index >= len(segments):
            # The next byte is the entry's own $FF terminator.
            close("end")
            return True
        after = peek()
        if after == "0xF7":
            # Terminates the same way $F7 does: the caller resumes past it.
            index += 1
            close("close")
            return False
        if after == "0xFD":
            index += 1
        close(waited)
        return False

    while index < len(segments):
        segment = segments[index]
        index += 1

        if "text" in segment:
            run = segment["text"]
            for position, char in enumerate(run):
                lines[line] += char
                column += 1
                if column < CHARS_PER_LINE:
                    continue
                column = 0
                # The loop peeks at the *next byte*, which is the next segment
                # only when the run ends on this character.
                at_run_end = position == len(run) - 1
                if at_run_end and peek() == "0xF5":
                    index += 1
                    close("choice")
                    return pages
                line += 1
                if at_run_end and peek() == "0xFC":
                    index += 1
                if line % LINES_PER_WINDOW:
                    lines.append("")
                elif not at_run_end:
                    # The next byte is another glyph, so the interrupt has
                    # nothing to swallow.
                    close("full")
                elif interrupt("full"):
                    return pages
            continue

        code = segment["ctrl"]
        if code == "0xFC":
            column = 0
            line += 1
            while len(lines) <= line:
                lines.append("")
        elif code == "0xFD":
            if interrupt("wait"):
                return pages
        elif code == "0xF7":
            close("close")
        elif code in ("0xFE", "0xFF"):
            close("end")
            return pages
        elif code == "0xF5":
            close("choice")
            return pages
        elif code in ("0xF0", "0xF1", "0xF3", "0xF8", "0xFB"):
            # TextCtrlCode_Null is an rts, and the preamble is already gone.
            close("end")
            return pages

    if any(lines):
        pages.append({"lines": lines, "end": "end"})
    return pages


# ---------------------------------------------------------------------------
# Font
# ---------------------------------------------------------------------------
def glyph_bitmaps(rom: bytes) -> list[list[str]]:
    """The 80 glyphs as rows of `'0'`/`'1'`, straight out of `Art_DialogueFont`."""
    raw = _slice(rom, FONT_ROM_OFFSET, FONT_SIZE, "Art_DialogueFont")
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
    raw = _slice(rom, FONT_ROM_OFFSET, FONT_SIZE, "Art_DialogueFont")
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
        raw = _slice(rom, offset, WIN_GROUP_RECORD_SIZE, "WinGroup_Dialogue")
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


# ---------------------------------------------------------------------------
# Portraits
# ---------------------------------------------------------------------------
def _safe(label: str) -> str:
    return "".join(c if c.isalnum() or c in "-_" else "_" for c in label)


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
        name = f"{index:02X}_{_safe(entry['symbol'] or f'Portrait{index:02X}')}.png"
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


# ---------------------------------------------------------------------------
# Trees
# ---------------------------------------------------------------------------
def window_json() -> dict[str, Any]:
    """Everything a renderer needs to lay a dialogue window out."""
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
        "scroll_arrow": {
            "screen_x": SCROLL_ARROW_SPRITE_X - SPRITE_ORIGIN,
            "screen_y": SCROLL_ARROW_SPRITE_Y - SPRITE_ORIGIN,
            "note": (
                "A sprite, so its stored position is VDP sprite space; the "
                "routine also subtracts the camera's sub-tile scroll so it "
                "stays locked to the window."
            ),
        },
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


def tree_json(tree: dict[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """One tree's runtime record, and its entries' page lists."""
    entries = []
    for entry in tree["entries"]:
        segments = entry_segments(entry["segments"])
        pages = paginate(entry["segments"])
        entries.append({
            "id": entry["id"],
            "block_offset": entry["block_offset"],
            "text": entry["text"],
            "raw_hex": entry["raw_hex"],
            "segments": segments,
            "pages": pages,
        })
    record = {
        "tree": tree["tree"],
        "label": tree["label"],
        "rom_offset": tree["rom_offset"],
        "rom_end_exclusive": tree["rom_end_exclusive"],
        "compression": tree["compression"],
        "compressed_size": tree["compressed_size"],
        "compressed_sha256": tree["compressed_sha256"],
        "decompressed_size": tree["decompressed_size"],
        "portrait_operand_bytes": tree["portrait_operand_bytes"],
        "is_talk_tree": tree["is_talk_tree"],
        "entry_count": tree["entry_count"],
        "entries": entries,
    }
    return record, entries


# ---------------------------------------------------------------------------
# Census
# ---------------------------------------------------------------------------
class Census:
    """What the retail script contains, as opposed to what the codes permit."""

    def __init__(self) -> None:
        self.control_codes: dict[str, int] = {}
        self.control_codes_by_tree: dict[int, dict[str, int]] = {}
        self.actions: dict[str, int] = {}
        self.portrait_ids: dict[int, int] = {}
        self.portrait_positions: dict[int, int] = {}
        self.delays: dict[int, int] = {}
        self.line_lengths: dict[int, int] = {}
        self.page_endings: dict[str, int] = {}
        #: Counted from decoded characters rather than raw bytes: a control
        #: code's operands are bytes below $F0 too, and they are not glyphs.
        self.glyph_bytes: dict[str, int] = {}
        self.empty_entries = 0
        self.entries = 0
        self.pages = 0
        self.longest_line = ""
        self.longest_line_at: dict[str, Any] | None = None
        self.preamble_events = 0
        self.preamble_keep_facing = 0
        self.codes_outside_preamble: list[dict[str, Any]] = []
        self.unreachable_tails: list[dict[str, Any]] = []
        self.line_overflow: list[dict[str, Any]] = []

    @staticmethod
    def _bump(counter: dict[Any, int], key: Any) -> None:
        counter[key] = counter.get(key, 0) + 1

    def add_entry(
        self, tree: int, entry: dict[str, Any], segments: Sequence[dict[str, Any]],
        pages: Sequence[dict[str, Any]],
    ) -> None:
        self.entries += 1
        by_tree = self.control_codes_by_tree.setdefault(tree, {})
        if not segments:
            self.empty_entries += 1
        for index, segment in enumerate(segments):
            if "text" in segment:
                for char in segment["text"]:
                    self._bump(self.glyph_bytes, char)
                continue
            code = segment["code"]
            self._bump(self.control_codes, code)
            self._bump(by_tree, code)
            name = segment["ctrl"]
            if name == "action":
                self._bump(self.actions, segment["action"])
            elif name == "portrait":
                self._bump(self.portrait_ids, segment["id"])
                if segment["position"] is not None:
                    self._bump(self.portrait_positions, segment["position"])
            elif name == "delay":
                self._bump(self.delays, segment["frames"])
            elif name in ("event", "keep_npc_facing"):
                # Both are preamble-only: everything before them must be a
                # flag check, or the interaction scanner never reaches them.
                preamble = all(
                    other.get("ctrl") == "flag_check" for other in segments[:index]
                )
                if preamble and name == "event":
                    self.preamble_events += 1
                elif preamble:
                    self.preamble_keep_facing += 1
                else:
                    self.codes_outside_preamble.append({
                        "tree": tree, "entry": entry["id"],
                        "segment": index, "ctrl": name,
                    })
            if name in ("yes_no",) and index != len(segments) - 1:
                self.unreachable_tails.append({
                    "tree": tree, "entry": entry["id"], "segment": index,
                    "ctrl": name, "trailing_segments": len(segments) - index - 1,
                })

        for page in pages:
            self.pages += 1
            self._bump(self.page_endings, page["end"])
            if len(page["lines"]) > LINES_PER_WINDOW:
                self.line_overflow.append({
                    "tree": tree, "entry": entry["id"], "lines": len(page["lines"]),
                })
            for line in page["lines"]:
                self._bump(self.line_lengths, len(line))
                if len(line) > len(self.longest_line):
                    self.longest_line = line
                    self.longest_line_at = {"tree": tree, "entry": entry["id"]}

    def to_json(
        self, portrait_ids: Iterable[int], blank_glyphs: Iterable[int]
    ) -> dict[str, Any]:
        known = set(portrait_ids)
        used = set(self.portrait_ids)
        blank = set(blank_glyphs)
        unused_glyphs = [
            {"byte": byte, "byte_hex": f"0x{byte:02X}", "char": char}
            for byte, char in sorted(DIALOGUE_CHARSET.items())
            if char not in self.glyph_bytes
        ]
        return {
            "entries": self.entries,
            "empty_entries": self.empty_entries,
            "pages": self.pages,
            "control_codes": dict(sorted(self.control_codes.items())),
            "control_codes_by_tree": {
                str(tree): dict(sorted(codes.items()))
                for tree, codes in sorted(self.control_codes_by_tree.items())
            },
            "unused_control_codes": sorted(
                f"0x{code:02X}" for code in CTRL_NAMES
                if f"0x{code:02X}" not in self.control_codes
            ),
            "text_actions": dict(sorted(self.actions.items())),
            "unused_text_actions": sorted(
                spec["name"] for spec in TEXT_ACTIONS.values()
                if spec["name"] not in self.actions
            ),
            "portrait_ids": {
                str(key): value for key, value in sorted(self.portrait_ids.items())
            },
            "portrait_ids_unused": sorted(known - used),
            "portrait_ids_outside_table": sorted(used - known - {0}),
            "portrait_positions": {
                str(key): value for key, value in sorted(self.portrait_positions.items())
            },
            "delay_frames": {
                str(key): value for key, value in sorted(self.delays.items())
            },
            "line_lengths": {
                str(key): value for key, value in sorted(self.line_lengths.items())
            },
            "longest_line": len(self.longest_line),
            "longest_line_text": self.longest_line,
            "longest_line_at": self.longest_line_at,
            "page_endings": dict(sorted(self.page_endings.items())),
            "preamble": {
                "events": self.preamble_events,
                "keep_npc_facing": self.preamble_keep_facing,
                "outside_preamble": self.codes_outside_preamble,
            },
            "unreachable_tails": self.unreachable_tails,
            "lines_past_the_window": self.line_overflow,
            "unused_charset_bytes": unused_glyphs,
            "blank_glyphs_in_charset": [
                {"byte": byte, "byte_hex": f"0x{byte:02X}", "char": char,
                 "used": char in self.glyph_bytes}
                for byte, char in sorted(DIALOGUE_CHARSET.items())
                if byte in blank and char != " "
            ],
            "note": (
                "Observed, not permitted. The script uses 10 of the 16 control "
                "codes and 66 of the 78 charset bytes; $FB is not a retail code "
                "at all; four charset bytes have an empty glyph in the font and "
                "none of the four is ever used; portrait id 0 means hide and is "
                "not a table entry; and no line anywhere exceeds 32 characters "
                "or spills past the window's two lines."
            ),
        }


# ---------------------------------------------------------------------------
# Emission
# ---------------------------------------------------------------------------
def _write_json(path: Path, payload: dict[str, Any]) -> str:
    """Write a pack JSON file and return its sha256.

    Sorted keys, fixed indent, no timestamps and nothing derived from the
    filesystem, so building the same pack twice produces the same bytes.
    """
    data = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8")
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def emit_dialogue(rom_bytes: bytes, out_dir: str | Path) -> dict[str, Any]:
    """Emit the dialogue half of the runtime pack; return the manifest fragment.

    `out_dir` is the pack root, the same directory `psiv_tools.pack.build_pack`
    writes `manifest.json` into; everything this module writes lands under
    `dialogue/` and every path in the returned fragment is relative to that
    root, so the fragment drops straight into the manifest.

    The output holds Sega-derived pixels and text and is never committed.
    """
    root = Path(out_dir)
    signatures = check_signatures(rom_bytes)
    palette = dialogue_palette(rom_bytes)
    (root / DIALOGUE_DIRECTORY).mkdir(parents=True, exist_ok=True)

    glyphs = glyph_bitmaps(rom_bytes)
    width, height, pixels = font_strip(glyphs)
    font_image = png.encode_indexed(
        width, height, pixels, list(palette), (BACKGROUND_COLOR_INDEX,)
    )
    (root / FONT_PNG_NAME).write_bytes(font_image)
    font = font_json(rom_bytes, glyphs, palette, font_image)
    font_sha = _write_json(root / FONT_JSON_NAME, font)

    window, window_image = emit_window(rom_bytes, root, palette)
    window_sha = _write_json(root / WINDOW_JSON_NAME, window)

    portraits, portrait_bytes = emit_portraits(rom_bytes, root, palette)
    portraits_sha = _write_json(root / PORTRAITS_NAME, portraits)

    decoded = extract_dialogue(rom_bytes)
    census = Census()
    trees = []
    for tree in decoded["trees"]:
        record, entries = tree_json(tree)
        trees.append(record)
        for entry in entries:
            census.add_entry(tree["tree"], entry, entry["segments"], entry["pages"])

    payload = {
        "format_version": DIALOGUE_FORMAT_VERSION,
        "kind": "dialogue_trees",
        "region": decoded["region"],
        "tree_count": len(trees),
        "entry_count": decoded["total_entries"],
        "charset": "dialogue",
        "window": window_json(),
        "interaction": interaction_json(),
        "control_codes": {
            f"0x{code:02X}": {"ctrl": name, "note": CTRL_NOTES[code]}
            for code, name in sorted(CTRL_NAMES.items())
        },
        "font": FONT_JSON_NAME,
        "chrome": WINDOW_JSON_NAME,
        "portraits": PORTRAITS_NAME,
        "trees": trees,
    }
    trees_sha = _write_json(root / TREES_NAME, payload)

    return {
        "format_version": DIALOGUE_FORMAT_VERSION,
        "directory": DIALOGUE_DIRECTORY,
        "trees": {
            "path": TREES_NAME,
            "sha256": trees_sha,
            "tree_count": len(trees),
            "entry_count": decoded["total_entries"],
        },
        "font": {
            "path": FONT_JSON_NAME,
            "sha256": font_sha,
            "png": FONT_PNG_NAME,
            "png_sha256": font["png_sha256"],
            "glyph_count": GLYPH_COUNT,
            "glyph_size": [GLYPH_WIDTH, GLYPH_HEIGHT],
        },
        "chrome": {
            "path": WINDOW_JSON_NAME,
            "sha256": window_sha,
            "png": WINDOW_PNG_NAME,
            "png_sha256": window["png_sha256"],
            "tile_count": window["tile"]["count"],
            "roles": {
                name: role["tile"] for name, role in sorted(window["roles"].items())
            },
            "rect": window["text_window"]["rect"],
        },
        "portraits": {
            "path": PORTRAITS_NAME,
            "sha256": portraits_sha,
            "directory": PORTRAITS_DIRECTORY,
            "count": portraits["count"],
            "distinct_art": portraits["distinct_art"],
            "bytes": portrait_bytes,
        },
        "window": window_json(),
        "signatures": signatures,
        "census": census.to_json(
            (entry["id"] for entry in portraits["portraits"]),
            (glyph["byte"] for glyph in font["glyphs"] if glyph["blank"]),
        ),
    }
