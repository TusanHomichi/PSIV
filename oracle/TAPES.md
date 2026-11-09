# Tapes and authoring

[Oracle setup and index](README.md)

## Tapes

| tape | what it proves |
|---|---|
| `01_newgame_to_first_control.tape` | the boot path is reachable deterministically from power-on; new-game starting state |
| `02_walk_timing.tape` | frames per cell, the four facing values, blocked-press behaviour |
| `03_npc_talk.tape` | talk range, dialogue open/close timing, text draw rate |
| `04_alys_joins.tape` | the first story gate: Alys joins and takes slot 1 |
| `05_principal_assignment.tape` | the principal's assignment scene (event `$8001`) |
| `06_basement_quest.tape` | the basement-stair NPCs, event `$0004`, Hahn joins |
| `07_first_battle.tape` | the first random encounter and the fight through to victory |
| `08_rng_characterization.tape` | RNG call counts per frame across title, field, walking and menu |
| `09_second_battle.tape` | a second encounter on a different seed path, including a critical hit |
| `10_levelup.tape` | three encounters back to back; Chaz reaches level 2 |
| `11_beside_press.tape` | Speak at an NPC that is adjacent but not in front |
| `12_escape.tape` | a successful escape attempt |
| `13_open_anim_press.tape` | Speak pressed inside the window-open animation |
| `14_defend.tape` | the DEFEND command and its `physical_prop` clobber |
| `15_text_hold.tape` | text draw acceleration while Speak is held |
| `16_page_boundary.tape` | holding Speak across the end of a page |
| `17_flag_alias.tape` | where a "temp" event flag write lands, on hardware |
| `18_flag_round_trip.tape` | that bit across a leave-and-reenter round trip |
| `19_chest_map_objects.tape` | object slots on a chest-bearing map, and opening a chest |
| `20_chest_round_trip.tape` | re-pressing an opened chest, and whether it is still open after leaving and returning |
| `21_second_chest.tape` | a second chest id, which is what pins the flag bank's bit arithmetic |
| `28_meeting_rika_retail_probe.tape` | power-on schedule plus explicit RAM patches for deterministic MeetingRika video frames |
| `29_vehicle_land_rover_probe.tape` | retail field boot plus a documented Land Rover movement, dismount-refusal and mounted-battle fixture |
| `31_natural_land_rover.tape` | power-on natural route through the verified Holt prefix; full Land Rover acquisition remains downstream |

`prelude_basement.tape` is a generated intermediate (the output of
`python3 -m oracle.scripts.navigate`) that both battle tapes are built from;
`python3 -m oracle.scripts.find_battle` consumes it.

Tape A's presses are placed from the disassembly, not by trial: `GameMode_Title`
begins at f226; `TitleRoutine_FadingText` takes a START to skip; then
`TitleRoutine_PressStartButton` polls `Joypad_Pressed` for START over a
`$233` = 563 frame window; then, with empty SRAM, `CheckPS4String` fails and
`Title_NoSavedData` opens window 4 and waits for Speak/Camp/Start.

Tape A advances the opening scene with **Cancel (B)**, deliberately. Cancel
advances dialogue pages but does nothing once field control returns, so
presses that overrun the end of the scene cannot perturb the state being
measured. Using Speak here would fire a Talk on the first frame of control.


### The encounter clock, and why long tapes need tuning

Any tape that walks far inside a dungeon is playing a dice game, and it is
worth stating the odds because they set what tape lengths are practical.
`RunRandomBattles` (`ps4.asm:116840`) counts `$FFFFECE4` down from 10 — reset
on every map entry and after every battle — on each *completed* step, then from
that point rolls on every step: `UpdateRNGSeed`, and a battle if
`RNG_Seed & $1F == 0` (`& $7F` in a vehicle). Flat 1/32 per step, no ramp.

So an N-step route survives with probability `(31/32)^(N-10)`: 25 steps is a
62% bet, 58 steps a 22% one. Tape 21's route is 58 steps, and sweeping its
pre-walk idle over 14 values produced exactly 3 clean runs — 21%, against 22%
predicted. That is a behavioural confirmation of the rule, and incidentally of
`psiv-runtime`'s `GRACE_STEPS = 10` and `FOOT_MASK = 0x1F`, which match.

The dodge is `python3 -m oracle.scripts.sweep_encounter`: idle frames advance
the seed without consuming a step, so varying one `N . <mark>` wait re-rolls
every check downstream while leaving the route byte-identical. It runs the
variants in parallel and reports which values walk clean. Tapes 20 and 21 both
carry a tuned wait, noted as such in their comments — a tape that walks far and
has no tuned wait in it is a tape that has not been re-run since it was written.

### Tape authoring tools

`route.py` plans a walk over the runtime pack's own collision grids and warp
graph, so a generated route that walks correctly on the cartridge is also a
check that the extraction agrees with the cartridge. It models the anti-ping-
pong rule (`GameMode_LoadFieldMap` initialising `Tile_Collision_Standing` to 1,
so a doorway never fires on the frame you are placed on its destination) — the
planner walks straight back through the door it came from without it.

`python3 -m oracle.scripts.navigate` closes the loop, because terrain is not
the whole story: it plans a short leg, runs it, reads where the character
actually ended up, marks observed object footprints and cells the cartridge
refused to enter, and re-plans. **The feedback is at authoring time only** — the
artifact it writes is an ordinary static tape with no runtime feedback in it, so
it still replays byte-identically like every other tape here.
