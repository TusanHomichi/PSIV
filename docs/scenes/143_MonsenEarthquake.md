# `Event_MonsenEarthquake`

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$002C` `Event_MonsenEarthquake` | `$06E0EA..$06E759` inclusive, 1648 bytes, its helpers and step tables from `$06E634` | `EventPtrs[$2C]` at `$05A2B4 + $B0` | `monsen_quake.rs`, `MONSEN_EARTHQUAKE` (117 ops) |

- **Trigger:** dialogue `$F6`, `DialogueTree12`, from Monsen `$073` object 6 (`$C480`, dialogue
  `$0A`). Entry `$0A` is `$FA $65 +2`, `$FA $61 +2`, `$FA $60 +1`, `$F6 $00 $2C`.
- **Flags:** `EventFlag_PlateSystem` (`$60`) is the only flag written, and the last write.
  `MapDataMan_PlateSysBarrier` (`ps4.asm:109980`) and the chain above read it. It is no row of the
  guild's table.

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every word matches
`ps4.asm:147499-147854`, including both step tables. It has no `revision` or `grand_cross`
conditional and loads no tree. `Event_UpdateObjFacing` (`$05A936`) has the `VInt_Prepare` the listing
puts under `grand_cross=0`: its US bytes end `...47FE`, `...204C`, `rts` (`$05A96C..$05A979`).

## Retail transcription

| Ops | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0-1 | `$06E0EA..$06E107` | `$C480` turns from the leader, `moveq #$40` / `Event_GetAndRunDialogue` | `FaceOppositeOf` object 6, entry `$40`, standard window |
| 2 | `$06E108..$06E10D` | `bset #2, Char_Move_Flags` | `SetFollowMode` `100b` |
| 3-4 | `$06E10E..$06E19F` | `loc_6E11E`, 120 frames on `loc_6E6B6` | `Quake(120, false)`, `Wait(120)` |
| 5 | `$06E1A0..$06E1AB` | `$C480`, facing 0 | `Face` object 6 down |
| 6-11 | `$06E1AC..$06E211` | `Event_GetCharacter` then `dest` of Chaz, Gryz, Rika, Demi; `bmi.s` over Rune's | `SetActorDest` x 4, `BranchIfPartyMember`, `SetActorDest` |
| 12 | `$06E212..$06E21D` | `bset #0`, `bset #1` | `SetFollowMode` `111b` |
| 13-14 | `$06E21E..$06E221` | `jsr loc_6E634`: 60 frames, opening on `loc_6E73A` | `Quake(60, true)`, `Wait(60)` |
| 15 | `$06E222..$06E26D` | step counters cleared, one frame of the object and camera updates | `Wait(1)` |
| 16-21 | `$06E26E..$06E2BF` | Chaz faces 8, Gryz 0, Rika `$C`, Demi 4; `bmi.s` over Rune's `$C` | `Face` x 4, `BranchIfPartyMember`, `Face` |
| 22 | `$06E2C0..$06E2C5` | `bclr #1` | `SetFollowMode` `101b` |
| 23 | `$06E2C6..$06E2D5` | `popdlg` / `jsr Event_RunDialogue` | `RunDialogueResume` |
| 24-63 | `$06E2D6..$06E30F` | Gryz, then `loc_6E2E2`: twenty turns through facings 0, 4, 8, `$C`, two `VInt_Prepare` after each | `Face`, `WaitFrames(3)`, twenty times |
| 64-72 | `$06E310..$06E39F` | resume; Gryz 0; `DoMapUpdateLoop($3B)`; resume; Chaz 0; Rika 0; resume; Gryz `$C`; resume | `RunDialogueResume`, `Face`, `Wait(60)`, and so on |
| 73-79 | `$06E3A0..$06E40D` | `$C500`: `$194`, tile `$42B` at (`$230,$220`), a frame, `Event_MoveSingleObject` to (`$2D0,$220`), `loc_6E670` facing `$C`, resume | `CreateFieldObject` 8, `RebuildSprites`, `WaitFrames(1)`, `WalkObject`, `Wait(80)`, `FaceParty`, `RunDialogueResume` |
| 80-86 | `$06E40E..$06E483` | `$C540`: tile `$3E3` at (`$240,$220`), a frame, to (`$2E0,$230`), its own facing 4 (renderer state, not typed), `loc_6E670` facing 0, resume | `CreateFieldObject` 9, as above, `Wait(88)` |
| 87-94 | `$06E484..$06E4D5` | Chaz `$C`, wait 30, 0, wait 30, `$C`, wait 30, 0, resume | `Face`, `Wait(30)` in turn, `RunDialogueResume` |
| 95-104 | `$06E4D6..$06E591` | `$C580` tile `$42B` and `$C5C0` tile `$39B` at (`$3A0,$210`) and (`$3A0,$220`), each a frame and a `dest` to X `$310`; `loc_6E698`, 120 frames of all objects; resume | `CreateFieldObject` 10, 11, `WalkObject`, `Wait(120)`, `RunDialogueResume` |
| 105-112 | `$06E592..$06E5F5` | `dest` of all four: X `$230`, `$230`, `$3D0`, `$3D0`; `bset #1`, `loc_6E698`, `bclr #1`; `clr.w` all four | `WalkObject` x 4, `SetFollowMode`, `Wait(120)`, `SetFollowMode`, `DespawnNpc` 8, four objects |
| 113 | `$06E5F6..$06E601` | `bclr #2`, `bclr #0` | `SetFollowMode` `000b` |
| 114 | `$06E602..$06E613` | `Event_MoveSingleObject` `Character_1` to (`$310,$220`) | `MoveActorTo` the leader |
| 115 | `$06E614..$06E627` | `Event_MoveCamera` to `Character_1`, speed 1 | `PanCamera` the leader |
| 116 | `$06E628..$06E633` | `clr.b $ECF2`; `moveq #$60` / `jmp EventFlags_Set` | `SetFlag` event `$60` |

**The shake.** `$ECF2` counts every shaking frame and `SFXID_GraveOpening` (`$DD`) sounds when its low
bits are zero, so every fourth frame. `$ECD0` picks the row:

| Table | Row 0 (X, Y) | Row 1 (X, Y) |
|---|---|---|
| `loc_6E6B6` | +8, 0 | -8, 0 |
| `loc_6E73A` | +8, -8 | -8, +8 |

After two frames `loc_6E6D6` reloads `loc_6E6B6`, not the table it began on, so the second shake has
two diagonal frames and then only X. The foreground and background counters take the same values,
and even frame counts end where they began. `PresentationOp::Quake` carries the length and which
table opens, and `psiv_runtime::quake` moves the camera and plays the sound. `Char_Move_Flags` bit 2
holds the view through it.

**The party's walk.** The destinations are written before `loc_6E634`, which runs every object for
its 60 frames, enough for the distances from the talk's cell. The scene's walks run on through the
waits; no later op depends on the arrival.

**The villagers' first frame.** Each creation sets `dest` Y from `$30`, the X word
(`move.w $30(a4), $3A(a4)`), not `$34`. `FieldObj_NPCScriptMove` runs one frame on that before the
real `dest` goes in, moving a pixel or two. It is not modelled.

**Frame differences.** `Event_UpdateObjFacing` costs a frame in the US build. The spin counts it
because it paces the turns. Elsewhere the port's `Face` costs none, as in every other scene here. The
two created objects' `Event_MoveSingleObject` walks are derived, 80 and 88 frames at eight frames a
cell, not measured on the oracle.

**The camera after the pan.** It ends 2 pixels past the framed X once the scene is over, where the
cartridge's stays on it. That is [#291](https://github.com/TusanHomichi/PSIV/issues/291), and the
pad-driven test allows it.

## Tests

- `rust/psiv-core/tests/monsen_quake_scene.rs`: one open and nine resumes of entry `$40`, both shakes,
  Rune's walk with and without him, the four objects and their clear.
- `rust/psiv-runtime/src/quake.rs`: each table's rows, and that both lengths cancel.
- Pad-driven: `rust/psiv-campaign/tests/monsen_quake.rs` on hand-built saves: the talk through to the
  flag with and without Rune, the leader's last cell and the camera framed on him, and a negative
  control with `$60` set. The release route has no step on Monsen.
