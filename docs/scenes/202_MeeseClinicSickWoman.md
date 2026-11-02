# `Event_MeeseClinicSickWoman`

- **Retail bytes:** `$0733CC..$07341F` code, table `loc_73420` `$073420..$07343F`.
- **Pointer:** `EventPtrs[$93]` at `$05A2B4 + $24C`; event `$0093`.
- **Trigger:** Dialogue `$F6 $00 $93`, `DialogueTree18` entry 102. Area: `MeeseClinic` (`$133`), objects `$D` to `$14`, the four beds and their blocks.
- **Data:** `motavia_side.rs`, `MEESE_CLINIC_SICK_WOMAN` (20 ops).
- **Flags:** reads `EventFlag_DarkForce2` (`$9E`); writes none.

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every word matches
`ps4.asm:153119-153162`. It has no `revision` or `grand_cross` conditional and loads no tree.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0-7 | `$0733CC..$0733F1` | compare the leader's `$30`/`$34` with each `loc_73420` row; past the last, `rts` | `BranchIfActorCoord` X and Y per row |
| 8-15 | `$0733F2..$073407` | `movea.l $4(a1), a4`, the leader's facing with bit 2 flipped, `Event_UpdateObjFacing` | `FaceOppositeOf` object `$13`, `$F`, `$D` or `$11` |
| 16 | `$073408..$073413` | `EventFlag_DarkForce2`, `EventFlags_Test` | `BranchFlag` `$9E` |
| 17-19 | `$073414..$07341F` | `moveq #$3C` or `#$3D`, `jmp Event_GetAndRunDialogue` | standard `RunDialogue` `$3C`, `$3D` |

| Row | Leader at | Object | Bed |
|---:|---|---|---|
| 0 | (`$110,$260`) | `$FFFFC7C0`, `$13` | `StudentInBed` |
| 1 | (`$110,$1A0`) | `$FFFFC6C0`, `$F` | `loc_4BE38` |
| 2 | (`$200,$140`) | `$FFFFC640`, `$D` | `loc_4BDF0` |
| 3 | (`$230,$140`) | `$FFFFC740`, `$11` | `loc_4BE80` |

Entry 102 is the whole of what every bed's objects say, so from any other cell the routine returns
and the talk ends with nothing on screen.

## Verification

`rust/psiv-core/tests/motavia_side_scenes.rs` holds each row, both lines, and cells off the table.
`rust/psiv-campaign/tests/motavia_side.rs` talks to a bed on the real map, from its table cell.
