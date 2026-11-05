# `Event_PlateSysEngine`

- **Retail bytes:** `$06E75A..$06E907` code; the light table `loc_6E908` and the wait helper `loc_6E922` follow, to `$06E92B`.
- **Pointer:** `EventPtrs[$2D]` at `$05A2B4 + $B4`; event `$002D`.
- **Trigger:** interaction areas 1 to 3 of `PlateSystem_F4` (`$0BE`), row 23 at x 28 to 33: `Interaction_EventIndexes[2]`, gated on `EventFlag_PlateEngine` (`$61`) clear.
- **Data:** `plate_wreckage.rs`, `PLATE_SYS_ENGINE` (33 ops).
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
| 17 | `loc_6E842`, sixteen passes: `Palette_Line_2` words 12 and 13 from `loc_6E908`, then `loc_6E922` (eight `VInt_Prepare`) | `PaletteCycleFromTable` (16 passes, 8 frames each) |
| 18-19 | words 12 and 13 := the code's immediates; `loc_6E922` x 5 | `SetPaletteWords`, `WaitFrames(40)` |
| 20 | `RunMapUpdates`, `VInt_Prepare` | `Wait(1)`: one frame that updates the map |
| 21-24 | Demi faces right, `DoMapUpdateLoop($E)`, faces down, `DoMapUpdateLoop($1D)` | `Face`, `Wait(15)`, `Face`, `Wait(30)` |
| 25 | `popdlg` / `Event_RunDialogue` | `RunDialogueResume` |
| 26-28 | `bclr #0, Char_Move_Flags`, `Event_MoveSingleObject` on `Character_1` to (`$1E0,$1C0`), `DoMainUpdatesLoop($3B)` | `SetFollowMode(0)`, `MoveActorTo` the leader, `Wait(60)` |
| 29-30 | `FieldObj_Step_Offset := 1`, `Event_MoveCamera` to the leader, speed 1 | `SetStepOffset(1)`, `PanCamera` |
| 31-32 | `loc_53F14` on `Map_Palettes_Addr`; `EventFlag_PlateEngine` | `ReloadMapPalette`, `SetFlag` `$61` |

**The light.** Each pass reads `move.w (a1)+, d0` from `loc_6E908`'s six colours; the seventh word
is negative, which restarts the table without spending a pass. The second word is `$E(a1)` with the
pointer already advanced. For the sixth colour that is `$06E922`, the first word of `loc_6E922`'s
code (`moveq #7, d0`): the cartridge writes it as a colour twice in the sixteen passes. The table is
the cartridge's, so the scene record carries only its address and the loop's shape
(`PaletteCycleFromTable`), and the pack holds the words: `psiv_tools.presentation_pack`'s
`PLATE_ENGINE_LIGHT` emits `$06E908..$06E924`, fourteen words through that code word, into
`presentation/panels.json`'s `palettes`, and `psiv_core::palette_cycle_writes` expands them as the
loop reads them. Both palette records address `Palette_Line_2 + $18` (`Palette_Table_Buffer + $38`)
as word offset `$1C`, the unit `SetPaletteWords` documents. The loop's waits are `VInt_Prepare`
only, so they and the forty-frame hold run no map update; the explicit `RunMapUpdates` after them
runs exactly one. The records are presentation only, and the renderer does not draw them yet.

## Verification

`rust/psiv-core/tests/plate_wreckage_scenes.rs` holds the line, the resume, the two palette records
and their offset, the frames that run map updates from the light to Demi's turns, the loop's
expansion on made-up words, Rune's turn only when he is in the party, and the flag.
`tests/test_presentation_pack.py` checks the pack's window against the ROM and that it ends at the
loop's deepest read; `rust/psiv-campaign/tests/plate_system.rs` expands the pack's words into the
sixteen writes.
`rust/psiv-campaign/tests/plate_system.rs` reads the engine with pads on the real map, and a second
read with the flag set starts nothing.
