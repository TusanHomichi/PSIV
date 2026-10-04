# `Event_MileSandWormBattle`

- **Retail bytes:** `$072654..$072677` inclusive, 36 bytes.
- **Pointer:** `EventPtrs[$71]` at `$05A2B4`; event `$0071`.
- **Trigger:** Mile, the custom `RunEvent_MileSandWorm` (slots `$5C..$70`, `trigger_custom.rs`): `$1B` clear, `x <= $170`, `$150 <= y <= $2B0`, then one RNG draw with `(seed & $1F) == 0`.
- **Data:** `census_events.rs`, `MILE_SAND_WORM_BATTLE` (5 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot; the clone's label
names it and nothing else. Transcribed by the event census lane (S7,
[EVENT_COVERAGE](EVENT_COVERAGE.md)).

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072654..$07265B` | `moveq #$1B, d0` / `jsr EventFlags_Set` (`$57666`, the event bank) | `SetFlag` `$1B` |
| 1 | `$07265C..$072661` | `move.b #$9E, Saved_Sound_Index` | `SetSavedMusic` `$9E` |
| 2 | `$072662..$072667` | `bset #3, Map_Load_Flags` | `SetMapLoadFlags` `0x08` |
| 3 | `$072668..$072673` | `move.b #2, Event_Battle_Index` / `bset #3, Routine_Exit_Flags` | `StartBattle` `2` |
| 4 | `$072674..$072676` | `moveq #1, d0` / `rts` | `Return` `1` |

`$1B` is set before the fight, so the worm fires once per save.
