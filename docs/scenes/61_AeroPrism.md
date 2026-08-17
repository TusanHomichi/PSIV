# `Cutscene_AeroPrism`

- **Retail bytes:** `$077F2E..$07818D` inclusive, 608 bytes.
- **Pointer:** `CutscenePtrs[$1A]`; scene `$801A`.
- **Trigger:** `RunEventsJmpTbl[$3B]`, Aero Prism chest `$10D` set and Dark
  Force 3 `$C5` clear.
- **Data:** `dezo_campaign.rs`, `AERO_PRISM` (27 ops).

## Clone audit

The retail cutscene table's `grand_cross=0` body is transcribed; the clone's
Grand Cross body is excluded. ROM range: `$077F2E..$07818E` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-6 | `$077F2E..$077F9B` | panel `$A0`, 30-frame fade, barrier SFX, dialogue entry `1` | panel/dialogue |
| 7-15 | `$077F9C..$07804C` | waits, stop SFX, resume, panel `$A4`, red alert, resume | presentation/dialogue |
| 16-22 | `$07804D..$07810B` | panel `$A5`, lightning, resume; remove/clear Seth | presentation/party |
| 23-26 | `$07810C..$07818D` | set `$C5`, map flag `$08`, battle `$12`, return `1` | flag/battle |

