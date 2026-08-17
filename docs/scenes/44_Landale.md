# `Cutscene_Landale`

- **Retail bytes:** `$077150..$0771D1` inclusive, 130 bytes.
- **Pointer:** `CutscenePtrs[$10]` at `$05A580`; scene event `$8010`.
- **Trigger:** `RunEventsJmpTbl[$2B]`, Tyler Grave `$84` set and Dezo Spaceport
  `$82` clear, leader in the trigger rectangle.
- **Data:** `post_zio_cutscenes.rs`, `LANDALE` (22 ops).

## Clone audit

`ps4.asm:120780` selects `Cutscene_Landale` in the retail pointer table. The
body at the corresponding retail cutscene block is used; the Grand Cross
pointer/include path is excluded. Actual-ROM range: `$077150..$0771D2`
(exclusive end).

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-6 | `$077150..$077183` | panel `$8B`, `30`-frame wait, dialogue entry `$24` | panel/wait/dialogue |
| 7-10 | `$077184..$0771A5` | load Dezolis from Hangar `$15F` at `($12,$92)`, save Dezo field music | map/music |
| 11-21 | `$0771A6..$0771D1` plus `Event_DezoSpaceportAppearing` | load three spaceport assets; camera; `369` update iterations; objects `$78/$84`; set `$82` | presentation/camera/flag |

`Event_DezoSpaceportAppearing` is a retail EventPtrs[$11] helper called by the
cutscene, not a Grand Cross scene body. Its event flag is kept in the same
scene record because the call is part of this byte range's observable handoff.

