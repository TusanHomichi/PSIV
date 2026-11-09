# `Event_GettingOffVehicle`

- **Retail bytes:** `$06BEF8..$06C023` inclusive; `CannotGetOffString` follows at `$06C024`.
- **Pointer:** `EventPtrs[$10]` at `$05A2B4 + $40`; event `$0010`.
- **Reached by:** the field input, not a map. While the party rides, `loc_56024` puts a press of
  Cancel, Speak or Camp (A, B or C) into `Field_Input_Buffer`; once the machine stands still the
  table at `loc_56092` turns it into `Game_Mode_Routine` `$C` and `loc_56084` writes `Event_Index`
  `$10`. In the port the core's mounted Action edge reports `VehicleEffect::GetOff` and the runtime
  installs the scene (`rust/psiv-runtime/src/vehicle.rs`).
- **Data:** `vehicles.rs`, `GETTING_OFF_VEHICLE` (9 ops).
- **Flags:** none. It writes `Vehicle_Index`, `Saved_Sound_Index` and `Sound_Index`.

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every word matches
`ps4.asm:145343-145413`. The three machines' tests (`subq.w #1` / `cmpi.w #2`) all reach the same
`tst.w d7 / bne loc_6BFB4`, so the decision does not depend on the machine. `CannotGetOffString` is
the `revision>0` side, in the window charset; the pack carries it (`trees.json`,
`system_messages.cannot_get_off`, read from the address the `lea` at `$06BFEC` names).

## Retail transcription

| Op | Retail primitive | Scene record |
|---:|---|---|
| 0 | `jsr loc_4587A` (`GetChunkAndCollision` at `Character_1`, the machine); nonzero `d7` to `loc_6BFB4` | `GetOff(BranchIfBlocked)` |
| 1-2 | `Field_BuildSprites`, `VInt_Prepare` | `RebuildSprites`, `WaitFrames(1)` |
| 3 | `Pal_Init_Line_3` to `Palette_Line_3`, `moveq #$10, d7 / trap #1`: seventeen words | `PaletteCopy` |
| 4 | `loc_6BF4E`: every party object rebuilt on `Character_1`'s position, destination and facing; `Vehicle_Index := 0` | `SetVehicleIndex(0)` (the runtime stacks the party on the machine) |
| 5 | the map record's music byte, `AdjustMusicIDs`, `d0` to `Saved_Sound_Index` and `Sound_Index` | `GetOff(PlayMapMusic)` |
| 6 | `rts` | `End` |
| 7 | `loc_6BFB4`: `Window_Create(0)`, `LoadWindowTiles` of `CannotGetOffString`, then `DMAPlane_A_VInt` until `Joypad_Pressed` holds a direction, A, B or C | `RunDialogue` `System(CannotGetOff)` |
| 8 | `Window_Destroy`, `jmp Map_LoadChunks` | `ReloadMapChunks` |

**The music.** The field loader writes `AdjustMusicIDs`'s answer only when it returns zero in `d1`;
this body writes `d0` whatever `d1` says, so the "already playing" cases put the raw byte back.
`map_change::adjust_music_ids` is the routine itself, which both paths now share.

## Deviations

The refusal opens in the dialogue window, closed by the dialogue's own press, rather than in
window 0 drawn with window tiles and closed by any direction, A, B or C. Recorded in
[the source notes](../source-notes/README.md).

## Verification

`rust/psiv-core/tests/vehicle_get_off_scenes.rs`: both arms against made-up collision.
`rust/psiv-runtime/tests/session_menu_scenes.rs`: board the Land Rover with pads, press Speak on open
ground and the party stands down on the machine's cell; press it on ground that refuses and the
line opens, a press closes it and the party stays mounted. `tests/test_dialogue_pack.py`: the line is
the string the `lea` names, and a moved `lea` stops the extraction.
