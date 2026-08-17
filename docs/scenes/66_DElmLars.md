# `Event_DElmLars`

- **Retail bytes:** `$070AEC..$070B0B` inclusive, 32 bytes.
- **Pointer:** `EventPtrs[$56]`; event `$0056`.
- **Trigger:** `RunEventsJmpTbl[$40]`, DElm Lars `$93` and Dark Force 2 `$9E`
  clear, at `y=$0F0`.
- **Data:** `dezo_campaign.rs`, `D_ELM_LARS` (4 ops).

## Clone audit

`EventPtrs[$56]` is the `grand_cross=0` retail body. The clone's hack-only
body is excluded. ROM range: `$070AEC..$070B0C` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-1 | `$070AEC..$070AFB` | dialogue tree entry `$33`; set DElm Lars `$93` | dialogue/flag |
| 2-3 | `$070AFC..$070B0B` | event battle `$0C`; return `1` | battle/return |

