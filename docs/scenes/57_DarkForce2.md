# `Event_DarkForce2`

- **Retail bytes:** `$070976..$0709A1` inclusive, 44 bytes.
- **Pointer:** `EventPtrs[$4E]`; event `$004E`.
- **Trigger:** `RunEventsJmpTbl[$37]`, Dark Force 2 `$9E` clear at `y=$100`.
- **Data:** `dezo_campaign.rs`, `DARK_FORCE_2` (6 ops).

## Clone audit

Retail `EventPtrs[$4E]` is selected by `grand_cross=0`; the clone's hack body
is excluded. ROM range: `$070976..$0709A2` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-1 | `$070976..$07098B` | dialogue tree entry `$3A`; set Dark Force 2 `$9E` | dialogue/flag |
| 2-5 | `$07098C..$0709A1` | stop all, map-load bit `$80`, battle `$11`, return `1` | battle/map flags |

