# The dying boy: `Event_DyingBoy`

The Hunter's Guild job "Dying Boy" (`HuntersGuildData`, `ps4.asm:151519`: taken
`EventFlag_DyingBoy` `$A8`, done `EventFlag_CulversAfterRecovery` `$AB`), open once
`EventFlag_Hydrofoil2` is set. Culvers's son lies in bed in `CulversHouse` (`$0E7`, tree 24). With
the father's word and an Alis Sword, the party gathers at the bed, Chaz raises the sword, and the
boy is cured. The father's two scenes, `$84` and `$85`, are #331's.

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0086` `Event_DyingBoy` | `$072D94..$072FB3` inclusive, 544 bytes | `EventPtrs[$86]` at `$05A2B4 + $218` | `dying_boy.rs`, `DYING_BOY` (53 ops) |

- **Trigger:** dialogue `$F6`. The boy's object 2 is not talkable; the invisible blocks 3 and 4 at
  the bed carry dialogue `$32`, which is `$F6 $00 $86`.
- **Flags:** reads `EventFlag_Culvers` (`$A9`, set by `$84`) and `EventFlag_AlisSword` (`$AA`); sets
  `$AA` as its last write. `$AA` is read by Culvers's own entry (to reach `$85`) and by
  `MapDataMan_CulversHouse` (`ps4.asm:110615`, the pack's `map_effects`): once set, the mother moves
  and becomes `NPCType1`, the bed's objects go, and the boy runs about the room.

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every instruction word
matches `ps4.asm:152726-152855`. It has no `revision` or `grand_cross` conditional and loads no
dialogue tree. The sword is `FieldObjectsJmpTbl` `$334`, `FieldObj_ChazAlisSword`
(`ps4.asm:99849`): its init places it at (`$1D0`,`$1B8`) with tile `$43C` and the animation script
`loc_4C39C`, and it writes `$FFFF` to `$1C` on the frame its animation leaves mapping 9 with a
duration of 1.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072D94..$072DA9` | `lea ($FFFFC380).w, a4` (object 2) … `Event_UpdateObjFacing` | `FaceOppositeOf` object 2, of the leader |
| 1-3 | `$072DAA..$072DDB` | `EventFlags_Test` `$AA` (`bne` to `loc_72FAC`), `$A9` (`beq` to `loc_72FA4`), `GetItem` `$8A` (`bne` to `loc_72FA4`), `clr.b (a0)`, `ReorderInventory` | `BranchFlag` x 2, `Item(ItemOp::Take)` |
| 4 | `$072DDC..$072DE3` | entry `$33` | `RunDialogue` |
| 5 | `$072DE4..$072DE9` | `bset #2, (Char_Move_Flags).w` | `SetFollowMode` `0b100` |
| 6-9 | `$072DEA..$072E39` | `Event_GetCharacter` + the `dest` words for Chaz (`$1F0`,`$1C0`), Rune (`$1E0`,`$1A0`), Rika (`$1F0`,`$1A0`), Wren (`$200`,`$1A0`); no membership test | `MoveActorTo` x 4, not waiting |
| 10-15 | `$072E3A..$072E7F` | `Event_GetCharacter` for Hahn, Gryz, Demi, Raja, Kyra, Seth, each `bmi` to the next or `bra loc_72E80` | `BranchIfPartyMember` x 6 |
| 16-27 | `loc_72E80`, `$072E80..$072E8B` | (`$210`,`$1A0`) for the member found | `MoveActorTo` + `Jump` x 6 |
| 28-30 | `loc_72E8C`, `$072E8C..$072E9F` | `clr.b (FieldObj_Step_Offset).w`, `bset #0, (Char_Move_Flags).w`, `DoMainUpdatesLoop($95)` | `SetStepOffset` 0, `SetFollowMode` `0b101`, `Wait(150)` |
| 31-33 | `$072EA0..$072EC5` | `loc_5A97C` with d0 = 0, Chaz `Event_UpdateObjFacing` `$C`, `popdlg` / `Event_RunDialogue` | `FaceParty` down, `Face` Chaz left, `RunDialogueResume` |
| 34-36 | `$072EC6..$072F17` | Chaz `bset #1, $2(a4)`; slot 6 (`$FFFFC480`) := `$334`; one pass, then `loc_72EF4` until `$1C` is `$FFFF`; each pass is the routine, `Field_LoadSprites`, `Field_BuildSprites`, `VInt_Prepare` | `SetCharacterVisible` Chaz off, `ObjectAnimation` slot 6, `WaitFrames(164)` |
| 37-40 | `$072F18..$072F43` | Chaz `bclr #1, $2(a4)`; `clr.w` slot 6, `RunSingleObject`, the two sprite calls, `VInt_Prepare` | `SetCharacterVisible` Chaz on, `DespawnNpc` 6, `RebuildSprites`, `WaitFrames(1)` |
| 41-42 | `$072F44..$072F67` | `popdlg` / `Event_RunDialogue`; `Event_MoveSingleObject` on Chaz to (`$1A0`,`$1C0`) | `RunDialogueResume`, `MoveActorTo` Chaz, waits |
| 43-45 | `$072F68..$072F8D` | `bclr #0, (Char_Move_Flags).w`; `popdlg` / `Event_RunDialogue`; `Event_MoveObjectStatic` on `Character_1` to (`$1A0`,`$1D0`) | `SetFollowMode` `0b100`, `RunDialogueResume`, `MoveActorTo` the leader, waits |
| 46-49 | `$072F8E..$072FA3` | step offset 1, `bclr #2, (Char_Move_Flags).w`, `EventFlags_Set` `$AA` (tail `jmp`) | `SetStepOffset` 1, `SetFollowMode` 0, `SetFlag`, `End` |
| 50-51 | `loc_72FA4` | entry `$3B`, tail `jmp` | `RunDialogue`, `End` |
| 52 | `loc_72FAC` | entry `$3E`, tail `jmp` | `RunDialogue` |

The fifth-member chain and the four untested destinations are `Event_DominatorsDefeated`'s shape
([110](110_VahalFortStory.md)), and reuse its `go` helper.

**The sword's 164 frames.** The script at `loc_4C6F6` (the pointer at `loc_4C39C` for facing 0) has
its count byte's bit 7 set, so `FieldObj_Animate2` takes its per-mapping branch (`loc_4477E`): ten
mappings lasting 4, 4, 44, 4, 44, 4, 4, 44, 4 and 4 frames. From a cleared slot the first call
moves to mapping 1, and the call that leaves mapping 9 with a duration of 1 is call 164, the
routine's first pass included. Only that object runs in the loop, and no map update, so it is a
164-frame `WaitFrames` beside the animation record.

**Limits.**

- Chaz, Rune, Rika and Wren get their `dest` words without a membership test. On the cartridge an
  absent one's `Event_GetCharacter` leaves `a4` on the previous object, which then gets the absent
  member's spot. The port's `MoveActorTo` on an absent character faults the scene instead, as for
  `$91`. The cartridge's own data assumes the four: they are in the party for the whole time the
  job is open.
- Recorded deviations, in the [source notes](../source-notes/README.md#other-recorded-deviations):
  the facing frames (#164), the slow step (#337), and the sword not drawn.

## Evidence

- State: `rust/psiv-core/tests/dying_boy_scene.rs`: both short arms, a missing sword writing
  nothing, the cure (the sword taken, the bag closing up, entry `$33` and its three resumes, Chaz
  hidden for exactly 164 frames, the leader's last walk, the flag), the gathering spots, and the
  fifth member found in order or not at all.
- Pad-driven in the headless runtime on the real map, from a hand-built save
  (`rust/psiv-campaign/tests/dying_boy.rs`): the cure from the bed's block with a five-member
  party, and the negative controls without the father's word and without the sword.
- No campaign chapter: the route does not take the guild's jobs. No oracle comparison, no visual
  claim and no persistence claim.
