# Build, run and verify

[Project overview](../README.md) · [Documentation index](README.md)

For agent-assisted contributions, read [AGENTS.md](../AGENTS.md) and the
[agent workflow](AGENT_WORKFLOW.md) before choosing a verification run.

## Current environment

This section owns the toolchain versions. The committed GDExtension library
paths target Linux x86-64, and the Steam Deck build is the same Linux target
(see [package for the Steam Deck](#package-for-the-steam-deck)). The supported
Godot version is **4.7.1**, with a 1280×800 viewport. Python 3.10 or newer;
Rust with edition 2024. Supported platforms are a [roadmap](ROADMAP.md)
decision.

The extraction tools use Python's standard library. Pixel-comparison scripts
also use Pillow and NumPy, and `tools/check_docs.py` uses PyYAML to parse the
documents' `yaml` blocks. Native automated checks on Linux use Xvfb.

## Prepare the local pack

From the repository root:

```bash
git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm
./oracle/build_core.sh
python3 -m psiv_tools inspect "/path/to/Phantasy Star IV (USA).md"
python3 -m psiv_tools pack "/path/to/Phantasy Star IV (USA).md" runtime-pack
```

`./oracle/build_core.sh` fetches the pinned emulation core into the ignored
`oracle/gpgx-src/` (it needs `git`, GNU `patch` and a C toolchain).
`psiv-sound`'s build script compiles the core's sound-chip sources from there,
so every Rust build that includes `psiv-godot` or the whole workspace needs it
first.

The oracle host is separate and needs no preparation step: `./oracle/verify.sh`
and every entry point that runs the host compile `oracle/host/` themselves
through `python3 -m oracle.build_host`, and a binary built from other sources is
rebuilt rather than run
([oracle guide](../oracle/README.md#building-the-host-and-why-it-cannot-be-stale)).

The accepted US ROM SHA-256 is:

```text
511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
```

The full pack includes all 361 real maps. `pack --map` deliberately restricts
coverage and is useful for development, but is not the full campaign pack.
The ROM, disassembly checkout and generated pack are
[local inputs](../AGENTS.md#protect-local-inputs-and-evidence).

`generated/` (the extractor's tables and PNGs) is stamped with the extractor
source that wrote it, and every tool that reads it refuses an older extract. After
changing anything under `psiv_tools/`, rebuild it with
`python3 -m psiv_tools regenerate "<ROM path>" generated`
([how the stamp works](EXTRACTION.md#rebuild-the-generated-directory)). A checkout
with no `generated/` is not affected: the readers that skip without it still skip.

A pack built by an older extractor fails to load with an explicit rebuild
message; rerun the full pack command above. Preserve any pack that retained
evidence depends on before regenerating it.

## Launch the native game

Close any running PSIV/Godot instance before rebuilding its loaded extension.

```bash
cargo build --manifest-path rust/Cargo.toml -p psiv-godot
PSIV_SAVE_DIR="$PWD/saves" godot --path godot
```

Here `godot` means your Godot executable. The explicit save directory makes
the example independent of a machine's default save location. Use the title
menu for START/CONTINUE and camp STATE → SAVE for normal persistence.

`godot/run.sh` is a local convenience launcher with build/launch locking.
It currently expects `~/.local/bin/psiv-godot-4.7.1`, `rg` and `flock`;
the commands above let you use another executable path.

`PSIV_GODOT` names the Godot executable for the scripted tools
(`tools/certify.py`, `tools/verify_native_tape.py`) and for the capture
commands in the subsystem documents, which write it as `"$PSIV_GODOT"`. The
tools default to `~/.local/bin/psiv-godot-4.7.1` when it is unset; set it once
in your shell for the documents' commands.

For campaign tape replay through the native game, use
`tools/verify_native_tape.py`; for a whole `psiv-campaign run`,
`tools/verify_native_route.py` replays its tape once from New Game and compares
every chapter save (`--continue-probes` loads each chapter save through the
title's CONTINUE instead). Both run under the heavy-run lock
([gate and coverage](#gate-and-coverage)). Their commands, isolated save and
output directories, exact-pad boundary, source-save hash check, snapshot
comparison and the extension build that makes a whole route take an hour rather
than half a day are in the
[native tape replay section](campaign/CAMPAIGN_RUNNER.md#native-tape-replay).
Which `tools/native/` drivers remain, and why, is in
[native drivers](campaign/NATIVE_DRIVERS.md).

The keyboard and controller bindings are listed in the
[README](../README.md#controls); they come from the input map in
[`godot/project.godot`](../godot/project.godot).

### Package for the Steam Deck

```bash
./tools/package/deploy-deck.sh <deck-address> [user]
```

`tools/package/deploy-deck.sh` builds the release extension with
`cargo zigbuild` against the Deck's glibc (a plain build links the host's newer
symbol versions and the Deck refuses to load it), exports the Godot project's
`Linux-SteamDeck` preset to `build/deck/`, stages the executable with a copy of
`runtime-pack/` beside it (an exported build looks for the pack next to the
executable), and copies the stage to `~/psiv` on the Deck with `rsync` over SSH.
It needs SSH key access to the Deck and, as written, `cargo-zigbuild` and
Godot under `~/.local/bin`. On the Deck, add `~/psiv/psiv.x86_64` as a
non-Steam game.

## Run checks

For the complete Python suite, place the supported image at the repository
root as `Phantasy Star IV (USA).md` and prepare the disassembly and full pack.
The battle-art emission tests also need the ignored indexed PNGs named by
`oracle/fixtures/battle_animation_remainder.json` under
`oracle/fixtures/battle_animation_art/` (184 in the current receipt). A missing
frame is an emission error. Preserve a verified local copy, or reproduce the
frames from the named oracle captures using the renderer documented in
[battle animations](battle/BATTLE_ANIMATIONS.md#runtime-contract-and-fixtures);
do not commit the ROM-derived PNGs.
The documentation check needs no local inputs. From the root:

```bash
python3 tools/check_docs.py
PYTHONPATH=. python3 -m unittest discover -s tests
cargo fmt --manifest-path rust/Cargo.toml --all --check
CARGO_BUILD_JOBS=1 cargo test --manifest-path rust/Cargo.toml --workspace -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
```

Run these sequentially: `python3 tools/gate.py` runs exactly this list, in
order, from one entry point and writes a receipt
([Gate and coverage](#gate-and-coverage)). The Python suite is most of the
gate's time. Serial Rust tests keep one pack-loading case in memory at a time.
`rust/Cargo.toml`'s `[profile.test]` builds tests at `opt-level = 1` with
overflow checks and debug assertions kept on (`psiv-core/tests/build_profile.rs`
fails if either is turned off; `tests/test_build_profiles.py` holds the dev
profile to the same). The workspace suite took 111 s on a warm build (AC, CPU
capped at 2.3 GHz, 2026-10-15), against 1150 s before the change; the gate's
`cargo test` stage, compile included, took 158.7 s on battery under the same
cap. Tests requiring local assets may fail or explicitly skip when
their fixtures are absent; report that distinction with the result.
`build_pack` replays the map-independent sections an earlier build in the same
process emitted from the same ROM; a test that patches an emitter's dependency,
or compares two builds, builds inside `psiv_tools.pack_sections.fresh()`.

The per-commit fast tier is `python3 tools/gate.py --fast`. It runs this list,
in order, with the same receipt format (`tier: "fast"`, directory suffix
`-fast`):

```bash
python3 tools/check_docs.py
cargo fmt --manifest-path rust/Cargo.toml --all --check
CARGO_BUILD_JOBS=1 cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
PYTHONPATH=. python3 tools/gate.py --quick-unittest
CARGO_BUILD_JOBS=1 cargo test --manifest-path rust/Cargo.toml --workspace -- --test-threads=1
```

`--quick-unittest` is the full gate's `unittest discover -s tests` minus the
modules in `SLOW_PYTHON_MODULES` (`tools/gate.py`), which lists each one with
its measured seconds. A new test module joins the fast tier unless it is added
there. The fast tier is a commit check, not a gate result: integration still
needs the full gate.

### Gate and coverage

PSIV has no hosted CI yet ([#15](https://github.com/TusanHomichi/PSIV/issues/15)).
The gate is `python3 tools/gate.py`, run from the root. It runs exactly the five
commands above, in order, with the env prefixes and flags written there, and
writes one log per command plus `receipt.json` under
`build/gate/<UTC stamp>-<short sha>/`: the candidate SHA, whether the tree was
dirty and its `git status --porcelain` paths, and each command's string, UTC
start and end, duration, exit code, log path and parsed counts. Report a gate
result from that receipt, not from a retyped summary. The gate refuses to start
(exit 2, nothing run) while another gate holds the gate lock or while a
process has the debug PSIV extension mapped
(`rust/target/debug/libpsiv_godot.so`, which its cargo commands rebuild);
`--wait` waits for the running gate instead of refusing, naming its holder
once, and `--list` prints the commands without running them. The gate takes
the gate lock and then the [heavy-run lock](#heavy-run-lock) itself, in the
exclusive class for the whole run (its receipt names the class); both are one file each for the whole repository, under the main
checkout's `build/` (`build/gate/.lock` and `build/continuation-heavy.lock`).
Presentation certification
is a separate entry point, `python3 tools/certify.py [--jobs N]` (Godot, Xvfb
and the local oracle frames required; pairs and doctrine in
[scene presentation](scenes/SCENE_PRESENTATION.md)); run it for any change to
Godot-visible paths and report it apart from the gate. An integration candidate
is verified as a whole by `python3 tools/verify_candidate.py --expect-digest <hex>
--expect-chapters <n>` on a clean tree: the release campaign route against the
expected digest and chapter count; then the resume check,
`python3 tools/resume_check.py --run <that run>`, which resumes the route from
every chapter save and fails on any halt
`rust/psiv-campaign/routes/resume_manifest.json` does not list (each entry an
issue link and a date) and on a listed halt that no longer occurs
([the rule](campaign/CAMPAIGN_RUNNER.md#the-runner); its headless resumes run in
parallel and take no lock); then certify, which takes the shared heavy lock
itself (`PSIV_HEAVY_LOCK` overrides it); then `gate.py --wait`, with one receipt under
`build/candidate/`. A route change that moves a halt changes the manifest in the
same commit: `python3 tools/resume_check.py --evaluate <check dir>` re-judges a
finished check against an edited manifest without running it again. When it
runs is set by the [verification tiers](#verification-tiers). Reporting a gate result means
those commands; a subset, a different `CARGO_BUILD_JOBS`, parallel test threads
or a per-crate run is a focused check and is reported as one. The gate does not
cover:

- tests skipped for a missing ROM, disassembly or full pack: a checkout without
  them yields a partial gate, and the report says which inputs were absent;
- native Godot drivers (`tools/native/native_*.gd`, `tools/native/verify_native_*.py`,
  `tools/verify_native_tape.py`, `tools/verify_native_route.py`) and anything
  visual: their ledgers own those runs;
- cartridge comparisons: `./oracle/verify.sh` (fast) and `--full` are separate
  lanes, described in the [oracle guide](../oracle/README.md).

Verify through repository entry points: these commands, the oracle lanes, the
native drivers and their verifiers. A check needed a second time is promoted
into the repository (a script under `tools/` or `oracle/`, or a test in
`tests/` or a crate's `tests/`), with a negative control that proves it can fail
and a line in this guide or the owning ledger. A script under `build/` or a
session's scratch space is not a repeatable check, and its result is not gate
evidence.

### Heavy-run lock

Heavy runs share one machine, so every worktree takes one lock file,
`<main checkout>/build/continuation-heavy.lock` (`PSIV_HEAVY_LOCK` overrides
it), owned by `tools/heavy_lock.py`. Its classes are split by peak memory.
The shared classes draw on one machine budget of `PSIV_LOCK_UNITS` units
(default 16) of 512 MB, 8 GB of the 13.5 GB development host; each holder takes
its class's units, all or none, so shared holders run side by side while their
units fit:

| Class | Holds | Taken by |
| --- | --- | --- |
| `exclusive` | the whole machine | `tools/gate.py` (both tiers, the whole run: its receipt is a timed, reproducible run), certify's extension build, and, through `python3 tools/heavy_lock.py run --class exclusive -- <command>`, pack builds (`python3 -m psiv_tools pack` rewrites `runtime-pack/` in place, which every lane and suite reads) and any other cargo build or test that compiles `psiv-godot` (`-p psiv-godot` or `--workspace`) |
| `build` | 7 units | every other cargo invocation (build, test or clippy, whether or not it loads the pack: compilation is its peak): `verify_candidate`'s release build and `heavy_lock.py run --class build -- cargo ...` |
| `godot` | 1 unit and one of `PSIV_GODOT_SLOTS` slots (default 3) per run | `tools/certify.py` (one slot per `--jobs` worker), `tools/verify_native_tape.py`, `tools/verify_native_route.py`, and `heavy_lock.py run --class godot -- <command>` for any other single Godot/Xvfb run |
| `python` | 2 units | the Python suite or part of it: `heavy_lock.py run --class python -- python3 -m unittest ...` |

A cargo run that compiles `psiv-godot` fits the budget, but stays exclusive:
it relinks the GDExtension library that a Godot run in the same checkout maps,
and the mapped-extension checks run once, before the lock, so only exclusion
keeps every Godot run out until the build ends.

`run` requires `--class`: without one it prints the classes with their units
and exits 2, so no command takes the whole machine by default.

An exclusive holder excludes every shared holder and they exclude it. A cargo
run, two Godot runs and a Python suite fit together (11 units); a second cargo
run then waits (18), and two cargo runs fit with one Godot run (15). The entry points in the
table take their class themselves: never wrap them. A headless
`psiv-campaign` run takes no lock. `PSIV_GODOT_SLOTS` and
`PSIV_LOCK_UNITS` must be the same for every process: each holder records them
in its slot and unit files, and a process that finds a held file recording
another value refuses with exit 2.

One sizing rule holds for every class: its units are the fewest that keep at
least 15% of the holding above its largest measured peak process-tree RSS,
ceil(peak / (0.85 × 512 MB)), the RSS summed over every process of the job.
`build` is sized by the cold `cargo test --workspace --exclude psiv-godot
--no-run` (2,964 MB in 7 units, 17% headroom), `godot` by a certify pair
(418 MB in 1, 18%), `python` by the suite (815 MB in 2, 20%); jobs that
compile `psiv-godot` are exclusive and do not size `build`. The budget is the
machine's margin: 16 units leave about 5.5 GB outside the lock for the
desktop, the agent sessions, unlocked headless runs and any job that peaks
above its measurement. One run each, 2026-10-16, on the 8-core 13.5 GB host on
AC with the CPU capped at 2.3 GHz (`bios_limit` 2300000); `/usr/bin/time -v`
reports the largest single process. Rows marked 0.1 s sampled the tree every
0.1 s at `4abbbc2`; the others every 0.2 s at `20a76de`:

| Job | Wall | Tree peak RSS | `time -v` max RSS | Pack | Class |
| --- | ---: | ---: | ---: | --- | --- |
| `cargo build --release -p psiv-campaign`, cold target, default jobs | 49 s | 1,073 MB | 633 MB | - | `build` |
| the same, after touching `psiv-core` | 45 s | 1,060 MB | 629 MB | - | `build` |
| `cargo build -p psiv-godot` (dev), cold target | 88 s | 3,609 MB | 3,617 MB | - | `exclusive` (relinks the extension) |
| the same, 0.1 s | 93 s | 3,608 MB | 3,591 MB | - | `exclusive` |
| the opt-level-3 extension build ([native tape replay](campaign/CAMPAIGN_RUNNER.md#native-tape-replay)), `CARGO_BUILD_JOBS=2`, cold, 0.1 s | 380 s | 4,817 MB | 3,018 MB | - | `exclusive` |
| the same at default jobs, cold, 0.1 s | 264 s | 5,582 MB | 4,140 MB | - | `exclusive` |
| `cargo build --workspace` (dev), cold, 0.1 s | 87 s | 3,523 MB | 3,496 MB | - | `exclusive` (compiles `psiv-godot`) |
| `cargo test --workspace --exclude psiv-godot --no-run`, cold, 0.1 s | 255 s | **2,964 MB** | 900 MB | - | `build` (sizes it) |
| `cargo clippy --workspace --all-targets`, cold, 0.1 s | 90 s | 2,560 MB | 1,984 MB | - | `build` |
| `cargo build --release -p psiv-campaign`, cold, 0.1 s | 92 s | 974 MB | 627 MB | - | `build` |
| `cargo test --workspace --no-run`, default jobs | 165 s | 3,050 MB | 887 MB | - | `exclusive` (compiles `psiv-godot`) |
| `cargo clippy --workspace --all-targets`, warm | 25 s | 1,762 MB | 525 MB | - | `build` |
| `cargo test -p psiv-data`, built | 5 s | 291 MB | 199 MB | reads | `build` |
| `cargo test -p psiv-core`, built | 2 s | 263 MB | 133 MB | reads | `build` |
| `cargo test -p psiv-runtime`, built | 69 s | 102 MB | 123 MB | reads | `build` |
| `cargo test -p psiv-sound`, built | 1 s | 36 MB | 77 MB | none | `build` |
| `cargo test -p psiv-godot`, built | 5 s | 63 MB | 33 MB | reads | `exclusive` (compiles `psiv-godot`) |
| `cargo test -p psiv-campaign`, built | 38 s | 149 MB | 118 MB | reads | `build` |
| the gate's workspace `cargo test`, built | 123 s | 292 MB | 199 MB | reads | `exclusive` (the gate) |
| the Python suite (`unittest discover -s tests`, 1,648 tests) | 229 s | 815 MB | 813 MB | reads | `python` |
| a full pack build (`python3 -m psiv_tools pack`) | 47 s | 129 MB | 130 MB | writes | `exclusive` |
| one certify pair (`--only battle-0x88 --jobs 1`, Godot and Xvfb) | 8 s | 418 MB | 340 MB | reads | `godot` |
| one headless `psiv-campaign run` chapter (`academy`) | 0.4 s | - | 28 MB | reads | none |

"Built" means the test binaries were compiled first, so the row is the test
run alone; compiling them is the `--no-run` row. A crate's pack use comes from
reading its code, not from its logs: its sources and tests name `runtime-pack`,
`PSIV_RUNTIME_PACK` or `PSIV_PACK` (`grep -rnE
'runtime-pack|PSIV_RUNTIME_PACK|PSIV_PACK' rust/<crate>/src rust/<crate>/tests`,
then read each hit). Every crate's tests read it except `psiv-sound`, whose hits
are comments. A run without the pack does not show it: a test that skips prints
its note with `eprintln!`, which libtest captures when the test passes.
Compilation,
not the pack, sets a cargo invocation's memory: every test run alone is under
300 MB, but `cargo test` compiles first (one crate's rebuild took
`psiv-runtime`'s run to 2,214 MB), so every cargo invocation takes `build`.

Transition: a holder started from a checkout older than the budget takes no
units, so the budget counts only holders running the current
`tools/heavy_lock.py`; exclusion is unaffected, because every class still
holds the lock file itself. Merge main into a lane to bring it onto the budget.

A gate takes the gate lock first and the heavy lock second, as every
deployed gate does; the reverse order deadlocks against them. So never wrap the
gate in `heavy_lock.py run`: a priority gate is `PSIV_LOCK_PRIORITY=1 python3
tools/gate.py ...`, and a gate nested under a heavy holder refuses (exit 2,
naming the holder) when the gate lock is busy instead of waiting.

Waiters queue in arrival order, and `--priority` (or `PSIV_LOCK_PRIORITY=1` for
an entry point that takes the lock itself, such as the gate) puts a waiter
ahead of every normal waiter, including earlier ones. A holder is never
preempted. Speedup lanes, whose work makes later runs faster, and the yield
that lets one go ahead use the flag; nothing else does. While waiting, a line
names the holders and how many waiters, and priority waiters, are ahead.
`python3 tools/heavy_lock.py status` prints the holders and waiters with each
pid's command line.

The holder exports `PSIV_HEAVY_LOCK_HELD` and its class, so an entry point
nested under an exclusive holder, or under a holder of its own class, takes
nothing new; a request under a shared holder of another class (an exclusive
run or a build under godot, a godot run under a build) exits 2, since that
holder excludes nobody and reserved no units for it. A raw `flock(1)` wrapper sets no marker: a gate,
candidate, certify or native replay under one exits 2 naming the parent
instead of waiting forever, and `verify_candidate` refuses it at startup,
before the build.

### Verification tiers

A lane iterates on `python3 tools/gate.py --fast` and the focused checks for
its change; it does not run the full gate or `verify_candidate.py` on its
branch. Those run once per integration candidate, after every reviewed lane
has merged, on the fastest merged process: speedup lanes merge first, so the
others verify on the faster tools. A failing candidate is bisected by its
merge commits, each verified on the same process.

### File size

Source files stay under 1,000 lines. A change that touches a file over the limit
reorganizes it into cohesive modules in the same change. Generated and data
files are exempt: `*.json`, `*.tsv`, `*.csv`, `*.lock` and
`**/replay_fixtures/**`. That list is `EXEMPT` in `tools/size_guard.py`, which
owns the rule; this document names the same globs.

`python3 tools/size_guard.py` scans every file of the change that is text (no
NUL byte in its first 8 KiB) and fails on a non-exempt file over the limit. The
change is what a commit made with `git add -A` would contain - a new unstaged
file included - and its one owner is `tools/repo_files.py`, which lists it for
this guard, for `tools/check_docs.py` and for the feature map's checks. The
scan runs inside the Python suite the gate runs (`tests/test_size_guard.py`),
so the limit is a gate check rather than a reviewer's memory.

The rule has one form and no exceptions list: `EXEMPT` is the only carve-out,
and any other file over the limit fails, so the way past it is to reorganize the
file into cohesive modules. The guard takes no argument that could record a
file the rule should pass.

## Native and original-game comparisons

`tools/native/native_*.gd` drives ordinary Godot input and reads runtime observations.
The campaign itself is replayed by the generic tape driver; the bespoke drivers
that remain use isolated fixtures and debug selectors for branches and captures
the tape does not carry ([native drivers](campaign/NATIVE_DRIVERS.md) says which
and why). A driver runs under Godot's `--script`, so it must name its run
directory with `PSIV_SAVE_DIR`
([repository instructions](../AGENTS.md#protect-local-inputs-and-evidence)).
Cartridge comparisons are described in the [oracle guide](../oracle/README.md).

Output under `build/` holds local logs, captures and saves; it is not part of
the repository. Use the default desktop viewport for visual checks: a reduced
window can clip camp windows.
