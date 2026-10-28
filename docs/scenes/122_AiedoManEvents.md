# Two Aiedo men: `Event_AiedoManCrushOnAlys` and `Event_AiedoManSavings`

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$000E` `Event_AiedoManCrushOnAlys` | `$06BE78..$06BEE7` inclusive, 112 bytes | `EventPtrs[$0E]` at `$05A2B4 + $38` | `aiedo_events.rs`, `AIEDO_MAN_CRUSH_ON_ALYS` (6 ops) |
| `$0022` `Event_AiedoManSavings` | `$06D468..$06D4A5` inclusive, 62 bytes | `EventPtrs[$22]` at `$05A2B4 + $88` | `aiedo_events.rs`, `AIEDO_MAN_SAVINGS` (4 ops) |

- **Triggers:** dialogue `$F6` in `DialogueTree11`. (`$FA`'s operand is a forward delta:
  [trigger census](TRIGGER_CENSUS.md).)
  - `$0E`: the man on Aiedo's street (`Aiedo` `$054`, object 5, `NPCType2`, dialogue 7).
    Entry 7 is `$FA $42 $01` then `$F6 $00 $0E`, so it fires while `EventFlag_Zio` (`$42`) is
    clear, which is while Alys travels with the party. After Zio he reads entry 8.
  - `$22`: the man of `AiedoHouse3` (`$060`, object 0, `loc_48F36`, dialogue `$18`). Entry
    `$18` is only `$F6 $00 $22`, so the scene runs on every talk.
- **Flags:** neither body reads or writes one.

## Clone audit

Both bodies were read from the US image through their `EventPtrs` slots. Every instruction
word matches `ps4.asm:145309-145337` (the `grand_cross=0` side, which is the US build) and
`ps4.asm:146935-146948`. Neither has a `revision` conditional or a `DialogueTreesToRAM` call.

## Retail transcription

### `Event_AiedoManCrushOnAlys`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06BE78..$06BE8D` | `lea ($FFFFC440).w, a4` (object 5) / `lea (Character_1).w, a3` / `move.w $6(a3), d0` / `bchg #2, d0` / `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 5, of the leader |
| 1 | `$06BE8E..$06BE95` | `moveq #$3C, d0` / `jsr Event_GetAndRunDialogue` | entry `$3C` to its `$F7` |
| 2 | `$06BE96..$06BEBD` | `move.w #$BC, (a4)`, then `loc_6BE9E`: `jsr loc_489D6` / `Field_LoadSprites` / `Field_BuildSprites` / `VInt_Prepare`, `tst.w (a4)` / `bne` | `RunObjectRoutine` object 5, routine `$BC` |
| 3 | `$06BEBE..$06BEC7` | `lea ($FFFFC440).w, a0` / `move.w #$F, d7` / `trap #0` | `DespawnNpc` object 5, one object |
| 4 | `$06BEC8..$06BED5` | `popdlg` / `jsr Event_RunDialogue` (`$5AC6C`) | `RunDialogueResume` |
| 5 | `$06BED6..$06BEE5` | `move.w #0, (Game_Mode_Routine).w` / `movea.l (Map_Chunk_Addr).w, a0` / `jsr Map_LoadChunks` | `Presentation(ReloadMapChunks)` |
| - | `$06BEE6` | `rts` | end of ops |

The man flatters Alys and the window closes at entry `$3C`'s `$F7`. His routine becomes
`FieldObjectsJmpTbl` `$BC`, which is `loc_489D6` (`ps4.asm:95479`). Its init sets the boundary
bytes `$3C..$3F` to `$20 $20 $10 $10` and clears `render_flags` bit 3. Each frame after that:

1. `FieldObj_OnScreenTest` runs. If the object is off screen it clears its own slot
   (`clr.w (a4)`).
2. Otherwise `FieldObj_NPCMove` runs with `d0 = 1` (up) and `d7 = 2` (the third speed table,
   2 pixels a frame), then the duration, position and sprite updates.

Only that object runs during the loop, and the map is not updated. The loop ends when his slot
reads zero. Then `trap #0` clears the 64-byte object, and the party's reaction resumes the same
entry after its `$F7`.

The US build is not assembled with `bugfixes`, so the move is the collision-checking
`FieldObj_NPCMove`: a wall above him would stop him, and the scene would wait forever. The fork's
`FieldObj_NPCMove2` exists only to remove that softlock. Nothing stands in his column on Aiedo's
street, and the port keeps the cartridge's behaviour.

`SceneOp::RunObjectRoutine` is new:

- The runner emits `ObjectRoutineStarted` and blocks until the map shows the object inactive.
- The runtime takes the object out of the random walkers and installs the routine as a
  `BespokeKind::Departing` actor (`psiv_core::scene_routine_kind`).
- Each scene frame, the runtime tests that one object against the camera with the routine's own
  test and either clears the slot or runs its move. `Departing` is `Fixed`'s command without the
  party and object gate, because the cleared bit 3 turns `loc_4A316` off. The
  [wander record](../field/NPC_WANDER.md#bespoke-completion-wave-8) lists it with the other
  bespoke classes.

### `Event_AiedoManSavings`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06D468..$06D46F` | `moveq #$3D, d0` / `jsr Event_GetAndRunDialogue` | entry `$3D` to its `$F7` |
| 1 | `$06D470..$06D485` | `lea (Field_Obj_Secondary).w, a4` (object 0) … `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 0, of the leader |
| 2 | `$06D486..$06D493` | `popdlg` / `jsr Event_RunDialogue` | `RunDialogueResume` |
| 3 | `$06D494..$06D4A3` | `Game_Mode_Routine = 0`, `Map_LoadChunks` | `Presentation(ReloadMapChunks)` |
| - | `$06D4A4` | `rts` | end of ops |

The man is looking for his secret savings with his back to the door. At entry `$3D`'s `$F7` he
turns to the party, and the text resumes.

## Tests

- `rust/psiv-core/tests/aiedo_scenes.rs`:
  - `the_admirer_speaks_runs_off_and_the_party_answers` and
    `the_man_turns_between_the_two_halves_of_his_line` check the op order.
  - `the_scene_waits_until_the_object_has_cleared_its_slot` holds the wait for 100 ticks while
    object 5 is on the map, then releases it the tick after its slot clears.
- `rust/psiv-core/src/bespoke_tests.rs`:
  - `the_departing_routine_is_loc_489d6s_walk_up` checks the `$BC` table entry, and that an
    unknown routine has none.
  - `a_departing_object_walks_through_the_party_but_not_a_wall` has two controls: a `Fixed`
    walker stops for the party, and a wall stops the departing man.
  - `install_replaces_and_keeps_object_order`.
- `rust/psiv-campaign/tests/aiedo_side.rs`, pad-driven:
  - `the_admirer_runs_off_the_screen`: before Zio, the talk ends with object 5's slot clear and
    nothing written, in about 900 frames. The talk has a 3,000-frame budget. With the walk
    command zeroed, the talk spends that budget and halts, because the man never leaves. The
    control after Zio (`$42` set) reads entry 8, and he stays.
  - `the_man_looking_for_his_savings_turns_to_the_party`: he ends facing the leader, and nothing
    is written.
