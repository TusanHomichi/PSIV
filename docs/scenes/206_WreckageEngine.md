# `Event_WreckageEngine`

- **Retail bytes:** `$06DBE8..$06DE7B` inclusive; the wait helper `loc_6DE7C` follows.
- **Pointer:** `EventPtrs[$29]` at `$05A2B4 + $A4`; event `$0029`.
- **Trigger:** interaction areas of `Wreckage_F2_Part4` (`$0B6`), row 33 at x 28 to 33 (`Interaction_EventIndexes[1]`), and dialogue `$F6`, tree 8 entry 70.
- **Data:** `wreckage.rs`, `WRECKAGE_ENGINE` (67 ops).
- **Flags:** reads and sets `EventFlag_WreckageSystem` (`$40`); once it is set the body speaks entry `$48` only.

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every word matches
`ps4.asm:147172-147370` with `revision` non-zero. The `if revision=0` block after the second
`popdlg` (a second portrait patch, into `$34(a0)`) is the Japanese build's and is absent from the US
bytes. The body loads no tree. In the US image `Char_Move_Flags` is `$ECFE` and
`FieldObj_Step_Offset` `$ECE0`.

## Retail transcription

`DoMapUpdateLoop(n)` is `Wait(n + 1)`; a bare `VInt_Prepare` is `WaitFrames(1)`.

| Op | Retail primitive | Scene record |
|---:|---|---|
| 0-2 | `EventFlag_WreckageSystem` set: `moveq #$48`, `jmp Event_GetAndRunDialogue` | `BranchFlag`, `RunDialogue` `$48`, `End` |
| 3-5 | entry `$47` to its first `$F7`; `FieldObj_Step_Offset := 0`; `bset #2, Char_Move_Flags` | `RunDialogue`, `SetStepOffset(0)`, `SetFollowMode(%100)` |
| 6 | `FindCharacterSlot(5)`, `tst.b d1`, `beq loc_6DCB2` | `Wreckage(BranchIfLeads)` Rika |
| 7-17 | `bset #0`; `loc_6DE7C` three times: the leader `$10` left and Rika `$10` right, then each takes the other's row, then back `$10`; `bclr #0` | `SetFollowMode`, `MoveActorOffset`, `MoveActorToActorAxis`, `WaitForActor` the leader |
| 18 | Rika (`a2`) faces up | `Face` |
| 19 | `popdlg`, then `loc_6DCBE`: the first `Current_Party_Slots` byte that is neither 5 nor 0, plus one, to `$1(a0)` | `Wreckage(ResumePortraitFromParty)` |
| 20-22 | `Event_RunDialogue`; `SpcSFXID_SpaceshipRadar`; `Event_MoveCamera` to (`Character_1`'s `$30`, `$1F0`), speed 1 | `RunDialogueResume`, `PlaySound`, `PanCamera` `ActorX` |
| 23-38 | `loc_6DD08`, eight passes: chunk `$A6` four times, `RefreshPlaneFG`, `DMAPlane_A_VInt`, three `VInt_Prepare` | `WriteMapChunks`, `WaitFrames(4)` |
| 39-46 | `addi.w #$220, Palette_Table_Buffer+$1A`, eight `VInt_Prepare`, four times | `AddPaletteWord`, `WaitFrames(8)` |
| 47-49 | `EventFlag_WreckageSystem`; `$77 + 1` frames of `RunMapUpdates`; `popdlg` / `Event_RunDialogue` | `SetFlag`, `Wait(120)`, `RunDialogueResume` |
| 50-63 | the same test; the swap back; the leader faces up; `Event_MoveCamera` to the leader, speed 1 | as 6-17, `Face`, `PanCamera` |
| 64-66 | `loc_6DE58`: `FieldObj_Step_Offset := 1`, `$ECD0 := 0`, `Palette_Table_Buffer+$7C` := `$620`, `$2CE`, `Sound_StopSpcSFX` | `SetStepOffset(1)`, `SetPaletteWords` (word `$3E`), `PlaySound` |

**The floor.** `GetMapLayoutOffset` adds `x + y * (Map_Row_Size_FG + 1)`; this map is 32 chunks wide,
so `$20(a1)` is the next row. Pass `k` writes columns `15 + k` and `14 - k` (`d7` runs `-1, -3 ... -15`)
on rows 12 and 13. `psiv_tools.map_patches.SCENE_CHUNK_WRITES` and its copy in the Rust builder
(`rust/psiv-pack/src/maps/patches.rs`) carry the row that puts chunk `$A6` in the map's atlas, pinned
to the four `move.b` sites (`$06DD1A`, `$06DD1E`, `$06DD24`, `$06DD2A`);
`tests/test_scene_chunk_atlas.py` holds the two copies equal.

**The portrait.** After the first `$F7` the saved text resumes on `$F4 $01` (Chaz). The body
overwrites that operand with the next speaker, so the line after the pause is spoken by whoever
follows Chaz and Rika in the party; with no one else, the empty slot's `$FF` writes `0` and no portrait
shows. `SceneEffect::ResumePortrait` carries the id, and the runtime patches its saved text
(`TextFlow::patch_resume_portrait`); a cursor that is not on a `$F4` faults the dialogue.

**Rika's slot.** `tst.b d1 / beq` skips the swap when Rika leads. With Rika absent `d1` is `-1`, which
is not zero, and the cartridge would move the object `$40` before `Character_1`; the port skips the
swap then, at both tests. Rika is in the party whenever the Wreckage is reachable. This, the
swap's pace, the two facing frames and the logged palette words are recorded in
[the source notes](../source-notes/README.md#other-recorded-deviations).

## Verification

`rust/psiv-core/tests/wreckage_scenes.rs`: the lines, the portrait, the thirty-two writes in order,
the swap and swap back, Rika leading, and the second visit. `rust/psiv-runtime/src/dialogue/tests.rs`:
the patched portrait on the resumed page, and a cursor without one. `rust/psiv-campaign/tests/wreckage.rs`
reads the system with pads on the real map from a hand-built save.

**Limits.** The palette records (`AddPaletteWord`, `SetPaletteWords`) are logged by the renderer, not
drawn. The swap's frame counts follow the runner's walking pace, not a measurement. No capture compared.
