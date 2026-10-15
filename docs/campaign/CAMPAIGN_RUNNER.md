# Campaign runner

The campaign runner (`rust/psiv-campaign`) plays the game from a route file
with ordinary joypad input, headlessly, and halts at the first blocker with a
report. It drives the same `Session` the native game runs
(`Session::frame(pad)`, one call per 60 Hz frame; the layers are in the
[runtime design](../RUNTIME_DESIGN.md)), so a headless run exercises the shipped
game's rules and input path. Each run writes a pad tape, and the native game
replays that tape in Godot and compares every chapter save byte for byte. The
runner reaches no `&mut Runtime` member: runtime mutators are crate-private
behind `Session`.

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
views to decide the next press.

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
no RNG state ([native tape replay](#native-tape-replay) explains why).

`prefix-check <base.tape> <base-report.json> <run.tape> <run-save-dir>` holds a
longer route to an earlier run: the run's tape must begin with every pad of the
base's, and each base chapter's save must hash the same in the run's save
directory (a halted run's saves count). It prints the first frame that moved,
with its chapter, or the first chapter save that differs, and exits 0 on a
match, 2 on a difference and 1 on bad input (`rust/psiv-campaign/src/prefix.rs`).
A change that appends chapters runs it against the base's own run.

**The boundary.** The runner calls `Session::frame(pad)` and reads
`Session::runtime()` and its `&self` methods. It never names a `&mut Runtime`
member, and a test greps for it (`the_runner_reaches_no_runtime_mutator`).
Construction goes through the runtime's own constructors
(`rust/psiv-runtime/src/session/start.rs`): `Session::start(data)
.with_battles(files)` then `new_game()` for START — the retail initializer with
the opening fired — and `from_slot_bytes(bytes, slot)` for a chapter save. Those
are the only two calls in `src/start.rs` that are not pads, and everything after
them is a session.

**Controllers**, one per objective kind, each in its own module and each
producing pads from views only:

| Objective | Module | What it presses |
| --- | --- | --- |
| `go_to`, `go_to_map`, `patrol`, `dismount` | `walk.rs` (plans in `cell_plan.rs`, `map_plan.rs`) | a direction for a step, re-planned from the real cell whenever the party comes to rest somewhere the plan did not expect; the planner works over the live map, and a warp graph built from the flags the game holds; a yes/no prompt that a scene opens on the target map (Chaz's house offers a rest on arrival) ends `go_to_map` with the prompt open, for the next `answer`; so does the ship's destination menu; the warp graph is built from the pack's records, so a cell where a record puts an object the game has since moved off (the Esper Mansion's door guards) is opened in it before the chain is planned; mounted, the same plans run on the vehicle's lattice (`Mover::Vehicle`: the standing cell is the vehicle's, a step is two cells, the four-cell footprint decides the terrain and the warp rule, as `VehicleState::tick` does), and `dismount` presses Action on open ground |
| `talk`, `open_chest`, `interact`, `answer` | `talk.rs` | walks next to the object (or across its counter), turns, presses Speak, and reads what opened; Cancel is retail's direct NO |
| `wait`, `board` | `field.rs`, `ship.rs` | `wait` presses nothing; `board` holds a step until the destination menu opens, or waits for the menu a scene opens (`go_to_map` ends with such a menu open, for the next `board`) |
| `buy`, `sell`, `rest_inn` | `shopping.rs` | the shop view's pages, rows and cursors |
| `equip`, `use_technique`, `use_item`, `reorder`, `save` | `camping.rs` | the camp view's pages; SAVE is answered by the driver with the file the runner writes; a boarding item (the Ice Digger, `ItemAction_IceDigger`) closes the menu on the press that accepts it, so `use_item` waits the scene out instead of reading a page |
| random and scripted battles | `battle.rs`, `policy.rs`, `policy_boss.rs` | the battle view's menus (ATTACK, TECH, SKILL, ITEM), one press at a time |
| `expect` | `expect.rs` | nothing: it settles the game and reads flags, map, cell, party, purse, vehicle, the pack (`items_held`, `items_absent`) and the party's status bytes (`status_clear`) |

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
rounds.

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

### Native tape replay

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
`build/` lock serializes only that one checkout. `python3 tools/heavy_lock.py`
prints it (the main checkout's `build/continuation-heavy.lock`, from any
worktree), and `python3 tools/heavy_lock.py run -- <command>` is the preferred
wrapper: it marks the hold, so a gate or candidate run nested under it does not
wait on its own parent ([gate and coverage](../DEVELOPMENT.md#gate-and-coverage)).

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

**The whole route.** `tools/verify_native_route.py` replays a
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

**Why one process, not one per chapter.** Replaying each chapter from the
preceding chapter's save cannot reproduce the run: a slot holds no RNG state
and no frame counter, so a loaded game restarts both and the same pads play
other battles. `psiv-campaign run --from-chapter holt` from the Academy save
plays a different number of frames than the full run and writes a slot that
differs from it; the split tape of that chapter, replayed natively from the
same save, fails its slot comparison. This is the runtime's contract, not a Godot divergence (the
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

The tape replay runs headless with the render loop off: it exercises input,
state and the save writer, not the picture, and takes no mid-run capture. The
fresh-process CONTINUE probes, rendered captures and the remaining fixture
drivers ([native drivers](NATIVE_DRIVERS.md)) cover what it does not.
`tests/test_verify_native_route.py` covers the verifier's refusals (stale run
directory, foreign report, missing save), the per-chapter receipts, a
divergence's first byte, a driver pass that hides a differing chapter, and the
probe summaries.

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
