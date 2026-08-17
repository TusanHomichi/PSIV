# `Cutscene_LeRoofAgain`

- **Retail bytes:** `$078346..$0784B5` inclusive, 368 bytes.
- **Pointer:** `CutscenePtrs[$1C]`; scene `$801C`.
- **Trigger:** `RunEventsJmpTbl[$34]`, strength chest `$D5` and courage chest
  `$D3` set, Le Roof story `$D6` clear.
- **Data:** `dezo_campaign.rs`, `LE_ROOF_AGAIN` (35 ops).

## Clone audit

The `grand_cross=0` cutscene table selects the retail body at
`Cutscene_LeRoofAgain`; the clone's `grand_cross=1` scene include is not used.
ROM range: `$078346..$0784B6` exclusive end. The final eleven ops are the
retail `$800D` spaceship route inlined because this body jumps to it.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-10 | `$078346..$0783BE` | move, red fade, art `$260`, scene asset, dialogue `$0B` | actor/presentation/dialogue |
| 11-21 | `$0783BF..$07843D` | panel `$11B`, resume, second panel `$125`, dialogue `$0C` | panel/dialogue |
| 22-24 | `$07843E..$078455` | set `$D6/$D7` | flags |
| 25-34 | `$078456..$0784B5` | takeoff music; load map `$000`, then `$18C`, then `$18D` with waits `224/195/257` | spaceship route |

