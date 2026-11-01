# The strip club: `Event_StrippersAppearing` and `Event_ExitingStripClub`

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0094` `Event_StrippersAppearing` | `$073440..$0735ED` inclusive, 430 bytes | `EventPtrs[$94]` at `$05A2B4 + $250` | `strip_club.rs`, `STRIPPERS_APPEARING` (33 ops) |
| `$0095` `Event_ExitingStripClub` | `$0735EE..$0735F5` inclusive, 8 bytes | `EventPtrs[$95]` at `$05A2B4 + $254` | `strip_club.rs`, `EXITING_STRIP_CLUB` (1 op) |

- **Triggers:** map trigger `$79` on `StripClub` (`$05A`), `RunEvent_StrippersDance`
  (`ps4.asm:116549-116562`): `TempEveFlag_StrippersDance` (`$1B`) clear, the leader at
  `y <= $240` and `$1E0 <= x <= $270`, which is the row in front of the stage. Map trigger `$7A`
  on `HuntersGuild` (`$057`), `RunEvent_ClrStrippersDance` (`ps4.asm:116564-116570`): the same
  flag set. Both gates are `trigger_table.rs` rows `$79` and `$7A`.
- **Flags:** `$94` sets `TempEveFlag_StrippersDance` as its last write; `$95` clears it, so the
  dancers come out on every visit. No other lane's event reads it.

## Clone audit

Both bodies were read from the US image through their `EventPtrs` slots, and every instruction
word matches `ps4.asm:153164-153253` and `ps4.asm:153254-153256`. Neither has a `revision` or
`grand_cross` conditional, and neither loads a dialogue tree. The four field routines the body
writes were read at their `FieldObjectsJmpTbl` entries (`ps4.asm:98312` onward):
`FieldObj_Stripper` (`$344`, `ps4.asm:99872`), `FieldObj_StripperCoat` (`$348`, `:99896`),
`FieldObj_StripClubCustomer` (`$34C`, `:99915`) and `FieldObj_DancingStripper1..3` (`$35C`,
`$360`, `$364`, `:99951` onward). The customer's two `dialogue_id` writes sit under
`if grand_cross=0`, so both are in the US build.

## Retail transcription

### `Event_StrippersAppearing`

Object addresses are `$FFFFC300 + slot * $40`. `StripClub`'s record places one object, the
customer (slot 0, `NPCType9`, dialogue `$3B`, at (34,36)); every other slot the body writes is
empty until it does. `DoMapUpdateLoop(n)` and `DoMainUpdatesLoop(n)` are `Wait(n + 1)`, and a bare
`VInt_Prepare` is `WaitFrames(1)`.

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0-1 | `$073440..$07344F` | `Sound_StopMusic` (`$FB`), `DoMapUpdateLoop(9)` | `PlaySound`, `Wait(10)` |
| 2 | `$073450..$073467` | `cmpi.b #MusicID_PaoPao ($B0), Saved_Sound_Index`; when different, both bytes take it | `PlayMusicIfSavedDifferent` |
| 3 | `$073468..$073477` | `Event_MoveCamera` to (`$220`, `$1C0`), speed 1 | `PanCamera` |
| 4-6 | `$073478..$0734AF` | objects 1 to 3 (`$C340`, `$C380`, `$C3C0`): routine `$344` at (`$160,$190`), (`$150,$190`), (`$170,$190`) | `CreateFieldObject` x 3, tile `$3BA` (the routine's `$16` word, the map's upload) |
| 7-8 | `$0734B0..$0734CD` | `Field_UpdateObjects`, `Field_LoadSprites`, `Field_BuildSprites`, `RunMapUpdates`, `VInt_Prepare` | `RebuildSprites`, `WaitFrames(1)` |
| 9-10 | `$0734CE..$0734ED` | `dest` (`$38`/`$3A`) of object 2 := (`$200,$1F0`), object 3 := (`$240,$1F0`) | `WalkObject` x 2 |
| 11-12 | `$0734EE..$0734FF` | `Event_MoveObjectStatic` on object 1 to (`$220,$210`) | `WalkObject`, `Wait(160)` |
| 13 | `$073500..$073507` | `DoMapUpdateLoop($3B)` | `Wait(60)` |
| - | `$073508..$073519` | `facing_dir := $10` on objects 1 to 3 | (renderer state, not typed) |
| 14-19 | `$07351A..$07354D` | objects 5 to 7 (`$C440`, `$C480`, `$C4C0`): routine `$348` at (`$210,$210`), (`$1F0,$1F0`), (`$230,$1F0`) | `CreateFieldObject`, `DriftObject` x 3 |
| 20-21 | `$07354E..$07356B` | the five-call rebuild | `RebuildSprites`, `WaitFrames(1)` |
| 22 | `$07356C..$073573` | `DoMainUpdatesLoop($59)` | `Wait(90)` |
| 23 | `$073574..$073583` | `clr.w` the routine words of `$C400`, `$C440`, `$C480` | `DespawnNpc` 4, three objects |
| 24 | `$073584..$073599` | `NemDecomp` of `$14A1CC` to tile `$372` | `LoadArt` |
| 25 | `$07359A..$0735AD` | `Event_MoveCamera` to `Character_1`'s `$30/$34`, speed 1 | `PanCamera` the leader |
| 26 | `$0735AE..$0735B5` | `Field_Obj_Secondary` routine := `$34C` | `SetObjectRoutine` object 0 |
| 27-29 | `$0735B6..$0735C7` | objects 1 to 3 routines := `$35C`, `$360`, `$364` | `CreateFieldObject` x 3 at the spots they walked to |
| 30-31 | `$0735C8..$0735E5` | the five-call rebuild | `RebuildSprites`, `WaitFrames(1)` |
| 32 | `$0735E6..$0735ED` | `moveq #TempEveFlag_StrippersDance, d0` / `jmp TempEveFlags_Set` | `SetFlag` temp `$1B` |

**The pans.** `Event_MoveCamera` (`ps4.asm:121468`) returns on arrival: X in its own loop, then
Y, a pixel a frame at speed 1, with map updates and the objects running and the pad cleared. A
leg already on its target takes no frames, so the strippers appear after exactly the outbound
distance from wherever the leader stood, and the dancers and the flag after the return's.
`SceneOp::PanCamera` is that routine, and a pan with nothing to do on either axis takes no frame
here either. Column `$220` has an empty X leg on both pans.

**The walk.** `FieldObj_Stripper`'s init copies its position into `dest`, so the creation frame
does not move it. `Event_MoveObjectStatic` then runs every object a frame at a time
(`Field_UpdateObjects`, `RunMapUpdates`, `VInt_Prepare`) until object 1 stands on its
destination. The stripper routine moves through `FieldObj_GetAutoInput` (X first, since
`Char_Move_Flags` bit 1 is clear) and `FieldObj_Move` at the normal step, `$0200`, which is eight
frames a cell. Object 1 has twelve cells of X and eight of Y to cover, so 160 frames. Objects 2
and 3 have seventeen and nineteen cells, so they stop first.

**The coats.** `FieldObj_StripperCoat` writes `$FFFE` and `$FFFF` into the high words of its step
constants every frame, so it flies two pixels left and one up a frame from where its stripper
stood. The clear at `$073574` covers objects 4 to 6, not 5 to 7: object 4 was never written,
and the third coat (object 7) keeps flying after the scene. The port keeps that.

**The customer.** Object 0 takes `FieldObj_StripClubCustomer`, with the art just decompressed
to `$372`. Its init clears `facing_dir` and its timer and sets render bit 3, so he stays solid
and talkable. From then on, every frame:

- when `timer & $FF` is zero he turns: a facing other than 0 becomes 0 (down) with
  `dialogue_id` `$38`, and 0 becomes 4 (up) with `$39`;
- the timer goes up by one.

His first frame turns him up with `$39`, and he turns again every 256 frames. The routine never
calls `FieldObj_OnScreenTest`, so he turns wherever the camera is. A map load rebuilds him from
the record, with routine `NPCType9` and dialogue `$3B` again. The port gives him the new routine
word as his object id and runs the routine as `BespokeKind::FacingToggle`
(`psiv_core::scene_routine_kind(0x34C)`). The line his routine last wrote comes before the
record's in `Runtime::npc_dialogue_id`.

`SceneOp::SetObjectRoutine` is new. It is `move.w #routine, (a4)` with the scene carrying on:
the runtime sets the object's id, takes it out of the random walkers and installs the routine.
Unlike `RunObjectRoutine` ([122](122_AiedoManEvents.md)), nothing waits on the object.
`PresentationOp::WalkObject` (the auto-input walk, X then Y at two pixels a frame) and
`PresentationOp::DriftObject` (constant step words from now on) are new presentation records
for scene-created objects. The scene's own `Wait` carries a walk's frames.

### `Event_ExitingStripClub`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$0735EE..$0735F5` | `moveq #TempEveFlag_StrippersDance, d0` / `jmp TempEveFlags_Clear` | `SetFlag` temp `$1B` clear |

## Evidence and limits

- State: `rust/psiv-core/tests/strip_club_scenes.rs` checks the objects and routines the scene
  writes, the 160 + 60 frames from the walk to the coats (the wait read back from the ops'
  own coordinates), the clear of objects 4 to 6 and the flag. A second case clears the flag.
  `bespoke_tests.rs` checks the customer's turn and line at frames 0, 255, 256, 511 and 512.
- Runtime: `rust/psiv-runtime/src/scene_strip_club_tests.rs` plays $94 from every column of the
  trigger row and checks each pan's boundaries: X before Y, the frame count, nothing created
  until the stage is framed, no dancer or flag until the view is home, the scene (so the pad)
  still running through the return, and empty legs on either pan.
- Pad-driven: `rust/psiv-campaign/tests/aiedo_side.rs` walks a party up to the stage (cell row 38
  does not fire, the step onto 37, `y = $240`, does), checks the customer's routine, facing and line after the scene and
  256 frames later, then walks out to the guild, where `$95` clears the flag.
- Not yet drawn: the pack has no sheets for the stripper, coat and dancer routines or for the
  customer's new art, and the renderer drops a scene's created objects when the scene ends,
  where the cartridge keeps them until the map changes. Until
  [#241](https://github.com/TusanHomichi/PSIV/issues/241) lands, the dancers are not on screen.
  This is a state transcription; no visual parity is claimed.
- The 160-frame walk is derived from the routine, not measured on the oracle.
