# `Event_ClmCenterAfterBattle`

- **Retail bytes:** `$070ADA..$070AEB` inclusive, 18 bytes.
- **Pointer:** `EventPtrs[$55]`; event `$0055`.
- **Trigger:** `RunEventsJmpTbl[$3F]`, Gyla `$92` set, Climate Center `$A4`
  and Dark Force 2 `$9E` clear.
- **Data:** `dezo_campaign.rs`, `CLM_CENTER_AFTER_BATTLE` (2 ops).

## Clone audit

This is the retail `grand_cross=0` pointer body; the Grand Cross branch is
excluded. ROM range: `$070ADA..$070AEC` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$070ADA..$070AE5` | dialogue tree entry `$31` | dialogue |
| 1 | `$070AE6..$070AEB` | set Climate Center `$A4` | flag |

