# `Cutscene_FindingAirCastle`

- **Retail bytes:** `$077A2E..$077A67` inclusive, 58 bytes.
- **Pointer:** `CutscenePtrs[$15]`; scene `$8015`.
- **Trigger:** `RunEventsJmpTbl[$42]`, Eclipse Torch stolen `$98` set and Air
  Castle found `$99` clear.
- **Data:** `dezo_campaign_late.rs`, `FINDING_AIR_CASTLE` (54 ops).

## Clone audit

The `grand_cross=0` CutscenePtrs body is the retail source. The clone's
Grand Cross body is excluded. ROM range: `$077A2E..$077A68` exclusive end.
Its final route is the retail `$800D` spaceship body, inlined: the shared menu
and flight, whose destinations depend on `World_Index` and the flags.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-4 | `$077A2E..$077A50` | panel `$18A`, dialogue tree `$20DB0E`, entry `$18` | panel/dialogue |
| 5 | `$077A51..$077A5B` | set `$99` | flag |
| 6-53 | `$077A5C..$077A67` | the inlined `$800D` route: the destination menu, takeoff, planet screen, transit, landing, and the cancel leg | `INSIDE_SPACESHIP_ROUTE` ([Inside the spaceship](41_InsideSpaceship.md)) |
