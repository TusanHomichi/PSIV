# `Event_KingRappyDefeated`

- **Retail bytes:** `$073018..$0730FB` inclusive, 228 bytes of code; the palette `loc_730FC` runs to `$07311D`.
- **Pointer:** `EventPtrs[$89]` at `$05A2B4 + $224`; event `$0089`.
- **Trigger:** map trigger `$73` on `RappyCave` (`$0ED`), `RunEvent_KingRappyDefeated` (`ps4.asm:116486-116495`): `EventFlag_KingRappy` (`$AF`) set, `EventFlag_SekreasReason` (`$BA`) clear. `trigger_table.rs` row `$73`.
- **Data:** `motavia_side.rs`, `KING_RAPPY_DEFEATED` (18 ops).
- **Flags:** sets `EventFlag_SekreasReason` (`$BA`) last. `MapDataMan_RappyCave` then despawns objects 0 to 2 on every load (`pack: map_effects`, entry `$80`).

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every word matches
`ps4.asm:152882-152932`. It has no `revision` or `grand_cross` conditional and loads no tree.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$073018..$073029` | `Event_MoveSingleObject`, `$C380`, to (`$3C0,$1C0`) | `MoveActorTo` object 2, blocking |
| 1 | `$07302A..$07302B` | `clr.w (a4)` | `DespawnNpc` 2 |
| 2 | `$07302C..$073033` | `moveq #$37, d0` / `jsr Event_GetAndRunDialogue` | standard `RunDialogue` `$37` |
| 3 | `$073034..$073045` | `Event_MoveCharacter`, `Character_1`, to (`$3E0,$110`) | `MoveActorTo` the leader, blocking |
| 4 | `$073046..$073053` | `popdlg` / `jsr Event_RunDialogue` | `RunDialogueResume` |
| 5 | `$073054..$073067` | sixteen words of `loc_730FC` to `Palette_Line_4` | `LoadPalette` |
| 6 | `$073068..$07306D` | `jsr VInt_Prepare` | `WaitFrames(1)` |
| 7 | `$07306E..$07309D` | `$C3C0`: routine `$194`, `$13 := $60`, tile `$202`, at (`$3E0,$1C0`) | `CreateFieldObject` slot 3 |
| 8-9 | `$07309E..$0730B5` | `RunSingleObject`, `Field_LoadSprites`, `Field_BuildSprites`, `VInt_Prepare` | `RebuildSprites`, `WaitFrames(1)` |
| 10-11 | `$0730B6..$0730C7` | `Event_MoveSingleObject`, `$C3C0`, to (`$3E0,$120`) | `WalkObject`, `Wait(80)` |
| 12 | `$0730C8..$0730D5` | `popdlg` / `jsr Event_RunDialogue` | `RunDialogueResume` |
| 13-14 | `$0730D6..$0730E7` | `Event_MoveSingleObject`, `$C3C0`, to (`$3E0,$1C0`) | `WalkObject`, `Wait(80)` |
| 15 | `$0730E8..$0730E9` | `clr.w (a4)` | `DespawnNpc` 3 |
| 16 | `$0730EA..$0730F1` | `moveq #$38, d0` / `jsr Event_GetAndRunDialogue` | standard `RunDialogue` `$38` |
| 17 | `$0730F2..$0730FB` | `move.b #$BA, d0` / `jmp EventFlags_Set` | `SetFlag` `$BA` |

**Object 2.** `KingRappyFlyingAway` sets `render_flags` bit 1, so it draws nothing: op 0 is a wait
the player sees as a pause. It is the map's own object, so the walk is the runner's.

**The walks of slot 3.** `$A0` pixels of Y at the normal step (`$0200`), ten cells at eight frames
a cell. The frame count is derived from `FieldObj_NPCScriptMove`'s routine, not measured on the
oracle.

## Verification

`rust/psiv-core/tests/motavia_side_scenes.rs` holds the lines, the two `$F7` resumes, the despawns
and the flag. `rust/psiv-campaign/tests/motavia_side.rs` steps on the real map with `$AF` set from a
hand-built save: the trigger fires, the leader ends on (`$3E0,$110`) and `$BA` is set; without `$AF`
the same step fires nothing. The route through the fight itself is not driven here.

**Limit.** The object in slot 3 is presentation only, and no capture has been compared. The pack has
no temporary-object sheet for routine `$194` on tile `$202`, so Sekreas's walk is not drawn yet ([tracking issue](https://github.com/TusanHomichi/PSIV/issues/295)).
