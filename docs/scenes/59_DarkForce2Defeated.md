# `Cutscene_DarkForce2Defeated`

- **Retail bytes:** `$077BDA..$077DC5` inclusive, 492 bytes.
- **Pointer:** `CutscenePtrs[$17]`; scene `$8017`.
- **Trigger:** `RunEventsJmpTbl[$38]`, Dark Force 2 `$9E` set and Snowstorm Gone
  `$A1` clear.
- **Data:** `dezo_campaign.rs`, `DARK_FORCE_2_DEFEATED` (33 ops).

## Clone audit

The `grand_cross=0` cutscene table selects the retail body. The clone's
Grand Cross-only body is excluded. ROM range: `$077BDA..$077DC6` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-14 | `$077BDA..$077C8D` | panels `$10E/$10F/$110`, explosion and four grave-opening waits | presentation/SFX |
| 15-23 | `$077C8E..$077D3B` | dialogue `$3B`, Dezo field music, panel teardown and resumes | dialogue/presentation |
| 24-27 | `$077D3C..$077D63` | set `$A1`; remove/clear Kyra; stop music and wait | party/flag |
| 28-32 | `$077D64..$077DC5` | Dezo field music; load Dezolis `($174,$0E)`; return | map/music |

