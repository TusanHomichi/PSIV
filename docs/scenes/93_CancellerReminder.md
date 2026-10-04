# `Event_CancellerReminder`

- **Retail bytes:** `$06DEAE..$06DEBD` inclusive, 16 bytes.
- **Pointer:** `EventPtrs[$2A]` at `$05A2B4`; event `$002A`.
- **Trigger:** Zelan F1 `$18E`, `RunEventsJmpTbl[$55]`, the elevator reminder (runner log H26, issue #56): the party reached the elevator before opening the Canceller chest.
- **Data:** `census_events.rs`, `CANCELLER_REMINDER` (2 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot; the clone's label
names it and nothing else. Transcribed by the event census lane (S7,
[EVENT_COVERAGE](EVENT_COVERAGE.md)).

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06DEAE..$06DEB5` | `moveq #$C, d0` / `jsr Event_GetAndRunDialogue` (`$5AC66`) | standard `RunDialogue` `$0C` |
| 1 | `$06DEB6..$06DEBD` | `moveq #$73, d0` / `jmp EventFlags_Set` (tail call) | `SetFlag` `$73` |

The scene is off the critical path: opening chest 5 first (`$72`) never reaches it (C5).
