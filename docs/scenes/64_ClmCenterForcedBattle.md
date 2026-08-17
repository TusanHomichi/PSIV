# `Event_ClmCenterForcedBattle`

- **Retail bytes:** `$070ABC..$070AD9` inclusive, 30 bytes.
- **Pointer:** `EventPtrs[$54]`; event `$0054`.
- **Trigger:** `RunEventsJmpTbl[$3E]`, Dezo Gyla Laugiah `$92` and Dark Force 2
  `$9E` clear.
- **Data:** `dezo_campaign.rs`, `CLM_CENTER_FORCED_BATTLE` (4 ops).

## Clone audit

Retail `grand_cross=0` selects the body at `EventPtrs[$54]`; the Grand Cross
body is not transcribed. ROM range: `$070ABC..$070ADA` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$070ABC..$070AC6` | set Dezo Gyla Laugiah `$92` | flag |
| 1-3 | `$070AC7..$070AD9` | map-load bit `$08`; event battle `$0B`; return `1` | map/battle |

