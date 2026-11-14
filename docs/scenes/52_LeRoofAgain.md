# `Cutscene_LeRoofAgain`

- **Retail bytes:** `$078346..$0784B5` inclusive, 368 bytes.
- **Pointer:** `CutscenePtrs[$1C]`; scene `$801C`.
- **Trigger:** `RunEventsJmpTbl[$34]`, strength chest `$D5` and courage chest
  `$D3` set, Le Roof story `$D6` clear.
- **Data:** `dezo_campaign.rs`, `LE_ROOF_AGAIN` (71 ops).

## Clone audit

The `grand_cross=0` cutscene table selects the retail body at
`Cutscene_LeRoofAgain`; the clone's `grand_cross=1` scene include is not used.
ROM range: `$078346..$0784B6` exclusive end. The final 44 ops are the
retail `$800D` spaceship route (`INSIDE_SPACESHIP_ROUTE`, menu and flight) inlined
because this body jumps to it.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-2 | `$078346..$07836B` | move Chaz to `($1F0,$1F0)`, `Pal_VariableFadeOut_Lines_1_2` (`$ED52` 2), `DoMapUpdateLoop` 60 | actor/`TwoLinePaletteFade` out/wait |
| 3 | `$07836C..$078397` | tower art `$1DB2C8` to tile `$260`; the scene asset `$1DDE0A` has no record here | presentation |
| 4-5 | `$078398..$0783AD` | `loc_1DE258`'s 16 words to `Palette_Table_Buffer_2`, its words 16-31 cleared | `PaletteCopy` ×2 |
| 6 | `$0783AE..$0783D9` | plane map, DMA | presentation |
| 7-8 | `$0783DA..$0783ED` | music `$A1`, `Pal_VariableFadeToPal2` (`$ED52` 2) | sound/`TwoLinePaletteFade` in |
| 9 | `$0783EE..$078405` | `move.w #$B3, d0`: 180 passes of `jsr loc_70638`, `VInt_Prepare` | `CounterPaletteCycle` 180 |
| 10 | `$078406..$07840B` | `jsr loc_705D6`: [`Event_MeetingLeRoof`](51_MeetingLeRoof.md)'s 71-pass fade-in | `PaletteGrowIn` 71 |
| 11 | `$07840C..$078421` | 120 passes (`moveq #$77, d0`) | `CounterPaletteCycle` 120 |
| 12 | `$078422..$078429` | dialogue `$0B` | dialogue |
| 13-17 | `$07842A..$078459` | `InitVRAMAndCRAM`, `Pal_FadeIn`, panel `$11B`, DMA, `popdlg` / `Event_RunDialogue3` | panel/resume |
| 18 | `$07845A..$078469` | `Pal_VariableFadeOut` (`$ED52` 5), `clr.b Panel_Num` | fade-out |
| 19 | `$07846A..$078471` | `moveq #$3B, d0` / `DoMapUpdateLoop`: 60 frames with map updates | wait |
| 20-24 | `$078472..$07849B` | `InitVRAMAndCRAM`, `Pal_FadeIn`, panel `$125`, DMA, dialogue `$0C` | panel/dialogue |
| 25-26 | `$07849C..$0784AF` | set `$D6/$D7` | flags |
| 27-70 | `$0784B0..$0784B5` | `jmp Cutscene_InsideSpaceship`: the inlined `$800D` route: the destination menu, takeoff, planet screen, transit, landing, and the cancel leg | `INSIDE_SPACESHIP_ROUTE` ([Inside the spaceship](41_InsideSpaceship.md)) |

The colour loops and the fade-in run `VInt_Prepare` only, no map or object
update; the scene blocks for every pass.

Gaps: no scene picture is drawn natively, and op 3 has no `LoadSceneAsset`
([#594](https://github.com/TusanHomichi/PSIV/issues/594)). The loops start 7
counted frames early, which moves the cycle's phase, and op 18 is
`PalFadeOut_ClrSpriteTbl` where retail calls `Pal_VariableFadeOut` with `$ED52`
5 ([#595](https://github.com/TusanHomichi/PSIV/issues/595)).
