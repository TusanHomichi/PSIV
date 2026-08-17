# `Event_XeAThoulBeforeBattle`

- **Retail bytes:** `$070B30..$070B55` inclusive, 38 bytes.
- **Pointer:** `EventPtrs[$59]`; event `$0059`.
- **Trigger:** `RunEventsJmpTbl[$44]`, Xe A Thoul `$9A` clear in the Air
  Castle position rectangle.
- **Data:** `dezo_campaign.rs`, `XE_ATHOUL_BEFORE_BATTLE` (5 ops).

## Clone audit

The retail `grand_cross=0` pointer body is used; Grand Cross-only code is
excluded (`ps4.options.asm:10`). ROM range: `$070B30..$070B56` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-1 | `$070B30..$070B42` | dialogue tree entry `$36`; set Xe A Thoul `$9A` | dialogue/flag |
| 2-4 | `$070B43..$070B55` | map-load bit `$08`; event battle `$0E`; return `1` | map/battle |

