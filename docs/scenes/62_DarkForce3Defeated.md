# `Event_DarkForce3Defeated`

- **Retail bytes:** `$070A2A..$070A4D` inclusive, 36 bytes.
- **Pointer:** `EventPtrs[$50]`; event `$0050`.
- **Trigger:** `RunEventsJmpTbl[$3C]`, Dark Force 3 `$C5` set and defeat `$C6`
  clear.
- **Data:** `dezo_campaign.rs`, `DARK_FORCE_3_DEFEATED` (2 ops).

## Clone audit

`EventPtrs[$50]` in the retail `grand_cross=0` table selects this body. The
Grand Cross alternative is excluded. ROM range: `$070A2A..$070A4E` exclusive
end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0 | `$070A2A..$070A3D` | dialogue tree entry `2` | dialogue |
| 1 | `$070A3E..$070A4D` | set Dark Force 3 defeated `$C6` | flag |

