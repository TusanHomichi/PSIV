# Runner log: the Hangar to Dark Force 1 (C6, H32 to H35)

Lane c6-kuran (2026-10-04): the route from the Hangar through `Cutscene_Landale`, the
Dezo spaceport, Kuran's elevator maze and `Event_DarkForce1` to the defeat of Dark Force 1
(`Cutscene_DarkForce1Defeated`), and the four halts it found on the way. The Dezolis arc
before it is [RUNNER_LOG_DEZOLIS.md](RUNNER_LOG_DEZOLIS.md); the index and current state
are in the [runner log](RUNNER_LOG.md).

## C6 runs

Base `00f4936` (main after campaign-21: 36 chapters to the Hangar, digest
`e6200509953185db`), release builds, `CARGO_BUILD_JOBS=2`, one heavy command at a time.
The pack is the owner's `runtime-pack` (manifest SHA-256 `02bb29ad…11f2`), read through a
symlink and never written; it already carries Tyler's chunk `$47` (H30) and Dezolis's
spaceport hook, so no pack rebuild was needed. Evidence is under the ignored `build/c6-*`
and `build/ladder/` of the lane's worktree; the commands regenerate it. "Snapshot clock"
below means a `--from-chapter` run: it loads the previous chapter's snapshot and restarts
the frame clock and RNG, so it is the same game on other dice; only a full run is evidence
for the route.

| Run | Command (from the worktree) | Result |
| --- | --- | --- |
| C6-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c6-base` (the 36 chapters of the base) | exit 0, 2,644,725 frames, digest `e6200509953185db`, 31 s: the base's. Its `35-dezolis-tyler-grave` snapshot is byte-identical to the final route's (`cmp`), so the runtime and planner changes below left the 35-chapter prefix untouched |
| C6-1 | scratch chapter `dezolis-landale`, `go_to (26,82)`, `--from-chapter` the C6-0 snapshot | halt `expect_failed` (`$82` clear): no trigger one row off. `go_to (26,83)`: halt `unreachable` ("no walk reaches the goal without firing another warp"): H32 |
| C6-2 | `step_onto (26,83)` after the runtime fix | exit 0, 2,691 frames, 2 battles: the scene plays and the party stands on Dezolis (9,74) with `$82`. A first `kuran-arrival` from that chapter's own snapshot flew at once; the same chapter run straight after it in one process halted `stuck` at (12,73), frame 5,206: H33 |
| C6-3 | `kuran-arrival`: `go_to_map 212 via_warp 9`, `go_to (30,20)`, `board up to 4` | exit 0, 2,668 frames: Kuran `$190` (31,46), `World_Index` 4, `$86` set by `Event_KuranArrival` on the first field frame |
| C6-4 | `kuran-elevators`, the 20 objectives of [the chain](#the-elevator-chain) | exit 0, 5,048 to 6,352 frames and 6 to 13 battles across the runs (the count moves with the clock), every one run from; F3 (30,50) |
| C6-5 | `kuran-near-dark-force` (`go_to (30,33)`), then `kuran-dark-force-1` | the first: exit 0, 663 frames, `$87` set. The second: halt `lost_battle` on the arrival party: H34 and the ladder below |
| C6-6 | `buy DIMATE x20` in the prepare chapter | halt `unexpected_state`: "You can't carry anything else." The pack has 40 slots and stacks nothing; six fit |
| C6-7 | the first `dezolis-training` patrol | halt `unexpected_state`, `Fallen(CharId(5))`, a field notice with no pad path: H35 |
| C6-F1, F2 | `run rust/psiv-campaign/routes/main.json --save-dir build/c6-route{,-b} --tape …/run.tape --report …/report.json` (43 chapters, twice from New Game) | both exit 0, **3,700,582 frames**, digest `fbe9fef7015aba2e`, tape SHA-256 `969ba6dc…4996` (35 s each); `cmp` says the two tapes and the two `42-kuran-dark-force-1` snapshots are byte-identical; the pad SAVE `route/slot_1.sram` is `4aeff0b1…ec62`, unchanged; the route file is `34ce8f99…1850`, the binary `6b37a951…4e3a` |
| C6-R | `psiv-campaign replay build/c6-route/run.tape` | digest `fbe9fef7015aba2e`, 3,700,582 frames, exit 0 |
| C6-W | `cargo test --release -p psiv-campaign --test runner -- --ignored --test-threads=1` | 2 passed in 42 s: the whole route with its new snapshot assertions (Landale, Kuran, F3, Zelan F1 with the Ice Digger), its replay and the pad SAVE |

Chapter frames of the final route: `dezolis-tyler-prepare` 1,794 (no battle), `dezolis-landale`
2,691 (2), `dezolis-training` 1,031,235 (738), `kuran-arrival` 2,668 (0), `kuran-elevators` 5,494
(10), `kuran-near-dark-force` 663 (0), `kuran-dark-force-1` 11,312 (1). Snapshots (SHA-256):
`35-…` `6fada43e…ccd1`, `36-…` `5049ca07…f8ab`, `37-…` `ce12bb39…c765`, `38-…` `cedffab4…bea3`,
`39-…` `7a503449…db15`, `40-…` `71953dbf…223c`, `41-…` `3da80ce1…4d3d`, `42-…` `f7b74e4f…34c2`.

## The halts

| Halt | Class | What it was | Fix |
| --- | --- | --- | --- |
| H32 | runtime order, runner objective | the trigger on the Hangar's warp row | `field_tick.rs` scans the trigger list before a warp; `step_onto` |
| H33 | scene transcription | Landale's spaceport chunk was never written | `LANDALE` ops 16 to 18, a `SCENE_CHUNK_WRITES` row |
| H34 | balance loss | Dark Force 1 against the party the route arrived with | purchases, an inn night and a Dezolis training chapter |
| H35 | runner | the first training patrol met Dezolis's cold | a `refuge` through the Tyler inn |

### H32: the Landale trigger sits on a warp, and neither the runtime nor the planner let it win

**What it is.** `RunEvent_FindingLandale` (`$2B`, `ps4.asm:115866-115882`) fires on `curr_y_pos
== $520`, `curr_x_pos $1A0..$1B0` with `$84` set and `$82` clear. A standing cell's y byte is
its row less one (`PixelPos::from_cell`), so the trigger is row 83, cells (26..27,83), and
that is the first row of the Hangar's warp 0 (26..27,83) to Dezolis. On foot
`FieldRoutine_Controls` runs `RunEvents` before `RunMapTransitions`
(`ps4.asm:116768-116773`), and a scene that starts ends the frame: the trigger wins. Two
things in the port said otherwise:

1. `Runtime::tick` handled the landing's `Effect::Warp` and skipped the trigger scan when the
   map changed (`field_tick.rs`), so the corrected cell (26,83) would have taken the warp.
2. The planner marks a warp's footprint as a terminal: `go_to (26,83)` was `unreachable`.

**Fix.** `field_tick.rs` now runs the map's trigger list at the landing, before the warp
(`triggers_run` keeps the elevator path's early scan from running twice); a scene that
starts returns the frame. The route gets one new objective, `step_onto {map, cell}`: the
planner's `Flood::onto_plan` finds the walk whose last step lands on the footprint cell, the
runner (`Driver::step_onto`) is done when a scene has run and halts `unexpected_state` if the
party arrives on another map with none, and the validator lets the position go unknown until
the next `expect` pins it.

**Tests and negative controls.** `scene_map_tests::landale::a_trigger_on_a_warp_cell_wins_over_the_warp`
(the step from (26,82) starts the scene, no `MapChanged`; with `$82` set the same step takes warp 0;
the test fails with the scan switched off, run); `tests/kuran_arc.rs`
`step_onto_the_landale_row_…` and `step_onto_halts_when_the_warp_fires_or_the_cell_is_not_a_footprint`
(gate shut: `unexpected_state` naming the warp; (26,70): `unreachable`); `tests/validator.rs`
`step_onto_must_name_a_footprint_on_the_map_the_party_is_on` (plain cell, wrong map).

### H33: Landale's spaceport chunk was never written

**What it is.** `Event_DezoSpaceportAppearing` writes BG chunk (6,36) `<- $2C` after pass `$3D`
of its 369-pass loop (`loc_6C148`, `ps4.asm:145530-145541`, the byte at `$06C15C`). The chunk makes the four
door cells (12..13,72..73) map-change (type 1), which is what lets Dezolis warp 9 fire; the map's load
hook writes the same chunk when `$82` is set (`loc_53E98`, `:111635-111646`). The transcription kept
the 369 passes as one `Wait`. A save loaded after the scene rebuilt the map with the hook and walked
into the spaceport; the same party that played the scene live stopped on the cell: the live map kept
open ground, `go_to_map` stood at (12,73) and halted `stuck` after eight walks. A `--from-chapter`
run reloads the snapshot and hides it; it showed only in one process, straight after the scene.

**Fix.** `LANDALE` is `Wait 308`, `WriteMapChunks (6,36) <- $2C`, `Wait 61` (24 ops;
[44](../scenes/44_Landale.md)). The write resolves through the pack's flag-`$82`
`overworld_patches` entry; `psiv_tools.map_patches.SCENE_CHUNK_WRITES` has the row
(`via="overworld_patches"`, bytes pinned), so `tests/test_scene_chunk_atlas.py` holds it. No pack
rebuild. **This is the one `psiv_tools` change of the lane, a table row; the pack is unchanged.**

**Tests and negative controls.** `scene_map_tests::landale::the_spaceport_door_is_on_the_live_map_when_landale_ends`
(the four cells read map-change the moment the scene ends and the scene wrote one chunk; with the op
removed it fails on "one chunk write in the whole scene: []", run); `without_the_scene_dezolis_has_no_spaceport_door`
(the flag-clear map has no door).

### H34: Dark Force 1 is a balance loss until the party is trained

**What it is.** Event battle 9, enemy 130 (`DarkForce1`): 4,540 HP, STR 100, attack 230, defence 5,
weak to anti-evil (4) and Efess (3), resistant to physical (1); abilities FLARE SHOT, PHONONMASR and
BURSTROC, each striking the whole party for 100 to 170 a member against the silver armour. The party
the route arrives with (Chaz L27, Rika L29, Rune L27, Wren L20, Raja L25) loses. The brief's class is
"a balance loss": nothing here is an unsupported ability, and `psiv-core/src/battle/**` is untouched.

| Probe | Clock | Party at the fight (Chaz, Rika, Rune, Wren, Raja) | Dark Force 1 |
| --- | --- | --- | --- |
| arrival: CRMC gear, TP spent (Chaz 17 of 101, Rika 26 of 122) | snapshot | L27, 29, 27, 20, 25 | lost in about five rounds, 3,267 HP left |
| + silver armour, six Dimates, an inn night | snapshot | the same | lost, 1,601 left |
| 150 battles of Kuran training, no inn: TP 2 | snapshot | L33, 36, 31, 28, 30 | lost, 1,422 left: no technique to cast |
| Dezolis training to level 27, inn | snapshot | L32, 35, 31, 27, 29 | lost, 938 left |
| the same to level 30 | snapshot | L35, 37, 33, 30, 31 | lost, 639 left |
| the same to level 38 | snapshot | L42, 45, 39, 38, 38 | **won**, 7 rounds, Wren at 89 of 384 |
| training to level 27 (178 battles, 251,642 frames) | full run | L32, 35, 31, 27, 29 | won, 17 Chaz commands, 5 HP left at the last estimate, Rune and Wren down |
| to level 30 (290 battles) | full run | L35, 37, 33, 30, 31 | won, 13 commands, Wren and Raja down |
| to level 33 (431 battles) | full run | L38, 40, 35, 33, 34 | won, 9 commands, Wren down |
| to level 36 (601 battles) | full run | L40, 43, 37, 36, 37 | won, 8 commands |
| **to level 38 (738 battles, 1,031,235 frames), the route** | full run | L42, 45, 39, 38, 38 | won, 8 commands, Wren down |

The two clocks disagree below level 38 (the same party at level 27 won one and lost the other by
938 HP): the fight is a coin toss until the party is well past it, so the route takes the target both
clocks won. The full runs' wins are the evidence; the snapshot rows are why the target is not 27.

**What the policy chooses.** `fight_to_win`/`run_then_win` take the highest-estimated action each round:
Chaz's NATHU (`$09`, anti-evil, est. 393) while his TP holds, then Rune's NAGRA, Wren's plain attack (129),
Rika's attack (72) and Raja's attack (21); the cures go first (Chaz's GIRES, Rika's or Raja's GISAR,
one single and one group cure a round). The engine runs no player skill beyond Crosscut and Vortex
(`policy_boss.rs`, `battle/skill.rs`), so Chaz's RAYBLADE (anti-evil, power 176, learned at level 27)
and Rune's TANDLE, EFESS (efess, which Dark Force 1 is weak to) and LEGEON are never cast: the party's best
answers to a boss that is weak to exactly those elements are not in the battle. That is a port gap in the
player's battle surface, not a route choice; it is why the fight is a race of TP and HP, and why the
grind is as long as it is. **Reported, not fixed: `battle/**` is another lane's, and the gap is for the
orchestrator to file.**

**The response.** Three route chapters, all things a player does:

- `dezolis-tyler-prepare` (Hangar to Tyler and back by the grave stairs, which stand again after a
  reload and are opened by speaking at the grave a second time, [92](../scenes/92_TylerGraveOpening.md)):
  two SILV-MAIL, a SILV-CROWN, a SILV-MANTL and six Dimates, the old gear equipped over (49 DEF for the
  mail against 25). The purchases are small next to the money the grind then earns (the party ends
  with 666,350 meseta) and can be dropped without changing the result's class.
- `dezolis-training`: a `patrol` on the Dezolis overworld by the spaceport door until every member is
  level 38 (encounter group 11: 1,363 to 4,236 experience a battle, split five ways, so about 540 a
  member), the `default` policy, the Tyler inn as the `refuge` and a last night there so TP is whole
  at the fight.
- `kuran-dark-force-1` fights the event battle with `fight_to_win`.

Training on Kuran itself was tried first and does not work: the camp cure after each battle spends TP
and the floor has no inn, so the party reached Dark Force 1 at TP 2 (third row above).

### H35: Dezolis wears the party down

The Dezolis map carries the poison flag (the cold): the first training patrol halted
`unexpected_state` on `Fallen(CharId(5))` (a member's HP ran out between battles, and a field notice
has no pad path). The patrol now carries a `refuge`, the Tyler inn out and back, which the runner takes
when a member has fallen or sits under half HP after the camp cure.

## The elevator chain

Kuran `$190` to F3 `$198` is a maze of regions joined by elevator doors (event `$13`, the
type-2 interaction areas): each door opens when the party stands under it and faces up, and a warp
behind it leads on. A breadth-first search over the pack's collision grids, doors and warps found the
shortest chain; the `kuran-elevators` chapter is that chain, twelve doors and warps:

`Kuran (30..31,17)` to F1 (46,51); F1 warp 4 to Part2; Part2's door (12..13,35) to Part3; Part3 warp 1
back to F1 at (16,53); F1 warp 3 to Part4; Part4's door (30..31,33) to F2 (46,83); F2's door (46..47,11)
to F1 (46,17); F1 warp 6 to Part5; Part5's door (18..19,29) to F2 Part2; F2 Part2 warp 1 to F1 (76,53);
F1's door (46..47,35) to F2 (46,27); F2's door (46..47,45) to F3 (30,49).

Random battles on the way are run from (`run_then_win`): nothing there is fought, so Siren386's BARRIER
(`$1D`, #86) is never reached; the training is on Dezolis for the same reason as H34's last row, and
its default policy casts no attack technique. `Event_Hijammer` (`$6D`, allowlisted #83) is the interaction
area on Part2's white chest at (14,16) (chest flag `$5E`): the route does not open that chest, and nothing
on the way needs it. F3 rolls no random battles.

## Negative controls

| Break | Guard | Result |
| --- | --- | --- |
| the `WriteMapChunks` op removed from `LANDALE` | `scene_map_tests::landale::the_spaceport_door_is_on_the_live_map_when_landale_ends` | fails: "one chunk write in the whole scene: []" |
| the trigger scan before a warp switched off | `scene_map_tests::landale::a_trigger_on_a_warp_cell_wins_over_the_warp` | fails on the first assertion: the warp wins |
| `$82` set at the Hangar | the same test, second half; `kuran_arc.rs` `step_onto_halts_…` | the step takes warp 0 (`MapChanged`); `step_onto` halts `unexpected_state`, "fired the warp" |
| `step_onto` of (26,70), or on map 1 | `kuran_arc.rs`; `validator.rs` `step_onto_must_name_…` | `unreachable`; rejected in `dezolis-landale`: "is a go_to", "has the party on map" |
| the Dezo spaceport unraised (`$82` clear) | `kuran_arc.rs` `the_spaceport_flies_to_kuran_only_once_it_has_appeared` | `menu_entry_missing`: the list has no Kuran row |
| Part4's door `opens` removed | `validator.rs` `the_kuran_chapters_reject_a_missing_door_and_a_bad_world` | rejected in `kuran-elevators`, naming `via_warp 0` |
| the boarding's `to: 9` | the same test | rejected in `kuran-arrival`: "not a World_Index" |
| `kuran-near-dark-force` stops on row 34 (a scratch copy of the route) | `run build/neg/main.json --from-chapter kuran-near-dark-force` | halts `expect_failed`: `$87` clear (exit 2) |
| the battle not fought | `kuran_arc.rs` `dark_force_1_defeated_…`, second half | `$89` stays clear and the party stays on F3 |
| the arrival party (L27, 29, 27, 20, 25) | `kuran-dark-force-1` | halts `lost_battle` (H34, first rows) |

## Checks

All from the lane's worktree, one heavy command at a time.

- `CARGO_BUILD_JOBS=2 cargo test --manifest-path rust/Cargo.toml --workspace -- --test-threads=1`:
  exit 0, 47 targets, **1,433 passed, 0 failed, 3 ignored** (the two whole-route tests and one in
  `psiv-core`); then the whole-route test in release (C6-W).
- `cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings`: exit 0;
  `cargo fmt --all --check`: exit 0.
- `PYTHONPATH=. python3 -m unittest discover -s tests`: ran 1,227 tests with 53 errors, every one a
  missing git-ignored local input the lane's worktree did not link (`oracle/fixtures/battle_animation_art/*.png`,
  `oracle/states/*.json`); none touches this lane. With those linked read-only from the owner's tree, the 15
  affected modules (270 tests) pass, and the rest passed in the first run. No test skipped.
- `python3 tools/size_guard.py`, `python3 tools/check_docs.py` and `git diff --check`: see the receipt.

## Not claimed

- **The balance.** The party wins Dark Force 1 at levels 38 to 46 on techniques and attacks alone, because
  the engine runs no player skill beyond Crosscut and Vortex. That says nothing about the level the
  cartridge's party fights it at, nor about the damage the enemy's three abilities deal in the
  cartridge: [ENEMY_ABILITIES_ROUTE.md](../battle/ENEMY_ABILITIES_ROUTE.md) records BURSTROC's and the
  event chains' campaign proof as open, and this run is that route result but not a cartridge comparison.
- **The training length.** 738 patrol battles (1,031,235 frames, over a quarter of the tape) is what the
  fixed policy needs for a margin on both clocks; a player with Rayblade would not grind. A change to any
  earlier chapter re-rolls the dice, and the target should be re-checked on both clocks.
- **Dark Force 1's intro.** `EnemyAttack_DarkForce1` loads an animation-only first act
  (ENEMY_ABILITIES_ROUTE, BURSTROC); the run does not check the first round against it.
- **Native play.** Headless `Session`, pads only. No Godot capture of Landale, the spaceport's rise,
  Kuran or the event battle.
