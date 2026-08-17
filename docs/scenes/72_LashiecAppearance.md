# `Event_LashiecAppearance`

- **Retail bytes:** `$070CB4..$070E4D` inclusive, 410 bytes.
- **Pointer:** `EventPtrs[$5D]`; event `$005D`.
- **Trigger:** `RunEventsJmpTbl[$46]`, Spector `$A6` set and Lashiec `$9B`
  clear.
- **Data:** `dezo_campaign.rs`, `LASHIEC_APPEARANCE` (12 ops).

## Clone audit

The retail `grand_cross=0` EventPtrs body is transcribed. The clone's
Grand Cross branch is excluded. ROM range: `$070CB4..$070E4E` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-6 | `$070CB4..$070DDA` | art `$1D7710` tile `$371`, camera/object animation, alert and dialogue `$37` | presentation/dialogue |
| 7-11 | `$070DDB..$070E3C` | set Lashiec `$9B`, saved red-alert music, map flags `$88`, battle `$10` | flag/music/battle |
| 11 | `$070E3D..$070E4D` | event return `1` | return |

