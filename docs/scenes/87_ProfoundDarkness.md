# `Cutscene_ProfoundDarkness`

- **Retail bytes:** `$078D30..$078F3D` inclusive, 526 bytes.
- **Pointer:** `CutscenePtrs[$20]`; scene `$8020`.
- **Trigger:** `RunEventsJmpTbl[$53]`, Profound Darkness `$E8` clear at
  `y=$160`.
- **Data:** `dezo_endgame.rs`, `PROFOUND_DARKNESS` (20 ops).

## Clone audit

The retail `grand_cross=0` cutscene body is selected. The Grand Cross body is
excluded. ROM range: `$078D30..$078F3E` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-7 | `$078D30..$078E45` | teleport SFX, `Pal_VariableFadeOut` (`$ED52` 1, `$078D3E`), `ClearPlaneABuf` (`$078D44`), zero FG camera (`$078D4A`), art `$1DD566` tile `$20E`, Enigma picture `$1DE03A` (`$620E`) to `$FFFF8198` (`(96, 24)`), `Character_1..5` V-formation facing Up (`$078D8C..$078E36`), DMA | SFX/`VariablePaletteFade` out/`ClearPlanes`/`SetCameraPos`/`LoadArt`/`LoadSceneAsset`/`PlacePartyFormation`/`DmaPlanes` |
| 8-10 | `$078E46..$078E69` | `Palette_Table_Buffer_2`'s 64 words back to the buffer (`moveq #$3F, d7` / `trap #1`), teleport SFX, `Pal_VariableFadeIn` (`$ED52` 7, `$078E64`) | `PaletteCopy`/SFX/`VariablePaletteFade` in |
| 11-15 | `$078E6A..$078E95` | stop all, wait `600`, Black Blood music, dialogue `$03` | wait/music/dialogue |
| 16-19 | `$078E96..$078F3D` | set `$E8`, map flags `$88`, final event battle `$1A`, return | flag/battle |

The cartridge calls no `InitVRAMAndCRAM` here: `ClearPlaneABuf` clears plane
A's buffer, and the fade-in returns to the palette the fade-out saved. The
native shell hides the field map at `ClearPlanes` (plane B included), draws
the decoded `$1DE03A` picture at `(96, 24)` on CRAM line 3, and places the
party in its five-slot V-formation facing Up.

The scene stops at the battle request. The retail `$8021` ending body is a
separate final presentation surface and is recorded as an explicit boundary,
not silently represented as a fake return edge.

The dialogue call is `Event_GetAndRunDialogue2` (`$5ACDC`), the retained window
(`retained(3)`): see [the Dialogue2 audit](DIALOGUE_CALLERS.md#the-dialogue2-audit).
