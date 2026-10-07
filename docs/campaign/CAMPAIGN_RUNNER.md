# Campaign runner

Opened 2026-10-01. Owner decision: replace per-segment native drivers and
segment ledgers with one headless campaign runner that plays New Game to the
Ending from a route file using ordinary joypad input, and halts at the first
blocker with a report. Each blocker becomes a fix lane. Native Godot checks
run at milestone saves by replaying the runner's own input tape.

## Why the previous method stalled

Through 2026-09-29 the native route reached the post-Rika north bank, roughly
the first sixth of the campaign, using 33 bespoke drivers in `tools/native/`
and one ledger per hop. The Aiedo hop (one warp, 67 steps) produced a 443-line
research ledger before any movement ([AIEDO.md](AIEDO.md)). That rate cannot
finish the game.

The root cause is architectural. An audit on 2026-10-01 found that the only
complete game loop lives in `psiv-godot`:

| Area | Lives in `psiv-godot` today | Runtime has |
| --- | --- | --- |
| Mode dispatch (`Game_Mode_Index`) | title, notices, game over, battle, pending `$F6`, dialogue, shop, camp, field priority; field input starvation; `set_field_suspended`, which changes the shared RNG stream (`lib.rs:460-655`) | `tick(Input)` for field and scenes only |
| Dialogue | `$FA` FlagCheck, `$F6` events, Yes/No branches, `Ctrl::Action`, `$F2` flag writes, `$F7` resume, a copied flag bank that can go stale (`dialogue/text_flow.rs`, `dialogue.rs`, `boot.rs`) | nothing that reads `Ctrl::` |
| Battle | command eligibility, target paging, defaults, item reservation, RUN, vehicle skill counts kept in a local copy; outcome and reward scraped from narration and passed back (`battle/commands.rs`, `battle/ui_input.rs`, `battle/ui.rs`) | rounds, plus a redundant `outcome()`/`pools()` |
| Shop, camp, loot | shop catalog parsed outside `psiv-data`, sell-price lookup, every menu state machine, equip decided by comparing a display string (`shop/`, `camp/`) | the state mutations only |
| Title and game over | CONTINUE/START/ERASE flow, empty-slot rules, game-over return (`title.rs`, `field_status.rs`) | constructors only |

Progression timing also lives there: the dialogue open animation and
typewriter gate page advance, battle dwell beats decide when the menu
reopens, and ORDER auto-closes on a frame count. A headless driver therefore
re-implements a subset (the runtime's own test harnesses ticked the field with
an `Input`, walked the dialogue preamble themselves, skipped mid-message
branches and `$F2` writes, and diverged from Godot's RNG stream; node S6 moved
those rule tests inside the crate, `rust/psiv-runtime/src/suites/`). That is why
each hop needed a Godot driver, and why a headless route is not evidence for the
shipped game.

## Target architecture

`psiv-runtime` gains one `Session`: the cartridge's main loop. Its whole
input surface is `Session::frame(pad)`, one call per 60 Hz frame with the
joypad state. It owns mode dispatch, the dialogue interpreter, battle command
menus and battle end, shop, inn, camp, loot, title, game over and every
frame-counted gate that affects progression. It exposes a read-only view
for presentation and a stream of presentation events (sound, transitions).

Godot sends the pad state each physics frame and draws the view. It decides
nothing about game state.

Consequences:

- The runner and the shipped game execute the same code on the same frames,
  so a headless run is evidence for the native game's rules and input path.
- A runner run emits a pad tape. Replaying that tape in Godot reproduces the
  run frame for frame, which replaces bespoke native drivers with one generic
  replay driver.
- The rule "Godot presents and sends input" becomes enforceable: runtime
  mutators become crate-private behind `Session`, so the Godot crate cannot
  call them (correction ladder rung 1).

## Route file

A route is an ordered list of chapters following the story order in
[docs/scenes](../scenes/README.md). Each chapter holds objectives the runner
resolves into pad input:

- reach a map and cell, path-finding across the pack's warp graph, on foot or
  in the vehicle the party rides (a mounted party moves two cells a press over
  the terrain its machine crosses, `psiv_core::can_enter`; the route says so
  with `"vehicle": N` in an `expect` and ends it with `dismount`);
- talk to an object, answer a choice, open a chest, use a field ability;
- buy, sell, equip, rest at an inn, reorder the party;
- `interact` with a cell (doors and elevators that open walls), and `patrol`
  between two cells until a condition holds (grinding the route requires),
  with an optional `refuge` list the patrol runs when a member falls or a
  living member drops under half HP;
- `step_onto` a cell where a map trigger starts a scene. On a warp's footprint
  the scene runs before the warp can fire (`RunEvents` runs before
  `RunMapTransitions` on foot, `ps4.asm:116768-116773`; the Hangar's Landale
  row): `go_to` plans a footprint as a terminal, never as a goal, and
  `step_onto` plans the firing step as an ordinary one and halts if the warp
  fires instead of a scene. On any other trigger cell (the carnivorous trees'
  corridor, `RunEvent_CarnivorousTrees`) it is the plain walk that ends there,
  because a `go_to` plans again from where the scene leaves the party and fires
  the trigger a second time; it halts when the party stands on the cell and no
  scene ran;
- `wait` frames while the field runs (an object a scene sent walking, the
  Esper Mansion's guards, finishes before the next walk is planned around it),
  and `opens` on a `talk` for the cells such a conversation clears, as on an
  `interact`; `board` without a `step` answers the destination menu a scene
  opens by itself (`Cutscene_FindingAirCastle`);
- fight scripted battles; random battles use a policy that issues commands
  through the battle menu with the same pad input a player would use.

Chapters end with their specified assertions on flags, party, map and
inventory taken from the scene transcriptions. The runner then writes a
read-only chapter checkpoint with `Runtime::save_slot`; that snapshot is not
an ordinary SAVE input. Only chapters with an explicit `save` objective press
camp STATE → SAVE. The Zio-defeat chapter does so after its scene and flags
settle. The runner halts on the first
failed objective, unsupported ability, scene fault, lost battle or stuck walk,
and writes a report: chapter, objective, frame, map, cell, mode and the last
events.

## The runner

`rust/psiv-campaign` plays a route. It builds a `Session`, then presses one
joypad byte per frame until each objective is met, reading the session's
views to decide the next press. The durable record of its runs, with every
halt and its diagnosis, is the [runner log](RUNNER_LOG.md).

```text
cargo build --release --manifest-path rust/Cargo.toml -p psiv-campaign
rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json \
    --save-dir build/campaign/run1
rust/target/release/psiv-campaign replay build/campaign/run1/run.tape
```

`run <route> [--from-chapter ID] [--until-chapter ID] [--save-dir DIR]
[--tape OUT] [--report OUT] [--pack DIR]`. The save directory defaults to
`build/campaign`, the tape to `<save-dir>/run.tape`, and a halt's report to
`<save-dir>/halt-report.json`. Exit status: 0 completed, 2 halted (a report was
written), 1 a usage or setup error. Release builds play the whole route in
about half a second; a debug build is an order of magnitude slower and plays
chapter one in three seconds.

A completed run's `--report` file lists its chapters (route position, id,
frames, battles, save path and the FNV-1a 64 of the chapter save) beside the
final digest and the frame count; a halted run's report is the halt report
below. `split-tape <run.tape> <report.json> <out-dir>` cuts the run's tape at
those frames into one `NN-id.tape` per chapter through the shared tape codec:
the first piece keeps the run's start and every later piece starts from the
previous chapter's save. The pieces concatenate back to the run's tape
(`rust/psiv-campaign/tests/split.rs`), and that is all they promise: a piece
replayed from its predecessor's save plays other battles, because a slot holds
no RNG state ([R2](#r2-native-tape-replay-in-progress) shows the proof).

**The boundary.** The runner calls `Session::frame(pad)` and reads
`Session::runtime()` and its `&self` methods. It never names a `&mut Runtime`
member, and a test greps for it (`the_runner_reaches_no_runtime_mutator`).
Construction goes through the runtime's own constructors
(`rust/psiv-runtime/src/session/start.rs`, node S6): `Session::start(data)
.with_battles(files)` then `new_game()` for START — the retail initializer with
the opening fired — and `from_slot_bytes(bytes, slot)` for a chapter save. Those
are the only two calls in `src/start.rs` that are not pads, and everything after
them is a session.

**Controllers**, one per objective kind, each in its own module and each
producing pads from views only:

| Objective | Module | What it presses |
| --- | --- | --- |
| `go_to`, `go_to_map`, `patrol`, `dismount` | `walk.rs` (plans in `cell_plan.rs`, `map_plan.rs`) | a direction for a step, re-planned from the real cell whenever the party comes to rest somewhere the plan did not expect; R0's planner over the live map, and a warp graph built from the flags the game holds; a yes/no prompt that a scene opens on the target map (Chaz's house offers a rest on arrival) ends `go_to_map` with the prompt open, for the next `answer`; so does the ship's destination menu; the warp graph is built from the pack's records, so a cell where a record puts an object the game has since moved off (the Esper Mansion's door guards) is opened in it before the chain is planned; mounted, the same plans run on the vehicle's lattice (`Mover::Vehicle`: the standing cell is the vehicle's, a step is two cells, the four-cell footprint decides the terrain and the warp rule, as `VehicleState::tick` does), and `dismount` presses Action on open ground |
| `talk`, `open_chest`, `interact`, `answer` | `talk.rs` | walks next to the object (or across its counter), turns, presses Speak, and reads what opened; Cancel is retail's direct NO |
| `wait`, `board` | `field.rs`, `ship.rs` | `wait` presses nothing; `board` holds a step until the destination menu opens, or waits for the menu a scene opens (`go_to_map` ends with such a menu open, for the next `board`) |
| `buy`, `sell`, `rest_inn` | `shopping.rs` | the shop view's pages, rows and cursors |
| `equip`, `use_technique`, `use_item`, `reorder`, `save` | `camping.rs` | the camp view's pages; SAVE is answered by the driver with the file the runner writes; a boarding item (the Ice Digger, `ItemAction_IceDigger`) closes the menu on the press that accepts it, so `use_item` waits the scene out instead of reading a page |
| random and scripted battles | `battle.rs`, `policy.rs`, `policy_boss.rs` | the battle view's menus (ATTACK, TECH, SKILL, ITEM), one press at a time |
| `expect` | `expect.rs` | nothing: it settles the game and reads flags, map, cell, party and purse |

`plan --vehicle N` prints a walking plan for a mounted party. Mounted walks and
the validator's `vehicle` claim exist because Motavia's sand blocks a walker and
the Land Rover crosses it: the Ladea Tower cannot be reached on foot from Krup
(`tests/cli.rs::plan_vehicle_crosses_the_sand_to_the_tower`).

`field.rs` is the loop every controller calls first: it plays frames until the
game hands control back, turning finished pages with Speak, acknowledging chest
results, fighting battles with the chapter's policy, and waiting out scenes.

**Battle policy.** `random_battle_policy` names one (`policy.rs`). Everyone
attacks the first living enemy; the first member who can cures the most hurt
eligible target below half HP with a healing technique, or failing that a
healing item. Both use the core target lists that build the command menu;
`run_unless_boss` runs from random encounters and fights scripted battles.
After a battle the party is cured through the camp (`recovery.rs`). Losing is a
halt. `attack_all` and `heal_then_attack` resolve to the default policy; a
route that needs another behaviour gets a type in `policy.rs` first.

`bioplant_survival` runs from every random encounter, like `run_unless_boss`.
`train_with_inn` fights and retreats (RUN) once a member has fallen or a living
one is under a third of their HP, so the inn that ends a training patrol cures
a party and does not find a dead one. Both exist because the cartridge's random
draws are one stream: any frame the runtime spends or saves anywhere moves
every later encounter (the cutscene-return reload,
[`FIELD_RELOAD.md`](../scenes/FIELD_RELOAD.md), moved all of them), and a route
that survives on only one stream is not a route.

`fight_to_win` and `run_then_win` (`policy_boss.rs`) are what a player does
against a boss, which the default policy loses to: the most hurt eligible member
under half HP gets the cheapest sufficient single-target cure (the strongest
when none is sufficient), and a learned all-human cure can also cover two or
more eligible humans below 70% HP in that round. A sealed caster avoids TECH
as a player heuristic; the retail menu still permits the wasteful selection.
Everyone else takes the damage action with the highest estimated damage to the
first living enemy: a plain attack, a damaging technique they can pay for, or a damaging skill with
uses left (`Crosscut`, `Vortex`: the two the engine runs). The estimate is the
cartridge's damage formula at its mean roll on the live fighters' stats, which a
player learns from the first rounds' damage numbers. `fight_to_win` fights every
battle; `run_then_win` runs from random encounters like `run_unless_boss` and
fights scripted ones this way. Juza (event battle 3) and Gy-Laguiah (event battle
5) are won with it; with the default policy Juza kills a level 12 party in four
rounds (run C2-1).

`psycho_wand_then_win` (`policy_opening.rs`) is `run_then_win` with an opening
item: in the first actual player-command round of a scripted battle the first
actor takes ITEM and picks the Psycho Wand (item `$39`, when the pack holds it),
everyone else fights, and afterward it is the boss policy. An enemy-only
ambush round offers no item choice and does not disarm the opening. It decides
an item intent only; `battle.rs`
steers the ITEM page to the row, as for a healing item. The driver tells every
policy when a battle begins and whether it is scripted (`Policy::battle_begins`),
which is how a random encounter gets no opening. The Zio fight at Nurvus is the
chapter that names it (`nurvus-zio`); `tests/runner.rs` pins the policy's
choices over a pack that holds an item bought at the Piata shop.

**Halts and the report.** Each objective has a frame budget. The run stops at
the first of: a missed `expect` or closing assertion, an exhausted budget, an
unsupported enemy ability, a scene fault (a faulted or missing scene, an
unsupported trigger, an unpacked warp target, an unmapped type-1 cell unless the
same frame began a scene or the cell borders a warp footprint (a doorway row
whose warp the cartridge keeps one row on, `AirCastle_F1` warp 0), a refused
battle round), a lost
battle, an unreachable target, a menu with no such entry, an object the route
named that is not the one the game reached, or a state the objective cannot
continue from. The report is JSON: `chapter`, `objective_index`,
`objective_kind`, `objective`, `halt {kind, detail}`, `frame`, `map`, `cell`,
`facing`, `mode`, `party` (HP, TP, level, status), `money`, the last 50 runtime
`events`, `surroundings` (the collision values around the party),
`nearby_warps`, `view` (the open dialogue, shop, camp, battle or chest) and the
`battles` fought. `PSIV_CAMPAIGN_TRACE=1` prints every event and every battle
decision to stderr as the run goes.

**Tape, saves and replay.** The tape is every frame's pad byte, run-length
encoded in a text file (`rust/psiv-runtime/src/tape.rs` documents the format;
`psiv-campaign::tape` reexports the same codec). A chapter that
completes writes the read-only checkpoint `<save-dir>/NN-id/slot_1.sram`, and
`--from-chapter ID` starts from the preceding chapter's checkpoint; the tape
then names that file by hash. Only route `save` objectives write
`<save-dir>/route/slot_N.sram` through the camp's SAVE page. `replay <tape>
[--from-save FILE]` plays the tape back and
prints the digest of the final state (map, cell, facing, purse, party and the
whole persistent snapshot), which a run printed too and a replay must
reproduce. `inspect <slot.sram>` prints a save's
position, purse, party (levels, HP, stats, equipment), pack and set event flags
through the runtime's own views: the question a route author asks of every
chapter save. A save loaded mid-route restarts the frame counter and RNG, so a
`--from-chapter` run is the same game from there but not the same frames as the
full run; only the full run from New Game is the route's evidence.

### R2 native tape replay (in progress)

`tools/verify_native_tape.py` is the generic Godot entry point. It opens one
campaign tape through the runtime-owned `TapeFeed` codec, checks a save-start
tape's FNV against the source bytes **before** copying them into a fresh,
isolated `PSIV_SAVE_DIR`, and sends each byte via ordinary `Input` actions.
The title receives ordinary START/CONTINUE button edges; the tape begins at
the first gameplay `Session::frame` call after that handoff. A central Field
observer counts actual session invocations in both field and battle dispatch,
records the last consumed pad, and excludes title frames. The driver stops at
exactly N bytes, freezes the Field, and checks that two more SceneTree callbacks
made no N+1 session call. It then compares `Runtime::slot_bytes(&self)` with a
chapter save byte-for-byte. That read-only snapshot is the normal SAVE writer's
encoding; equality does **not** by itself prove a camp SAVE action occurred.

```bash
PSIV_HEAVY_LOCK=/absolute/path/to/primary/build/continuation-heavy.lock
CARGO_BUILD_JOBS=1 flock -x "$PSIV_HEAVY_LOCK" \
  python3 tools/verify_native_tape.py --tape /path/to/run.tape \
  --expect-save /path/to/chapter/slot_1.sram --pack /path/to/runtime-pack \
  --out build/native-tape-academy
```

Use the same absolute ignored lock path from every worktree; a relative
`build/` lock serializes only that one checkout.

For a save-start tape, add `--from-save /path/to/preceding/slot_1.sram`.
For a fresh-process CONTINUE check of a newly written slot, use
`psiv-campaign save-probe-tape <slot.sram> <out.tape> [neutral-frames]` to
create a save-start tape through the same Rust codec. The optional count
defaults to zero and is capped at 600. A bounded neutral-pad tape lets the
normal title fade finish and the loaded field render; every neutral pad still
counts as one real gameplay Session frame. Pass that tape and its source slot
to the verifier. The probe loads through ordinary title input without
claiming a new SAVE edge.
If the tape uses the camp SAVE command, `--expect-written-save` compares the
slot the native game actually wrote in its isolated save directory with the
runner's route SAVE file; that is distinct from the read-only endpoint
snapshot. It also requires the successful slot-1 SAVE edge returned by
`Session::camp_frame` and counted at the real Field session boundary: a
save-start tape's pre-copied slot cannot pass on byte equality alone.
`--expect-map` and `--expect-cell x,y` guard the endpoint. The entry point
requires a fresh output directory and retains `receipt.json`, the native
report, exact snapshot bytes, and Godot logs. Its receipt records source-save
SHA-256 before and after, the final live slot hash, selected pack hash, command,
effective replay configuration and elapsed time. It also records the Git HEAD,
dirty diff and authoritative tracked-plus-untracked repository-entry identities
(symlink targets are never opened),
exact pre/post GDExtension, driver, verifier and Godot identities, and the
extension path/inode observed in the Godot process map. A changed source,
tape, pack manifest or executable artifact fails the receipt. A timed-out
launch terminates its own Godot/Xvfb process group and records exit 124. On a
fresh checkout it first runs a local one-shot headless Godot editor scan to
register the GDExtension. It strips inherited `PSIV_*` selectors, then
enables only the read-only probes and explicit local inputs.
Headless checks disable only the render loop;
the Field physics callback and every input/Session boundary still run. Selected
1280×800 captures use a separate rendered run and are separate
presentation evidence, not proof of input or persistent state.

**The whole route, 2026-10-06.** `tools/verify_native_route.py` replays a
completed run's tape once, from New Game, through the same driver and checks
every chapter save at the frame its chapter ended on. `native_tape.gd` takes
a checkpoint list (`PSIV_TAPE_CHECKPOINTS`: frame, label, expected save) and
compares `Runtime::slot_bytes` with each saved chapter the moment the Field has
consumed that many Session frames; the first mismatch fails the run naming the
chapter, its frame and the first differing byte, and keeps the native bytes next
to the receipt. `--until-chapter ID` replays the prefix that ends with a chapter
(`PSIV_TAPE_STOP_AT`). The script writes one receipt per chapter
(`chapters/NN-id.json`: frames, wall time, native snapshot SHA-256, match) and a
`summary.json`/`summary.md`, and refuses a run directory whose chapter saves no
longer hash to the report's `save_fnv`.

```bash
# Once, in the worktree that will replay (never while a Godot process has the
# extension loaded): an extension that replays at about 1,000 frames/s.
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_OPT_LEVEL=3 CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=false \
  cargo build --manifest-path rust/Cargo.toml -p psiv-godot
CARGO_BUILD_JOBS=2 cargo build --release --manifest-path rust/Cargo.toml -p psiv-campaign
rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json \
  --save-dir build/r2-route --tape build/r2-route/run.tape --report build/r2-route/report.json
PSIV_HEAVY_LOCK=/absolute/path/to/primary/build/continuation-heavy.lock
flock -x "$PSIV_HEAVY_LOCK" python3 tools/verify_native_route.py \
  --run-dir build/r2-route --out build/r2-native
flock -x "$PSIV_HEAVY_LOCK" python3 tools/verify_native_route.py \
  --run-dir build/r2-route --out build/r2-native-continue --continue-probes
```

**Why one process, not one per chapter.** The brief for this node was to replay
each chapter from the preceding chapter's save. That cannot reproduce the run:
a slot holds no RNG state and no frame counter, so a loaded game restarts both
and the same pads play other battles. `psiv-campaign run --from-chapter holt`
from the Academy save plays 11,408 frames (the 2026-10-03 figure) against the
full run's 6,933 and writes a slot that differs from the full run's at byte 44;
the split tape of that chapter, replayed natively from the same save, fails its
slot comparison. This is the runtime's contract, not a Godot divergence (the
headless replay shows the same), so the whole tape replays once and the
checkpoints carry the chapter saves. `split-tape` is kept because the pieces
add up to the tape, but no check replays one alone.

**The extension build.** The plain debug extension replays chapter one
(32,308 frames) in 359.7 s, 90 frames/s, which would take about eleven hours
for the route. Sampling the process shows the time in the YM2612 core
(`ym3438.c`, which `cc` compiles at -O0 in a dev profile) and in godot-rust's
strict safeguards, not in the session. The same chapter takes 30.5 s, 1,059
frames/s, with `CARGO_PROFILE_DEV_OPT_LEVEL=3` and debug assertions off. The
library lands at the path the `.gdextension` names, so a later plain
`cargo build -p psiv-godot` replaces it with the slow one again; the receipt
records the library's SHA-256 and the mapped path and inode. Debug assertions are
off in this build and on in the Rust suites.

**Result, at clean `93c7453`.** Run directory `build/r2-route` (ignored; release
`psiv-campaign` run: exit 0, 3,743,328 frames, digest `40c7f505ecc5c357`, tape
SHA-256 `db8ae7fe…ae70`, 24 `save` objectives). The replay ran from a clean clone
of that commit with the optimized extension (SHA-256 `75fce4d1…3e0f`, mapped
path and inode matched the preflight), pack manifest `02bb29ad…11f2`, exit 0 in
5,945.3 s (99 minutes), identity stable before and after. Every one of the 50
chapter saves matched the native snapshot byte for byte:

| # | Chapter | Frames | Battles | Wall (s) | Frames/s | Native slot bytes vs chapter save |
| ---: | --- | ---: | ---: | ---: | ---: | --- |
| 0 | `academy` | 32,308 | 4 | 29.3 | 1102 | match |
| 1 | `holt` | 6,933 | 3 | 8.4 | 824 | match |
| 2 | `rune-dorin` | 17,911 | 11 | 17.0 | 1054 | match |
| 3 | `basement-training` | 42,212 | 23 | 71.5 | 590 | match |
| 4 | `alshline` | 9,316 | 9 | 10.2 | 909 | match |
| 5 | `zema-rescue` | 13,244 | 7 | 13.3 | 998 | match |
| 6 | `zema-outfit` | 1,307 | 0 | 3.4 | 383 | match |
| 7 | `zema-training` | 4,752 | 4 | 5.2 | 916 | match |
| 8 | `zema-armour` | 718 | 0 | 1.3 | 558 | match |
| 9 | `bioplant-elevators` | 5,756 | 4 | 8.2 | 698 | match |
| 10 | `bioplant-order` | 60 | 0 | 0.3 | 192 | match |
| 11 | `bioplant-rika` | 19,010 | 3 | 18.9 | 1007 | match |
| 12 | `north-bank` | 1,307 | 1 | 1.9 | 694 | match |
| 13 | `aiedo` | 1,185 | 1 | 1.7 | 703 | match |
| 14 | `aiedo-chaz-house` | 1,130 | 0 | 0.7 | 1624 | match |
| 15 | `aiedo-training` | 859,087 | 630 | 1440.4 | 596 | match |
| 16 | `aiedo-shopping` | 1,132 | 0 | 3.6 | 311 | match |
| 17 | `passage-to-zio-fort` | 2,689 | 2 | 2.3 | 1152 | match |
| 18 | `zio-fort-approach` | 2,680 | 4 | 4.0 | 671 | match |
| 19 | `zio-fort-juza` | 3,406 | 2 | 7.4 | 458 | match |
| 20 | `zio-fort-demi` | 17,998 | 3 | 18.0 | 1002 | match |
| 21 | `krup-training` | 1,528,066 | 1874 | 2667.0 | 573 | match |
| 22 | `motavia-machine-center` | 1,089 | 0 | 1.4 | 790 | match |
| 23 | `machine-center-control-key` | 4,837 | 0 | 4.4 | 1106 | match |
| 24 | `ladea-tower-rune` | 5,446 | 6 | 5.4 | 1012 | match |
| 25 | `ladea-tower-psycho-wand` | 13,285 | 4 | 12.9 | 1026 | match |
| 26 | `zio-fort-barrier` | 5,931 | 9 | 6.1 | 970 | match |
| 27 | `nurvus-descent` | 4,590 | 6 | 5.8 | 794 | match |
| 28 | `nurvus-zio` | 8,393 | 1 | 12.3 | 682 | match |
| 29 | `mota-spaceport` | 2,346 | 1 | 1.6 | 1430 | match |
| 30 | `zelan-wren-canceller` | 6,944 | 0 | 5.5 | 1268 | match |
| 31 | `zelan-sabotage` | 13,919 | 1 | 11.8 | 1180 | match |
| 32 | `dezolis-first-control` | 98 | 0 | 0.0 | 12250 | match |
| 33 | `dezolis-outside-raja-temple` | 1,339 | 0 | 1.6 | 843 | match |
| 34 | `dezolis-gyuna` | 1,944 | 0 | 1.3 | 1446 | match |
| 35 | `dezolis-tyler-grave` | 2,357 | 1 | 2.3 | 1018 | match |
| 36 | `dezolis-tyler-prepare` | 1,794 | 0 | 2.6 | 687 | match |
| 37 | `dezolis-landale` | 2,691 | 2 | 2.7 | 986 | match |
| 38 | `dezolis-training` | 1,031,235 | 738 | 1474.1 | 700 | match |
| 39 | `kuran-arrival` | 2,668 | 0 | 1.4 | 1952 | match |
| 40 | `kuran-elevators` | 5,494 | 10 | 6.3 | 870 | match |
| 41 | `kuran-near-dark-force` | 663 | 0 | 0.4 | 1645 | match |
| 42 | `kuran-dark-force-1` | 11,312 | 1 | 13.0 | 870 | match |
| 43 | `dezolis-ice-digger` | 2,443 | 0 | 1.9 | 1267 | match |
| 44 | `meese-raja-sick` | 4,599 | 0 | 3.0 | 1551 | match |
| 45 | `dezolis-saving-kyra` | 7,689 | 1 | 8.6 | 890 | match |
| 46 | `esper-mansion` | 2,972 | 1 | 2.1 | 1390 | match |
| 47 | `esper-inner-sanctuary` | 9,452 | 0 | 6.7 | 1409 | match |
| 48 | `gumbious-torch-stolen` | 9,615 | 1 | 7.4 | 1292 | match |
| 49 | `air-castle-arrival` | 5,976 | 3 | 6.5 | 924 | match |
| | **50 chapters** | **3,743,328** | | **5943** | **630** | **50/50 match** |

The route's 24 `save` objectives were answered by 24 ordinary camp SAVE
acknowledgements (slot 1; slots 2 and 3: none), and the slot file the native
game wrote at the end equals the runner's last pad SAVE (`route/slot_1.sram`,
SHA-256 `4aeff0b1…ec62`). The final snapshot is chapter 49's save
(`06feecfa…41d7`) at map `$171` (63,54), and two further callbacks after the
last pad made no extra Session call. The three training chapters, 91 percent of the frames, run at 570 to 700 frames/s;
Godot's resident memory grew from 470 MB to 2.3 GB over the replay (about
20 MB a minute, which tracks the battle count and is not investigated here).
The log carries 1,383 map-0 battle-background fallbacks (issue #40), 43
scene-panel diagnostics and 2 type-1 doorway notices, none tied to a state
difference.

**Persistence.** The replay never loads a save. `--continue-probes` loads each
of the 50 chapter saves in its own Godot process through the ordinary title
CONTINUE (a zero-pad save-start tape from `psiv-campaign save-probe-tape`) and
requires the snapshot to re-encode the same bytes: 50 of 50 pass in 103 s, source
saves unchanged, at clean `138da9f` (`build/r2-native-continue/continue-summary.json`).

**Negative controls.** A chapter save with one byte flipped (and the report's
hash made to agree) fails at that chapter: `01-holt` differs at byte 1000,
chapter `02-rune-dorin` is not reached, exit 1. A pad dropped inside `holt`
stops the replay on the driver's pad boundary before that chapter's checkpoint
(`academy` had matched), exit 1.
`tests/test_verify_native_route.py` covers the refusals (stale run directory,
foreign report, missing save), the per-chapter wall times, a divergence's first
byte, a driver pass that hides a differing chapter, and the probe summaries;
the `split` tests were mutated (a chain hash off by one chapter) and fail.

The replay is a headless run with the render loop off: it proves input, state and
the save writer, not the picture, and it takes no mid-run capture. It does not
replace the fresh-process CONTINUE probes, the selected rendered captures above,
or the fixtures that remain ([native drivers](NATIVE_DRIVERS.md)).

**Combined post-Zio checkpoint, 2026-10-03/04.** The selected pack is the ignored stable
copy at `/home/peter/PSIV/build/accepted-p1-pack-7fe1e64a` (manifest SHA-256
`7fe1e64abfb4d55230a1039f5ac2deea4b45f6e94e5b029bba10107a39a016de`).
Raw receipts and logs are under ignored `build/r2-integration/` here and in
the stable primary checkpoint
`/home/peter/PSIV/build/accepted-combined-5ec096f/worktree/build/r2-integration/`;
they are not Git assets. The failed five-stage gate has a separate immutable
receipt under `build/gate/20261003T231702Z-5ec096f/` and is not called green.
The affected Python/docs recheck, repaired-CLI proof, primary default-pack
proof, and all raw logs are retained separately in that stable checkpoint.

| Native replay | Observed result | Raw receipt |
| --- | --- | --- |
| New Game → Academy | 32,308 exact Session pads; chapter snapshot matches | `native-academy-current-2/receipt.json` |
| Earned Academy CONTINUE → Holt | 11,408 pads, camp SAVE acknowledgement, snapshot matches | `native-holt-current/receipt.json` |
| New Game → initial Aiedo | 156,019 pads in 2,066.765 s, title START t7 → first gameplay Session t8, two post-stop callbacks without N+1, map `$54` (47,83), snapshot SHA-256 `2d82632af8f7a001483b04493d294b5b8325bc631337cae471716bf02d0c0ef2` matches the full-run chapter | `native-aiedo-current/receipt.json` |
| Earned pre-Zio save → Zio, then fresh-process CONTINUE | 8,823 pads, one ordinary camp SAVE acknowledgement and matching written slot; separate zero-pad ordinary CONTINUE reloads that slot with matching bytes | `native-zio-segment/receipt.json`, `native-zio-continue-zero/receipt.json` |
| Rendered earned-save → Zio | 8,823 pads, same SAVE and endpoint bytes, pre/post identities stable, actual mapped extension matches; selected 1280×800 post-Zio field capture SHA-256 `a094885723bbe7b850ef263146334ebeeb4588635ce4912e8600eff17ed91983` | `native-zio-rendered-segment/receipt.json`, `selected-1280x800.png` |
| Rendered fresh-process CONTINUE from that native-written slot | 180 neutral pads in the tape, 180 exact Session frames, no N+1, source/snapshot bytes SHA-256 `8d831c47fc859257e28c46ead3583668fe81b54b4d8eb5ce8eeb798427818baa`; selected 1280×800 loaded Motavia field SHA-256 `5b2698c5bfe618348a495b0d115bbd4e7c24638913b4c34ec4a3e0f8a278703f` | `native-zio-visible-continue/receipt.json`, `selected-1280x800.png` |
| Repaired verifier CLI on clean `e9372bc` | One rendered 180-pad CONTINUE, title handoff then exact Session frames and no N+1; pre/post source, pack, driver, Godot and library identities stable; mapped extension path/inode matched; source/live/snapshot/expected slot all `8d831c47…`. Zero SAVE acknowledgements. | Stable primary `build/accepted-combined-5ec096f/native-cli-e9372bc/receipt.json` and `APPENDIX.json` |
| Ordinary primary default-pack CONTINUE | Detached primary `e9372bc`, freshly built extension SHA-256 `97bbb622…`, `PSIV_RUNTIME_PACK` **absent**; dev `runtime-pack` resolves to the accepted stable pack. Ordinary title CONTINUE and 180 exact neutral Session pads match the same `8d831c47…` slot. Mapped primary extension inode matched preflight; source/pack/code hashes stayed stable. The inspected 1280×800 field capture is SHA-256 `5b2698c5…`; no new SAVE. | Stable primary `build/accepted-combined-5ec096f/primary-default-180cont-e9372bc/receipt.json`, `VISUAL_REVIEW.json`, `APPENDIX.json` |

The earlier native receipts used the prior untracked-entry source guard and
remain historical. The scoped verifier repair uses `tools/repo_files.py` for
the complete non-ignored file set, including dangling symlinks, and retains
`lstat`/`readlink` identity without following linked asset directories. Its
focused owner/negative tests pass. The actual repaired-CLI 180-pad replay in
the table passed in 14.596 seconds; the primary no-override replay passed in
14.587 seconds. Neither reran the long Aiedo or Zio tape. The primary's older
real pack was hash-checked against all 4,787 archived files and moved to a
named ignored backup before `runtime-pack` became a link to the accepted pack;
the former primary extension and the tested rebuilt extension are retained
separately. No private-worktree pack path is needed for a default launch.

The long Aiedo receipt predates the preflight identity guard. Its
`native-aiedo-current/during-run-provenance.json` binds the active Godot PID's
mapped library path/inode and its then-current SHA-256; the separate
`after-run-provenance.json` shows the same on-disk script/library identities
after exit. That is during/after observation, **not** a backdated preflight
claim. The later rendered Zio run has full preflight/mapping/postflight
identity proof. Negative controls retain malformed tape, wrong source hash,
wrong endpoint, dropped pad, and copied-source-without-SAVE failures; the
identity guard's temp-artifact tests change each of the driver and extension,
retarget a directory symlink, and prove process-group cleanup of a child that
ignores TERM. The integrated stub timeout receipt is
`provenance-timeout-negative/receipt.json`: exit 124, verification refused,
and its child absent after cleanup. `save-probe-negative.json` records the
existing-output and over-600-frame refusals with no overwritten file. Earlier
failed import, cell-format and terminated
attempts remain in the ignored ledger. The Aiedo run logged five map-0
background errors; map-0 fallback predates this candidate (issue #40). Its
seven Alshline panel warnings are a false typed art-ID diagnostic for retail
operand-free stack pops, tracked by [#74](https://github.com/TusanHomichi/PSIV/issues/74)
and the [scene op ledger](../scenes/SCENE_PRESENTATION.md#op-coverage); no
visual mismatch was observed in that classification, and no source fix is
claimed here. The selected Zio field image is a rendered milestone, not a
scene-wide oracle comparison.
The older `native-zio-rendered-continue-2` zero-pad capture is almost black
because it freezes during the title fade; its state/save proof remains valid,
but that image is **not** evidence of a visible loaded world. The 180-pad
fresh CONTINUE capture above supplies that separate presentation observation
without advancing any untaped gameplay frame. It makes no new SAVE request;
the ordinary SAVE evidence is the preceding Zio segment. The fresh CONTINUE
receipt's `ordinary_save_matches: false` means `--expect-written-save` was not
requested for that no-SAVE tape.
The 11-pair static certificate on this candidate passes with zero RMSE,
including unchanged battle-0x88. The sole five-stage gate on clean `5ec096f`
passed four stages and failed Python on 31 missing local oracle-frame errors
and one file-list authority guard; that receipt remains FAILED. The protected
184 receipt-named PNGs were copied with source/copy readback. The scoped repair
at clean `e9372bc` passed 18 focused owner tests and one affected recheck:
docs checked 176 files with zero problems, full Python ran 1,241 tests with
zero failures/errors and one skip. The composed five-row matrix applies the
prior fmt/Rust (1,287 passed, three ignored)/Clippy results through an unchanged
558-file Rust/config input map; it is not a new full five-command gate.
The checkpoint above left the bespoke-driver retirement open. The whole-route
replay and the [native driver inventory](NATIVE_DRIVERS.md) answer it: twelve
campaign-path drivers are retired, twenty fixtures and observers stay (with two
of the three verifiers) for branches and captures the tape does not carry, and
`native_opening.gd` (a rendered capture) and `verify_native_alarm.py` (whose
producer was retired) stay until the tape replay takes frame-addressed captures
([#93](https://github.com/TusanHomichi/PSIV/issues/93)).

**Tests.** `cargo test --manifest-path rust/Cargo.toml -p psiv-campaign --
--test-threads=1` runs the planner and validator suites and
`tests/runner.rs`: chapter one twice gives identical tapes, digests and saves;
a tape replays to its digest, also from a chapter save; a wrong object index,
an expectation that cannot hold, an unreachable cell and a spent budget each
halt naming the objective; the exit statuses; `tests/spaceport.rs` walks onto the
Mota Spaceport's boarding row, whose scene must not halt on its type-1 cell;
`tests/split.rs` checks that a completed report names its chapters with the
hash of each save, that the cut tapes add up to the run's tape and chain through
those hashes, and that a split refuses a halted report, a foreign tape and an
existing output. The
whole route is `#[ignore]`d:
`cargo test --release --manifest-path rust/Cargo.toml -p psiv-campaign --test
runner -- --ignored --test-threads=1`. Cases that need the pack skip with a
message when it is absent.

## Task graph

```yaml
outcome: "A fresh install plays New Game to the Ending headlessly from the route file with ordinary pad input, and milestone tapes replay identically in Godot"
canonical_record: "docs/campaign/CAMPAIGN_RUNNER.md#task-graph"
authority: "Owner 2026-10-01: campaign runner approach (docs/AGENT_WORKFLOW.md#authority-effort-and-continuation). The bounded post-Zio checkpoint has reviewed local evidence; publication and integration follow the workflow authority record. Later-arc implementation is parked for the next session."
effort_policy: "Continue scoped repairs until acceptance passes; no fixed cycle limit (inherited)"
exclusions: ["modding", "visual-parity claims beyond existing certifications", "gameplay changes that are not cartridge behavior", "later arc beyond Zio defeated in the current lane"]
next_action: "C8: restore air-castle-xe-athoul-room (RUNNER_LOG_ICEDIGGER.md H38, no wait) and play on through Event_XeAThoulBeforeBattle, Lashiec and Dark Force 2"
nodes:
  - id: S1
    outcome: "Dialogue interpreter in psiv-runtime: control codes, branches, choices, actions, $F2/$F6/$F7, live flags, typewriter and open-animation gates, driven by a cartridge-layout Pad"
    depends_on: []
    acceptance: "psiv-godot no longer matches on Ctrl::; the window renders a runtime view; the TextFlow pagination test still matches all 2,736 entries; mid-message branch, $F2 and live-flag regression tests; the headless example harness's own dialogue walk is deleted; opening p1/p2 and MeetingRika certifications stay 0.000000"
    evidence: ["build/lane-evidence/workspace-tests.txt", "build/lane-evidence/clippy.txt", "build/lane-evidence/fmt.txt", "build/lane-evidence/examples-build.txt", "build/lane-evidence/python-checks.txt", "build/lane-evidence/testnames-psiv-godot.diff", "build/lane-evidence/testnames-psiv-runtime.diff", "build/lane-evidence/ctrl-grep.txt", "build/lane-evidence/negative-controls.md", "integration: tools/certify.py on candidate 46810a3 vs main e9e58a2 baseline: meeting-rika, title, battle 0.000000 on both; opening-p2 and camp-root captures byte-identical to baseline; opening-p1 differs by the expected two-frame retail-pace shift (dialogue close t278 -> t276); opening and camp pairs had already rotted on main (#44), so their 0.000000 is restored there, not here", "integration: native opening smoke (title START, trigger 124, Up) reaches map $13 (48,18), Chaz alone, 500 meseta, town 80008040"]
    state: verified
  - id: S2
    outcome: "Session::frame(pad) owns mode dispatch, field, scenes, dialogue, pending $F6 and field suspension; Godot's lib.rs dispatcher calls it"
    depends_on: [S1]
    acceptance: "Godot's per-frame dispatcher is replaced by Session::frame; the RNG-relevant field suspension and the dismiss-press latch are decided in the runtime; all existing tests pass; the six certified captures stay at rmse 0.000000; the opening native driver passes unchanged"
    evidence: ["lane s2-session (base 64d9d7b), 2026-10-01: Session::frame(pad) + Session::window_tick() own the dialogue input half, a pending $F6 and the field or scene tick; the dismiss latch and the scene/window starvation live in rust/psiv-runtime/src/session.rs; build/lane-evidence/S2-NOTES.md indexes the raw output", "workspace 1094 passed / 0 failed / 1 ignored (63 targets); clippy --workspace --all-targets -D warnings clean; fmt --check clean; examples build; academy_route exits 0 (build/lane-evidence/{workspace-tests,clippy,fmt,examples-build,academy-route}.txt)", "test names: psiv-runtime +5, nothing removed (three FieldObj_MovementsTbl mask cases, the headless opening test and its negative control), psiv-godot +0 of 57 (build/lane-evidence/testnames-*.diff)", "headless opening test rust/psiv-runtime/tests/session_opening.rs: Runtime::new_game through Session::frame with pads only reaches map $13, standing cell (48,18), Chaz alone, no scene, 500 meseta, town flags 80008040; negative control (neutral pads) leaves the box up and the scene active (build/lane-evidence/negative-control.txt)", "fidelity fix: the pad->Input direction order is now the cartridge's FieldObj_MovementsTbl (ps4.asm:93675): an opposing pair cancels and a horizontal beats a vertical, where the shell's read_input was Up first; named in rust/psiv-runtime/src/pad.rs and pinned by pad::tests", "native opening smoke (tools/native/native_opening.gd) run headless through the shipped shell reaches the recorded state at tick 3360: map $13 (48,18), Chaz alone, no scene, 500 meseta, town 80008040 (build/lane-evidence/native-opening.txt); oracle/frames is absent in the lane, so the certified captures remain the integration step", "grep -n 'tick(' rust/psiv-godot/src/*.rs: lib.rs has no Runtime::tick; the only runtime ticks left are battle/mod.rs:363 (S3), camp/mod.rs:356 and shop.rs:264 (S4) (build/lane-evidence/tick-grep.txt)", "follow-up (same lane, after review): a runtime can no longer exist without its dialogue pack — GameData::load reads dialogue/ with the rest of the pack, GameData::dialogue shares it, and save::construct_runtime installs it, so the title's START and CONTINUE get a message box; load_dialogue is retired and Runtime::dialogue_pack() is no longer an Option. Acceptance test a_new_game_runtime_opens_the_openings_first_scene_dialogue (Runtime::new_game, no other setup, opens the opening's first SceneDialogue) and the negative control dialogue::glue::tests::a_runner_without_the_pack_cannot_open_the_openings_first_line both pass; the opening smoke under PSIV_DEBUG_RETAIL_PACE=1 (autoclose off, real boxes) reaches first control with 0 tree-absent errors; CONTINUE boot verified (build/lane-evidence/S2-NOTES.md)", "psiv-campaign 27/27 and the full workspace stay green; rust/psiv-campaign untouched"]
    integration_evidence: ["certify on 213b348: 6/6 pairs 0.000000, every capture byte-identical to main's #47 receipt (20261002T070040Z-a4da324)", "gate 20261002T072235Z-213b348: 1191 Python, 1097 Rust passed", "fidelity fix verified against pinned ps4.asm:93675 FieldObj_MovementsTbl: horizontal beats vertical, opposing pair cancels", "run 2: a Runtime cannot be built without its dialogue pack (main's Godot-side prepare_runtime removed at integration)", "open: Session::window_tick is a second per-frame call until S4 moves Interact routing; S4's acceptance folds it into frame"]
    state: verified
  - id: S3
    outcome: "Battle menus and battle end in psiv-runtime, driven by pad input"
    depends_on: [S2]
    acceptance: "RoundOrders are built only inside the runtime from menu input; outcome and rewards come from the battle itself; vehicle skill counts live in game state; the 87-fixture oracle replay stays exact; battle idle certification stays 0.000000"
    evidence: ["lane s3-battle (base dedf6b5), 2026-10-02: the battle is a session mode. rust/psiv-runtime/src/session/battle/ owns it — mod.rs (the loop), menu/{mod,commands}.rs (the main options, the per-character window, the mounted window, the eligibility and reservation rules), view.rs (the BattleView the shell draws), narration.rs (the retail lines and beats, moved from Godot), queue.rs (a runtime timeline with its sound/animation sidecars) and presentation.rs. Session::frame dispatches to it and Frame::battle carries the view, the sound cues, the start cue and the close edge; godot's battle/commands.rs, ui_input.rs, timeline.rs and sfx.rs are gone, battle/ui.rs (1,000 lines) became battle/ui/{mod,build,draw}.rs, battle/menu_draw.rs draws the window, and build/lane-evidence/S3-NOTES.md indexes the raw output", "workspace 1106 passed / 0 failed / 1 ignored (64 targets); clippy --workspace --all-targets -D warnings clean; fmt --check clean; examples build; academy_route exits 0 and fights four battles through the menu (build/lane-evidence/{workspace-tests,clippy,fmt,examples-build,academy-route}.txt)", "grep -rn 'RoundOrders|battle_round|finish_battle_for_outcome|start_battle_timeline' rust/psiv-godot/src prints nothing (build/lane-evidence/battle-seam-grep.txt); the shell's remaining runtime ticks are camp, shop and the boot fixtures, and the battle's Neutral tick is in session/battle/mod.rs (build/lane-evidence/tick-grep.txt)", "headless acceptance test rust/psiv-runtime/tests/session_battle.rs: a field state on AcademyBasement (map $15) whose first landing rolls tape 07's formation ($8A, two Zoran Bults at $0E/$1A) is fought with pads only — COMD, TECH -> RES -> a party target, ATTACK, the confirm-waiting results pages, victory, meseta and experience in game state, the return to the field. Two negative controls print what they saw: a pad that never confirms leaves the battle running with the command window up and the party untouched (party [20, 53, 21] after 600 neutral frames), and cancelling out of a target list returns to Actions with the same actor and no order (build/lane-evidence/negative-controls.txt)", "buttons are the cartridge's where its routines name them: accept is ButtonSpeak|ButtonCamp and the main options wrap on Up/Down (Battle_MainOptions, ps4.asm:1916; Battle_UpdateRedCursor2, ps4.asm:1572), the post-battle pages take any face button (ps4.asm:4706, 6351), and the mounted window's cancel is ButtonCancel (ps4.asm:7463). The per-character and list windows keep the shell's four-direction mapping because this port's window is one vertical list where the cartridge's is a horizontal five-icon strip, and the native input drivers steer those lists with ui_up/ui_down; rust/psiv-runtime/src/session/battle/menu/mod.rs records the difference", "fidelity fix (with test): the mounted skill window reads the battle record's live uses instead of the shell's own copy, which the shell decremented at battle/ui_input.rs:102 while the engine decrements the record when the command resolves (psiv-core/src/battle/engine.rs, Command::VehicleSkill); pinned by session::battle::menu::commands::tests::mounted_skill_slots_read_the_live_battle_record", "deviation (presentation only, named): an enemy attack animation that begins on the frame its predecessor's beat ends now gets its art-clock advance on that same frame, because the shell applies the frame's view before advancing the animation clock, where the old shell advanced first inside advance_frame. The battle's first beat, each round's opening beat and the epilogue's pages are exact; no certified capture covers a mid-round attack", "the debug selectors keep their paths: PSIV_DEBUG_BATTLE=0x88 draws the tape-07 command-idle fixture from a static view with no runtime battle (and no field frame: the presentation still owns the frame), 0x89 runs the newly-exact probe through Session::debug_battle_probe, and a real formation id goes through Session::debug_battle; the shell's one new seam is Session::abort_battle, the presentation-failure escape its own error paths already answered (finish_battle_for_outcome(Escaped, 0))", "test names: psiv-runtime +25, nothing removed (10 command-window tests, 4 narration tests, the queue test and the dwell table moved from psiv-godot; 5 new pad/narration cases and 3 integration tests); psiv-godot +0 of 58, 16 removed, every one of them moved (build/lane-evidence/testnames-*.diff)", "native smoke (lane, headless, extension built from this revision): PSIV_DEBUG_BATTLE=0x88 runs the tape-07 command-idle fixture with no error and one 'battle started' line, and PSIV_DEBUG_BATTLE=0x8a starts a real formation-$8A battle through the session — the tick-200 state dump reads Chaz (fighter 1, 25 HP) against two ZORAN BULTs (fighters 6 and 7, 25 HP), the tape-07 roster, with the menu waiting for input (build/lane-evidence/smoke-battle-0x{88,8a}.txt and -state.json)", "open: the integration step must certify battle-0x88 at tick 200 and the 87 oracle replay fixtures; oracle/frames is absent in the lane, so no capture was compared here"]
    integration_evidence: ["merged with S3/S4 at 479b823 (session/mod.rs reconciled by hand: Mode::{Field, Battle} beside the menu state, one-call frame); certify 20261002T094329Z-479b823: 6/6 pairs 0.000000, every capture byte-identical to main 213b348; gate 20261002T094917Z-479b823: 1191 Python, 1127 Rust passed", "filed: #51 per-character list window is vertical where the cartridge draws a horizontal strip; #52 back-to-back enemy attack animation starts one frame early"]
    state: verified
  - id: S4
    outcome: "Shop, inn, camp, loot and their catalogs in psiv-runtime/psiv-data"
    depends_on: [S2]
    acceptance: "shops.json loads only through psiv-data; sell price, equip/unequip and auto-target rules have runtime tests; camp root certification stays 0.000000"
    integration_evidence: ["merged with S3/S4 at 479b823 (session/mod.rs reconciled by hand: Mode::{Field, Battle} beside the menu state, one-call frame); certify 20261002T094329Z-479b823: 6/6 pairs 0.000000, every capture byte-identical to main 213b348; gate 20261002T094917Z-479b823: 1191 Python, 1127 Rust passed", "review sent back once: one key pressed two cartridge buttons (Cancel set Camp) and menus dropped the Speak|Camp confirm (ps4.asm:135155); fixed with one Godot action per button", "filed: #49 shop greeting/BUY-SELL Cancel flow, #50 same-frame panel op and dialogue close; #39 Aiedo inn event still open"]
    state: verified
    evidence: ["lane s4-shopcamp (base dedf6b5): Session::frame(pad) now owns the shop and inn window (session/shop.rs), the camp menu and chest windows (session/camp/), Interact routing (counter to shop, else NPC talk, facing included) and the scene-line, choice and nothing-here window openers (session/route.rs); Session::window_tick is folded into frame (grep window_tick rust: no hits); psiv-data loads shops.json (ShopData); equip against unequip decides from CampCharacter::equipment_ids; psiv-godot keeps drawing only (shop/catalog.rs, shop/input.rs, camp/input.rs, camp/travel.rs gone; grep for shop_buy|shop_sell|shop_stay|use_camp_item|use_camp_ability|equip_camp_item|order_camp_party|shops.json in rust/psiv-godot/src prints nothing)", "tests rust/psiv-runtime/tests/session_menus.rs, pads only: Piata buy, sell at half the record price word for an unstocked item, empty-pack refusal, inn night (HP/TP/status restored, 5 per head), camp heal technique, equip and unequip, ORDER, SAVE request, three chest windows; negative controls a_purchase_the_purse_cannot_cover_is_refused_with_no_state_change, an_inn_bill_the_purse_cannot_cover_changes_nothing and camp_pressed_while_a_scene_runs_does_not_open_the_menu pass; the chest confirm default failed before its fix (solo party read YES)", "tools/certify.py on the lane build: 6/6 pairs rmse 0.000000 and every capture byte-identical to main 213b348's receipt, camp-root included (build/lane-evidence/certify.txt, capture-compare.txt); native opening smoke reaches map $13 (48,18), Chaz alone, 500 meseta, town 80008040, no ERROR lines; PSIV_DEBUG_SHOP=12 screenshot shows the session's shop view", "academy_route exits 0; motavia_route's camp heal ran through pad presses (CAMP RES: Alys 22 HP) before the copied save stopped it at an unrelated walk", "fixes (separate): sell price reads the item record price word for any item (ps4.asm:135570), chest confirm gets its own cursor so a lone member keeps the documented default NO (docs/field/CHESTS.md); open: #39 left as AiedoEventPending (the scene must run mid-transaction)", "review repairs (second commit): each cartridge button has its own Godot action (ui_accept Speak, ui_cancel Cancel, psiv_camp Camp, psiv_start Start; controller button 6 moved from ui_accept to Start and the title confirms on Start) and the session menus use the cartridge's masks (Speak|Camp confirm, Cancel back, Start closes the camp or dismisses a result line; ps4.asm citations in session/camp/mod.rs and session/shop.rs); 15 tools/native drivers that opened the camp with ui_cancel press psiv_camp and parse with --check-only; the panel-layout byte lives in the Runtime (scene_panel_sprites, cleared on a dialogue close and scene end) and cutscene/state.rs reads it; debug.rs opens the debug counter in the Session directly and title.rs and lib.rs build every Session through new_session; the BUY list opens on its first row after a SELL visit (ps4.asm:135182, 135508)"]
  - id: S5
    outcome: "Title, CONTINUE/START/ERASE and game over in the Session; runtime mutators crate-private"
    depends_on: [S3, S4]
    acceptance: "psiv-godot compiles against the Session view API only (a mutator call from Godot fails to compile); title certification stays 0.000000; fresh-process CONTINUE verified"
    evidence: ["lane s5-title (base 016cdc7), 2026-10-02: the title and a defeat are session modes and every slot operation runs inside the session. rust/psiv-runtime/src/session/title.rs owns the phases and their frame counts (the retail #$233 Press Start hold, ps4.asm:86926), the CONTINUE/START/ERASE DATA rows and their buttons (TitleRoutine_PickOption, ps4.asm:87512-87513), the three-row slot lists and the ARE YOU SURE? window (ps4.asm:87908-87956); session/game_over.rs owns the fourteen fade frames that gate the title's return, and session/notices.rs owns the field-status windows the defeat queues (DisplayPerishedMessage, ps4.asm:117087). START builds the retail initializer and CONTINUE loads a slot inside the session, so the shell answers no request", "saves: one store type constructed with the run directory. SaveStore (rust/psiv-runtime/src/save.rs) is the only path to a slot file and the session keeps it across START and CONTINUE; the shell resolves PSIV_SAVE_DIR and refuses a scripted run without it (rust/psiv-godot/src/save_dir.rs) and hands the store over at construction (configure_session). Camp SAVE writes inline and reports the result line on the frame it happened; Frame::save_request and Session::finish_camp_save are gone, and the shell's round trip with them (Frame::camp_save_error keeps only its log line)", "boundary (correction ladder rung 1): Session::runtime_mut is removed and every Runtime mutator the shell used is crate-private, so psiv-godot compiles against Session::frame, its constructors and Session::runtime() -> &Runtime. A shell that tries a mutator through that reference fails to compile for the right reason: the two compile_fail doctests on Session fail with E0596 (build/lane-evidence/negative-controls.md, compile-fail/*.rs), and grep -rn 'runtime_mut' rust/psiv-godot/src rust/psiv-runtime/src prints nothing (build/lane-evidence/no-runtime-mut.txt)", "debug fixtures: psiv-runtime/src/session/debug.rs holds the certification fixtures (camp_fixture for PSIV_DEBUG_CAMP=1 with the tape-22 object positions and the receipt's settled camera, scene_fixture for PSIV_DEBUG_EVENT=0x8007) and the debug selectors (debug_battle, debug_battle_probe, debug_start_event, debug_mount_vehicle); psiv-godot/src/boot.rs keeps only the selectors' list, the fallback spawn and the retail-initializer test. The #44 guard moved with the fixture (session::debug_tests::the_camp_fixture_does_not_replay_piata_chaz_alone, negative control: a blank GameState does replay the trigger)", "headless tests (rust/psiv-runtime/tests/session_title.rs, pads only): power-on reaches the certified title state (clone tick 480 = reveal, ticks 279, no window) and START opens the opening's first dialogue; CONTINUE loads the slot a camp SAVE wrote in the same test (same map, cell, party, purse and flags); ERASE DATA zeroes the picked slot, drops its row and leaves the other slot loading; a defeat (Chaz at one HP, poisoned, on Motavia) reaches game over and the title after the fourteen fade frames. Negative controls in the same file: CONTINUE and ERASE DATA on an empty row return entered/erased None with no state change at all, and session_menus::a_save_without_a_run_directory_is_refused refuses with no file written (build/lane-evidence/negative-controls.md)", "buttons: the option rows confirm on ButtonSpeak|ButtonCamp|ButtonStart. The port's held-pad reader had dropped Camp; this lane restores it with a citation and a unit test (session::title::title_tests::the_option_rows_confirm_on_speak_camp_and_start), and an accept press no longer doubles as a cursor step (the cartridge's confirm branch runs first, ps4.asm:87514-87534; the_accept_button_does_not_step_the_cursor)", "kept, uncited port behavior, listed not changed: the Press Start hold opens the option window where the cartridge moves to TitleRoutine_CharPortraits (ps4.asm:86941) and the port has no portraits or scrolling-text phase; every phase accepts the option window's three buttons where the Press Start hold leaves only on ButtonStart (ps4.asm:86934); the Sega and reveal phases advance on a press, which TitleRoutine_FadingText never reads (ps4.asm:86220-86387); ARE YOU SURE? opens on YES where the cartridge initializes Window_Option_Index_3 to the other row (ps4.asm:87905-87908); the title is silent where the cartridge plays SFXID_Selection and SFXID_EnemyKilled (ps4.asm:87930-87936); Cancel opens the cartridge's sound test (ps4.asm:87517-87526) and does nothing here. All six are recorded in the module doc of session/title.rs", "files: session/mod.rs 537 + the new session/{title,title/title_tests,game_over,notices,saves,debug,debug_tests}.rs; psiv-godot/src/title.rs is presentation only now (726 lines, was 987) and every touched file is under 1,000 lines (python3 tools/size_guard.py: 0 problem(s))", "test names: psiv-runtime 201 -> 218 (one case replaced, nothing else removed: the camp SAVE test now expects the store to write), psiv-godot 41 -> 40 (the only removal is the #44 fixture test, which moved into the runtime as session::debug_tests::the_camp_fixture_does_not_replay_piata_chaz_alone); psiv-core, psiv-data, psiv-sound and psiv-campaign unchanged (build/lane-evidence/testnames-before-after.txt, testnames-psiv-{runtime,godot}.{before,after}.txt)", "open: the mutators the crate's own integration tests and examples drive directly (enable_battles, tick, battle_round, finish_battle_for_outcome, set_rng_seed, save_slot, start_event, dialogue_closed/choice/ended, open_scene_dialogue, return_to_field, repair_legacy_progression, order_camp_party, use_camp_item, shop_stay, set_event_flag, set_vehicle_index, set_camera, face_npc, set_field_suspended, ending_continue, acknowledge_field_notice, open_status_dialogue, finish_battle_absorbing) stay pub: a pub(crate) mutator is invisible to a target that lives outside the crate, so closing those needs the harnesses on the session first. 68 of the crate's 91 state-changing methods are crate-private now; docs/RUNTIME_DESIGN.md names the remainder", "open: the certified captures are the integration step. oracle/frames is absent in the lane, so no capture was compared here; the title pair (clone tick 480) and the five others ran through the moved paths only in the sense that the state they render is produced by the same code on the same frames (the title's own frame count, its phase lengths and the shell's per-frame presentation are unchanged)", "gate (lane, one heavy command at a time): cargo test --workspace 1145 passed / 0 failed, the two compile_fail doctests on Session included (build/lane-evidence/workspace-tests.txt); clippy --workspace --all-targets -D warnings and fmt --all --check clean (clippy.txt, fmt.txt); cargo build -p psiv-runtime --examples and --example academy_route exit 0 (examples-build.txt, academy-route.txt); python3 tools/size_guard.py, python3 tools/check_docs.py and python3 -m unittest tests.test_feature_map exit 0 (python-checks.txt); git diff --check clean"]
    integration_evidence: ["merged with R1 and #54 at d2d4096; runner camp SAVE moved onto the Session store; full route digest unchanged by S5 (52ccbf8d... before #54, 5e80a50e... after)", "certify 20261002T143246Z-d2d4096: 6/6 0.000000, captures byte-identical to main", "gate 20261002T143834Z-d2d4096: 1191 Python, 1177 Rust passed", "boundary only partly locked: 23 Runtime mutators remain pub for out-of-crate callers; node S6"]
    state: verified
  - id: R0
    outcome: "Route planner (cross-map warp graph + in-map BFS over the runtime's own FieldMap) and the route-file format with a pack validator; rust/psiv-campaign"
    depends_on: []
    acceptance: "Motavia (84,64) to Aiedo plans one warp (0x100706), arrival (47,83), 67 steps, matching docs/campaign/AIEDO.md's independent oracle/route.py result; validator accepts routes/main.json and rejects mutated copies"
    state: verified
    evidence: ["psiv-campaign 27/27 and psiv-runtime suites green at bf763ee", "validate routes/main.json: 13 chapters, 196 objectives, 100 warps, 3672 steps, 0 errors", "review: the planner's attach_chests copy replaced by one public fresh-entry builder, field_map_entered, also used by map_change and save", "12 objectives marked verify:true (R1 resolves them by running)"]
  - id: S6
    outcome: "Every state-changing Runtime method is crate-private: the runtime's integration tests, examples and the campaign runner build and drive games only through Session constructors and Session::frame"
    depends_on: [S5, R1]
    acceptance: "Session constructors cover new game (with battles armed and the opening started), CONTINUE from a store or a slot file, and the debug fixtures; psiv-runtime tests/examples and rust/psiv-campaign use only them, Session::frame and &self views; no pub fn on Runtime takes &mut self (grep proof); a compile_fail doctest builds a Runtime outside the crate and calls a mutator; the full route digest is unchanged; certify 6/6 byte-identical"
    evidence: ["lane s6-lock (base 8c19769), 2026-10-02: the runtime's public surface is the session and its views. `Session::start(data)` (rust/psiv-runtime/src/session/start.rs) is the only way to a game: `with_battles`/`with_saves`/`with_step_frames` and the two harness switches configure the run, and `power_on` (retail's front door), `field` (the same boot, the debug selectors' fast path), `new_game` (the title's START: initializer + battles + opening event), `continue_slot(n)` (CONTINUE through the store), `from_slot_bytes(bytes, slot)` (a slot file's bytes, the runner's `--from-chapter`) and `from_save` (one decoded save, what the fixtures and the tests build) start it. The switches are setters on `Start`, not calls after construction, because the title reads its slot rows and its autostart switch as it installs; the shell passes the run's store the same way (rust/psiv-godot/src/lib.rs, `session_store`)", "boundary: **54 `pub fn (&mut self)` methods on `Runtime` before, 0 after** (build/lane-evidence/mutator-grep.txt scans both trees the same way). Every constructor is crate-private with them (`Runtime::new`, `new_game`, `from_save`, `load_slot`, `erase_slot`, `slot_path`; `Session::new`/`with_saves` and `SaveStore::load`/`write` too), `Runtime::save_slot` stays `pub` because it is `&self`: a caller holding a session's `&Runtime` may record the game it is looking at — the runner writes each chapter save that way — and cannot change it. Proof for a caller: three out-of-crate files compiled by rustc against the built rlibs fail with E0624 (build/lane-evidence/compile-fail/), and the new `compile_fail` doctest on `Runtime` (rust/psiv-runtime/src/lib.rs) plus the two on `Session` run in `cargo test --doc`. Proof for the source: `suites::visibility::no_runtime_mutator_is_public` fails when a new `pub fn (&mut self)` lands in an `impl Runtime` block (negative control: one mutator flipped back to `pub` names camera.rs:29 in the failure, then reverted — build/lane-evidence/negative-control-visibility.txt)", "rule tests moved inside the crate: 32 files from `rust/psiv-runtime/tests/` became `rust/psiv-runtime/src/suites/*.rs` (`#[cfg(test)] mod suites`, next to `encounters_elements_tests` and `dialogue/runner_tests`), because a test that ticks the field with an `Input`, builds `RoundOrders`, seeds the RNG or acknowledges a scene's dialogue is a test of a rule, and a `pub(crate)` rule is invisible to a target outside the crate. Nothing was removed or weakened: **61 test names moved file, 0 disappeared, 1 added** (build/lane-evidence/testnames-before-after.txt; psiv-core, psiv-data and psiv-sound unchanged, psiv-godot's `title_start_preserves_the_retail_initial_state` keeps its name and now asks `Session::start(data).new_game()`). The pad-driven suites stayed integration tests (`tests/session_{opening,menus,battle,title}.rs`, `tests/chest_slots.rs`) and were converted to the constructors: session_title's `power_on()` helper is now `Session::start(data).with_saves(store).power_on()`, session_menus' `session_at`/`fresh_game` are `from_save`/`new_game`, session_battle's `field_session` is `from_save`, and session_opening's opening-dialogue case reads `Frame::routed` instead of calling `open_scene_dialogue` itself", "examples: `academy_route`, `motavia_route`, `tonoe_route` and their `support` harness are deleted — the runner plays START through Aiedo (`academy`, `holt`, `rune-dorin`, … , `aiedo`), so they duplicated it and needed native saves to run at all. The fixtures stay and build through the constructors (`chest_fixture`, `pipe_fixture`, `travel_fixture`, `status_fixture`, `field_status_fixture` → `Session::start(..).with_battles(..).with_saves(..).from_save(..)` + `Session::save_slot`). Two jobs are not games and keep a whole operation inside the crate (rust/psiv-runtime/src/tools/): `oracle_replay` (the `psiv-replay` driver, whose bin is now a command line over it) and `repair` (`psiv-runtime::tools::repair_legacy_progression`, the body of the `repair_progression` example, idempotence check included). Neither hands out a `&mut Runtime`", "play is identical: the full route's digest is `5e80a50e50dad15c` over 168,843 frames before and after, with the tape byte-identical (sha256 24ad3ae3…, build/lane-evidence/route-digest.txt). The oracle replay tool is the other moved driver, compared directly against the base revision's own binary on tape 02 with and without inherited state (--seed/--camera): `cmp` is byte-identical, sha256 d115506c… (build/lane-evidence/replay-parity.txt). Native smokes through the shipped shell, headless: the opening (title autostart → START → first control) reaches map $13 (48,18), Chaz alone, 500 meseta, no scene, town flags 80008040 with 0 ERROR lines, and `PSIV_LOAD_SLOT=1` over the runner's own 00-academy chapter save logs `save boot: loaded slot 1` and loads it (build/lane-evidence/native-{opening,continue}.txt)", "gate (lane, one heavy command at a time): `cargo test --workspace -- --test-threads=1` 35 targets, 1179 passed / 0 failed / 2 ignored, doctests included (build/lane-evidence/workspace-tests.txt); `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` clean (clippy.txt, fmt.txt); psiv-campaign's suite green (campaign-tests.txt); python3 tools/size_guard.py, python3 tools/check_docs.py and PYTHONPATH=. python3 -m unittest tests.test_feature_map exit 0; git diff --check clean", "open: three documents outside this lane's write set still show the pre-S6 command for moved tests — docs/AGENT_WORKFLOW.md (`-p psiv-runtime --test camp_order`), docs/campaign/PLAYABILITY_ACADEMY.md (`--test combat_skills`, `--test combat_techniques`) and docs/campaign/PLAYABILITY_FOUNDATIONS.md (`--test new_game`); each is now `--lib suites::<name>::`. docs/campaign/PLAYABILITY_*.md and rust/psiv-campaign/routes/main.json also cite the deleted route examples as their source notes. `Runtime::finish_battle` and `Runtime::refresh_map_camera_gates` (+geometry::refresh_camera_gates) have no caller at all and carry `#[allow(dead_code)]` with the reason; `set_event_flag` is used by one suite only", "open: the certified captures are the integration step. oracle/frames is absent in the lane, so no capture was compared here; tools/certify.py runs at integration"]
    integration_evidence: ["merged with C1 at 2f4e608: route digest 96d2835a8633def8 and tape 53adf11f... identical", "certify 20261002T161600Z-2f4e608: 6/6 0.000000, captures byte-identical", "gate 20261002T162144Z-2f4e608: 1191 Python, 1180 Rust passed", "dead code: finish_battle deleted, TextFlow::open/is_holding and set_event_flag test-scoped; the camera refresh seam is a real gap, #59"]
    state: verified
  - id: R1
    outcome: "Campaign runner binary driving a Session from routes/main.json; chapters through the post-Rika checkpoint"
    depends_on: [S5, R0]
    acceptance: "The runner plays New Game to the north bank (Motavia $00 (84,64), five members) from the route file with pad input only, writes a pad tape and chapter saves, and a deliberately broken objective halts with a report (negative control)"
    evidence: ["lane r1-runner (base 016cdc7), 2026-10-02: rust/psiv-campaign gains the runner (docs: 'The runner' above; record: RUNNER_LOG.md). `psiv-campaign run routes/main.json` plays all 13 chapters from New Game to the Aiedo arrival with pad presses only: 175,497 frames, 92 random battles and 2 scripted, exit 0, final-state digest 52ccbf8d9a7de61a, in about half a second in a release build. The north-bank checkpoint holds (Motavia $00 (84,64), Gryz, Alys, Chaz, Hahn and Rika alive, flag $35) and the Aiedo chapter arrives at $54 (47,83) after its one warp", "tape and replay: the run writes its pad tape (text, run-length encoded, src/tape.rs) and a chapter save per chapter; `psiv-campaign replay <tape> [--from-save FILE]` reproduces the digest, also for a tape that starts from a chapter save; two full runs printed the same digest", "tests: `cargo test --manifest-path rust/Cargo.toml -p psiv-campaign -- --test-threads=1` passes (50 passed, 1 ignored: the whole route, run in release); tests/runner.rs holds the determinism case (chapter one twice: identical tapes, digests and chapter saves), replay, resume from a chapter save, a boundary guard that greps the runner for runtime mutators, and the negative controls: a wrong object index halts at that objective with exit 2 and a report naming chapter, index and kind, an expectation that cannot hold halts at its objective, an unreachable cell, a spent budget, a pack with no item to use", "12 `verify` objectives resolved by playing and their flags dropped (RUNNER_LOG.md, 'verify items resolved'): Hahn and Dorin are object 0, Gryz's storage door is object 1 (the route had 0), Igglanova's press starts interaction area 0 rather than an object, the Holt scene lands the party at Zema (30,11) not (60,21), the Mile detour is unnecessary, the patrol condition holds, and the elevator doors' `opens` cells are walkable", "one port defect diagnosed, route works past it with a player's alternative: RunEvent_MileSandWorm is Unsupported unconditionally (rust/psiv-core/src/trigger_custom.rs:43) where the cartridge answers NoEvent outside its box without touching the RNG (ps4.asm:116449), so every visit to Mile halts; the route skips Mile and rests at the Piata inn (RUNNER_LOG.md, H1)", "open for S5: the runner builds its Session through Runtime::new_game, enable_battles, start_event and from_save in src/start.rs; when S5 makes them crate-private the runner needs a Session constructor over the pack and the battle files"]
    integration_evidence: ["independent rerun at integration: run exit 0, 175,497 frames, digest 52ccbf8d9a7de61a, tape sha256 dd5ff60f... identical to the lane evidence; replay reproduces the digest", "gate 20261002T114112Z-b1d38ea: 1191 Python, 1150 Rust passed", "halt H1 filed as #54 (four custom triggers Unsupported); Mile removed from the route until fixed", "boundary: construction in start.rs still calls Runtime constructors and enable_battles/start_event; reconciled with S5"]
    state: verified
  - id: R2
    outcome: "Tape replay in Godot"
    depends_on: [R1]
    acceptance: "One generic Godot replay driver reproduces R1's tape and chapter-save bytes; the bespoke tools/native drivers it covers are retired"
    integration_evidence: ["bounded native milestone: New Game to initial Aiedo, 156019 exact pads and matching chapter bytes; earned-save Zio segment 8823 pads with ordinary SAVE acknowledgement and fresh-process CONTINUE; selected 1280x800 rendered Zio and fresh CONTINUE captures are separate presentation evidence", "clean e9372bc: repaired generic CLI and primary no-override default-pack CONTINUE each consumed 180 exact neutral pads, matched native-written slot bytes, and observed the mapped extension; stable primary build/accepted-combined-5ec096f retains receipts, images and binaries", "the original 5ec five-stage gate remains FAILED; affected e937 Python/docs pass, and the composed matrix applies unchanged Rust/fmt/Clippy inputs", "open: bespoke-driver retirement not inventoried or completed; full R2 remains in progress", "lane r2-native, 2026-10-06: tools/verify_native_route.py replays the whole 50-chapter campaign tape in one native Godot process from New Game and compares the slot at every chapter-end frame: 50/50 byte-identical over 3743328 frames (5945 s, opt-level-3 extension), tape db8ae7fe, digest 40c7f505ecc5c357; 24 native SAVE acknowledgements equal the route's 24 save objectives and the written slot equals the runner's last SAVE; --continue-probes loads all 50 chapter saves through title CONTINUE (50/50). Per-chapter replay from saves is impossible by design (a slot holds no RNG state). Twelve campaign-path drivers retired (docs/campaign/NATIVE_DRIVERS.md); native_opening.gd and verify_native_alarm.py stay for frame-addressed captures (#93); Godot RSS growth filed as #92. Lane evidence at 93c7453; the replay path is unchanged to the integrated head"]
    state: verified
  - id: C70
    outcome: "Issue #70: Session automatically resolves every enemy-only ambush round without a command surface or item use"
    depends_on: [R1]
    acceptance: "Cite loc_52D6, Battle_ProcessCOMD, loc_5380 and Battle_OrderTurns for every command-opening condition; a connected-pad scripted ambush shows no menu or Psycho Wand spend until the first real player round, a normal round still opens COMD, an alive but all-$6E-status party submits on its single COMD press, and the static battle-0x88 certification is unchanged on the combined candidate"
    evidence: ["local candidate from 82eed831: read-only Battle::pending_priority and runtime bridge, automatic ambush gate, pad-only Zio event battle 6 regression and Academy normal-menu negative control, empty-actor command regression; docs/source-notes/battle-party.md records source conditions", "RUNNER_LOG.md #70 frozen headless acceptance: first actual player menu uses the Psycho Wand; no item is spent during the ambush. Mixed-party cure policy uses core target lists, excludes Demi from RES/MONOMATE and chooses GISAR when two humans are hurt; final focused tests under the shared heavy lock."]
    integration_evidence: ["combined 11-pair static certificate on clean 90d2ef1 is RMSE 0 for all pairs, including unchanged battle-0x88; only docs/Python authority changed through e9372bc", "connected-pad scripted ambush opens no party COMD or Psycho Wand spend until the first real player round; separate ordinary all-$6E no-actor COMD and normal-menu regressions pass against the cited source conditions", "composed applicable-check matrix: e9372bc docs/1,241 Python pass; unchanged 558 Rust/config inputs carry 5ec fmt/1,287 Rust/Clippy passes; original failed gate remains immutable"]
    state: verified
  - id: CZ
    outcome: "New Game through Cutscene_ZioDefeated with ordinary pad route, SAVE, fresh Session load and deterministic tape replay"
    depends_on: [C70]
    acceptance: "All 29 chapters complete from New Game twice on the frozen candidate with identical full tapes, digests and final ordinary pad-SAVE bytes at route/slot_1.sram; replay reproduces the digest with no faults; a fresh Session loads that route slot and asserts Motavia map $00, Chaz/Rika/Rune, Chaz alive and event flags $65/$68/$66/$61. Preserve losing trials. Do not claim the later arc or Zio oracle parity. Native Godot milestone proof is a separate integration check."
    evidence: ["RUNNER_LOG.md #70 route trials preserve the old-level, level-22 and old one-cure losses; the trained party with earned GISAR wins a copied-save chapter, while 64e4023's stricter target legality exposed a different full-route Zio loss. Rung-1 shared targeting and an ordinary proactive group-heal policy fix it without balance changes.", "isolated frozen source 6fb1a70: full-group-a/b complete all 29 chapters from New Game at 2,430,333 frames each, digest d4a124057f4439cd; tapes SHA256 9b3ebe16... and pad SAVE bytes SHA256 8443a11e... identical; both replay, fresh Session inspections assert map $00, Chaz/Rika/Rune alive and $65/$68/$66/$61. The final camp SAVE is a separate route slot, not merely Runtime::save_slot; full hashes and failed runs in RUNNER_LOG.md. Combined rerun and native checks were pending at this isolated freeze; the integration receipts below close that gap."]
    integration_evidence: ["combined candidate 5ec096f: two full 29-chapter New Game routes at 2615778 frames, digest 949c2abe3342e838, identical tapes and final ordinary pad-SAVE bytes; earned pre-Zio save SHA256 5392489e and full-chain final slot SHA256 4aeff0b1; fresh Session inspection/replay in the combined runner receipt", "native earned-checkpoint Zio CONTINUE, 8823 pads, real camp SAVE acknowledgement and byte-matched written slot; fresh-process CONTINUE and rendered field are separate from the full headless New Game origin, not a full native New Game-to-Zio claim", "composed applicable-check matrix accepted on clean e9372bc; original failed 5ec gate is retained; primary default-pack no-override CONTINUE passed separately with the native-written slot"]
    state: verified
  - id: C
    outcome: "Route chapters to the Ending, one lane per blocker class"
    depends_on: [R1, CZ]
    acceptance: "The runner reaches Game_Cleared_Flag from New Game; each blocker it hit is fixed with a regression test or filed as an issue with a link from the route"
    evidence: ["lane c1-motavia (base 8c19769): routes/main.json grows to 18 chapters and reaches the Zio Fort's Juza room (map $87, (32,21)) with all five alive at level 12 to 13: 457,356 frames, digest 96d2835a8633def8, tape sha256 53adf11f092bb99982c299aa2620b75a4078f6a4fe3ba69f0adae0a3d749bd57, identical on three runs, replay reproduces the digest; scenes 28 and 29 pass in-route; RUNNER_LOG.md H13 to H16", "the route stops at Juza: H16, enemy 114's ZAN and FORCEFLASH are not run by the engine, and the stairs to F3 and F4 open only after his battle; H15 lists the unsupported abilities on the way (FUSION, FIREBREATH, DEBAN) that the route passes only by running; H13 (#39) does not block the story", "lane c2-zio (base 0c6ae8b): routes/main.json grows to 27 chapters through scenes 31 to 37 (Juza's battle, the Demi rescue and Alys's wounding, the Machine Center and Land Rover, Ladea Tower with Rune, the Psycho Wand, the walk to the Zio Fort barrier); the runner learns to ride a vehicle (a two-cell lattice planner, `vehicle` assertions, `dismount`, `plan --vehicle`) and to fight a boss (`fight_to_win`, `run_then_win`). On the committed engine the run halts at its first port defect, `Cutscene_AlysWounded` reading the wrong dialogue tree (H17): 1,023,474 frames, exit 2, digest 49f5c47df7971428, identical on two runs, replay reproduces it. An experimental four-line patch for H17 plays on to `Event_ZioFortBarrier` (H18, not transcribed, no alternative); a stub for it completes all 27 chapters (digest e77a7b9cf1f5fa37, identical on two runs). H19 to H21 recorded (live map ignores story flags, vehicle parking and boarding, EVIL EYE in the tower)", "lane c3-nurvus (base 5c62765): routes/main.json grows to 29 chapters; nurvus-descent plays Zio Fort to Nurvus B4 (six elevator doors, the B1 tunnel, B5 and the stairs; RUNNER_LOG.md N1 to N4) and nurvus-zio names the new psycho_wand_then_win policy (round-1 Psycho Wand through the battle ITEM menu, then fight_to_win). The full run halts at Zio's trigger on port defects H22 (Event_ZioNurvus op 11 resumes a dialogue instead of running entry $0B; the Zio and Zio2 phase counters are unmodelled, so BLACK WAVE is rolled in round 1): 1,730,441 frames, exit 2, digest 6eb19b166ed24d2f, tape sha256 e3cff60eaf31dacac06bfeb8819d983ad3a848da042d37108a5c73cc9d121402, identical on two runs, replay reproduces it", "lane c4-ship (base 3344f18): routes/main.json grows to 30 chapters; mota-spaceport walks Zio's checkpoint to the foot of the Mota Spaceport's boarding row and stops there on purpose. Two full runs: exit 0, 2,616,234 frames, digest dccc4df3ce01bd22, identical tapes, replay reproduces it. The runner stops treating an unmapped type-1 cell as a fault in a frame that began a scene (H24). Probes behind the stop (RUNNER_LOG C4-3 to C4-6): Wren joins on Zelan F1, the Canceller chest opens, and the run halts because MapUpdate_ZelanCanceller is not run, so $72 never sets and the sabotage cannot fire (H25, no alternative). The ship destination menu (loc_63BC4: a flag-built world list, cursor, Cancel, World_Index write) is not in the Session (H23)", "lane c5-dezolis (base f6e84f9): routes/main.json grows to 33 chapters; zelan-wren-canceller, zelan-sabotage (the one-row Kuran menu by board, the Chaos Sorcerer, Cutscene_CrashLanding with the #67 BG write) and dezolis-first-control reach first control in Raja Temple $14C with Raja joined and World_Index 1, with no runner change. Two full runs: exit 0, 2,638,546 frames, digest 6d365219c15bc775, identical tapes (SHA-256 4af507fa...15af), replay reproduces it. The route stops at the temple exit on port defect H28: Event_OutsideRajaTemple (event $43) has no scene, SceneMissing { event: 67 } (RUNNER_LOG_DEZOLIS.md)", "integration campaign-21 (e551b36), 2026-10-04: a4-status + a3-damage reconciled (m34); every enemy ability on the zelan-kuran stretch implemented except BARRIER ($1D, #86); 136 replay fixtures with an empty manifest; release route 36 chapters to the Hangar, digest e6200509953185db; gate build/gate/20261004T143020Z-e551b36 (1330 Python, 1423 Rust), certify 12/12 build/certify/20261004T142129Z-e551b36", "integration campaign-22 (869a4a4), 2026-10-04: lane c6-kuran; release route 43 chapters through Cutscene_DarkForce1Defeated, 3700582 frames, digest fbe9fef7015aba2e, tape sha256 969ba6dc identical to the lane; pack rebuilt byte-identical; gate build/gate/20261004T204506Z-869a4a4 (1330 Python, 1433 Rust), certify 12/12 build/certify/20261004T203454Z-869a4a4; the Dezolis training chapter stands in for untranscribed player skills (#88)", "integration campaign-23 (20f22e6), 2026-10-04: lane x86-policy (#86 party command scripts, BARRIER, Zio Psycho Wand); 140 fixtures replay exactly; route unchanged at 43 chapters, digest fbe9fef7015aba2e; gate build/gate/20261004T215357Z-20f22e6 (1352 Python, 1437 Rust), certify 12/12 build/certify/20261004T214431Z-20f22e6", "integration campaign-24 (fb23d2e), 2026-10-06: lane c7-icedigger; route 50 chapters to air-castle-arrival, 3743328 frames, digest 40c7f505ecc5c357, tape db8ae7fe identical to the lane; 14 enemy abilities on the dezolis-air-castle stretch unsupported (next lane A5); candidate receipt build/candidate/20261006T140306Z-fb23d2e (certify 12/12, gate 1359 Python, 1455 Rust)", "integration campaign-26 (99d3ddd), 2026-10-06: lanes p88-player (every player technique and skill, 163 fixtures) and s8-reload (cutscene zero-return field reload), runner wanderer/talk re-plan; route 50 chapters, 3852511 frames, digest 29c81ae27664b15b; candidate receipt build/candidate/20261006T232301Z-99d3ddd (certify 12/12, gate 1407 Python, 1496 Rust)", "integration campaign-27 (37a02a2), 2026-10-07: lane a5-aircastle (every dezolis-air-castle enemy ability, COMBINE inline formation in the pack, 9 captures incl. Lashiec and Dark Force 2); pack build/accepted-c27-pack-37a02a2; route unchanged at 50 chapters, digest 29c81ae27664b15b; the restored Xe-A-Thoul chapter completes in the lane without a wait; candidate receipt build/candidate/20261007T001316Z-37a02a2 (certify 12/12, gate 1420 Python, 1529 Rust)", "integration campaign-28 (ff747a5), 2026-10-07: lane s9-vahal (#82, all 19 Vahal Fort and Weapon Plant events, ahead of the route; allowlist 81 to 62); pack build/accepted-c28-pack-ff747a5; route unchanged at 50 chapters, digest 29c81ae27664b15b; candidate receipt build/candidate/20261007T175417Z-ff747a5 (certify 12/12, gate 1437 Python, 1556 Rust)"]
    state: ready
```
