# `Event_EnterAngerTower`

- **Retail bytes:** `$0737BE..$07387F` inclusive, 194 bytes.
- **Pointer:** `EventPtrs[$99]` at `$05A2B4`; event `$0099`.
- **Trigger:** `RunEvent_EnterAngerTower` (`ps4.asm:116397-116403`), map trigger `$56` on the
  Anger Tower (`$FC`): fires while `EventFlag_AngerTower` (`$E0`) is clear.
- **Data:** `dezo_endgame.rs`, `ENTER_ANGER_TOWER` (16 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot and every
instruction word matches `ps4.asm:153370-153411`. No `revision` conditional and
no `DialogueTreesToRAM`.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$0737BE..$0737CB` | `move.b #EventFlag_Elsydeon ($D9), d0` / `jsr EventFlags_Test` / `bne.w loc_73820` | `BranchFlag` `$D9`: set to op 8 |
| 1 | `$0737CC..$0737D3` | `moveq #5, d0` / `jsr Event_GetAndRunDialogue` | entry 5 |
| 2-3 | `$0737D4..$0737E1` | `move.b #Sound_StopMusic ($FB), Sound_Index` / `jsr PalFadeOut_ClrSpriteTbl` | `PlaySound` `$FB`, `FadeOut` |
| 4 | `$0737E2..$073811` | `Field_Map_Index = $2` (Rykros), `Field_Map_Index_2 = $FC`, start `$44,$C2`, facing 0, alignment 0, `bclr #3, Map_Load_Flags`, `jsr RefreshMap` | `LoadMap` |
| 5-7 | `$073812..$07381F` | `move.b #MusicID_RyucrossField ($90), Sound_Index` / `jmp Pal_FadeIn` | `PlaySound` `$90`, `FadeIn`, `End` |
| 8 | `$073820..$073827` | `loc_73820`: `moveq #6, d0` / `jsr Event_GetAndRunDialogue` | entry 6 |
| 9 | `$073828..$07382F` | `move.b #SFXID_BarrierBroken ($E6), Sound_Index` | `PlaySound` `$E6` |
| 10-11 | `$073830..$073855` | `moveq #$27, d7`; per iteration `jsr Pal_IncreaseTone` (`$58608`), `lea Pal_Init_Line_3 ($296300), a0` / `lea Palette_Line_3 ($FFFFFB40), a1` / `move.w #$F, d7` / `trap #1`, `jsr VInt_Prepare`, `dbf` | `PaletteIncreaseTone` 40 frames with `hold` (`$296300` -> `$FFFFFB40`, 16 words); `WaitFrames` 40 |
| 12-13 | `$073856..$073867` | `moveq #$27, d7`; per iteration `jsr loc_58666`, `jsr VInt_Prepare`, `dbf` | `PaletteToneFlash` back toward `Palette_Table_Buffer_2`, 64 words, 40 frames; `WaitFrames` 40 |
| 14 | `$073868..$073875` | `popdlg` / `jsr Event_RunDialogue` | `RunDialogueResume` |
| 15 | `$073876..$07387F` | `move.b #EventFlag_AngerTower ($E0), d0` / `jmp EventFlags_Set` | `SetFlag` `$E0` |

**The barrier.** `Pal_IncreaseTone` (`$58608`) acts on frames whose
`Main_Frame_Count & 3` is zero: each of the 64 `Palette_Table_Buffer` words not
already `$EEE` has red, green and blue raised by one step, each capped at `$E`.
The loop then copies the sixteen words of `Pal_Init_Line_3` over the third
palette line before the frame is shown, so that line never brightens: the
`hold` of `PresentationOp::PaletteIncreaseTone`. `loc_58666` is the reverse,
one step per component toward `Palette_Table_Buffer_2` on the same frames,
the record [`Event_Recovery`](102_Recovery.md) already uses for its flash.

The route reaches the Elsydeon arm only: the Reunion ([84](84_Reunion.md))
comes after Elsydeon.

## Tests

`rust/psiv-core/tests/late_dezolis_scenes.rs`: the Rykros arm's dialogue, music,
fade and map load with no flag, and the Elsydeon arm's dialogue, SFX, the held
tone loop and its frames, the restoring loop, the resume and `$E0`.
