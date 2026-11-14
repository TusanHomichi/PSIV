# `Event_MeetingLeRoof`

- **Retail bytes:** `$070482..$07069D` inclusive, 540 bytes.
- **Pointer:** `EventPtrs[$48]`; event `$0048`.
- **Trigger:** `RunEventsJmpTbl[$33]`, Le Roof `$D1` clear.
- **Data:** `dezo_campaign.rs`, `MEETING_LE_ROOF` (23 ops).

## Clone audit

The retail `EventPtrs` body is selected by the `grand_cross=0` table at
`ps4.asm:120622`. `reference/ps4.options.asm:10` sets `grand_cross=1` for the
clone build, so its hack-only body is excluded. ROM range: `$070482..$07069E`
exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-3 | `$070482..$0704AB` | move Chaz to `($1F0,$1E0)`, `DoMapUpdateLoop` 60, `Pal_VariableFadeOut_Lines_1_2` (`$ED52` 2), `DoMapUpdateLoop` 60 | actor/wait/`TwoLinePaletteFade` out/wait |
| 4-5 | `$0704AC..$0704D7` | tower art `$1DB2C8` to tile `$260`, scene asset `$1DDE0A` | presentation |
| 6-7 | `$0704D8..$0704ED` | `loc_1DE258`'s 16 words to `Palette_Table_Buffer_2` (`trap #1`), its words 16-31 cleared (`trap #0`): what the fade up returns to | `PaletteCopy` ×2 |
| 8 | `$0704EE..$07051F` | plane map, DMA | presentation |
| 9-10 | `$070520..$070533` | music `$A1`, `Pal_VariableFadeToPal2` (`$ED52` 2) | sound/`TwoLinePaletteFade` in |
| 11 | `$070534..$070549` | `move.w #$B3, d0`: 180 passes of `loc_70638`, `VInt_Prepare` | `CounterPaletteCycle` 180 |
| 12 | `$07054A..$07054D`; `$0705D6..$070625` | `bsr loc_705D6`: 71 passes (`moveq #$46, d7`) of the ramp `loc_70626` into words 2-10, `loc_70638`, `VInt_Prepare` | `PaletteGrowIn` 71 |
| 13 | `$07054E..$070563` | 120 passes (`move.w #$77, d0`) | `CounterPaletteCycle` 120 |
| 14 | `$070564..$07056B` | dialogue entry `0` | dialogue |
| 15 | `$07056C..$070581` | 120 passes | `CounterPaletteCycle` 120 |
| 16-17 | `$070582..$070595` | music stop `$FB`, `Pal_VariableFadeOut_Lines_1_2` (`$ED52` 2) | sound/`TwoLinePaletteFade` out |
| 18 | `$070596..$0705A7` | `RefreshPlaneFG`, `RefreshPlaneBG`, `DMAPlanes_VInt` | `ReloadMapChunks` |
| 19-20 | `$0705A8..$0705C3` | `Pal_SilenceTm`'s words 0-31 to `Palette_Table_Buffer_2` (`move.w #$1F, d7` / `trap #1`), `Pal_VariableFadeToPal2` (`$ED52` 2): the room's own colours back before the scene returns | `PaletteCopy`/`TwoLinePaletteFade` in |
| 21 | `$0705C4..$0705CB` | tower music `$9A` | sound |
| 22 | `$0705CC..$0705D5` | set `$D1`, return | flag |

The helpers after the body (`$0705D6..$07069D`) are shared with
[`Cutscene_LeRoofAgain`](52_LeRoofAgain.md): `loc_705D6`, its ramp
`loc_70626`, the colour cycle `loc_70638` (on frames whose
`Main_Frame_Count & 3` is zero, words 11 and 12 from the two eleven-word
series `loc_70672`, indexed by `$FFFFECF3`) and the series. None of the
loops runs map or object updates; the scene blocks for every pass. The
cycle's words and the fade-in are the runtime's CRAM model
([CRAM state](SCENE_PRESENTATION.md#cram-state)); the wait census rows are in
`rust/psiv-core/src/scenes/wait_census.rs`.

Against a retail capture (tape 28's boot schedule, Le Roof Room loaded by RAM
patch at frame 7000, the map trigger starting `$48`), the runtime's words 0-31
equal retail's on every frame of the 180, 71 and 120 passes. Two gaps remain:
the native shell draws no scene picture ([#594](https://github.com/TusanHomichi/PSIV/issues/594)),
and the scene reaches its first pass 8 counted frames early, which keeps the
cycle's phase only by coincidence ([#595](https://github.com/TusanHomichi/PSIV/issues/595)).
