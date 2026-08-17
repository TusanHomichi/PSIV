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
| 0-10 | `$078D30..$078DD8` | teleport/fade, art `$1DD566` tile `$20E`, five object clears, DMA | presentation/SFX |
| 11-15 | `$078DD9..$078E95` | stop all, wait `600`, Black Blood music, dialogue `$03` | wait/music/dialogue |
| 16-19 | `$078E96..$078F3D` | set `$E8`, map flags `$88`, final event battle `$1A`, return | flag/battle |

The scene stops at the battle request. The retail `$8021` ending body is a
separate final presentation surface and is recorded as an explicit boundary,
not silently represented as a fake return edge.

