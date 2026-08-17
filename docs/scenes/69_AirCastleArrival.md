# `Event_AirCastleArrival`

- **Retail bytes:** `$070B1E..$070B2F` inclusive, 18 bytes.
- **Pointer:** `EventPtrs[$58]`; event `$0058`.
- **Trigger:** `RunEventsJmpTbl[$43]`, Air Castle `$9F` clear.
- **Data:** `dezo_campaign.rs`, `AIR_CASTLE_ARRIVAL` (2 ops).

## Clone audit

Retail `grand_cross=0` selects this EventPtrs body; the hack-only alternative
is excluded. ROM range: `$070B1E..$070B30` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$070B1E..$070B29` | dialogue tree entry `$35` | dialogue |
| 1 | `$070B2A..$070B2F` | set Air Castle `$9F` | flag |

