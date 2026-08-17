# `Cutscene_GumbiousBishop`

- **Retail bytes:** `$077DC6..$077EAB` inclusive, 230 bytes.
- **Pointer:** `CutscenePtrs[$18]`; scene `$8018`.
- **Trigger:** `RunEventsJmpTbl[$48]`, Hydrofoil `$9D` clear at Gumbious.
- **Data:** `dezo_campaign.rs`, `GUMBIOUS_BISHOP` (17 ops).

## Clone audit

The retail `grand_cross=0` cutscene body is selected; the Grand Cross body is
excluded. ROM range: `$077DC6..$077EAC` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-6 | `$077DC6..$077E1E` | dialogue `$33`, fade, takeoff music, load Mota Spaceport | dialogue/map |
| 7-12 | `$077E1F..$077E72` | Machine Center music, move Chaz, tree `$2080BE`, dialogue `$17` | music/actor/dialogue |
| 13-16 | `$077E73..$077EAB` | remove Torch `$8E`, add Hydrofoil `$98`, set `$9D/$C0` | inventory/flags |

