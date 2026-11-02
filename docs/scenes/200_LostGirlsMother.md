# `Event_LostGirlsMother`

- **Retail bytes:** `$072C08..$072C2D` inclusive, 38 bytes.
- **Pointer:** `EventPtrs[$7F]` at `$05A2B4 + $1FC`; event `$007F`.
- **Trigger:** Dialogue `$F6 $00 $7F`, `DialogueTree25` entry 13. Area: Uzo, `UzoHouse3` (`$0E4`) object 1.
- **Data:** `motavia_side.rs`, `LOST_GIRLS_MOTHER` (3 ops).
- **Flags:** sets `EventFlag_MissingGirlsMom` (`$79`), which `Event_GirlPrison` (`$7A`) reads.

## Clone audit

The body was read from the US image through its `EventPtrs` slot, and every word matches
`ps4.asm:152612-152621`. It has no `revision` or `grand_cross` conditional and loads no tree.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072C08..$072C1D` | `lea $C340, a4`; `lea Character_1, a3`; `$6(a3)` with bit 2 flipped; `Event_UpdateObjFacing` | `FaceOppositeOf` object 1, the leader |
| 1 | `$072C1E..$072C25` | `moveq #$13, d0` / `jsr Event_GetAndRunDialogue` | standard `RunDialogue` `$13` |
| 2 | `$072C26..$072C2D` | `moveq #$79, d0` / `jmp EventFlags_Set` | `SetFlag` `$79` |

## Verification

`rust/psiv-core/tests/motavia_side_scenes.rs` holds the scene's outcome.
`rust/psiv-campaign/tests/motavia_side.rs` talks to the mother on the real map with the guild's
job taken (`$78`), and she ends with the flag set and facing the leader.
