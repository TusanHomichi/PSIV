"""The dialogue half of the runtime pack: text, font, arrow and portraits.

`psiv_tools.pack` emits what field mode needs to *walk*; this module emits what
it needs to *talk*. Three files and one directory, all under `dialogue/`:

    dialogue/trees.json          43 trees, every entry as renderer-ready segments
    dialogue/font.png            the 80 dialogue glyphs as an 8x16 strip
    dialogue/font.json           byte -> glyph rect, plus the window's palette
    dialogue/menu_font.png       the retail 8x8 battle/menu glyph sheet
    dialogue/window.png          ArtNem_WindowTiles, the box's chrome
    dialogue/window.json         which tile is which corner, and the box's size
    dialogue/scroll_arrow.png    the retail 2x1 waiting sprite
    dialogue/portraits.json      the 39 portrait ids and their provenance
    dialogue/portraits/*.png     48x48 composed portraits

`trees.json` also carries `system_messages`: `WinTiles_PlayerNothingMsg`, the
eleven uncompressed lines a party leader says when Talk finds nothing, in the
same shape as a tree entry because the cartridge runs them through the same
`RunText`.

Nothing here re-decodes anything. The trees come from `psiv_tools.text`, the
font, arrow and portraits from `psiv_tools.gfx` plus the retail loader
signatures. What lives here and nowhere else is the *window*: how many
characters fit on a line, how many lines fit in the window, which colours the
glyphs are drawn in, and where the box and its waiting sprite sit on the
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

Where things live
-----------------

    common.py      the file names, the error, and the three shared helpers
    signatures.py  the retail opcodes every constant below is read off
    window.py      the text window's metrics, palette and geometry
    chrome.py      ArtNem_WindowTiles, the frame's role map, WinGroup_Dialogue
    art.py         the font strip, waiting sprite and portraits
    flow.py        control codes, typed segments, pagination, the census
    emit.py        `emit_dialogue`, which writes all of the above

Importing `psiv_tools.dialogue_pack` gives the whole public surface, so a
consumer never needs to know which module a name came from.
"""

from __future__ import annotations

from .art import (
    PORTRAIT_CUTSCENE_TILE_X,
    PORTRAIT_CUTSCENE_TILE_Y,
    PORTRAIT_PLANE_MAP,
    PORTRAIT_TALK_SLOT_TILES,
    PORTRAIT_TILE_X,
    PORTRAIT_TILE_Y,
    PORTRAIT_TRANSPARENT_INDEX,
    PORTRAIT_VRAM_TILE,
    SCROLL_ARROW_ART_COMPRESSED_SIZE,
    SCROLL_ARROW_ART_ROM_OFFSET,
    SCROLL_ARROW_FONT_VRAM_TILE,
    SCROLL_ARROW_MAPPING_BYTES,
    SCROLL_ARROW_MAPPING_ROM_OFFSET,
    SCROLL_ARROW_TILE_INDEX,
    SCROLL_ARROW_TILE_COUNT,
    SCROLL_ARROW_VRAM_TILE,
    emit_portraits,
    emit_scroll_arrow,
    font_json,
    font_strip,
    glyph_bitmaps,
    portrait_png,
)
from .chrome import (
    DIALOGUE_WINDOW_INDEX,
    MENU_FONT_TILES,
    MENU_FONT_VRAM_TILE,
    PORTRAIT_WINDOW_INDEX,
    WIN_GROUP_DIALOGUE,
    WIN_GROUP_DIALOGUE_NAMES,
    WIN_GROUP_RECORD_SIZE,
    WINDOW_ART_COMPRESSED_SIZE,
    WINDOW_ART_ROM_OFFSET,
    WINDOW_ART_TILES,
    WINDOW_ART_VRAM_TILE,
    WINDOW_BORDER_CELLS,
    WINDOW_FRAME_WORDS,
    WINDOW_OPEN_STEP_CELLS,
    check_window_fits_the_text,
    emit_window,
    frame_roles,
    pattern_word,
    window_json_document,
    window_records,
)
from .common import (
    DIALOGUE_DIRECTORY,
    DIALOGUE_FORMAT_VERSION,
    FONT_JSON_NAME,
    FONT_PNG_NAME,
    MENU_FONT_PNG_NAME,
    PORTRAITS_DIRECTORY,
    PORTRAITS_NAME,
    SCROLL_ARROW_PNG_NAME,
    TREES_NAME,
    WINDOW_JSON_NAME,
    WINDOW_PNG_NAME,
    DialoguePackError,
    rom_slice,
    safe_name,
    write_json,
)
from .emit import emit_dialogue
from .flow import (
    CTRL_NAMES,
    CTRL_NOTES,
    SYSTEM_MESSAGE_COUNT,
    SYSTEM_MESSAGES_LABEL,
    SYSTEM_MESSAGES_ROM_OFFSET,
    Census,
    control_segment,
    entry_segments,
    paginate,
    split_preamble,
    system_messages,
    tree_json,
)
from .signatures import SIGNATURES, check_signatures
from .window import (
    BACKGROUND_COLOR_INDEX,
    CHARS_PER_LINE,
    DIALOGUE_CRAM_LINE,
    FONT_ROM_OFFSET,
    FONT_SIZE,
    GLYPH_BYTES,
    GLYPH_COUNT,
    GLYPH_HEIGHT,
    GLYPH_INDEX_MASK,
    GLYPH_WIDTH,
    LINES_PER_WINDOW,
    PAGE_ENDINGS,
    PAL_INIT,
    PAL_INIT_LINE_3,
    PAL_INIT_MIRRORED_LINE,
    SCROLL_ARROW_SPRITE_X,
    SCROLL_ARROW_SPRITE_Y,
    SPRITE_ORIGIN,
    TEXT_COLOR_INDEX,
    TILE_PIXELS,
    WINDOW_TILE_HEIGHT,
    WINDOW_TILE_WIDTH,
    WINDOW_TILE_X,
    WINDOW_TILE_Y,
    WINDOW_VRAM_ADDRESS,
    WINDOW_VRAM_TILE,
    dialogue_palette,
    interaction_json,
    window_json,
)
