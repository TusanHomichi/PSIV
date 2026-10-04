# `Event_KingRappy`

- **Retail bytes:** `$072FDC..$073017` inclusive, 60 bytes.
- **Pointer:** `EventPtrs[$88]` at `$05A2B4`; event `$0088`.
- **Trigger:** Dialogue `$F6 $00 $88`, `DialogueTree24` entry 61 and `DialogueTree25` entry 61 (Torinco and Uzo with Rappy Cave); an `Event_GetAndRunDialogue2` caller (`jsr $5ACDC` at `$072FEA`, row 13, issue #71).
- **Data:** `census_events.rs`, `KING_RAPPY` (7 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot; the clone's label
names it and nothing else. Transcribed by the event census lane (S7,
[EVENT_COVERAGE](EVENT_COVERAGE.md)).

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072FDC..$072FE6` | `move.b #$AE, d0` / `jsr EventFlags_Test` / `beq loc_73010` | `BranchFlag` `$AE`: set falls through, clear to op 6 |
| 1 | `$072FE8..$072FEF` | `moveq #$36, d0` / `jsr Event_GetAndRunDialogue2` | `retained(0x36)` |
| 2 | `$072FF0..$072FF9` | `move.b #$AF, d0` / `jsr EventFlags_Set` | `SetFlag` `$AF` |
| 3 | `$072FFA..$072FFF` | `bset #3, Map_Load_Flags` | `SetMapLoadFlags` `0x08` |
| 4 | `$073000..$07300B` | `move.b #$13, Event_Battle_Index` / `bset #3, Routine_Exit_Flags` | `StartBattle` `0x13` |
| 5 | `$07300C..$07300E` | `moveq #1, d0` / `rts` | `Return` `1` |
| 6 | `$073010..$073016` | `loc_73010`: `moveq #$35, d0` / `jmp Event_GetAndRunDialogue` | standard `RunDialogue` `$35` |

With `$AE` clear the king only talks (entry `$35`); with it set the fight starts.
