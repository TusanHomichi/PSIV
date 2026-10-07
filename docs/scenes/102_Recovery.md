# `Event_Recovery`

- **Retail bytes:** `$06D37C..$06D467` inclusive, 236 bytes: the body (`$06D37C..$06D3B9`,
  to the `rts` at `$06D3B8`) and its two flash subroutines `loc_6D3BA` and `loc_6D40A`.
- **Pointer:** `EventPtrs[$21]` at `$05A2B4`; event `$0021`.
- **Trigger:** `RunEventsJmpTbl[$12]`, `RunEvent_Recovery` (`ps4.asm:115574-115581`): the
  leader's `Tile_Collision_Standing` is 2 and `Saved_Tile_Collision_Standing` is not, so the
  tile fires once per step onto it. Maps whose event list names slot `$12`: the Air Castle's
  `AirCastle_F1_Part9` `$176` (cells (31..32,28..29)) and `AirCastleXeAThoulRoom` `$184`
  ((31..32,18..19)), `DezoSpaceport` and three more (the [census](EVENT_COVERAGE.md)).
  `Event_RuneHealingChaz` (`$67`) also calls it (`ps4.asm:151678`), so it is part of that
  scene when that scene lands.
- **Data:** `recovery.rs`, `RECOVERY` (16 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot and every instruction word
matches `ps4.asm:146852-146934`. No `revision` or `grand_cross` conditional touches it.

## Retail transcription

| Ops | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06D37C..$06D389` | `lea Palette_Table_Buffer, a0` / `lea Palette_Table_Buffer_2, a1` / `move.w #$1F, d7` / `trap #1` (32 longwords) | `Presentation` `CopyRamWords` `$FB00` to `$FB80`, 64 words |
| 1 | `$06D38A..$06D391` | `move.b #SFXID_Recovery ($CD), (Sound_Index).l` | `PlaySound` `$CD` |
| 2-13 | `$06D392..$06D3A9` | `bsr loc_6D3BA` / `bsr loc_6D40A`, three times | `Presentation` `PaletteToneFlash` (brighten, then back) and `WaitFrames` 16 after each |
| 14 | `$06D3AA..$06D3B1` | `move.b #SFXID_Res ($CC), (Sound_Index).l` | `PlaySound` `$CC` |
| 15 | `$06D3B2..$06D3B8` | `jsr RecoverStats` (`$0662DA`, `ps4.asm:136503`) / `rts` | `RecoverStats` |

`loc_6D3BA` (`$06D3BA`, `ps4.asm:146867`) and `loc_6D40A` (`$06D40A`, `:146898`) each loop
`moveq #$F, d7` (16) times around one `VInt_Prepare`, so each is 16 frames with no map update:
`WaitFrames`. On a frame whose `Main_Frame_Count & 3` is zero the first steps each of the first
`$1F + 1` = 32 palette words one tone toward white, and the second steps them back toward the
copy op 0 saved. The scene spends 96 frames before the cure.

`RecoverStats` refills HP, TP and skill uses and writes 0 to the status byte of every member in
`Current_Party_Slots` (`DoCharRecovery`, `ps4.asm:136522-136534`), so an android shut down by
bit 6 (`StatusAndroidDead`) comes back; it then refills the three vehicles' skill uses
(`DoVehicleRecovery`). The event returns through `loc_5A27A`, which ignores `d0`
(`ps4.asm:120553-120568`).

`PaletteToneFlash` is a new presentation record: Godot logs it with the other palette records it
does not draw (`rust/psiv-godot/src/cutscene/ops.rs`).

## In the route (C8)

`air-castle-xe-athoul-room` walks the party over `AirCastle_F1_Part9`'s tile on the way to the
Xe-A-Thoul room: Wren, shut down since Dark Force 1, is repaired there, as a player would.
