# `Cutscene_FindingAirCastle`

- **Retail bytes:** `$077A2E..$077A67` inclusive, 58 bytes.
- **Pointer:** `CutscenePtrs[$15]`; scene `$8015`.
- **Trigger:** `RunEventsJmpTbl[$42]`, Eclipse Torch stolen `$98` set and Air
  Castle found `$99` clear.
- **Data:** `dezo_campaign.rs`, `FINDING_AIR_CASTLE` (18 ops).

## Clone audit

The `grand_cross=0` CutscenePtrs body is the retail source. The clone's
Grand Cross body is excluded. ROM range: `$077A2E..$077A68` exclusive end.
Its final route is the retail `$800D` spaceship body, inlined with the
intermediate `$18C` load and wait.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-5 | `$077A2E..$077A50` | panel `$18A`, dialogue tree `$20DB0E`, entry `$18` | panel/dialogue |
| 6-17 | `$077A51..$077A67` | set `$99`; takeoff route `$000 -> $18C -> $18D` | flag/spaceship route |

