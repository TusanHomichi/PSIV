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

- reach a map and cell, path-finding across the pack's warp graph (each hop
  lands where `GameMode_LoadFieldMap`'s four story redirects send it, the same
  `psiv_runtime::adjust_field_load` the field loader applies), on foot or
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
- `go_to_map` with `arrival_scene` onto a map whose load starts a scene that
  carries the party off it (`Cutscene_Reunion` on the Dezo spaceport): the
  objective ends only when the map has loaded and a scene that ended after
  that load has left the party on another map, and the next `expect` says
  where. A scene that ended before the load does not count, and a marked
  objective whose party stays on the map (no scene, or one that leaves it
  there with a menu or prompt open) halts. An unmarked `go_to_map` that a
  scene carries off its map halts instead of walking back. `plan --from-save FILE` plans from a chapter
  save's position and event, temp and town flags;
- `wait` frames while the field runs (an object a scene sent walking, the
  Esper Mansion's guards, finishes before the next walk is planned around it),
  and `opens` on a `talk` for the cells such a conversation clears, as on an
  `interact`, or on a `wait` for the cells objects the field moves by itself
  clear (the inner guards once `$96` is set); `board` without a `step` answers the destination menu a scene
  opens by itself (`Cutscene_FindingAirCastle`);
- fight scripted battles; random battles use a policy that issues commands
  through the battle menu with the same pad input a player would use;
- `restore` the party through the camp before a boss: the fallen are raised
  with a reviving technique (battle effect 21, `REVER`) and the living cured
  to full, with whatever TP the party has (`recovery.rs`).

Chapters end with their specified assertions on flags, party, map and
inventory taken from the scene transcriptions. The route's last chapter presses
Start at the ending's final gate (`press`, `Cutscene_Ending`'s `WaitForStart`,
`docs/scenes/89_Ending.md`): the session turns the press into the scene's
continue input, and `expect`'s `game_cleared` claims the latched
`Game_Cleared_Flag`. An `expect` may also check `ending_gate` where the run
stops at the gate instead of pressing it. The runner then writes a
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
frames, battles, save path, the FNV-1a 64 of the chapter save and the
controllers' `decisions`, each `f<frame>: <line>`: an objective beginning, a
scene carrying the party off, a patrol's refuge, a camp action, a cure) beside the
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
| random and scripted battles | `battle.rs`, `policy.rs`, `policy_plan.rs`, `policy_value.rs`, `policy_estimate.rs`, `policy_board.rs`, `policy_board_read.rs`, `policy_opening.rs` | the battle view's menus (ATTACK, TECH, SKILL, ITEM), one press at a time |
| `press` | `exec.rs` | one pad button on a frame (`start`); the ending's final gate turns it into the scene's continue input |
| `expect` | `expect.rs` | nothing: it settles the game and reads flags, map, cell, party, purse, vehicle, the pack (`items_held`, `items_absent`), the party's status bytes (`status_clear`) and the ending's gate or cleared byte (`ending_gate`, `game_cleared`) |

The planner never walks onto a map-change cell that no warp covers (the
engine's `WarpUnmapped`, which the runner halts on unless the cell is a doorway
row, `cell_plan::borders_warp`, one predicate for both); only a `step_onto` may
end on one, where a trigger answers the step (`RunEvent_ProfoundDarkness` on
`TheEdge_Part9`'s row 23). The validator forgets whether the party rides when
an `arrival_scene` loads a map (a scene's `RefreshMap` keeps the vehicle only
with `Map_Load_Flags` bit 3), so the next walk needs an `expect` that names
`vehicle`.

`plan --vehicle N` prints a walking plan for a mounted party. Mounted walks and
the validator's `vehicle` claim exist because Motavia's sand blocks a walker and
the Land Rover crosses it: the Ladea Tower cannot be reached on foot from Krup
(`rust/psiv-campaign/tests/cli.rs::plan_vehicle_crosses_the_sand_to_the_tower`).

`field.rs` is the loop every controller calls first: it plays frames until the
game hands control back, turning finished pages with Speak, acknowledging chest
results, fighting battles with the chapter's policy, and waiting out scenes.

**Battle policy.** `random_battle_policy` names a temperament of the one party
policy (`policy.rs`); the names differ only in when the party RUNs. At a
round's first command window the planner (`policy_plan.rs`, `policy_value.rs`,
`policy_estimate.rs`) orders every member from the board (`policy_board.rs`):
each fighter as the engine holds it, and what each member's window would enable
(`Runtime::window_techniques`, `window_skills`, `window_items`), valued with the
engine's own formulas and rules. It returns an `Intent` and `battle.rs` presses
the menus to it. The rules are in those modules' docs, their tests in
`policy_plan_tests.rs` and `rust/psiv-campaign/tests/party_policy.rs`. Losing is a halt; after a
battle the camp cures the party (`recovery.rs`).

**Battle ends.** `run` prints one line under its chapter for every scripted
battle, who stood as the field took the screen back (`BattleRecord::end_line`),
and the report carries them (`event_battles`, or `battles` for a halted run).

**Every resume finishes.** A fresh process resumed from a chapter save starts
the cartridge's random stream again, so the route must finish from every
chapter save, except the halts `rust/psiv-campaign/routes/resume_manifest.json`
lists, each with its issue and date (the entry format is in the file's `about`
and in `tools/resume_check.py`). `python3 tools/resume_check.py --run <completed
run>` resumes the route from every chapter save of the run and fails on an
unlisted halt or a stale entry; `--evaluate <check dir>` re-judges a finished
check against an edited manifest. `tools/verify_candidate.py` runs it for every
integration candidate ([DEVELOPMENT.md](../DEVELOPMENT.md#gate-and-coverage)).
The training chapters keep their earlier levels (owner decision,
2026-10-07); the manifest lists the halts the route still has.
`python3 tools/route_sweep.py NAME --level CHAPTER=N` plays a variant of the
route and summarizes its battle ends.

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
`nearby_warps`, `view` (the open dialogue, shop, camp, battle or chest), the
`battles` fought and the halted chapter's `decisions`. `PSIV_CAMPAIGN_TRACE=1` prints every event and every battle
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

### Boss-margin probe

`probe <route> --chapter ID --from-save FILE --trials N [--spacing FRAMES]
[--work-dir DIR] [--report OUT] [--pack DIR]` plays one chapter from the save
its predecessor ended on, `N` times, each in a fresh session through the run
loop's own chapter play (the same controllers, policy and recovery). Trial `i`
first stands still for `i * spacing` frames: the cartridge stirs its seed word
every frame (`UpdateRNGSeed` in the VInt handler), so each trial meets the
chapter on another stream, with no seam into the runtime. The default spacing,
65,536 frames, keeps the trials' windows of the one LCG sequence apart for an
attempt of up to some 65,000 frames. A defeat counts as a loss, any other halt
is reported apart, and the summary gives the wins over the fought trials with
a Wilson 95% lower bound (`src/probe.rs`). A training chapter's target is
chosen from such a table and the table goes in the chapter's `source`
(`edge-training`; #154).

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
python3 tools/verify_native_tape.py --tape /path/to/run.tape \
  --expect-save /path/to/chapter/slot_1.sram --pack /path/to/runtime-pack \
  --out build/native-tape-academy
```

The verifier takes one godot-class slot of the repository's heavy-run lock
itself, so it needs no wrapper; a raw `flock(1)` around it makes it exit 2
([heavy-run lock](../DEVELOPMENT.md#heavy-run-lock)).

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
longer hash to the report's `save_fnv`. Pass `--library` with an
opt-level-3 extension to replay the route in hours rather than a day (the
extension-build paragraph below measures the plain debug build at 90 frames/s;
the release build replayed the full 84-chapter route at 730 frames/s, 8,575.7 s
for 6,260,544 frames). The flag reaches `--continue-probes` too.

```bash
# Once, in the worktree that will replay: an extension that replays at about
# 1,000 frames/s. Its peak (5.6 GB) makes it exclusive.
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_OPT_LEVEL=3 CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=false \
  python3 tools/heavy_lock.py run --class exclusive -- cargo build --manifest-path rust/Cargo.toml -p psiv-godot
CARGO_BUILD_JOBS=2 python3 tools/heavy_lock.py run --class build -- \
  cargo build --release --manifest-path rust/Cargo.toml -p psiv-campaign
rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json \
  --save-dir build/r2-route --tape build/r2-route/run.tape --report build/r2-route/report.json
# Each verifier takes its own godot-class slot of the heavy-run lock.
python3 tools/verify_native_route.py --run-dir build/r2-route --out build/r2-native
python3 tools/verify_native_route.py \
  --run-dir build/r2-route --out build/r2-native-continue --continue-probes

# Or a release extension passed explicitly (same flag on the probe run).
python3 tools/heavy_lock.py run --class exclusive -- \
  cargo build --release --manifest-path rust/Cargo.toml -p psiv-godot
python3 tools/verify_native_route.py --run-dir build/r2-route --out build/r2-native \
  --library rust/target/release/libpsiv_godot.so
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
`cargo build -p psiv-godot` replaces it with the slow one again. The replay
runs from a run root holding a copy of it (`create_run_root` in
`tools/godot.py`), so a build during the replay does not touch it; the
receipt records the copy's SHA-256 and the mapped path and inode. Debug assertions are
off in this build and on in the Rust suites.

The tape replay runs headless with the render loop off: it exercises input,
state and the save writer, not the picture, and takes no mid-run capture. The
fresh-process CONTINUE probes, rendered captures and the remaining fixture
drivers ([native drivers](NATIVE_DRIVERS.md)) cover what it does not.
`tests/test_verify_native_route.py` covers the verifier's refusals (stale run
directory, foreign report, missing save), the per-chapter receipts, a
divergence's first byte, a driver pass that hides a differing chapter, and the
probe summaries.

**Tests.** `cargo test --manifest-path rust/Cargo.toml -p psiv-campaign` runs the planner and validator suites and
`rust/psiv-campaign/tests/runner.rs`: chapter one twice gives identical tapes, digests and saves;
a tape replays to its digest, also from a chapter save; a wrong object index,
an expectation that cannot hold, an unreachable cell and a spent budget each
halt naming the objective; the exit statuses; `rust/psiv-campaign/tests/spaceport.rs` walks onto the
Mota Spaceport's boarding row, whose scene must not halt on its type-1 cell;
`rust/psiv-campaign/tests/split.rs` checks that a completed report names its chapters with the
hash of each save, that the cut tapes add up to the run's tape and chain through
those hashes, and that a split refuses a halted report, a foreign tape and an
existing output. The
whole route is `#[ignore]`d:
`cargo test --release --manifest-path rust/Cargo.toml -p psiv-campaign --test
runner -- --ignored`. Cases that need the pack skip with a
message when it is absent.
