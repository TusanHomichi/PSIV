# The ranch owner: `Event_RanchOwner` and `Event_RanchOwnerAfterBattle`

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0070` `Event_RanchOwner` | `$07259E..$072653` inclusive, 182 bytes | `EventPtrs[$70]` at `$05A2B4 + $1C0` | `ranch.rs`, `RANCH_OWNER` (12 ops) |
| `$0072` `Event_RanchOwnerAfterBattle` | `$072678..$07269D` inclusive, 38 bytes | `EventPtrs[$72]` at `$05A2B4 + $1C8` | `ranch.rs`, `RANCH_OWNER_AFTER_BATTLE` (3 ops) |

- **Trigger:** dialogue `$F6`, `DialogueTree3`, from the owner (`Mile` `$01D`, object 0, dialogue
  `$11`). Entry `$11` chains `$FA $1D +6`, `$FA $1C +5`, `$FA $1B +4`, `$FA $1A +3`, `$FA $19 +2`,
  `$FA $18 +1`, `$FA $35 +1`. `$19`, the guild's job, reaches entry `$13`, `$F6 $00 $70`. `$1B`, the
  worm dead, with `$1C` clear reaches entry `$15`, `$F6 $00 $72`.
- **Flags:** `$1A` and `$1C` are written here. `$19` is the guild's `taken` flag for the job and
  `$1B` its `earned` flag, which [94](94_MileSandWormBattle.md) sets, so nothing written here is a
  flag the guild reads. `$1A` is read by the chain above and `MapDataMan_RanchOwner`
  (`ps4.asm:110360`), and it unlocks the worm in `RunEvent_MileSandWorm`
  ([94](94_MileSandWormBattle.md)).

## Clone audit

Both bodies were read from the US image through their `EventPtrs` slots, and every word matches
`ps4.asm:152178-152220` and `ps4.asm:152232-152241`. Neither has a `revision` or `grand_cross`
conditional, and neither loads a tree.

## Retail transcription

### `Event_RanchOwner`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$07259E..$0725CF` | `Character_1`'s X against `Field_Obj_Secondary`'s; while they differ, `FieldObj_NPCType7`, `Field_LoadSprites`, `Field_BuildSprites`, `VInt_Prepare` | `MoveActorToActorAxis` object 0, the leader, X |
| 1 | `$0725D0..$0725DB` | facing 0, `jsr Event_UpdateObjFacing` | `Face` object 0 down |
| 2 | `$0725DC..$0725E3` | `moveq #$20, d0` / `jsr Event_GetAndRunDialogue` | entry `$20`, standard window |
| 3 | `$0725E4..$0725ED` | `lea ($FFFFC500).w, a4`, `clr.w (a4)`, `clr.w $40(a4)` | `DespawnNpc` 8, two objects |
| 4 | `$0725EE..$07260F` | `move.w #$194, (a4)`, `$13 := $60`, `$16 := $24D` (the record's tile), `dest := curr` | `SetObjectRoutine` object 0 |
| 5-6 | `$072610..$072627` | `RunSingleObject`, `Field_LoadSprites`, `Field_BuildSprites`, `VInt_Prepare` | `RebuildSprites`, `WaitFrames(1)` |
| 7 | `$072628..$07262D` | `bset #1, Char_Move_Flags` | `SetFollowMode` 2 |
| 8 | `$07262E..$07263D` | `Event_MoveSingleObject` to (`$110,$2D0`) | `MoveActorTo`, blocking |
| 9 | `$07263E..$072647` | facing 0, `jsr Event_UpdateObjFacing` | `Face` object 0 down |
| 10 | `$072648..$07264D` | `bclr #1, Char_Move_Flags` | `SetFollowMode` 0 |
| 11 | `$07264E..$072653` | `moveq #$1A, d0` / `jmp EventFlags_Set` | `SetFlag` event `$1A` |

**Op 0.** `FieldObj_NPCType7` (`ps4.asm:95230`) takes its input from `loc_49C86`: within `$10` of the
leader's Y it steps along X toward him, and otherwise stands, which would spin the loop. The talk
range keeps him within it. `MoveActorToActorAxis` closes the same gap by whole cells at the normal
step. The routine's own pace is not modelled, a difference in frames, on a walk of at most a cell.
With the leader above or below him, the X words already match and no frame passes on either side.

**A reload.** `MapDataMan_RanchOwner` (`ps4.asm:110360`) gives him routine `$38` at (`$110,$2D0`)
once `$1A` is set and clears the blocks. The pack's map effects now carry the position words
(`object_x`, `object_y`), which were only noted as deferred before. That also places the other
objects whose map-load routine writes a position: `EspMansionNorth`, `ZemaHouse2`, `CulversHouse`
and `MystVale_Part2`.

### `Event_RanchOwnerAfterBattle`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072678..$07268D` | `lea Field_Obj_Secondary, a4`, `lea Character_1, a3`, `bchg #2`, `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 0, of the leader |
| 1 | `$07268E..$072695` | `moveq #$21, d0` / `jsr Event_GetAndRunDialogue` | entry `$21`, standard window |
| 2 | `$072696..$07269D` | `moveq #$1C, d0` / `jmp EventFlags_Set` | `SetFlag` event `$1C` |

## Tests

- `rust/psiv-core/tests/ranch_scenes.rs`: the order of `$70`'s writes and the last turn, and `$72`.
- Pad-driven: `rust/psiv-campaign/tests/ranch_side.rs` on hand-built saves: the talk, his cell and
  the blocks, the same after a fresh load with `$1A`, a negative control without the job, and the
  thanks.
- `tests/test_map_effects.py`, `rust/psiv-pack/src/maps/effects/decoder/tests.rs`: the position words.
