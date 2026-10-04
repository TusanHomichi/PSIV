# `Event_FractOozeFound`

- **Retail bytes:** `$072B2C..$072B51` inclusive, 38 bytes.
- **Pointer:** `EventPtrs[$7D]` at `$05A2B4`; event `$007D`.
- **Trigger:** Dialogue `$F6 $00 $7D`, `DialogueTree12` entry 38 (Monsen); an `Event_GetAndRunDialogue2` caller (`jsr $5ACDC` at `$072B2E`, row 12 of [DIALOGUE2_CALLERS](DIALOGUE2_CALLERS.md), issue #71).
- **Data:** `census_events.rs`, `FRACT_OOZE_FOUND` (5 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot; the clone's label
names it and nothing else. Transcribed by the event census lane (S7,
[EVENT_COVERAGE](EVENT_COVERAGE.md)).

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072B2C..$072B33` | `moveq #$2F, d0` / `jsr Event_GetAndRunDialogue2` (`$5ACDC`) | `retained(0x2F)` |
| 1 | `$072B34..$072B3B` | `moveq #$2F, d0` / `jsr EventFlags_Set` | `SetFlag` `$2F` |
| 2 | `$072B3C..$072B41` | `bset #3, Map_Load_Flags` | `SetMapLoadFlags` `0x08` |
| 3 | `$072B42..$072B4D` | `move.b #7, Event_Battle_Index` / `bset #3, Routine_Exit_Flags` | `StartBattle` `7` |
| 4 | `$072B4E..$072B50` | `moveq #1, d0` / `rts` | `Return` `1` |

The retained window stays up through the battle request, as for every `Dialogue2` caller.
