# The Garuberk Tower's doors

Four events, two to open a door and two to go through one.

| Event | Routine | Retail bytes | Reached by | Data |
|---|---|---|---|---|
| `$0035` | `Event_GaruberkTwDoorOpening1` | `$06F43A..$06F4EF` | interaction area, `Interaction_EventIndexes` parameter 3 | `DOOR_OPENING_1` (19 ops) |
| `$0036` | `Event_GaruberkTwDoorOpening2` | `$06F4F0..$06F5A3` | interaction area, parameter 4 | `DOOR_OPENING_2` (19 ops) |
| `$0037` | `Event_GaruberkTwDoorEntered1` | `$06F5A4..$06F723` | `RunEvent_EnterGrbkTwDoor` (`RunEventsJmpTbl[$22]`) | `DOOR_ENTERED_1` (21 ops) |
| `$0038` | `Event_GaruberkTwDoorEntered2` | `$06F724..$06F8A3` | `RunEvent_EnterGrbkTwDoor` | `DOOR_ENTERED_2` (21 ops) |

- **Pointers:** `EventPtrs[$35..$38]` at `$05A2B4`.
- **Data:** `garuberk_events.rs`.
- **Maps:** `GaruberkTower` `$199` to `GaruberkTower_Part7` `$19F`. Every warp of the tower
  (but the way out to Dezolis) sits on a door, with a type-2 interaction area on its cell.

## Clone audit

The bodies were read from the US image through their `EventPtrs` slots; every instruction word
and the four tables match `ps4.asm:148638-148950`. No `revision` or `grand_cross` conditional
touches them.

## How a door works

A door is two layout chunks: the one under the leader's object (`GetMapLayoutOffset` of
`curr_x_pos >> 5`, `curr_y_pos >> 5`) and the one above it (`curr_y_pos - $20`). The party stands
below the door cell and speaks at it.

1. **Opening** (`$35`, `$36`). `cmpi.b #$38, (a1)` (`ps4.asm:148657`, `:148713`): the chunk under
   the leader must be the closed door, or the routine returns at once. The runtime's interaction
   probe makes that test before it starts the scene (`CLOSED_DOOR_GUARDS`,
   `rust/psiv-runtime/src/field_triggers.rs`, beside the elevator door's `$4F`). Then
   `SFXID_Fusion` and nine table rows; each writes the pair and runs four frames of
   `RefreshPlane`, `RunMapUpdates` and `DMAPlanes_VInt` (`d0 = 3`). `loc_6F4D4` ends on `$34`
   above and `$3C` under, `loc_6F588` on `$3E` and `$3C`. 36 frames.
2. **Entering.** With `$3C` under the leader the door cell above is open ground. Stepping onto it,
   `RunEvent_EnterGrbkTwDoor` (`ps4.asm:115755-115786`) reads `$3C` at `y + $10` and fires `$37`,
   or `$38` when the chunk at `y - $10` is `$3E` (a door opened by `$36`). `RunEvents` runs before
   `RunMapTransitions`, so the event, not the warp, takes the step.
3. **Entered** (`$37`, `$38`). `PalFadeOut_ClrSpriteTbl`, then `DoMapTransitionData` on the map's
   own transition table (`Map_Transition_Data_Addr`, which `RefreshMap` points at table 1) and
   `RefreshMap`: the warp the door stands on. On the arrival map the routine writes `$3C` under
   the leader and `$3E` (`$37`) or `$34` (`$38`) at `y - $10`, refreshes one frame and fades in;
   `SFXID_EnemyAttack3`, the leader's destination one cell down (`$3A(a4) = y + $10`) and the
   `Field_UpdateObjects` loop until `tst.w $2A(a4)` reads zero; `Event_OverlapCharacters`;
   `SFXID_EnemyAttack4` and five rows that shut the door again (`loc_6F714`, `loc_6F894`).

So a door opened by `$35` (a type-1 door, `$34` on top) is entered by `$37`, which leaves a
type-2 door open behind the party at the arrival, and the reverse; the interaction areas agree:
parameter 3 on one side of each warp and 4 on the other.

## Retail transcription

| Ops | Retail primitive | Scene record |
|---:|---|---|
| `$35`/`$36` 0 | `move.b #SFXID_Fusion, (Sound_Index).l` | `PlaySound` `$D9` |
| `$35`/`$36` 1-18 | nine rows `(3, above, under)`: `move.b d1, (a2)` / `move.b d2, (a1)`, four frames | `WriteActorMapChunks` leader `(0,-32)` and `(0,0)`, `Wait` 4 |
| `$37`/`$38` 0-1 | `PalFadeOut_ClrSpriteTbl` | `FadeOut`, `WaitFrames` 14 (the elevator ride's model) |
| `$37`/`$38` 2 | `DoMapTransitionData` / `RefreshMap` | `TakeMapTransition` |
| `$37`/`$38` 3-4 | `move.b #$3C, (a1)` / `move.b #$3E or #$34, (a2)`, one refreshed frame | `WriteActorMapChunks` `(0,0)` and `(0,-16)`, `WaitFrames` 1 |
| `$37`/`$38` 5-6 | `Pal_FadeIn` | `FadeIn`, `WaitFrames` 14 |
| `$37`/`$38` 7-9 | `SFXID_EnemyAttack3`, the walk loop, `Event_OverlapCharacters` | `PlaySound` `$D7`, `MoveActorOffset` leader `(0,16)`, `OverlapCharacters` |
| `$37`/`$38` 10-20 | `SFXID_EnemyAttack4`, five rows | `PlaySound` `$D8`, `WriteActorMapChunks` and `Wait` 4 |

The chunks the four tables write (`$30..$3E`) are carried in each tower map's scene chunk atlas
(`garuberk_door_chunks`, `psiv_tools/map_patches.py`), which `write_scene_map_chunks` needs for
their pixels and collision.

## In the route (C8)

`garuberk-tower` opens and walks through every door on the way to `GaruberkTower_Part7`.
