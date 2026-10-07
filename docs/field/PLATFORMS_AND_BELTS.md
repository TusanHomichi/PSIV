# Moving platforms, terminals and conveyor belts

Twelve events of Vahal Fort and the Weapon Plant (issue #82) are field
mechanics: a trigger or an interaction runs an event routine whose body is a
frame loop over the **party objects**. They are scenes in the registry (the
cartridge dispatches them through `Event_Index` like any event) and each moves
the party the way the loop does, not as scripted cast: [`SceneOp::RidePlatform`]
adds a step to every party object per frame, and [`SceneOp::ConveyorRide`] reads
the live collision-plane layout to decide where the carry ends.

| Event | Routine | Retail bytes | Fires from |
|---:|---|---|---|
| `$15` | `Event_VahFortMovingPlatform1` | `$06C478..$06C607` | trigger `$0E`, Vahal Fort F2 |
| `$16` | `Event_VahFortMovingPlatform2` | `$06C608..$06C797` | trigger `$0E`, Vahal Fort F2 |
| `$17` | `Event_WpnPlntMovingPlatform1` | `$06C798..$06C927` | trigger `$0F`, Weapon Plant F1 |
| `$18` | `Event_WpnPlntMovingPlatform2` | `$06C928..$06CAB7` | trigger `$0F`, Weapon Plant F1 |
| `$19` | `Event_WpnPlntMovingPlatform3` | `$06CAB8..$06CC47` | trigger `$0F`, Weapon Plant F1 |
| `$1A` | `Event_WpnPlntMovingPlatform4` | `$06CC48..$06CDD3` | trigger `$0F`, Weapon Plant F1 |
| `$1B` | `Event_VahalFortTerminal` | `$06CDD4..$06CE17` | interaction area, Vahal Fort F2 |
| `$1C` | `Event_WeaponPlantTerminal` | `$06CE18..$06CE5B` | interaction area, Weapon Plant F3 |
| `$1D` | `Event_ConveyorBeltDown` | `$06CE5C..$06CFA3` | triggers `$10`, `$11` |
| `$1E` | `Event_ConveyorBeltUp` | `$06CFA4..$06D0EB` | triggers `$10`, `$11` |
| `$1F` | `Event_ConveyorBeltRight` | `$06D0EC..$06D233` | triggers `$10`, `$11` |
| `$20` | `Event_ConveyorBeltLeft` | `$06D234..$06D37B` | triggers `$10`, `$11` |

The trigger side (which event a landing selects, from the party's pixel
position and the temp flags) was already decoded: `trigger_custom.rs`,
`RunEvent_VahFortMovingPlatform` (`ps4.asm:115267`) and its three siblings. The
retail bytes of every event were read from the US image through `EventPtrs`
(`$05A2B4`); the clone's bodies (`ps4.asm:145718..146700`) agree instruction for
instruction and are corroboration, not the source.
`tests/test_vahal_events.py` re-derives every literal below from the image and
compares it with the scene source.

## The platforms

All six routines have one shape; only the temp flag and the two tables differ.
`lea table(pc), a3` / `moveq #flag, d0` / `TempEveFlags_Test` (`$57638`)
selects row 0 (flag clear) or row 1 (set) of two tables:

- the **start table** (6 bytes a row): platform `x`, `y`, and the two chunk ids
  the routine writes at the start, `move.b $4(a3), (a1)` and `$5(a3), $1(a1)`;
- the **ride table** (8 bytes a row): the step word, `x`, the destination `y`,
  and the two chunk ids written at the end (`$6(a3)`, `$7(a3)`).

The body then runs, in order:

1. `Field_LoadObject` (`$44D82`) takes the first free object slot at or after
   `$FFFFC300`, `move.w #$124, (a4)` names it (object `$124`, the platform), and
   its `curr_x_pos`/`curr_y_pos` become the start row's `x` and `y - $20`.
2. `Event_OverlapCharacters`: the party stacks on the leader.
3. `GetMapLayoutOffset` on the foreground plane (`d3 = 0`, which is these maps'
   collision plane) at `(x >> 5, y >> 5)`: the two start chunk ids are written at
   that chunk and the next. Then one `RefreshPlaneFG`, `RunMapUpdates` and
   `DMAPlane_A_VInt`.
4. Sixty `VInt_Prepare` frames, then `SFXID_ConveyorBelt` (`$E7`).
5. The ride loop (`loc_6C50E` and siblings). The ride row's step word `d0` is
   sign-extended and shifted left eight, a signed 16.16 step: `$0200` is **two
   pixels a frame**. Each frame adds it to the platform object's `curr_y_pos`
   (`$34`), to `curr_y_pos` and `dest_y_pos` (`$34`, `$3A`) of all five party
   objects, and to the camera step counters (the BG counter by `d0 - d0/4`);
   then `Field_UpdateObjects`, `RunMapUpdates`, `VInt_Prepare`. The loop ends
   when the platform's `curr_y_pos` equals the destination `- $20`.
6. `TempEveFlags_Toggle` flips the flag; the ride row's chunk pair is written at
   `(x >> 5, dest_y >> 5)`; `clr.w (a4)` removes the platform object; the loop
   sound stops (`Sound_StopSFX`, `$FC`).

### The rows

Chunk coordinates are `(x >> 5, y >> 5)`; the pair is that chunk and the next one
to the right. Flag clear rides first:

| Event | Flag | Row | Start (x, y) and pair | Ride | Frames | End pair |
|---:|---|---|---|---|---:|---|
| `$15` | temp `$09` | clear | `($2C0,$220)` `$C2,$C3` at (22,17) | down (`$0200`) to `$2C0` | 80 | `$CC,$CD` at (22,22) |
| | | set | `($2C0,$2C0)` `$C0,$C1` at (22,22) | up (`$FE00`) to `$220` | 80 | `$CC,$CD` at (22,17) |
| `$16` | temp `$0A` | clear | `($2C0,$3A0)` `$C0,$C1` at (22,29) | up to `$300` | 80 | `$CC,$CD` at (22,24) |
| | | set | `($2C0,$300)` `$C0,$C1` at (22,24) | down to `$3A0` | 80 | `$CC,$CD` at (22,29) |
| `$17` | temp `$0D` | clear | `($1C0,$2A0)` `$C2,$C3` at (14,21) | down to `$380` | 112 | `$CC,$CD` at (14,28) |
| | | set | `($1C0,$380)` `$C0,$C1` at (14,28) | up to `$2A0` | 112 | `$CC,$CD` at (14,21) |
| `$18` | temp `$0E` | clear | `($220,$160)` `$C2,$C3` at (17,11) | down to `$240` | 112 | `$CC,$CD` at (17,18) |
| | | set | `($220,$240)` `$C0,$C1` at (17,18) | up to `$160` | 112 | `$CC,$CD` at (17,11) |
| `$19` | temp `$0F` | clear | `($380,$160)` `$C2,$C3` at (28,11) | down to `$240` | 112 | `$CC,$CD` at (28,18) |
| | | set | `($380,$240)` `$C0,$C1` at (28,18) | up to `$160` | 112 | `$CC,$CD` at (28,11) |
| `$1A` | temp `$10` | clear | `($3E0,$2A0)` `$C2,$C3` at (31,21) | down to `$380` | 112 | `$CC,$CD` at (31,28) |
| | | set | `($3E0,$380)` `$C0,$C1` at (31,28) | up to `$2A0` | 112 | `$CC,$CD` at (31,21) |

Tables: `$06C5EC`/`$06C5F8`, `$06C77C`/`$06C788`, `$06C90C`/`$06C918`,
`$06CA9C`/`$06CAA8`, `$06CC2C`/`$06CC38`, `$06CDB8`/`$06CDC4` (start/ride). The
frame count is `|destination - start| / 2`; the draft of this transcription had
platforms `$18` and `$19` at 64 frames from a mis-subtraction, and
`tests/test_vahal_events.py` is what pins the 112.

The flag is the platform's *position*: set means "moved down" for platform 1 and
`$17..$1A`, "moved up" for platform 2 (`ps4.constants.asm:1826-1833`). The map's
own load effect (`MapDataManager`, `$05219A` and siblings, gated on the same temp
flags: the pack labels that bank `chest_flags`, and the runtime reads it as
`Flag::temp`) writes `$CC,$CD` at the platform's resting chunk for the flag's
state, so a reload agrees with the live writes. The runtime test reloads a
post-ride state and compares.

### Why the ride does not fire again

`RunEvent_VahFortMovingPlatform` opens with `Saved_Tile_Collision_Standing == 2
-> no event`. `$CC/$CD` have collision type 2 on all four cells (the atlas carries
it), so a leader who steps from one platform cell onto another, or arrives by
riding, has a previous cell of type 2 and the probe stays quiet. Stepping off and
on again fires it, riding the other way (the flag has flipped). The vacated
chunks `$C0..$C3` are solid (type 8).

### Port model

Per scene (22 ops, two halves around the flag test at op 0):

| Retail | Op |
|---|---|
| flag test, row select | `BranchFlag` on the temp flag |
| `Field_LoadObject`, `$124`, position | `CreateFieldObject {slot, object_id: 0x124, x, y - $20}`; the slot is the first free one: Vahal Fort F2 has three map objects (slot 3), Weapon Plant F1 five (slot 5) |
| `Event_OverlapCharacters` | `OverlapCharacters` |
| start `GetMapLayoutOffset` writes | `WriteMapChunks` (literal chunk coordinates) |
| 60-frame settle | `WaitFrames 60` |
| `SFXID_ConveyorBelt` | `PlaySound $E7` |
| ride loop | `RidePlatform {slot, step_y, frames}` |
| `TempEveFlags_Toggle` | `ToggleFlag` |
| end writes | `WriteMapChunks` |
| `clr.w (a4)` | `DespawnNpc {slot, 1}` |
| `Sound_StopSFX` | `PlaySound $FC` |

`RidePlatform` moves every party actor's pixel position by the step each tick
(a whole-pixel slide that rolls over into the cell), emits the platform's own
motion to the renderer as a `StepFieldObject`, blocks for the frame count with
map updates running (the retail loop calls `RunMapUpdates` each frame), and
faults if `step * frames` is not a whole number of 16-pixel cells. The scene
camera follows the leader, which is how the cartridge's camera step counters
read the same step.

## The terminals

`Event_VahalFortTerminal` and `Event_WeaponPlantTerminal` are the small
terminals that change the belts' direction. Each plays `SFXID_Alarm` (`$DB`) and
`Pal_VariableFadeToRed` with `$ED52 = 3` (eight stages of four frames), plays the
alarm again and fades back (`Pal_VariableFadeFromRed`), then compares the
leader's `curr_x_pos` (`cmpi.w`, `bcs`) and toggles one of two temp flags:

| Event | Compare | Leader at or above | Leader below |
|---|---|---|---|
| `$1B` | `cmpi.w #$1F0, $30(a4)` | temp `$0B` | temp `$0C` |
| `$1C` | `cmpi.w #$300, $30(a4)` | temp `$12` | temp `$11` |

(`Event_VahalFortTerminal`: `$06CE00..$06CE12`; the second `TempEveFlags_Toggle`
is a tail `jmp`.) The flags are the belts' selectors in
`RunEvent_VahFortConveyorBelt` (`$0B`, `$0C`) and
`RunEvent_WpnPlntConveyorBelt` (`$11`, `$12`): using a terminal swaps which
event the swapped probe groups fire; using it again swaps them back. Scene:
`PlaySound`, `FadeToRed`, `PlaySound`, `FadeFromRed`, `BranchIfActorCoord`,
`ToggleFlag`.

The interaction areas are map type-2 areas with story flag 0, so a terminal
never retires: Vahal Fort F2 at cells (38,60), (64,44) and (28,44), Weapon Plant
F3 at (32,22) and (60,22).

## The conveyor belts

Each of the four routines is one loop with one direction and one chunk range:

```text
clr.b FieldObj_Step_Offset            ; 1 pixel a frame, 16 frames a cell
clear the five x/y step constants
save each member's status byte; set status bit 2 on the leader
SFXID_ConveyorBelt
loop:   set status bit 2 on every follower standing on a belt chunk
        if the leader's x/y step durations are zero: dest = curr +/- $10
        Field_UpdateObjects, RunMapUpdates, camera, DMAPlane_A_VInt
        GetChunkAndCollision for the leader; stay while lo <= chunk <= hi
Event_OverlapCharacters; restore the status bytes
FieldObj_Step_Offset = 1; Sound_StopSFX
```

| Event | Direction (destination write) | Chunk range |
|---:|---|---|
| `$1D` | `curr_y_pos + $10` (down) | `$A8..$AB` |
| `$1E` | `curr_y_pos + $FFF0` (up) | `$A8..$AB` |
| `$1F` | `curr_x_pos + $10` (right) | `$AC..$AF` |
| `$20` | `curr_x_pos + $FFF0` (left) | `$AC..$AF` |

The four-id ranges are the belts' animation frames
(`MapUpdate_VahFortConveyorBelts`, `$054F20`, rewrites a belt's chunk ids in
turn), so the loop keeps going whichever frame is showing.
`GetChunkAndCollision` (`$45A52`) reads the chunk at `(curr_x_pos, curr_y_pos +
$10)` plus the offsets in `d0`/`d1` (zero here) from the live layout, so a
patched chunk counts.

**Where the carry ends.** The loop tests the chunk after every frame, so it stops
at the first pixel offset `t >= 1` along the belt whose chunk is outside the
range, and the step in flight finishes: the leader ends `ceil(t / 16)` cells
along. Taking the standing-cell shift into account, that is always **the first
cell outside the belt in the belt's direction**, from whichever half of a chunk
the leader started on (a down belt stops after the step that *arrives* in the
next chunk, an up belt after the first pixel that leaves the chunk), and at
least one cell, because the destination is set before the first test. The Rust
test derives the ends on paper for both halves and all four directions.

`SceneOp::ConveyorRide` scans the layout once, when it starts, for that `t` (the
retail test is per pixel, so the scan is per pixel), then walks the leader the
resulting number of cells at 16 frames a cell; the followers trail through the
runner's ordinary follow chain, and `OverlapCharacters` after it collapses them
onto the leader exactly as the routine does. The layout is read through
`SceneRunner::tick_with`; a runner ticked without one, or a belt that never
ends, faults with `SceneFault::NoLayout` instead of moving anyone.

## What is not modelled

- **The dead-status trick.** The belts set status bit 2 (the *dead* bit) in the
  party members' records so `FieldObj_UpdateStatus` freezes their walk
  animation while they are carried, and restore every byte afterwards. Nothing
  reads the bit but the sprite code, so the port leaves the roster alone and
  the carried party shows its ordinary walk animation. Cosmetic.
- **The platform's sprite.** Object `$124` has no map placement and the pack's
  sprite catalogue has no sheet for it (`sprites/npcs.json`, object type 73,
  `placements: 0`), so the renderer receives the platform's motion as a
  `StepFieldObject` it cannot draw. The collision, chunk and party state are
  exact; the moving platform picture during a ride is not drawn.
- **`MapUpdate_VahFortConveyorBelts` / `MapUpdate_WpnPlntConveyorBelts`**
  (`$054F20`, `$055094`): the belts' *animation* is still the missing-input
  program recorded in [MAP_UPDATES](MAP_UPDATES.md); the belts carry correctly
  on their static chunk ids.
- **`Event_OverlapCharacters` is a frame loop** (`$05A88A`) that runs until the
  followers converge; the port's op is instant, as everywhere else it is used.
- **The single `RunMapUpdates` call** before the 60-frame settle and the camera's
  BG parallax step (`d0 - d0/4`) beyond what the scene camera follows.

## Verification

- `rust/psiv-core/src/scene_runner/mechanics_tests.rs`: all six platforms in both
  flag states (chunk pairs in order, rigid party, flag flip, platform object
  step and despawn), the ride's per-frame slide and cell roll-over, the
  non-cell distance fault; ten belt cases from both halves of a chunk in four
  directions, the one-step floor, and the no-layout and endless-belt faults;
  both terminals on both sides of their threshold, and toggling twice.
- `rust/psiv-runtime/src/scene_vahal_tests.rs`, on the real maps through the
  ordinary landing trigger: platform 1 down and back (live chunk writes,
  collision, no re-fire, a reload agreeing), platform 2 rising, the four Weapon
  Plant platforms, seven belt cases (including the terminal-flag swap and Weapon
  Plant F3's trigger `$11`), a walk along a belt column outside its entry boxes
  starting nothing, and all five terminals.
- `rust/psiv-runtime/tests/session_vahal.rs`: the platform ridden with held pad
  presses through `Session::frame`.
- `tests/test_vahal_events.py` and `tests/test_scene_chunk_atlas.py` (the six
  `SCENE_CHUNK_WRITES` rows that put `$C0..$C3` in the two maps' atlases).

These are state tests from constructed saves: **the campaign route does not
reach Vahal Fort**, so nothing here is route evidence, and the route lane that
reaches the fort runs the mechanics for real.
