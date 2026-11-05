# Kadary: `Event_FaintingPriest` and `Event_ZioFanatic`

Two Kadary events outside the guild's job (tree 9). Before Zio is beaten, the priest at the altar of
`KadaryChurch` (`$069`) faints in front of the party. After Zio, the believer standing in Kadary's
street (`Kadary`, `$068`) falls to the ground writhing. The coordinating issue grouped both under
"Piata and the student"; they share only Kadary with that job ([160](160_MissingStudent.md)).

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0024` `Event_FaintingPriest` | `$06D616..$06D6C7` inclusive, 178 bytes | `EventPtrs[$24]` at `$05A2B4 + $90` | `kadary_events.rs`, `FAINTING_PRIEST` (12 ops) |
| `$0031` `Event_ZioFanatic` | `$06EE40..$06EEBD` inclusive, 126 bytes | `EventPtrs[$31]` at `$05A2B4 + $C4` | `kadary_events.rs`, `ZIO_FANATIC` (6 ops) |

- **Triggers:** dialogue `$F6`. (`$FA`'s operand is a forward delta:
  [trigger census](TRIGGER_CENSUS.md).)
  - The priest (`KadaryChurch` object 9, routine `$12C`, dialogue `$37`): `$FA $65 +2`, then
    `$F6 $00 $24`. With `EventFlag_ZioNurvus` (`$65`) clear he faints; after Zio his entry skips it.
  - The believer (`Kadary` object 4, `NPCType2`, dialogue `$20`): `$F6 $00 $31`. He is on the map only
    after Zio: `MapDataMan_Kadary` (`ps4.asm:109328`, the pack's `map_effects`) shows objects 4 and 7
    and hides object 9 once `$65` is set, and the reverse before.
- **Flags:** neither event writes one. What they change lasts until the next map load, which
  rebuilds both rooms from their records.

## Clone audit

Both bodies were read from the US image through their `EventPtrs` slots, and every instruction word
matches `ps4.asm:147047-147092` and `ps4.asm:148262-148298`. `Event_ZioFanatic` sits inside
`if grand_cross=0`, so the US build has it. Neither loads a dialogue tree.

`Event_FaintingPriest` builds its block with `move.w ($74).w, (a4)` (`38B8 0074` at `$06D670`): an
absolute read of the word at `$000074`, which is `$0000` in the US image. `FieldObj_InvisibleBlock`'s
init then sets bit 15, so the slot holds `$8000`, `FieldObj_None` with its init done. The block never
runs again, but its init set render bits 3 and 1, so it blocks and answers the talk probe.

## Retail transcription

### `Event_FaintingPriest`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06D616..$06D62B` | `lea ($FFFFC540).w, a4` (object 9) … `Event_UpdateObjFacing` | `FaceOppositeOf` object 9, of the leader |
| 1 | `$06D62C..$06D633` | entry `$45`, to its first stop | `RunDialogue` |
| 2-4 | `$06D634..$06D66B` | routine `$180` (`FieldObj_FaintedPriest`), `clr.w $6(a4)`; `.animateloop` until mapping 5 holds a duration of 1 | `SetObjectRoutine`, `Face` down, `WaitFrames(46)` |
| 5-8 | `$06D66C..$06D69D` | slot 10 (`$FFFFC580`): id from `($74).w`, (`$200`,`$140`), dialogue `$38`, `FieldObj_InvisibleBlock`; one sprite pass and `VInt_Prepare` | `Object(ObjectOp::Spawn)` slot 10, `SetNpcDialogue` 10, `RebuildSprites`, `WaitFrames(1)` |
| 9-11 | `$06D69E..$06D6C7` | `popdlg` / `Event_RunDialogue`; object 9's dialogue `$38`; `Game_Mode_Routine` 0, `Map_LoadChunks` | `RunDialogueResume`, `SetNpcDialogue` 9, `Presentation(ReloadMapChunks)` |

**The 46 frames.** `FieldObj_FaintedPriest`'s script (`loc_4A79E`, the pointer at `loc_4A5C8` for
facing 0) is six mappings of seven frames. `Event_UpdateObjFacing` left the priest on mapping 0, and
the routine's init sets the duration to 7. The call that leaves mapping 5 with a duration of 1 is
call 46. Only the priest runs in the loop, and no map update, so it is a `WaitFrames(46)`.

**Two cells.** The fainted priest lies across his own cell (`$1F0`,`$140`) and the next one; the
block at (`$200`,`$140`) is that second cell, and answers like him.

### `Event_ZioFanatic`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06EE40..$06EE55` | `lea ($FFFFC400).w, a4` (object 4) … `Event_UpdateObjFacing` | `FaceOppositeOf` object 4, of the leader |
| 1 | `$06EE56..$06EE5D` | entry `$49` | `RunDialogue` |
| 2 | `$06EE5E..$06EE87` | object 9 (`$FFFFC540`) := `$148` at object 4's `$30`/`$34`, each rounded down to 8 pixels; one `loc_49212` pass | `Object(ObjectOp::SpawnAt)` slot 9, at object 4 |
| 3 | `$06EE88..$06EE8F` | `loc_5B0C0` on object 4: 64 bytes cleared | `DespawnNpc` 4 |
| 4-5 | `$06EE90..$06EEB5` | `moveq #$1D, d7` and a `dbf` loop of `loc_49212` and the sprite calls, `VInt_Prepare` each | `Presentation(RebuildSprites)`, `WaitFrames(30)` |
| 6 | `$06EEB6..$06EEBD` | entry `$1F`, tail `jmp` | `RunDialogue` |

**`ObjectOp::SpawnAt`.** Object 4 wanders (`NPCType2`), so the spot the writhing believer takes is
only known when the scene runs. `SpawnAt` is `Spawn` with the position read from a cast member's live
cell; it has its own test. The cartridge rounds to 8 pixels and the port's objects stand on
16-pixel cells, so a believer talked to mid-step lands up to a cell back along his step on the
cartridge; standing still, he does not. The cases are in the
[source notes](../source-notes/README.md#other-recorded-deviations).

Object 9 is one of `Kadary`'s own record objects, hidden by `MapDataMan_Kadary`. The spawn places it
again at that index. The renderer creates sprites only on a rebuild, so the first pass of the loop's
`Field_LoadSprites`/`Field_BuildSprites` is a `RebuildSprites` before the 30 frames: from it on, object
9 is drawn with its own writhing sheet at the cell it took, through the wait and entry `$1F`, and
object 4 is not. The registry checks that every `ObjectOp` spawn is followed by its rebuild before a
wait, a line or the scene's end (`every_spawned_object_is_drawn_before_the_scene_goes_on`).

## Evidence

- State: `rust/psiv-core/tests/kadary_scenes.rs`: the faint's 46 frames, the block's slot, cell and
  solidity, both `$38` writes and the resume; the believer replaced on object 4's cell, object 4
  cleared, the sprite rebuild on the spawn's tick and the 30 writhing frames after it before `$1F`;
  and `SpawnAt` on its own, including the fault for an actor not in the cast.
- Pad-driven in the headless runtime on the real maps, from hand-built saves
  (`rust/psiv-campaign/tests/kadary_side.rs`): the priest faints and his block answers `$38`, with
  the after-Zio negative control writing no slot; the believer writhes where he stood, and keeps
  that cell, in front of the leader, after the scene ends.
- No campaign chapter: the route does not stop to talk to either. No oracle comparison, no visual
  claim and no persistence claim. Recorded deviations: the facing frames, the undrawn faint and the
  believer's cell for the 8-pixel rounding, in
  the [source notes](../source-notes/README.md#other-recorded-deviations).
