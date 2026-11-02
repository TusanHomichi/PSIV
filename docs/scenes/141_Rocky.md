# Rocky: `Event_RockyFound` and `Event_CatchingRocky`

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0073` `Event_RockyFound` | `$07269E..$0726C3` inclusive, 38 bytes | `EventPtrs[$73]` at `$05A2B4 + $1CC` | `rocky.rs`, `ROCKY_FOUND` (3 ops) |
| `$0074` `Event_CatchingRocky` | `$0726C4..$072871` inclusive, 430 bytes | `EventPtrs[$74]` at `$05A2B4 + $1D0` | `rocky.rs`, `CATCHING_ROCKY` (58 ops) |

- **Triggers:** dialogue `$F6` from the runaway dog, `FieldObj_StrayRocky` (`$2F4`): Monsen `$073`
  object 7 dialogue `$28`, Krup `$039` object 11 dialogue `$33`, Termi `$07B` object 9 dialogue
  `$4B`. Tree 12 entry `$28` is `$FA $22 +1`, `$FA $20 +1`, `$F6 $00 $73`, then entry `$29` is
  `$F6 $00 $74`. So the first talk at Monsen is `$73`, and after the Krup escape it is `$74`.
  Tree 5 entry `$33` is `$FA $21 +1` to entry `$34`, `$74`, and otherwise his own line. Tree 13's
  `$4B` is `$74` at once.
- **Other census sites:** tree 6 entry `$34` and tree 12 entry `$5C` also hold `$F6 $00 $74`. No
  map object on a tree 6 map names `$33` or `$34`, and none on a tree 12 map names `$5C`. Nothing
  here covers a chain from another entry to them.
- **Where he stands:** the map effects (`MapDataMan_KrupRocky`, `MapDataMan_MonsenRocky`,
  `MapDataMan_Termi`, `ps4.asm:110391-110470`) place him from the flags. These come through the
  pack's `map_effects`, so the scene needs no code for them.
- **Flags:** `$20` is written by `$73`, `$21`, `$22`, `$23` and `$24` by `$74`. `$24`,
  `EventFlag_RockyHome`, is the `earned` flag of the guild's dog job (row `$1E`, `$24`, `$25`,
  [124](124_HuntersGuild.md)): the guild pays on it. `MapDataMan_RockyHouse` reads it too.

## Clone audit

Both bodies were read from the US image through their `EventPtrs` slots, and every word matches
`ps4.asm:152242-152251` and `ps4.asm:152252-152386`, including the `loc_72866` table. Neither has a
`revision` or `grand_cross` conditional, and neither loads a tree. The load's tree 11 comes from
`RefreshMap` on `RockyHouse`.

## Retail transcription

### `Event_RockyFound`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$07269E..$0726B3` | `lea ($FFFFC4C0).w, a4`, `lea Character_1, a3`, `bchg #2`, `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 7, of the leader |
| 1 | `$0726B4..$0726BB` | `moveq #$2C, d0` / `jsr Event_GetAndRunDialogue` | entry `$2C`, standard window |
| 2 | `$0726BC..$0726C3` | `moveq #$20, d0` / `jmp EventFlags_Set` | `SetFlag` event `$20` |

### `Event_CatchingRocky`

The routine picks its object and row by reading `Field_Map_Index` each time. Here each map is an
arm, with `SkipUnlessMap` where the routine compares.

| Map | Object | `loc_72866` row: first line, escape flag, caught line |
|---|---|---|
| Krup `$39` | 11, `$FFFFC5C0` | `$35`, `$22`, `$36` |
| Monsen `$73` | 7, `$FFFFC4C0` | `$2A`, `$23`, `$2B` |
| Termi `$7B` | 9, `$FFFFC540` | `$4C`, `$21`, `$4D` |

The Termi arm is the routine's fall-through, so it needs no compare for its object. The row scan
would run off the table on a fourth map, which no object reaches.

| Ops | Retail primitive | Scene record |
|---:|---|---|
| 0-6 | the object by map, then `bchg #2`, `Event_UpdateObjFacing` | `SkipUnlessMap`, `FaceOppositeOf`, `SkipOps` |
| 7 | `move.b #ItemID_ShortCake ($91), d0` / `jsr GetItem` / `beq.w loc_727CA` | `Rocky(BranchIfHolding)` |
| 8-10 | `EventFlags_Clear` `$21`, `$22`, `$23` | `SetFlag` false x 3 |
| 11, 22 | the row's scan | `SkipUnlessMap` |
| arm 1 | `Event_GetAndRunDialogue` on the row's first line | entry, standard window |
| arm 2-4 | `move.w #$2F8, (a4)`, `RunSingleObject`, `Field_LoadSprites`, `Field_BuildSprites`, `VInt_Prepare` | `SetObjectRoutine`, `RebuildSprites`, `WaitFrames(1)` |
| arm 5-6 | `move.w $30(a4), d0 / subi.w #$B0, d0 / move.w $34(a4), d1 / jsr Event_MoveSingleObject` | `Rocky(RunFast)`, `MoveActorOffset` -`$B0`, 0 |
| arm 7 | `clr.w (a4)` | `DespawnNpc` |
| arm 8 | `popdlg` / `jsr Event_RunDialogue` | `RunDialogueResume` |
| arm 9-10 | `move.b $2(a0), d0` / `jmp EventFlags_Set` | `SetFlag` the row's flag, `End` |
| 43-49 | `loc_727CA`: the row's scan, `Event_GetAndRunDialogue` on its last byte | `SkipUnlessMap`, entry, `SkipOps` |
| 50 | `loc_727EE`: `GetItem`, `clr.b (a0)`, `ReorderInventory`, until `GetItem` finds none | `Rocky(RemoveAll)` |
| 51 | `PalFadeOut_ClrSpriteTbl` | `FadeOut` |
| 52 | `moveq #$24, d0` / `jsr EventFlags_Set` | `SetFlag` event `$24` |
| 53 | `RockyHouse`, `Field_Map_Index_2` Aiedo, start (`$3C,$42`), facing `$C`, align `$10`, `bclr #3, Map_Load_Flags`, `RefreshMap` | `LoadMap` |
| 54-55 | `move.b #MusicID_AHappySettlement, Sound_Index`, `Pal_FadeIn` | `PlaySound` `$9E`, `FadeIn` |
| 56-57 | `lea ($FFFFC340).w, a4`, facing 8, `moveq #$46` / `jmp Event_GetAndRunDialogue` | `Face` object 1 right, entry `$46`, standard window |

**The run.** `loc_4BC80` (slot `$2F8`, `ps4.asm:99606`) sets `FieldObj_Step_Offset` to 2 around its
own move and back to 1, which is the movement table's third block: `$0400` a frame, four frames a
cell. `Rocky(RunFast)` gives the object that pace. `$B0` is eleven cells.

**The escape flags.** Only the arm that runs clears them. The caught arm leaves whichever was on:
`$24` wins in every `MapDataMan_*` that reads them, since each tests it first.

**The cake scan.** `GetItem` (`ps4.asm:121975`) compares only the first `occupied` bytes, and
`ReorderInventory` (`ps4.asm:124264`) closes only the first hole, so a battle's holes change both
the catch and the removal. With `[__ 01 91]` the window is two bytes: he runs and the cake stays.
With `[01 91 __ 91]` the first pass leaves `[01 __ 91 __]`, window two: he is caught and one cake
stays. `Rocky(BranchIfHolding)` and `Rocky(RemoveAll)` both ask `Inventory::find_item`, again after
each removal.

## Tests

- `rust/psiv-core/tests/rocky_scenes.rs`: `$73`'s turn, line and flag; each row without a shortcake
  (object, line, routine, 44 ticks for the run, resume, only that map's flag on, whichever was on);
  each row with shortcakes (line, every cake gone with the holes closed, `$24`, the load, the
  owner's turn and line); the scan window on each row (a cake past it at the catch, and packed,
  hole-first and past-the-window lists through the removal loop).
- Pad-driven: `rust/psiv-campaign/tests/rocky_side.rs` on hand-built saves, since the route does
  not take the guild's jobs: found at Monsen with the negative control before the owner, the chase
  from Termi to Krup to Monsen with the object the map effects show on each, and the catch at Termi
  ending in `RockyHouse`.
