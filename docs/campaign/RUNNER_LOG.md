# Runner log

The durable record of campaign-runner runs: each run's command, base revision,
how far it got, and every halt with its diagnosis. The runner and its report
format are described in [the campaign runner plan](CAMPAIGN_RUNNER.md#the-runner);
this file holds what the runs found. Evidence files live under the git-ignored
`build/campaign/` of the checkout that ran them, so a fresh clone does not hold
them; the commands below regenerate them.

A run that halts on a port defect keeps its report here and a diagnosis with
`file:line` and the cartridge evidence. The orchestrator turns a diagnosis into
a fix lane or an issue; the runner never fixes a port defect and never adds a
shortcut, a state edit or a skip to get past one.

## Current state

`routes/main.json` completes from New Game with pads only: 13 chapters, New
Game to the post-Rika north-bank checkpoint and the Aiedo arrival, 175,497
frames, 92 random battles and 2 scripted ones, exit 0. The run is a traversal
proof, not a balance proof: the default policy lets members fall in the
BioPlant and relies on an inn to restore them (see "Not claimed").

One port defect is diagnosed and the route works past it with a legitimate
alternative: [H1](#h1-the-mile-sand-worm-trigger-halts-every-visit-to-mile).

## Runs

Base revision for every run: `016cdc7` (PR #53, the battle, shop and camp
Session modes), runner code of this lane on top. Pack: the local
`runtime-pack`. All runs are release builds unless said otherwise; a debug
build gives the same tape byte for byte (checked on chapter one).

| Run | Command | Result |
| --- | --- | --- |
| R1-1 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/campaign/full1` (route as R0 shipped it) | academy completed (30,822 frames, 2 battles); halted in `holt` objective 0, `TriggerUnsupported { trigger: 112 }` on arriving in Mile: [H1](#h1-the-mile-sand-worm-trigger-halts-every-visit-to-mile). Report: `build/campaign/evidence/halt-1-mile-sandworm-trigger.json` |
| R1-2 | `... --from-chapter holt` while the route and the runner were repaired | `holt` completed once Mile was left out (H1) and the `expect` cell corrected (H2); `rune-dorin` halted on the Zema inn (H3), then lost a battle in the Valley Maze (H4) until the runner cured the party between battles |
| R1-3 | `... --from-chapter alshline` | storage door object settled (H5); lost a battle in the Alshline basement until `run_unless_boss` existed (H6); then chest (H7), equipment with every slot full (H8), the BioPlant elevator doors (H9) and the Rika scene's `go_to` (H10) |
| R1-4 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/campaign/full4` | **completed**, exit 0. 175,497 frames, digest `52ccbf8d9a7de61a`; `psiv-campaign replay build/campaign/full4/run.tape` reproduces the digest. A second full run (`full3`, after H11) printed the same digest |

R1-4 per chapter (frames, battles, party at the chapter's end):

| Chapter | Frames | Battles | Party at the end |
| --- | --- | --- | --- |
| academy | 32,308 | 4 | Alys L7 50/53, Chaz L2 31/31, Hahn L2 27/27 |
| holt | 9,958 | 6 | Alys L7 41/53, Chaz L3 28/34, Hahn L3 28/30 |
| rune-dorin | 29,215 | 9 | Alys L7 53/53, Chaz L5 49/49, Hahn L4 27/33, Gryz L6 66/66 |
| alshline | 8,871 | 6 | Alys L7 53/53, Chaz L5 49/49, Hahn L4 33/33, Gryz L6 66/66 |
| zema-rescue | 14,198 | 9 | Alys L7 52/53, Chaz L5 49/49, Hahn L5 33/38, Gryz L6 66/66 |
| zema-outfit | 1,282 | 0 | Alys L7 53/53, Chaz L5 49/49, Hahn L5 38/38, Gryz L6 66/66 |
| zema-training | 53,841 | 55 | Alys L7 53/53, Chaz L6 53/53, Hahn L6 45/45, Gryz L6 66/66 |
| zema-armour | 718 | 0 | the same, all full |
| bioplant-elevators | 4,887 | 3 | Alys L7 52/53, Chaz L6 53/53, Hahn L6 44/45, Gryz L6 65/66 |
| bioplant-order | 60 | 0 | Gryz leads: Gryz, Alys, Chaz, Hahn |
| bioplant-rika | 18,763 | 2 | Gryz L7 0/76, Alys L8 0/61, Chaz L6 53/53, Hahn L6 0/45, Rika L1 39/39 |
| north-bank | 850 | 0 | Gryz L7 76/76, Alys L8 61/61, Chaz L6 53/53, Hahn L6 45/45, Rika L1 39/39 |
| aiedo | 546 | 0 | the same; standing at Aiedo (47,83) |

Evidence of R1-4 (local, under `build/campaign/`): `evidence/r1-4-full-route.tape`
(sha256 `dd5ff60f0cd2ef06ff483ecf7f55ee398cc22bed85752a7181dc9291c2e5e6c5`),
the chapter saves `full4/NN-id/slot_1.sram` (north bank
`29b717fdba5bdd1d5047025b2f6f63c7448dedf792f609a273b25512bf1897dc`, Aiedo
`866c29cd343403cb3babe77f31abdde6da7620af08fecd0272929a0141c999a2`), and the
route's own saves in `full4/route/`.

Acceptance target: after `north-bank` the party stands on Motavia `$00` at
(84,64) with Gryz, Alys, Chaz, Hahn and Rika, all alive, flag `$35` set (the
chapter's `expect` and `closing` hold), and `aiedo` arrives at map `$54`
(47,83), one warp, as R0 planned.

## Halts and their diagnoses

H1 is a port defect. H2 to H11 are route claims or runner gaps, each fixed in
this lane; they are here so the next runner author does not rediscover them.

### H1: the Mile sand worm trigger halts every visit to Mile

- **Halt:** R1-1, chapter `holt`, objective 0 (`go_to_map 29 via_warp 1`), on the
  frame the party arrives in Mile (map `$1D`, cell (20,49)):
  `scene_fault: TriggerUnsupported { trigger: 112 }` (`$70`).
- **Cause (port):** `rust/psiv-core/src/trigger_custom.rs:43` answers
  `TriggerResult::Unsupported(.., Unsupported::Rng)` for `MileSandWorm`
  unconditionally, and `trigger_custom.rs:434` pins it (`the_four_undecidable_routines_say_so`).
  The trigger table binds `$5C..$70` to it (`rust/psiv-core/src/trigger_table.rs:407`).
  Most of the routine is decidable without any RNG.
- **Cartridge:** `RunEvent_MileSandWorm`, `reference/ps4disasm/ps4.asm:116449`:
  it tests `EventFlag_MileSandWorm` (`$1B`, `ps4.constants.asm:1505`) and
  returns NoEvent when set; returns NoEvent unless the leader's `curr_x_pos`
  is at most `$170` and `curr_y_pos` is within `$150..$2B0`; only then calls
  `UpdateRNGSeed` and fires event `$71` when `RNG_Seed & $1F == 0`. Arrival is
  at pixel y `$310`, so the cartridge answers NoEvent, takes no RNG draw, and
  the party walks on. The port halts where the cartridge does nothing.
- **Fix lane:** decide the flag test and the position box in `trigger_custom.rs`
  as the routine does, and raise `Unsupported::Rng` only inside the box with the
  flag clear (or give `TriggerContext` the shared RNG and run the draw and event
  `$71`, which is the real fix). Add the position cases as unit tests. Until
  then every visit to Mile halts a runner, and a player who walks into the
  worm's box meets the same gap.
- **Route alternative used:** Mile is not needed. The two Mile objectives of
  `holt` (a detour before Zema, flagged `verify` by R0) are gone; the route goes
  to Zema directly and every later claim still holds. The Mile inn at the start
  of `rune-dorin` is replaced by the Piata inn: the Zema inn is shut
  (`psiv-campaign plan --from-map 36 --from-cell 31,49 --to-map 39` with the
  flags the route holds finds no walk; with `event:0x33` and `event:0x37` it
  finds the 23-step one), so `go_to_map 25`, stand at (41,32), `rest_inn`,
  `go_to_map 0`. A player can choose that inn.

### H2: the Holt scene's landing cell (route claim)

`expect` after Professor Holt claimed Zema (60,21). The scene writes
`Map_Start = ($3C,$14)` in 8-pixel units (`docs/scenes/13_ProfHolt.md:26`):
cell (30,10), and the port stands the party one row south, (30,11), as at first
control. The route now says (30,11). No port defect: the claim halved the wrong
unit.

### H3: the Zema inn is shut early (not a defect)

`rune-dorin` first replaced the Mile inn with the Zema inn and halted with
`unreachable: no walk from (31,49) fires warp 4 of map 0x24`. The inn's door is
patched shut until the Igglanova rescue (static plan evidence under H1). The
story keeps the town sealed; the route uses the Piata inn.

### H4: a lost battle in the Valley Maze (runner gap)

The default policy only attacked, never cured between battles and never
revived; Chaz and Hahn fell in the first maze fights and Rune walked alone until
the party was wiped (`lost_battle`). The runner now cures the party through the
camp after each battle (`rust/psiv-campaign/src/recovery.rs`: the same TECH menu
a player uses, a healing technique from whoever has the TP, until nobody is
below 70%), and the in-battle threshold is 50%. With the cure in place R1-4
walks the maze with nobody down.

### H5: Gryz's storage door is object 1 (route `verify`)

`alshline` objective 7 named object 0; the press reaches object 1
(`talk` halts `wrong_object` naming both). Route fixed.

### H6: a lost battle in the Alshline basement (runner gap)

The chapters `alshline` and `zema-rescue` name `run_unless_boss`; the runner
resolved every name to the default policy and fought a basement formation it
cannot beat. `run_unless_boss` now runs from random encounters (RUN on the main
options until it works) and fights scripted battles.

### H7 to H11: runner gaps and route claims, fixed

- **H7** `open_chest` halted `wrong_object`: a chest press opens a chest
  window, not a dialogue. The talk controller reads the loot window as opened.
- **H8** `equip` for a member who wears something in every slot: the EQUIP
  menu opens its list only from an empty slot and takes off what a full slot
  holds (`rust/psiv-runtime/src/session/camp/equipment.rs`). The controller
  clears the item's own hand, or a head or body piece it puts back on after.
- **H9** the BioPlant elevator: after event `$13` the door cells are walkable
  map-change ground (collision 1) while the warp record is a ground trigger, so
  the stepping rule never "fires" it and R0's flood found no walk. The
  controller walks into the warp's trigger area and lets the game's elevator
  event do the rest. Confirms the route's `opens` claims.
- **H10** `bioplant-rika` objective 8: the Rika scene fires on the way and
  carries the party off the map, so the `go_to`'s target no longer exists. A
  scene that ends the walk off the target map now ends the objective; the next
  objective's `expect` asserts what the scene left.
- **H11** north bank: the `expect` claimed at least 1103 meseta, one native run's
  purse; a New Game run had 914. The purse is what the battles paid, not a rule,
  so the clause is gone.

An earlier gap, found on the first academy run: `Igglanova: talk` presses Speak
and the engine starts interaction area 0 of map 23 (event `$6B`), not an object's
dialogue; the controller accepts an area event as what the press opened.

## `verify` items resolved by playing

R0 left 12 objectives marked `"verify": true`. All are resolved and the flags
dropped; the run that settled each is in the route's `note`.

| Chapter and objective | Question | Answer |
| --- | --- | --- |
| academy 10 | Hahn: object 0 or 1 | object 0 |
| academy 16 | Igglanova: which object | none: the press starts interaction area 0 (event `$6B`); object 0 only places the party |
| holt 0 | the Mile detour | not needed; removed (H1) |
| holt 7 | Zema cell after the Holt scene | (30,11), not (60,21) (H2) |
| rune-dorin 20 | Dorin: object 0, 5 or 8 | object 0 |
| alshline 7 | Gryz's storage door | object 1 (H5) |
| zema-training 3 | patrol until level 6 and 1,180 meseta | holds after 55 battles in R1-4; the runner cures the party between them |
| bioplant-elevators 4 | `temp:0` after the door | set |
| bioplant-elevators 8, 10 | `opens` cells of the elevator doors | walkable (collision 1) after the scene (H9) |
| bioplant-rika 0, 4 | the same | the same |

## Not claimed

- **Balance.** Members fall in `bioplant-rika` (ten command windows open with a
  member down; Gryz, Alys and Hahn stand at 0 HP when the Rika scene ends); the
  inn restores them (`north-bank` rests at Zema before the crossing). The policy
  does not revive and does not grind for safety, and the party reaches Aiedo at
  levels 6 to 8. A route that wants a healthier party names an inn or a
  `patrol`.
- **Seeds.** Frames and battles of a run are a function of the whole pad
  history. A `--from-chapter` run restarts the frame counter and RNG, so it is
  the same game but not the same frames; only the full run is evidence.
- **Unexercised objectives.** `sell`, `use_item` and an explicit
  `use_technique` are not in `routes/main.json`; `tests/runner.rs` plays them at
  the Piata item shop. `Session` field notices (a member falling to poison) have
  no pad path, so the runner halts on one (`unexpected_state`); none occurred.
- **The boundary.** Construction (`Runtime::new_game`, `enable_battles`,
  `start_event`, `from_save`) is the one thing the runner does outside
  `Session::frame` and views, in `src/start.rs`. Node S5 will make those
  crate-private; the runner then needs a `Session` constructor taking the pack
  and the battle files.
