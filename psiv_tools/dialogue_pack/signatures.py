"""The retail opcodes this package transcribes, and the check that they are.

The `reference/` clone is a Grand Cross build whose `revision` and
`large_dialog_window` switches change every number in `window.py`, so nothing
is emitted until the image on disk is the one these bytes came from.
"""

from __future__ import annotations

from typing import Any

from .common import DialoguePackError, rom_slice

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
    "system_messages": (
        0x058CE8, "1a38f40a41f9001fe660c1454eb90005917cc1454eb90006a090",
        "Interaction_DoPlayerNothingMsg: Current_Party_Slots indexes "
        "WinTiles_PlayerNothingMsg through GetOffsetByID, then RunText",
    ),
}

def check_signatures(rom: bytes) -> list[dict[str, Any]]:
    """Assert every routine this module transcribes is the retail one."""
    checked = []
    for name, (offset, expected, note) in sorted(SIGNATURES.items()):
        raw = bytes.fromhex(expected)
        actual = rom_slice(rom, offset, len(raw), name)
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
