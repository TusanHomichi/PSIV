# `Cutscene_ProfoundDarkness`

- **Retail bytes:** `$078D30..$078F3D` inclusive, 526 bytes.
- **Pointer:** `CutscenePtrs[$20]`; scene `$8020`.
- **Trigger:** `RunEventsJmpTbl[$53]`, Profound Darkness `$E8` clear at
  `y=$160`.
- **Data:** `dezo_endgame.rs`, `PROFOUND_DARKNESS` (22 ops).

## Clone audit

The retail `grand_cross=0` cutscene body is selected. The Grand Cross body is
excluded. ROM range: `$078D30..$078F3E` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-9 | `$078D30..$078E45` | teleport SFX, `Pal_VariableFadeOut` (`$ED52` 1, `$078D3E`), `ClearPlaneABuf` (`$078D44`), art `$1DD566` tile `$20E`, five object clears, DMA | SFX/`VariablePaletteFade` out/`ClearPlanes`/presentation |
| 10-12 | `$078E46..$078E69` | `Palette_Table_Buffer_2`'s 64 words back to the buffer (`moveq #$3F, d7` / `trap #1`), teleport SFX, `Pal_VariableFadeIn` (`$ED52` 7, `$078E64`) | `PaletteCopy`/SFX/`VariablePaletteFade` in |
| 13-17 | `$078E6A..$078E95` | stop all, wait `600`, Black Blood music, dialogue `$03` | wait/music/dialogue |
| 18-21 | `$078E96..$078F3D` | set `$E8`, map flags `$88`, final event battle `$1A`, return | flag/battle |

The cartridge calls no `InitVRAMAndCRAM` here: `ClearPlaneABuf` clears plane
A's buffer, and the fade-in returns to the palette the fade-out saved. The
native shell hides the whole map at `ClearPlanes` and draws neither the
`$1DE03A` picture nor the party's five fixed positions.

The scene stops at the battle request. The retail `$8021` ending body is a
separate final presentation surface and is recorded as an explicit boundary,
not silently represented as a fake return edge.

The dialogue call is `Event_GetAndRunDialogue2` (`$5ACDC`), the retained window
(`retained(3)`): see [the Dialogue2 audit](DIALOGUE_CALLERS.md#the-dialogue2-audit).
