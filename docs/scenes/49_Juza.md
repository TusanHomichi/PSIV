# `Event_Juza`

- **Retail bytes:** `$06FB1E..$06FB5D` inclusive, 64 bytes.
- **Pointer:** `EventPtrs[$40]`; event `$0040`.
- **Reachability:** Kuran-side interaction path; the map trigger immediately
  after it is `RunEventsJmpTbl[$2F]` / `Event_JuzaDefeated`.
- **Data:** `post_zio_cutscenes.rs`, `JUZA` (5 ops).

## Clone audit

The source body is explicitly wrapped in `if grand_cross=0` at the retail
clone's `Event_Juza` label (`ps4.asm:149144`). The Grand Cross branch is not
transcribed. Actual-ROM pointer range: `$06FB1E..$06FB5E` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FB1E..$06FB2B` | face/fade-lines helper | typed red fade |
| 1-2 | `$06FB2C..$06FB4D` | dialogue entry `$48`; set Juza `$41` | dialogue/flag |
| 3-4 | `$06FB4E..$06FB5D` | event battle index `3`; routine-exit handoff | battle/return |

