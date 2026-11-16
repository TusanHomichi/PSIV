# `Cutscene_FindingAirCastle`

- **Retail bytes:** `$077A2E..$077A67` inclusive, 58 bytes.
- **Pointer:** `CutscenePtrs[$15]`; scene `$8015`.
- **Trigger:** `RunEventsJmpTbl[$42]`, Eclipse Torch stolen `$98` set and Air
  Castle found `$99` clear.
- **Data:** `dezo_campaign_late.rs`, `FINDING_AIR_CASTLE` (61 ops).

## Clone audit

The `grand_cross=0` CutscenePtrs body is the retail source. The clone's
Grand Cross body is excluded. ROM range: `$077A2E..$077A68` exclusive end.
Its final route is the retail `$800D` spaceship body, inlined: the shared menu
and flight, whose destinations depend on `World_Index` and the flags. The
censuses read this scene's own bytes against ops 0-6; the route is
[`Cutscene_InsideSpaceship`](41_InsideSpaceship.md)'s.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-5 | `$077A2E..$077A57` | `InitVRAMAndCRAM` (`$077A2E`), `Pal_FadeIn` (`$077A34`), panel `$18A`, dialogue tree `$20DB0E`, entry `$18` | panel/dialogue |
| 6 | `$077A58..$077A61` | set `$99` | flag |
| 7-60 | `$077A62..$077A67` | `jmp Cutscene_InsideSpaceship` (`$077A62`): the inlined `$800D` route: the destination menu, takeoff, planet screen, transit, landing, and the cancel leg | `INSIDE_SPACESHIP_ROUTE` ([Inside the spaceship](41_InsideSpaceship.md)) |
