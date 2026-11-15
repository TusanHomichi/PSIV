# `Event_EclipseTorchUsed`

- **Retail bytes:** `$07018A..$070481` inclusive, 760 bytes.
- **Pointer:** `EventPtrs[$47]`; event `$0047`.
- **Trigger:** `RunEventsJmpTbl[$36]`, Lashiec `$9B` set and Eclipse Torch
  `$9C` clear, position `x=$B80..$BB0`, `y<=$E0`.
- **Data:** `dezo_campaign.rs`, `ECLIPSE_TORCH_USED` (43 ops).

## Clone audit

The retail `grand_cross=0` EventPtrs body is used. The clone's Grand Cross
alternative is excluded (`ps4.options.asm:10`). ROM range:
`$07018A..$070482` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-8 | `$07018A..$070225` | `beq.w loc_70226` (`$07018E`): on foot to op 9; mounted: save Dezolis music `$99`, stop SPC, a bare frame (`$0701A0`), play `$99` (`$0701A6`), rebuild sprites and a bare frame (`$0701B4`), dismount, `Field_UpdateObjects` and a frame (`$070220`) | vehicle/presentation/`WaitFrames` |
| 9-11 | `$070226..$07023D` | panel `$10D`, DMA, `DoMapUpdateLoop($77)` (`$070238`): 120 frames of map updates | panel/`Wait` |
| 12-17 | `$07023E..$07027F` | Deban; `Palette_Table_Buffer` saved to `Palette_Table_Buffer_2` (`move.w #$3F, d7` / `trap #1`); thirty-two longwords of `$0EEE0EEE` (`loc_7025A`); `DoMapUpdateLoop(1)` (`$070266`), two frames of map updates; forty passes of `loc_58666` / `VInt_Prepare` (`$070270`), no update | SFX/`PaletteCopy`/`Wait`/`PaletteRestoreTone` + `WaitFrames` |
| 18-23 | `$070280..$0702C1` | the second flash (`if revision>0`): the same bytes, waits `$0702A8` and `$0702B2` | as 12-17 |
| 24-26 | `$0702C2..$0702E3` | `Panel_Destroy`, `DMAPlanes_VInt`, art `$1D712C` at tile `$3AF` | panel/art |
| 27-35 | `$0702E4..$07041D` | nine `$338` objects at `($BA0,$C0)`, `($B80..$BC0,$A0)`, `($B60..$BE0,$80)`, no frame between them | `CreateFieldObject` |
| 36-38 | `$07041E..$070451` | chunk copy `$6100` to `$6960`, `Field_UpdateObjects`, `RefreshPlaneBG` (not transcribed, see below), `DMAPlane_B_VInt`'s frame (`$07043C`); EnemyKilled `$B9` (`$070442`); `DoMainUpdatesLoop($4F)` (`$07044C`), 80 frames | `WaitFrames`/SFX/`Wait` |
| 39-42 | `$070452..$070481` | tree `$1F9580`, dialogue `$2A`, chunk copy again (not transcribed), set `$9C`, return | dialogue/flag |

The frame at `$070220` runs `Field_UpdateObjects`; the port's is a bare frame
(`docs/source-notes/README.md`, "Dezolis dismount frames").

The flashes run through the runtime's CRAM model: the save, the fill and
the restore are the `PaletteCopy` and `PaletteRestoreTone` records
`Cutscene_Elsydeon` uses, and the renderer draws the CRAM they leave. What
the port does not draw or run of `$0702E4..$070481`, and why, is in
`docs/source-notes/README.md`, "Eclipse Torch".
