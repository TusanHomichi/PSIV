# The missing student: `Event_PiataDormOwner`, `Event_MissingStudentFound`, `Event_OldManNearMissingStudent`, `Event_MissingStudentInBed`

The Hunter's Guild job "Missing Student" (`HuntersGuildData`, `ps4.asm:151507`: taken
`EventFlag_MissingStudent` `$26`, earned `EventFlag_StudentRecovered` `$2A`). The dorm owner in
Piata says where she went, she collapses in Kadary's church, the old man at Kadary's inn takes a
Perolymate, and she goes home.

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0076` `Event_PiataDormOwner` | `$072898..$0728BD` inclusive, 38 bytes | `EventPtrs[$76]` at `$05A2B4 + $1D8` | `missing_student.rs`, `PIATA_DORM_OWNER` (3 ops) |
| `$0077` `Event_MissingStudentFound` | `$0728BE..$0729E3` inclusive, 294 bytes | `EventPtrs[$77]` at `$05A2B4 + $1DC` | `missing_student.rs`, `MISSING_STUDENT_FOUND` (29 ops) |
| `$0078` `Event_OldManNearMissingStudent` | `$0729E4..$072A25` inclusive, 66 bytes | `EventPtrs[$78]` at `$05A2B4 + $1E0` | `missing_student.rs`, `OLD_MAN_NEAR_MISSING_STUDENT` (6 ops) |
| `$0079` `Event_MissingStudentInBed` | `$072A26..$072ACF` inclusive, 170 bytes | `EventPtrs[$79]` at `$05A2B4 + $1E4` | `missing_student.rs`, `MISSING_STUDENT_IN_BED` (14 ops) |

- **Where they run.** Only `$76` is in Piata (`PiataDorm` `$018`, tree 2). The other three are in
  Kadary (`KadaryChurch` `$069` and `KadaryInn` `$071`, tree 9).
- **Triggers:** dialogue `$F6`. (`$FA`'s operand is a forward delta:
  [trigger census](TRIGGER_CENSUS.md).)
  - The owner (object 0, dialogue `$57`): `$FA $2A +5`, `$FA $27 +3`, `$FA $26 +2`, `$FA $44 +1`.
    With the job taken and nothing later, entry `$59` fires `$76`.
  - The student in the church (object 4, dialogue `$4A`): `$FA $27 +1`, `$FA $65 +1`. Entry `$4B`
    fires `$77`, which tests `$27` again itself, because `EventFlag_ZioNurvus` (`$65`) reaches it
    without the owner's word.
  - The old man (object 1, dialogue `$4E`): `$FA $29 +7`, then `$F6 $78`.
  - The bed (invisible blocks 3 and 4, dialogue `$51`): `$F6 $79`. The student's own object 2 is
    not talkable; it is the blocks that answer.
- **Flags** (`ps4.constants.asm:1517-1520`): `EventFlag_DormOwner` `$27`, `EventFlag_StudentSick`
  `$28`, `EventFlag_Perolymate` `$29`, `EventFlag_StudentRecovered` `$2A`. Readers outside these
  bodies: the guild (`$66`) reads `$2A` as the job's done flag, and the map-data routines hide or
  show her through the pack's `map_effects`: `MapDataMan_PiataDorm` (`ps4.asm:108253`, object 4
  until `$2A`), `MapDataMan_KadaryInn` (`ps4.asm:110492`, objects 1 to 4 only between `$28` and
  `$2A`), `MapDataMan_KadaryChurchStudent` (`ps4.asm:110659`, object 4 until `$28`).

## Clone audit

All four bodies were read from the US image through their `EventPtrs` slots, and every instruction
word matches `ps4.asm:152399-152535`. None has a `revision` or `grand_cross` conditional, and none
calls `DialogueTreesToRAM`: `$77`'s last line is entry `$53` of `KadaryInn`'s tree 9, and `$79`'s
is entry `$5B` of `PiataDorm`'s tree 2, each loaded by `RefreshMap`.

## Retail transcription

`Event_UpdateObjFacing` writes `facing_dir` directly: 0 down, 4 up, 8 right, `$C` left.
`DoMapUpdateLoop(n)` is `Wait(n + 1)`. `DoMapUpdateLoop` saves `a4` (`48E7 FFF8` at `$05A722`), so
`$77`'s run of turns all land on the student.

### `Event_PiataDormOwner`

Shares `Event_RockyOwner`'s shape ([120](120_RockyOwner.md)): `$072898..$0728AD` turns object 0 to
the leader, `$0728AE` speaks entry `$6C`, `$0728B6..$0728BD` sets `$27`.

### `Event_MissingStudentFound`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$0728BE..$0728D3` | `lea ($FFFFC400).w, a4` (object 4) … `Event_UpdateObjFacing` | `FaceOppositeOf` object 4, of the leader |
| 1 | `$0728D4..$0728DD` | `EventFlags_Test` `$27`, `bne.s loc_728E6` | `BranchFlag` |
| 2-3 | `$0728DE..$0728E5` | entry `$52`, tail `jmp` | `RunDialogue`, `End` |
| 4 | `$0728E6..$0728ED` | entry `$4C` | `RunDialogue` |
| 5-16 | `$0728EE..$072951` | facings 0, 8, 4, `$C`, 0, 8, each `DoMapUpdateLoop(9)` | `Face`, `Wait(10)` x 6 |
| 17-18 | `$072952..$072961` | facing 0, `DoMapUpdateLoop($1D)` | `Face`, `Wait(30)` |
| 19 | `$072962..$072969` | entry `$4D` | `RunDialogue` |
| 20 | `$07296A..$07296F` | `PalFadeOut_ClrSpriteTbl` | `FadeOut` |
| 21-22 | `$072970..$07297F` | `Sound_StopMusic`, `DoMapUpdateLoop($E)` | `PlaySound`, `Wait(15)` |
| 23 | `$072980..$072997` | `MusicID_Inn` unless saved | `PlayMusicIfSavedDifferent` |
| 24 | `$072998..$07299D` | `VInt_Prepare` | `WaitFrames(1)` |
| 25 | `$07299E..$0729A5` | `EventFlags_Set` `$28` | `SetFlag` |
| 26 | `$0729A6..$0729D5` | `KadaryInn`, from `Kadary`, start (`$48`,`$38`), facing 4, align 4, `bclr #3`, `RefreshMap` | `LoadMap` |
| 27 | `$0729D6..$0729DB` | `Pal_FadeIn` | `FadeIn` |
| 28 | `$0729DC..$0729E3` | entry `$53`, tail `jmp` | `RunDialogue` |

### `Event_OldManNearMissingStudent`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$0729E4..$0729F9` | `lea ($FFFFC340).w, a4` (object 1) … `Event_UpdateObjFacing` | `FaceOppositeOf` object 1, of the leader |
| 1 | `$0729FA..$072A0D` | `GetItem` `$93`, `bne.s loc_72A1E`, `clr.b (a0)`, `ReorderInventory` | `Item(ItemOp::Take)` |
| 2 | `$072A0E..$072A15` | entry `$4F` | `RunDialogue` |
| 3-4 | `$072A16..$072A1D` | `EventFlags_Set` `$29`, tail `jmp` | `SetFlag`, `End` |
| 5 | `$072A1E..$072A25` | entry `$53`, tail `jmp` | `RunDialogue` |

**`ItemOp::Take`.** `GetItem` (`$05B100`, `ps4.asm:121975`) counts the occupied slots and compares
only that many from the front, so a battle's hole can hide the item. `Inventory::get_item_slot` is
that scan, returning the slot `a0` holds. `ReorderInventory` (`$05CB72`, `ps4.asm:124264`) shifts
the tail left once at the first hole, which is `Inventory::remove_in_field`. The older
`SceneOp::RemoveItem` writes a hole and asks nothing, so it cannot carry this branch.

### `Event_MissingStudentInBed`

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072A26..$072A3B` | `lea ($FFFFC380).w, a4` (object 2) … `Event_UpdateObjFacing` | `FaceOppositeOf` object 2, of the leader |
| 1 | `$072A3C..$072A45` | `EventFlags_Test` `$29`, `bne.s loc_72A4E` | `BranchFlag` |
| 2-3 | `$072A46..$072A4D` | entry `$50`, tail `jmp` | `RunDialogue`, `End` |
| 4 | `$072A4E..$072A55` | entry `$54` | `RunDialogue` |
| 5 | `$072A56..$072A5B` | `PalFadeOut_ClrSpriteTbl` | `FadeOut` |
| 6-7 | `$072A5C..$072A6B` | `Sound_StopMusic`, `DoMapUpdateLoop($E)` | `PlaySound`, `Wait(15)` |
| 8 | `$072A6C..$072A83` | `MusicID_AHappySettlement` unless saved | `PlayMusicIfSavedDifferent` |
| 9 | `$072A84..$072A89` | `Pal_FadeIn` | `FadeIn` |
| 10 | `$072A8A..$072A8F` | `VInt_Prepare` | `WaitFrames(1)` |
| 11 | `$072A90..$072A97` | `EventFlags_Set` `$2A` | `SetFlag` |
| 12 | `$072A98..$072AC7` | `PiataDorm`, from `Piata`, start (`$5A`,`$36`), facing 4, align 4, `bclr #3`, `RefreshMap` | `LoadMap` |
| 13 | `$072AC8..$072ACF` | entry `$5B`, tail `jmp` | `RunDialogue` |

**The two fades differ.** `$77` fades in after its `RefreshMap`. `$79` fades in before its own, and
no fade follows the load. The port keeps that order; neither capture has been compared with the
oracle.

**The music after `$79`.** `RefreshMap` never writes the music word (`rust/psiv-runtime/src/map_change.rs`),
so A Happy Settlement keeps playing in the dorm until the next ordinary map load.

## Evidence

- State: `rust/psiv-core/tests/missing_student_scenes.rs`: both arms of each scene, the line order,
  the seven turns, the bag closing up, a Perolymate behind a hole going unfound, and the two fades'
  order.
- Pad-driven in the headless runtime on the real maps, from hand-built saves
  (`rust/psiv-campaign/tests/student_side.rs`), since the route does not take the guild's jobs: the
  whole job from the church to the dorm, and the negative controls for the owner's word, the job
  and the empty bag.
- No oracle comparison, no visual claim and no persistence claim.
