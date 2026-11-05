# `Event_PlateSysEngine`

- **Retail bytes:** `$06E75A..$06E907` code; the light table `loc_6E908` and the wait helper `loc_6E922` follow, to `$06E92B`.
- **Pointer:** `EventPtrs[$2D]` at `$05A2B4 + $B4`; event `$002D`.
- **Trigger:** interaction areas 1 to 3 of `PlateSystem_F4` (`$0BE`), row 23 at x 28 to 33: `Interaction_EventIndexes[2]`, gated on `EventFlag_PlateEngine` (`$61`) clear.
- **Data:** `plate_wreckage.rs`, `PLATE_SYS_ENGINE` (64 ops).
- **Flags:** sets `EventFlag_PlateEngine` (`$61`) last, which closes the three areas.

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every word matches
`ps4.asm:147856-147968`. It has no `revision` or `grand_cross` conditional and loads no tree. In the US
image `Char_Move_Flags` is `$ECFE` and `FieldObj_Step_Offset` `$ECE0`.

## Retail transcription

| Op | Retail primitive | Scene record |
|---:|---|---|
| 0-5 | `Event_GetCharacter` then `$38`/`$3A` for Chaz (`$1E0,$190`), Gryz (`$1C0,$180`), Demi (`$1E0,$170`), Rika (`$200,$190`), Rune (`$230,$170`) only when `bmi` does not skip | `SetActorDest` x 5, `BranchIfPartyMember` Rune |
| 6-8 | `FieldObj_Step_Offset := 0`, `bset #0, Char_Move_Flags`, `DoMainUpdatesLoop($4F)` | `SetStepOffset(0)`, `SetFollowMode(1)`, `Wait(80)` |
| 9-14 | Chaz, Gryz, Rika and Demi face up; Rune faces right when present | `Face`, `BranchIfPartyMember` |
| 15-16 | entry `$41`; `SFXID_PowerDown` | `RunDialogue`, `PlaySound` |
| 17-48 | `loc_6E842`, sixteen passes: `Palette_Line_2` words 12 and 13, then `loc_6E922` (eight `VInt_Prepare`) | `SetPaletteWords` + `WaitFrames(8)`, x 16 |
| 49-50 | words 12 and 13 := `$EC0`, `$CE`; `loc_6E922` x 5 | `SetPaletteWords`, `WaitFrames(40)` |
| 51 | `RunMapUpdates`, `VInt_Prepare` | `WaitFrames(1)` |
| 52-55 | Demi faces right, `DoMapUpdateLoop($E)`, faces down, `DoMapUpdateLoop($1D)` | `Face`, `Wait(15)`, `Face`, `Wait(30)` |
| 56 | `popdlg` / `Event_RunDialogue` | `RunDialogueResume` |
| 57-59 | `bclr #0, Char_Move_Flags`, `Event_MoveSingleObject` on `Character_1` to (`$1E0,$1C0`), `DoMainUpdatesLoop($3B)` | `SetFollowMode(0)`, `MoveActorTo` the leader, `Wait(60)` |
| 60-61 | `FieldObj_Step_Offset := 1`, `Event_MoveCamera` to the leader, speed 1 | `SetStepOffset(1)`, `PanCamera` |
| 62-63 | `loc_53F14` on `Map_Palettes_Addr`; `EventFlag_PlateEngine` | `ReloadMapPalette`, `SetFlag` `$61` |

**The light.** Each pass reads `move.w (a1)+, d0` from `loc_6E908` (`$860`, `$A80`, `$CA0`, `$EC0`,
`$CA0`, `$A80`, then `$FFFF`, which restarts the table without spending a pass) and `$E(a1)` with
the pointer already advanced, so the second word comes from `$10` past the pair. For the sixth pair
that is `$06E922`, the first word of `loc_6E922`'s code, `$7007`: the cartridge writes it as a colour
twice in the sixteen passes. The records keep it; the CRAM write masks it to `$0006`. The
`SetPaletteWords` offset is a byte offset from `Palette_Table_Buffer` here (`$38`, line 2's word
12). The record is presentation only, and the renderer does not draw it yet.

## Verification

`rust/psiv-core/tests/plate_wreckage_scenes.rs` holds the line, the resume, the seventeen palette
records with the read past the table, Rune's turn only when he is in the party, and the flag.
`rust/psiv-campaign/tests/plate_system.rs` reads the engine with pads on the real map, and a second
read with the flag set starts nothing.
