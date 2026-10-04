# `Cutscene_LeRoofAgain`

- **Retail bytes:** `$078346..$0784B5` inclusive, 368 bytes.
- **Pointer:** `CutscenePtrs[$1C]`; scene `$801C`.
- **Trigger:** `RunEventsJmpTbl[$34]`, strength chest `$D5` and courage chest
  `$D3` set, Le Roof story `$D6` clear.
- **Data:** `dezo_campaign.rs`, `LE_ROOF_AGAIN` (71 ops).

## Clone audit

The `grand_cross=0` cutscene table selects the retail body at
`Cutscene_LeRoofAgain`; the clone's `grand_cross=1` scene include is not used.
ROM range: `$078346..$0784B6` exclusive end. The final 48 ops are the
retail `$800D` spaceship route (`INSIDE_SPACESHIP_ROUTE`, menu and flight) inlined
because this body jumps to it.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-9 | `$078346..$0783BE` | move, red fade, art `$260`, scene asset, dialogue `$0B` | actor/presentation/dialogue |
| 10-20 | `$0783BF..$07843D` | panel `$11B`, resume, second panel `$125`, dialogue `$0C` | panel/dialogue |
| 21-22 | `$07843E..$078455` | set `$D6/$D7` | flags |
| 23-70 | `$078456..$0784B5` | the inlined `$800D` route: the destination menu, takeoff, planet screen, transit, landing, and the cancel leg | `INSIDE_SPACESHIP_ROUTE` ([Inside the spaceship](41_InsideSpaceship.md)) |
