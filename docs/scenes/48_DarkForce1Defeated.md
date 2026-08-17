# `Cutscene_DarkForce1Defeated`

- **Retail bytes:** `$0771D2..$07734B` inclusive, 378 bytes.
- **Pointer:** `CutscenePtrs[$11]`; scene `$8011`.
- **Trigger:** `RunEventsJmpTbl[$32]`, Dark Force 1 `$83` set and Ice Digger
  `$89` clear.
- **Data:** `post_zio_cutscenes.rs`, `DARK_FORCE_1_DEFEATED` (27 ops).

## Clone audit

The retail pointer table's `grand_cross=0` entry at `ps4.asm:120781` selects
the body beginning at `Cutscene_DarkForce1Defeated`. Actual-ROM range is
`$0771D2..$07734C` exclusive end. The hack branch is not used.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-8 | `$0771D2..$07723A` | panel `$8E`, entry `7`, waits `60/60`, Takeoff music | panel/dialogue/wait |
| 9-16 | `$07723B..$0772CD` | clear temporary objects; world index 3; load Zelan F1 at `($3E,$20)` | map/music |
| 17-22 | `$0772CE..$07731E` | panels `$92/$93`, waits `60/120`, radar SFX and stop | presentation |
| 23-26 | `$07731F..$07734B` | dialogue entry `8`; remove Canceller `$9A`; add Ice Digger `$97`; set `$89`; return 0 | dialogue/inventory/flag |

The inventory mutation is persistent: the source calls `GetItem`, clears and
reorders Canceller, then writes Ice Digger into a free slot. The runtime scene
uses the existing inventory ops and the arc test asserts the resulting `$97`.

