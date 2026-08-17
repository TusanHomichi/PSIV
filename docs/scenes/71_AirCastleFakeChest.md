# `Event_AirCastleFakeChest`

- **Retail bytes:** `$070B56..$070C2F` inclusive, 218 bytes.
- **Pointer:** `EventPtrs[$5A]`; event `$005A`.
- **Trigger:** `RunEventsJmpTbl[$45]`, Eclipse Torch chest `$10C` set and
  Spector `$A6` clear.
- **Data:** `dezo_campaign.rs`, `AIR_CASTLE_FAKE_CHEST` (11 ops).

## Clone audit

Retail `grand_cross=0` selects the EventPtrs body. The clone's Grand Cross
body is excluded. ROM range: `$070B56..$070C30` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-2 | `$070B56..$070B75` | dialogue `$3C`; set Spector `$A6`; remove Eclipse Torch `$8E` | dialogue/inventory/flag |
| 3-9 | `$070B76..$070C18` | six object clear/animation groups and map-load bit `$08` | object/map presentation |
| 10 | `$070C19..$070C2F` | event battle `$0F`, return `1` | battle |

