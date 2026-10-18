# Walking and collision

How the party moves on the field and what stops it, measured on the cartridge
with the [oracle](../../oracle/README.md); frame numbers cite marks in the
named tape's log, which `oracle/verify.sh` regenerates. The camera that
follows the party is [CAMERA.md](CAMERA.md); townsfolk movement is
[NPC_WANDER.md](NPC_WANDER.md).

## Walk timing

**Exactly 8.00 frames per 16px cell, all four directions** (`logs/02_walk_timing.csv`).
Position advances 2 px/frame; `x_step_duration` / `y_step_duration` count down
from `$1000` to 0 in steps of `$0200`, which is 8 states. 

Facing values confirmed as `constants:107` states: 0 DOWN, 4 UP, 8 RIGHT,
`$C` LEFT.

**Any direction press commits a full 16px step — there is no turn-in-place.**
A 1-frame tap moves a whole cell (`walk_up` leg; a 1-frame LEFT tap at f7185
had already moved 2px on the press frame and completed the cell 8 frames
later). A character only turns without moving when the target cell is blocked.
This is a real constraint on scene authoring and on any "face this way" op.

## Collision

Blocked presses turn the character to face the obstacle and hold position.
Terrain blocking shows up as collision type `08` in the direction field
(`Tile_Collision_*`), matching the solid type in `RUNTIME_DESIGN.md`.

**Field objects block movement independently of the terrain collision grid.**
Walking UP from the start cell stops at (752, 160) with `coll_up` reading
`00` — walkable terrain — because the NPC at (752, 144) is in the way. The
collision array describes terrain only, so `psiv-core` needs a separate
object-occupancy test; a pure grid lookup will let the player walk through
NPCs.

## Collision grid indexing, independently confirmed

`GetChunkAndCollision` adds `$10` to Y before shifting down to a cell, so the
cell a character *occupies* is one row below `curr_y_pos / 16`. The packer
already applies that shift when it emits `y_cell`, and
`rust/psiv-data/src/map.rs` documents it.

This harness measured the same offset from the other direction, against the
game's own `Tile_Collision_Standing` / `_Up` / `_Down` / `_Left` / `_Right`
readouts over 6510 cell-aligned samples: **dx=0, dy=+1 matches 99.86%**, versus
93.2% for the next-best offset and 79.8% for the naive `y_px // 16`. The
convention in the pack is right, and it is now confirmed behaviourally rather
than only by reading the disassembly.

## Field objects block, and block taller than the grid

Two independent confirmations:

- An NPC standing at pixel row R blocks a character trying to enter pixel
  row R+1, so **objects occupy a two-row footprint**, not one cell.
  `python3 -m oracle.scripts.navigate` encodes this as `OBJ_FOOTPRINT`.
- Object blocking is invisible to the terrain grid. Walking west along the
  map-$13 corridor stops dead with `coll_left` reading `00`.

This matters more than it first looks: on map $13 a single NPC (Alys) closes
the only two-row corridor completely, so the terrain grid alone says the map is
traversable when the cartridge says it is not.

## Story gates found by walking into them

Two gates showed up while trying to reach an encounter map on foot, both of
which a route planner working from map data alone would not predict:

- **`NPCAlysPiata`** (map $13 object 7, dialogue 41) is parked in the only
  corridor to the doorway. Talking to her fires event `$03` and
  `Current_Party_Slots` becomes `0100FFFF` at f7478 — **Alys in slot 1, Chaz
  in slot 2**, confirming `Event_AlysFound` and that Alys leads. Tape
  `04_alys_joins.tape`.
- **The Piata town gate is closed.** The gate is the four-cell gap at cols
  30-33 in the wall spanning rows 46-47; guards were observed at cells (31,46)
  and (33,46). The navigator tried all four columns in turn and the cartridge
  refused every one, so this is a hard story gate
  (`Event_PiataGuardsReprimand`), not two NPCs that can be walked around.
  **The Motavia overworld is not reachable on foot at this point in the
  story**, which is why the game's first random encounters are in the Academy
  Basement (encounter group 14) rather than outside town.
