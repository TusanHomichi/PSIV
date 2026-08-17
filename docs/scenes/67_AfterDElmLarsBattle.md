# `Event_AfterDElmLarsBattle`

- **Retail bytes:** `$070B0C..$070B1D` inclusive, 18 bytes.
- **Pointer:** `EventPtrs[$57]`; event `$0057`.
- **Trigger:** `RunEventsJmpTbl[$41]`, DElm Lars `$93` set, defeat `$A5` and
  Dark Force 2 `$9E` clear.
- **Data:** `dezo_campaign.rs`, `AFTER_D_ELM_LARS_BATTLE` (2 ops).

## Clone audit

The retail `grand_cross=0` EventPtrs body is selected; the Grand Cross body is
excluded. ROM range: `$070B0C..$070B1E` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$070B0C..$070B17` | dialogue tree entry `$34` | dialogue |
| 1 | `$070B18..$070B1D` | set DElm Lars defeated `$A5` | flag |

