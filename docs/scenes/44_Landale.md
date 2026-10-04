# `Cutscene_Landale`

- **Retail bytes:** `$077150..$0771D1` inclusive, 130 bytes.
- **Pointer:** `CutscenePtrs[$10]` at `$05A580`; scene event `$8010`.
- **Trigger:** `RunEventsJmpTbl[$2B]`, Tyler Grave `$84` set and Dezo Spaceport
  `$82` clear, leader in the trigger rectangle.
- **Data:** `post_zio_cutscenes.rs`, `LANDALE` (24 ops).

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
| 11-15 | `$0771A6..$0771D1` plus `Event_DezoSpaceportAppearing` (`ps4.asm:145470-145496`) | load three spaceport assets; camera to (`$C0`,`$480`); grave-opening sound | presentation/camera |
| 16-18 | `ps4.asm:145497-145541` (`loc_6C0A2` loop, `loc_6C148`) | the `d7 = $170 .. 0` loop, 369 passes; `loc_6C148` writes BG chunk (6,36) `<- $2C` after the pass with `d7 == $3D`, at `$06C15C` | `Wait` 308, `WriteMapChunks` `(6,36) <- $2C`, `Wait` 61 |
| 19-23 | `ps4.asm:145503-145511`, `loc_6C0FA` (`:145512-145529`) | objects `$78/$84` (retail spawns them at pass `$F8`, inside the loop: the scene keeps them after it, presentation only); camera back to the leader; set `$82`; return | presentation/camera/flag |

`Event_DezoSpaceportAppearing` is a retail EventPtrs[$11] helper called by the
cutscene, not a Grand Cross scene body. Its event flag is kept in the same
scene record because the call is part of this byte range's observable handoff.

## The spaceport chunk is a live write (C6, 2026-10-04)

The first transcription kept the 369 passes as one `Wait` and dropped the
chunk write `loc_6C148` makes, so the live Dezolis map kept open ground where
the spaceport door stands. The cells are collision type 0 in the pack's base
layout; the chunk `$2C` is what makes them map-change (type 1), and the same
chunk is written by the map's load hook `loc_53E98` (`ps4.asm:111635-111646`,
gated on `$82`), which is why a save loaded after the scene walked into the
door and the party that played the scene live did not. The route halted on it
as a stuck walk at (12,73) in the first full run (`RUNNER_LOG_KURAN.md`, H33).

- The write resolves through the pack's `overworld_patches` entry for flag `$82`
  (chunk (6,36), `collision_chunk_id $2C`, four map-change cells), so no atlas
  rebuild was needed. `psiv_tools.map_patches.SCENE_CHUNK_WRITES` has the row
  (`via="overworld_patches"`, bytes pinned at `$06C15C`).
- Tests: `rust/psiv-runtime/src/scene_landale_tests.rs`. The four door cells read
  map-change the moment the scene ends and the scene wrote exactly one chunk;
  the negative control removes the op and the first test fails on the missing
  write.

## The trigger sits on a warp (C6)

`RunEvent_FindingLandale` tests `curr_y_pos == $520` and `$1A0 <= curr_x_pos <= $1B0`
(`ps4.asm:115866-115882`). A standing cell's y byte is its row less one, so
`$520` is row 83, the first row of the Hangar's warp 0 (cells (26..27, 83) to
Dezolis). On foot `FieldRoutine_Controls` runs `RunEvents` before
`RunMapTransitions` (`ps4.asm:116768-116773`), and a scene that starts ends the
frame: the trigger wins. The runtime evaluated the map's trigger list only after
a warp that did not fire, so the warp won; `field_tick.rs` now scans the list
before it takes a warp (`a_trigger_on_a_warp_cell_wins_over_the_warp`, with the
shut-gate step that takes the warp as the negative control). The route's
`step_onto` objective is the planner's side of the same fact: `go_to` plans a
warp footprint as a terminal, never as a goal.
