# `Event_DaughterTerminal`

- **Retail bytes:** `$0731B2..$0731D9` inclusive, 40 bytes.
- **Pointer:** `EventPtrs[$8F]` at `$05A2B4`; event `$008F`.
- **Trigger:** Dialogue `$F6 $00 $8F`, `DialogueTree43` entry 10 (the tree shared by the overworld, Weapon Plant and Vahal Fort); an `Event_GetAndRunDialogue2` caller (`jsr $5ACDC` at `$0731B4`, row 14, issue #71).
- **Data:** `census_events.rs`, `DAUGHTER_TERMINAL` (5 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot; the clone's label
names it and nothing else. Transcribed by the event census lane (S7,
[EVENT_COVERAGE](EVENT_COVERAGE.md)).

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$0731B2..$0731B9` | `moveq #4, d0` / `jsr Event_GetAndRunDialogue2` | `retained(4)` |
| 1 | `$0731BA..$0731C3` | `move.b #$B6, d0` / `jsr EventFlags_Set` | `SetFlag` `$B6` |
| 2 | `$0731C4..$0731C9` | `bset #3, Map_Load_Flags` | `SetMapLoadFlags` `0x08` |
| 3 | `$0731CA..$0731D5` | `move.b #$15, Event_Battle_Index` / `bset #3, Routine_Exit_Flags` | `StartBattle` `0x15` |
| 4 | `$0731D6..$0731D8` | `moveq #1, d0` / `rts` | `Return` `1` |
