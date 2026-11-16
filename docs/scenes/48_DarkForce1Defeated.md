# `Cutscene_DarkForce1Defeated`

- **Retail bytes:** `$0771D2..$07734B` inclusive, 378 bytes.
- **Pointer:** `CutscenePtrs[$11]`; scene `$8011`.
- **Trigger:** `RunEventsJmpTbl[$32]`, Dark Force 1 `$83` set and Ice Digger
  `$89` clear.
- **Data:** `post_zio_cutscenes.rs`, `DARK_FORCE_1_DEFEATED` (37 ops).

## Clone audit

The retail pointer table's `grand_cross=0` entry at `ps4.asm:120781` selects
the body beginning at `Cutscene_DarkForce1Defeated`. Actual-ROM range is
`$0771D2..$07734C` exclusive end. The hack branch is not used.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-8 | `$0771D2..$077213` | panel `$8E`, entry `7`, waits `60/60`, Takeoff music | panel/dialogue/wait |
| 9-22 | `$077214..$077279` | clear temporary objects; world index 3; call `loc_64C1E` (planet screen with its fade-in/out); load Zelan F1 at `($3E,$20)`; init/fade again | map/music |
| 23-31 | `$07727A..$0772CF` | panels `$92/$93`, waits `60/120`, radar SFX and stop | presentation |
| 32-36 | `$0772D0..$0772FF` | dialogue entry `8`; remove Canceller `$9A`; add Ice Digger `$97`; set `$89`; return 0 | dialogue/inventory/flag |

The direct calls are `Init, In, Init, Init, In`. Between the second and
third Init, `$07722A` calls the shared `loc_64C1E`: world 3 bypasses its
Rykros branch, prepares the planet screen, fades in at `$064C8E`, holds
300 frames and fades out at `$064CFA`. The executed order is therefore
`Init, In, Init, In, Out, Init, In`. The caller's range stays its own; the
fixed-fade census attributes these two helper calls at the call boundary.
The scene uses the same planet-screen ops as the shared ship route.

The inventory mutation is persistent: the source calls `GetItem`, clears and
reorders Canceller, then writes Ice Digger into a free slot. The runtime scene
uses the existing inventory ops and the arc test asserts the resulting `$97`.


## In the route
`RunEvent_DarkForce1Defeated` (`$32`) is flags-only (`$83` set, `$89` clear), so
it fires on the first field frame after the battle's victory page. The scene's
`LoadMap` leaves the party on **Zelan F1 `$18E` at (31,17)** with `$89` set, the
Canceller gone and the Ice Digger (`$97`) in the pack; the `kuran-dark-force-1`
chapter's closing asserts exactly that, and `kuran_arc.rs`
(`dark_force_1_defeated_takes_the_party_to_zelan_and_swaps_the_items`) checks the
items from a hand-built save, with the unfought battle as the negative control.
