# `Event_StrengthTowerTop`

- **Retail bytes:** `$070E4E..$071101` inclusive, 692 bytes.
- **Pointer:** `EventPtrs[$5E]`; event `$005E`.
- **Trigger:** `RunEventsJmpTbl[$49]`, Strength Tower top `$E2` clear.
- **Data:** `dezo_endgame.rs`, `STRENGTH_TOWER_TOP` (15 ops).

## Clone audit

The retail `grand_cross=0` EventPtrs body is transcribed; Grand Cross-only
code is excluded. ROM range: `$070E4E..$071102` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-12 | `$070E4E..$0710D5` | camera, four object animation slots `$2AC`, Foi/Efess/killed SFX and waits | camera/object/SFX |
| 13-14 | `$0710D6..$071101` | camera to `($1F0,$270)`; set `$E2` | camera/flag |

