# Tallas's rescue: his mother, the crevice and the walk out

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$007A` `Event_TallasMom` | `$072AD0..$072AF5` inclusive, 38 bytes | `EventPtrs[$7A]` at `$05A2B4 + $1E8` | `monsen.rs`, `TALLAS_MOM` (3 ops) |
| `$007B` `Event_TallasMomAfterRescue` | `$072AF6..$072B1B` inclusive, 38 bytes | `EventPtrs[$7B]` at `$05A2B4 + $1EC` | `monsen.rs`, `TALLAS_MOM_AFTER_RESCUE` (3 ops) |
| `$007C` `Event_InsideMonsenHole` | `$072B1C..$072B2B` inclusive, 16 bytes | `EventPtrs[$7C]` at `$05A2B4 + $1F0` | `monsen.rs`, `INSIDE_MONSEN_HOLE` (2 ops) |
| `$007E` `Event_TallasRescued` | `$072B52..$072C07` inclusive, 182 bytes | `EventPtrs[$7E]` at `$05A2B4 + $1F8` | `monsen.rs`, `TALLAS_RESCUED` (14 ops) |

- **Triggers:** `$7A` and `$7B` are dialogue `$F6`, `DialogueTree12` entries `$1C` and `$1E`, from
  the mother (`MonsenHouse4` `$078`, object 0, dialogue `$1A`). Entry `$1A`'s chain is `$FA $3D +6`,
  `$FA $3C +5`, `$FA $2F +4`, `$FA $2D +3`, `$FA $2C +2`
  ([trigger census](TRIGGER_CENSUS.md) has `$FA`'s forward delta). So `EventFlag_FissureOfFear`
  (`$2C`, the guild's job) with `$2D` clear reaches `$7A`, and `EventFlag_FractOoze` (`$2F`) with
  `$3C` clear reaches `$7B`. `$7C` is map trigger `$71` on `MonsenCave` (`$0EC`),
  `RunEvent_InsideMonsenCave` (`ps4.asm:116468`): `$2E` clear. `$7E` is map trigger `$72`,
  `RunEvent_SavingTallas` (`ps4.asm:116475`): `$2F` set and `$3E` clear. Both are
  `trigger_table.rs` rows and take no position.
- **Flags:** `$2D`, `$2E`, `$3C` and `$3E` are written here. `$2C` is the guild's `taken` flag for
  this job and `$2F` its `earned` flag (the pack's guild rows; [124](124_HuntersGuild.md) sets the
  first and [95](95_FractOozeFound.md) the second), so this lane's scenes write nothing the guild
  reads. `$2D` is read by `MapDataMan_MonsenSign` (`ps4.asm:110530`), which takes the danger sign
  off the crevice, and by the chain above, as is `$3C`.

## Clone audit

All four bodies were read from the US image through their `EventPtrs` slots, and every word
matches `ps4.asm:152536-152560` and `ps4.asm:152571-152611`. None has a `revision` or
`grand_cross` conditional, and none loads a tree.

## Retail transcription

### `Event_TallasMom` and `Event_TallasMomAfterRescue`

| Op | ROM offsets (`$7A` / `$7B`) | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072AD0..$072AE5` / `$072AF6..$072B0B` | `lea Field_Obj_Secondary, a4` / `lea Character_1, a3` / `move.w $6(a3), d0` / `bchg #2, d0` / `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 0, of the leader |
| 1 | `$072AE6..$072AED` / `$072B0C..$072B13` | `moveq #$2D` or `#$2E, d0` / `jsr Event_GetAndRunDialogue` | entry `$2D` or `$2E`, standard window |
| 2 | `$072AEE..$072AF5` / `$072B14..$072B1B` | `moveq #$2D` or `#$3C, d0` / `jmp EventFlags_Set` | `SetFlag` event `$2D` or `$3C` |

### `Event_InsideMonsenHole`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072B1C..$072B23` | `moveq #$24, d0` / `jsr Event_GetAndRunDialogue` | entry `$24`, standard window |
| 1 | `$072B24..$072B2B` | `moveq #$2E, d0` / `jmp EventFlags_Set` | `SetFlag` event `$2E` |

### `Event_TallasRescued`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072B52..$072B59` | `moveq #$27, d0` / `jsr Event_GetAndRunDialogue` | entry `$27`, standard window |
| 1 | `$072B5A..$072B61` | `lea ($FFFFC380).w, a4` / `move.w #$2FC, (a4)` | `SetObjectRoutine` object 2 |
| 2-3 | `$072B62..$072B79` | `RunSingleObject`, `Field_LoadSprites`, `Field_BuildSprites`, `VInt_Prepare` | `RebuildSprites`, `WaitFrames(1)` |
| 4-10 | `$072B7A..$072BF5` | `Event_MoveSingleObject` to (`$610,$2D0`) twice, (`$610,$320`), (`$670,$320`), (`$670,$300`), (`$5F0,$300`), (`$5F0,$390`) | `MoveActorTo`, seven, each blocking |
| 11 | `$072BF6..$072BF7` | `clr.w (a4)` | `DespawnNpc` object 2 |
| 12 | `$072BF8..$072BFD` | `move.b #MusicID_InTheCave ($8A), Saved_Sound_Index` | `SetSavedMusic` |
| 13 | `$072BFE..$072C07` | `moveq #$3E, d0` / `jmp EventFlags_Set` | `SetFlag` event `$3E` |

`FieldObj_Tallas` (`ps4.asm:96607`, slot `$2FC`) copies its position into `dest` on its first frame,
so that frame does not move him, and then walks by `FieldObj_GetInput` at the normal step. The
record's slot on `MonsenCave` is `$C380`, the kid `NPCType16` (`$C0`), at (`$650,$2D0`).

**The second call.** `Event_MoveSingleObject` runs the object once before it compares, so the
repeat of (`$610,$2D0`) costs one frame on the cartridge. `MoveActorTo` to where the actor already
stands takes none here: one frame's deviation, on no observable state.

**A reload.** `MapDataMan_MonsenCaveObjs` (`ps4.asm:109964`) clears objects 0, 1 and 3 once `$2F` is
set, and not object 2, so a reload past this scene rebuilds Tallas from the record. The pack's map
effect has the same three writes, and the port does what those bytes say. No capture has been
compared.

## Tests

- `rust/psiv-core/tests/tallas_scenes.rs`: both of the mother's talks, the fall's line and flag, and
  Tallas's corners in order, with the arrival, slot clear, saved music and flag in that order.
- Pad-driven: `rust/psiv-campaign/tests/monsen_side.rs` on hand-built saves, since the route takes
  neither: the mother's chain with each flag and a negative control without the job; the landing
  once only, and the walk out with its negative control without `$2F`.
