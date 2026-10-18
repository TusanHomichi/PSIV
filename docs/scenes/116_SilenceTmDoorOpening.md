# `Event_SilenceTmDoorOpening`

- **Retail bytes:** `$06FBEE..$06FC75` inclusive, 136 bytes (the routine to the `rts` at `$06FC5A`, then its table `loc_6FC5C`).
- **Pointer:** `EventPtrs[$42]` at `$05A2B4`; event `$0042`.
- **Trigger:** LeRoofRoom `$F0`'s one interaction area: rect (31..32,19), type 2
  (`Interaction_GetEvent`), temp flag `$19`, parameter 7, and
  `Interaction_EventIndexes[7]` is `$42`. Speaking at the closed door of Le
  Roof's ship runs it; the map's effect entry `$47` clears temp `$19` on every
  load, so the door is shut again whenever the room reloads.
- **Data:** `dezo_endgame.rs`, `SILENCE_TM_DOOR_OPENING` (11 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot and every
instruction word matches `ps4.asm:149200-149234`; the table bytes match the
clone's `loc_6FC5C` rows (`:149236-149238`). No `revision` conditional, no
dialogue.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FBEE..$06FBF5` | `move.b #SFXID_DoorOpened ($E2), Sound_Index` | `PlaySound` `$E2` |
| — | `$06FBF6..$06FC21` | `GetMapLayoutOffset(d1=$F, d2=9, d3=1)` (BG; `lea (a1), a2`), then `(d1=$F, d2=8, d3=1)` into `a1`; `lea loc_6FC5C, a0` | the four chunk addresses below |
| 1-10 | `$06FC22..$06FC58` | per row: `move.b (a0)+` the delay, four chunk ids into `(a1)`, `$1(a1)`, `(a2)`, `$1(a2)`; `jsr RefreshPlaneBG`; `jsr DMAPlane_B_VInt` (`$0416A6`) `delay + 1` times; until `$FF` | `WriteMapChunks` BG `(15,8)`, `(16,8)`, `(15,9)`, `(16,9)`, then `WaitFrames` 7, five times |

`GetMapLayoutOffset` (`$053514`) with `d3 = 1` takes the BG layout at
`$FFFFB000`, row stride `Map_BG_Width + 1`: `a1 = $B000 + d2 * stride + d1`.

| Row | Delay | `(15,8)` | `(16,8)` | `(15,9)` | `(16,9)` |
|---:|---:|---|---|---|---|
| 1 | 6 | `$54` | `$55` | `$5E` | `$5F` |
| 2 | 6 | `$56` | `$57` | `$60` | `$61` |
| 3 | 6 | `$58` | `$59` | `$62` | `$63` |
| 4 | 6 | `$5A` | `$5B` | `$64` | `$65` |
| 5 | 6 | `$5C` | `$5D` | `$66` | `$67` |

Chunks are 32 pixels, so the door is cells (30..33,16..19). Collision is read
from the layout, so the last row opens the boarding row the same frame:
`RunEvent_SilenceTmEnterSpaceship` (`RunEventsJmpTbl[$25]`, leader Y `$120`,
row 19) then starts `Cutscene_InsideSpaceship` and the destination menu. That
is how the party leaves Rykros once Le Roof's flight
([52](52_LeRoofAgain.md)) is over.

The chunk ids are the pack's: `psiv_tools.map_patches.SCENE_CHUNK_WRITES` has
the row for map `$0F0` with `$54..$67` and pins the writes at `$06FC34` and
the table at `$06FC5C`, so a moved table fails the pack build
([live-layout census](LIVE_LAYOUT_WRITES.md)).

## Tests

`rust/psiv-core/tests/late_dezolis_scenes.rs`,
`the_silence_temple_door_steps_open_by_retails_table`: the sound, the five
rows' chunk writes and seven-frame waits, read from the table bytes typed as a
literal, and no flag or dialogue. The census's negative control covers its
registration.
