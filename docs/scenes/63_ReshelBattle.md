# `Event_ReshelBattle`

- **Retail bytes:** `$070A9E..$070ABB` inclusive, 30 bytes.
- **Pointer:** `EventPtrs[$53]`; event `$0053`.
- **Trigger:** `RunEventsJmpTbl[$3D]`, Reshel `$8B` clear.
- **Data:** `dezo_campaign.rs`, `RESHEL_BATTLE` (4 ops).

## Clone audit

The `grand_cross=0` EventPtrs body is the retail source; the clone's hack-only
branch is excluded. ROM range: `$070A9E..$070ABC` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$070A9E..$070AA8` | set Reshel `$8B` | flag |
| 1-3 | `$070AA9..$070ABB` | set map-load bit `$08`; event battle `$0D`; return `1` | map/battle |

