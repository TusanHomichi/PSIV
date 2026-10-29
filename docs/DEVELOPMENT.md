# Build, run and verify

[Project overview](../README.md) · [Documentation index](README.md)

This guide is developer setup and the checks. Playing needs less: the
[quick start](../README.md#quick-start-linux) covers the ROM check, the
prerequisites and the commands to build and start the game.

## Current environment

This section owns the toolchain versions. The committed GDExtension library
paths target Linux x86-64, and the Steam Deck build is the same Linux target
(see [package for the Steam Deck](#package-for-the-steam-deck)). The supported
Godot version is the one [`.godot-version`](../.godot-version) pins (a standard
build), with a 1280×800 viewport. Python 3.10 or newer. Rust is the toolchain
[`rust-toolchain.toml`](../rust-toolchain.toml) pins, with edition 2024; rustup
reads that file and installs the toolchain on first use. Each file is the
version's one owner, so a version bump is a one-line change there. Supported
platforms are a [roadmap](ROADMAP.md) decision.

The extraction tools use Python's standard library. Pixel-comparison scripts
also use Pillow and NumPy, and `tools/check_docs.py` uses PyYAML to parse the
documents' `yaml` blocks. Native automated checks on Linux use Xvfb.

On an x86-64 Nix host with flakes enabled, `nix develop`, or direnv reading
`.envrc`, enters a shell with all of these: Python with Pillow, NumPy and
PyYAML (and ruff), the Rust toolchain `rust-toolchain.toml` pins, shellcheck, Xvfb with Mesa's software rasterizer, and the
official Godot build that `.godot-version` pins, exported as `PSIV_GODOT`. That
Godot is set up for headless and Xvfb runs, the checks' paths; for interactive
play on a desktop GPU, set `PSIV_GODOT` to the host's own Godot. [`flake.nix`](../flake.nix) also
holds the pinned release's download hash in `godotHashes`, so a Godot bump adds
the new zip's hash there (`tests/test_flake.py` fails until it does). The
shell's Python links stock zlib, whose PNG bytes differ from zlib-ng's for the
same pixels. That changes no digest: every hash that names an image, in the
tests and in the pack's `png_sha256` fields, is `image_sha256` from
[`psiv_tools/png.py`](../psiv_tools/png.py), taken over what the file decodes to.

## Prepare the local pack

From the repository root:

```bash
git clone https://github.com/alechenninger/ps4disasm reference/ps4disasm
git -C reference/ps4disasm checkout --detach 5d8dcb56943d2d9430b702950daab4436fb109e8
./oracle/build_core.sh
python3 -m psiv_tools inspect "/path/to/Phantasy Star IV (USA).md"
python3 -m psiv_tools pack "/path/to/Phantasy Star IV (USA).md" runtime-pack
```

The disassembly is a research reference: the tests, the oracle and the
documents cite it, and the pack does not read it. The commit above is the one
the citations were checked against. `inspect` prints the ROM's verified SHA-256
and refuses any other image; the ROM is an argument to both commands and can
live anywhere.

`./oracle/build_core.sh` fetches Genesis Plus GX at its pinned commit
`2d7131c5efa606f649d36e1685a8ca47c24f31b3` into the ignored `oracle/gpgx-src/`,
applies `oracle/patches/` and builds the oracle's core (it needs `git`, GNU
`patch`, `make` and a C toolchain).
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

```bash
cargo build --manifest-path rust/Cargo.toml -p psiv-godot
python3 tools/godot.py prepare
PSIV_SAVE_DIR="$PWD/saves" python3 tools/godot.py run
```

`python3 tools/godot.py run` starts the game from a run root: a prepared copy
of the project beside a snapshot of the built library, removed when Godot
exits (`--keep` keeps it, `--dir` names it, and arguments after `--` go to
Godot).

`python3 tools/godot.py prepare` writes the checkout project's extension index,
`godot/.godot/extension_list.cfg`, which Godot's editor import would write and
Git ignores; without it the editor starts on a black screen ("Cannot get
class 'Field'"). `run` prepares its own project copy, so this step is only
needed to open the checkout in the editor. It does nothing once the index lists
the game's extension. The explicit save directory makes
the example independent of a machine's default save location. Use the title
menu for START/CONTINUE and camp STATE → SAVE for normal persistence.

`godot/run.sh` is a local convenience launcher with build/launch locking; it
needs `rg` and `flock`.

`PSIV_GODOT` names the Godot executable; when it is unset, `godot` on `PATH`
is used. `tools/godot.py` is the one lookup: it prints the executable, or
refuses one that does not report the version pinned in `.godot-version` or is
not a `stable` build (`python3 tools/godot.py check` checks your setup).
`python3 tools/godot.py run [-- <godot arguments>]` is the one way to start
the game: it resolves Godot and makes the run root, so a command never names
the executable or a project path. `godot/run.sh`,
`tools/package/deploy-deck.sh`, `tools/certify.py` and
`tools/verify_native_tape.py` ask it too, and the ones that start the game run
its `prepare` step first.

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
symbol versions and the Deck refuses to load it), makes a run root whose
project is prepared (`python3 tools/godot.py run`), exports its
`Linux-SteamDeck` preset from it to
`build/deck/` (the full export log is `build/deck-export.log`; a failed export
stops the script), stages the executable with a copy of
`runtime-pack/` beside it (an exported build looks for the pack next to the
executable), and copies the stage to `~/psiv` on the Deck with `rsync` over SSH.
It needs:

- SSH key access to the Deck;
- [zig](https://ziglang.org/) and `cargo-zigbuild` (`cargo install
  cargo-zigbuild`), on `PATH` or under `~/.local/bin`, which the script adds;
- the Godot that `tools/godot.py` resolves, with the Linux export templates of
  the same version installed (the editor's Export Template Manager, or the
  templates file from the [download archive](https://godotengine.org/download/archive/)).

On the Deck, add `~/psiv/psiv.x86_64` as a non-Steam game.

## Working with agents

Agent-assisted contributions read [AGENTS.md](../AGENTS.md) and the
[agent workflow](AGENT_WORKFLOW.md) before choosing a verification run.

## Run checks

The complete Python suite reads the ROM from a fixed place: put the supported
image at the repository root as `Phantasy Star IV (USA).md` (a copy or a
symbolic link; Git ignores it), and prepare the disassembly and the full pack.
The battle-art emission tests also need the ignored indexed PNGs named by
`oracle/fixtures/battle_animation_remainder.json` under
`oracle/fixtures/battle_animation_art/` (184 in the current receipt). A missing
frame is an emission error. Preserve a verified local copy, or reproduce the
frames from the named oracle captures using the renderer documented in
[battle animations](battle/BATTLE_ANIMATIONS.md#runtime-contract-and-fixtures);
do not commit the ROM-derived PNGs.
The documentation check needs no local inputs; with the disassembly present
it also reads every `ps4.asm:N` citation against it (`tools/check_citations.py`)
and prints `citations: skipped` without it. From the root:

```bash
python3 tools/check_docs.py
PYTHONPATH=. python3 tools/py_suite.py
cargo fmt --manifest-path rust/Cargo.toml --all --check
cargo test --manifest-path rust/Cargo.toml --workspace
cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
python3 tools/pack_parity.py
```

Run these sequentially: `python3 tools/gate.py` runs exactly this list, in
order, from one entry point and writes a receipt
([Gate and coverage](#gate-and-coverage)). `tools/py_suite.py` runs the
Python suite: the modules `python3 -m unittest discover -s tests` finds, one
module per task, on min(cores, 4) fresh worker processes
(`PYTHON_MEASURED_WORKERS`), slowest first, and it prints unittest's own report
summed over every module: each failure's block, `Ran N tests` and `OK` or
`FAILED (...)`. A failure, a module that cannot be imported and a worker that
dies each fail the step; a dead worker's unfinished modules are named. A worker
pays the one-time fixtures (the pack's map-independent sections, the shared
`PackFixtureCase` pack and the map table, about 13 s) once, before its first
module that needs them, so 4 workers pay them up to 4 times. Four is the
measured fastest count on the 8-thread, 4-core host: the full suite took 270 s
in one process and 151, 117, 91, 92, 93 and 100 s on 2, 3, 4, 5, 6 and 8
workers ([the table](#heavy-run-lock)); past the physical cores the extra
fixtures and the shared cores cost more than a worker gains. On 4 workers the
slowest module, `test_pack_maps` (about 50 s), and the summed module seconds
all workers spend (355 s against 268 s in one process, one-time fixtures and
waits included) are what is left. A test that fails
only beside another module is a defect to fix, as in the Rust suites. The gate
runs every command with `CARGO_BUILD_JOBS` and
`RUST_TEST_THREADS` set to the cores available, capped at 8
(`GATE_MEASURED_PARALLELISM` in `tools/gate.py`), and its receipt records both.
Eight is the configuration measured to fit the exclusive class the gate holds:
its largest peak, a cold `cargo test --workspace --no-run` at 8 jobs (4,979 MB
tree RSS on the 8-core host, [the table](#heavy-run-lock)), leaves 39% of the
class's 16 units (8,192 MB) free, above the sizing rule's 15%, and the built
suite peaks at 364 MB on 8 threads. Cargo's own defaults grow with the core
count and the budget does not, so raising the cap takes a new measurement at
the higher count. To repeat the gate's cargo steps by hand, set the same two
variables. Tests in one binary run on
parallel threads, so a test's scratch directory is keyed by process and by call
(`tests/test_rust_temp_paths.py` checks every `temp_dir()`
under `rust/`). `rust/Cargo.toml`'s `[profile.test]` builds tests at
`opt-level = 1` with overflow checks and debug assertions kept on
(`psiv-core/tests/build_profile.rs` fails if either is turned off;
`tests/test_build_profiles.py` holds the dev profile to the same). The built
workspace suite took 47 s on 8 threads against 133 s on one (2026-10-18, mean
clock 2.7 and 2.5 GHz); cargo runs its 57 test binaries one after another, and
the `psiv-runtime` unit tests (16 s) and `psiv-campaign`'s `validator` (11 s)
are most of what is left. After touching `psiv-core`, the two cargo steps took
76 s together against 182 s at one job and one thread. Tests requiring local
assets may fail or explicitly skip when their fixtures are absent; report that
distinction with the result.
`build_pack` replays the map-independent sections an earlier build in the same
process emitted from the same ROM; a test that patches an emitter's dependency,
or compares two builds, builds inside `psiv_tools.pack_sections.fresh()`.

The last step, `tools/pack_parity.py`, holds `psiv-pack`, the Rust pack
builder that replaces `python3 -m psiv_tools pack` at 1.0
([#216](https://github.com/TusanHomichi/PSIV/issues/216)), to the Python
builder. It builds `psiv-pack` in release, runs both builders on a file that is
not the retail ROM and requires the same refusal, and, once `psiv-pack` claims
a section (`psiv-pack --claims` lists each section's paths), builds both packs
from the ROM: every claimed JSON or binary file must be byte-identical, every
PNG equal by `image_sha256`, and `psiv-pack` may write nothing outside its
claims. A ported extractor that no claimed section writes yet is a probe:
`examples/probe.rs` prints its records as one JSON document, `PROBES` in the
tool builds the same from the Python extractors, and the two must be the same
text. It prints `psiv-pack`'s wall time beside the load and clock, the number
a port PR records. Until the switch the Python builder stays the pack everything
uses, so a PR that edits a section `psiv-pack` already claims changes both
builders alike: the gate fails on the Python change alone. Find whether a
section is claimed with `cargo run --release --manifest-path rust/Cargo.toml -p
psiv-pack -- --claims`; `python3 tools/pack_parity.py --keep <dir>` keeps both
packs for a look at a difference. A change to `psiv_tools/png.py` or to how
the builder writes JSON also reruns `python3 tools/pack_parity.py
--write-fixtures`, the Python-written cases `rust/psiv-pack`'s writer tests
read (`tests/test_pack_parity.py` fails while they are stale).

The per-commit fast tier is `python3 tools/gate.py --fast`. It runs this list,
in order, with the same receipt format (`tier: "fast"`, directory suffix
`-fast`):

```bash
python3 tools/check_docs.py
cargo fmt --manifest-path rust/Cargo.toml --all --check
cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
PYTHONPATH=. python3 tools/py_suite.py --fast
cargo test --manifest-path rust/Cargo.toml --workspace
```

`py_suite.py --fast` is the full gate's Python suite minus the modules in
`SLOW_PYTHON_MODULES` (`tools/py_suite.py`), which lists each one with its
measured seconds. A new test module joins the fast tier unless it is added
there. The fast tier is a commit check, not a gate result: integration still
needs the full gate. It took 160 s on a warm target and 189 s after touching
`psiv-core`, against 254 s and 291 s at one job and one thread (2026-10-18,
receipts `20261018T090024Z` and `20261018T085042Z`). Its Python step then took
154 s in one process and takes 56 s on 4 workers, with the same tests (the
gate's own step on 2026-10-20 ran 1,593 in 80.954 s at 2,420 MHz and load
7.67; the merged head after #175/#176 ran 1,645 in 72.1 s at 2,649 MHz; 56 s
is the isolated warm measurement).

### Gate and coverage

Hosted CI (`.github/workflows/flake.yml`) runs `nix flake check` on every pull
request and push to `main`: the part of the gate that needs no local input, in
the Nix sandbox from the tracked tree alone. That is `tools/check_docs.py`
(citations skipped), `cargo fmt --check`, strict Clippy and the Rust workspace
tests except those that load `runtime-pack` (listed by name in `packTests` in
`flake.nix`), plus ruff's syntax and undefined-name rules over the Python; the Rust
build fetches the Genesis Plus GX sound core at the commit
`oracle/build_core.sh` pins. The Python suite, which reads the ROM, the
disassembly and the pack, does not run there, and no ROM or derived asset
reaches the runner. When the pack builder changes, hosted CI also runs its
portability tests (the atomic swap and the text-file census) on Windows and
macOS (`.github/workflows/platforms.yml`), since players build the pack there.
The rest of [#15](https://github.com/TusanHomichi/PSIV/issues/15) (the other
ROM-free Python tests, a Godot smoke) is open.
On a pull request, the `gate` commit status adds the rest: once the review
bots are clean on a head, the steward has an agent on the machine that holds
the local inputs run the full gate, `python3 tools/gate.py --wait`, on that
exact commit, in its own worktree with the inputs linked in, and record the
verdict; the steward merges only once it is green. A gate missing an input is
partial, and a partial gate is recorded as a failure.
The full gate is `python3 tools/gate.py`, run from the root. It runs exactly the six
commands above, in order, with the env prefixes and flags written there, and
writes one log per command plus `receipt.json` under
`build/gate/<UTC stamp>-<short sha>/`: the candidate SHA, whether the tree was
dirty and its `git status --porcelain` paths, the cores it could use
(`cpu_count`), the `cargo_build_jobs` and `rust_test_threads` it set, and each
command's string, UTC start and end, duration, exit code, CPU clock and
highest 1-minute load average while it ran (`cpu_mhz` and `load_1m_max`,
sampled every second; the heavy lock does not exclude unlocked headless runs,
so a slow step is read beside its load), log path and parsed counts. Report a gate
result from that receipt, not from a retyped summary. The gate refuses to start
(exit 2, nothing run) while another gate holds the gate lock;
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
parallel and take no lock); then the closed-issue deferral check,
`python3 tools/check_issue_refs.py --cache <receipt dir>/issue-states.json`,
which asks `gh` for the issue states in one cached call and fails on a
statement that still defers to a closed issue (an `until #N` or `tracked in #N`
sentence); a history mention (`fixed in #N`, `PR #N merged`) carries no
deferral phrase and is not reported, and without `gh` it reports skipped rather
than passed; then certify, which takes the shared heavy lock
itself (`PSIV_HEAVY_LOCK` overrides it); then `gate.py --wait`, with one receipt under
`build/candidate/`. A route change that moves a halt changes the manifest in the
same commit: `python3 tools/resume_check.py --evaluate <check dir>` re-judges a
finished check against an edited manifest without running it again. When it
runs is set by the [verification tiers](#verification-tiers). Reporting a gate result means
those commands, at the gate's jobs and threads; a subset, another
`CARGO_BUILD_JOBS` or test-thread count, or a per-crate run is a focused check
and is reported as one. The gate does not
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
| `exclusive` | the whole machine | `tools/gate.py` (both tiers, the whole run: its receipt is a timed, reproducible run), and, through `python3 tools/heavy_lock.py run --class exclusive -- <command>`, the opt-level-3 extension build of the [native tape replay](campaign/CAMPAIGN_RUNNER.md#native-tape-replay) |
| `build` | 7 units | a cargo invocation that does not compile `psiv-godot` (build, test or clippy, whether or not it loads the pack: compilation is its peak): `verify_candidate`'s release build and `heavy_lock.py run --class build -- cargo ...` |
| `extension` | 12 units | a dev cargo invocation that compiles `psiv-godot` (`-p psiv-godot`, `--workspace` build, test or clippy, or no `-p` at the workspace root): certify's build and `heavy_lock.py run --class extension -- cargo ...` |
| `godot` | 1 unit and one of `PSIV_GODOT_SLOTS` slots (default 3) per run | `tools/certify.py` (one slot per `--jobs` worker), `tools/verify_native_tape.py`, `tools/verify_native_route.py`, and `heavy_lock.py run --class godot -- <command>` for any other single Godot/Xvfb run |
| `python` | 3 units | the Python suite or part of it: `heavy_lock.py run --class python -- python3 -m unittest ...` |
| `pack` | 1 unit | a pack build: `heavy_lock.py run --class pack -- python3 -m psiv_tools pack ...` |

The caller declares the class, and the lock trusts it. A wrong declaration, a
`psiv-godot` build in `build`, costs memory headroom, not a crash: every Godot
a repository tool starts runs from a run root with its own copy of the library
(`create_run_root` in `tools/godot.py`), cargo replaces the checkout's library
with a new file rather than rewriting it (the linker, mold here, writes a new
inode), and a non-editor Godot never re-reads a loaded extension. So an extension build shares the machine with Godot runs:
12 units fit beside three Godot runs (15 of 16). What the one class costs: it is
sized for its largest job, the cold workspace test build, so an incremental
`cargo build -p psiv-godot` (3.6 GB) reserves 6 GB, and an extension build
waits for any `build` holder (12 + 7) and for a second extension build.

A pack build shares the machine with the runs that read the pack:
`psiv_tools pack` builds into a sibling directory and swaps it in whole
(`psiv_tools/pack_swap.py`), so a reader sees the old pack or the new one,
never a half-written one, and a killed build leaves the old pack in place.
`runtime-pack` becomes a link to the newest of three kept versions,
`.runtime-pack.v<UTC stamp>-<pid>` beside it. A reader that must see one
version for its whole run resolves the link once, as a run root and the game
do; one that reopens `runtime-pack/...` file by file can mix two complete
versions across a swap. On Windows, where an ordinary user cannot make the
link, `runtime-pack` stays a real directory that the swap renames aside and
replaces, and no older version is kept.

`run` requires `--class`: without one it prints the classes with their units
and exits 2, so no command takes the whole machine by default.

An exclusive holder excludes every shared holder and they exclude it. A cargo
run, two Godot runs and a Python suite fit together (12 units); a second cargo
run then waits (19), and two cargo runs fit with one Godot run (15). The entry points in the
table take their class themselves: never wrap them. A headless
`psiv-campaign` run takes no lock. `PSIV_GODOT_SLOTS` and
`PSIV_LOCK_UNITS` must be the same for every process: each holder records them
in its slot and unit files, and a process that finds a held file recording
another value refuses with exit 2.

One sizing rule holds for every class: its units are the fewest that keep at
least 15% of the holding above its largest measured peak process-tree RSS,
ceil(peak / (0.85 × 512 MB)), the RSS summed over every process of the job.
Measure a job, to size a class or to remeasure a row below, with
`python3 tools/heavy_lock.py run --priority --class exclusive -- python3
tools/tree_rss.py --json <file> -- <job>`: `tools/tree_rss.py` samples the
job's whole process tree every 0.1 s and reports its peak, the wall time, the
mean and fastest-core live clock and the highest 1-minute load, and the
exclusive hold keeps every other locked job off the machine while it runs.
`build` is sized by the cold `cargo test --workspace --exclude psiv-godot
--no-run` (2,964 MB in 7 units, 17% headroom), `extension` by the cold
`cargo test --workspace --no-run` at 8 jobs (4,979 MB in 12, 19%), `godot` by a certify pair
(418 MB in 1, 18%), `python` by the suite (926 MB in one process and 1,256 MB
on four workers; 3 units, 18%); the opt-level-3
extension build (5,582 MB, 13 units) is exclusive and sizes nothing. The budget is the
machine's margin: 16 units leave about 5.5 GB outside the lock for the
desktop, the agent sessions, unlocked headless runs and any job that peaks
above its measurement. One run each, 2026-10-16, on the 8-core 13.5 GB host on
AC with the CPU capped at 2.3 GHz (`bios_limit` 2300000); `/usr/bin/time -v`
reports the largest single process. Rows marked 0.1 s sampled the tree every
0.1 s at `4abbbc2`; the others every 0.2 s at `20a76de`. Rows marked #157 were
measured 2026-10-18 at `0c64b19`, sampled every 0.1 s, on AC with the CPU
uncapped (boosting to 3.9 GHz); each names its mean live clock
(`/proc/cpuinfo`) and has no `time -v` figure. Rows marked #166 were measured
the same way with `tools/tree_rss.py`, at `6da25cd` plus the suite runner:

| Job | Wall | Tree peak RSS | `time -v` max RSS | Pack | Class |
| --- | ---: | ---: | ---: | --- | --- |
| `cargo build --release -p psiv-campaign`, cold target, default jobs | 49 s | 1,073 MB | 633 MB | - | `build` |
| the same, after touching `psiv-core` | 45 s | 1,060 MB | 629 MB | - | `build` |
| `cargo build -p psiv-godot` (dev), cold target | 88 s | 3,609 MB | 3,617 MB | - | `extension` |
| the same, 0.1 s | 93 s | 3,608 MB | 3,591 MB | - | `extension` |
| the opt-level-3 extension build ([native tape replay](campaign/CAMPAIGN_RUNNER.md#native-tape-replay)), `CARGO_BUILD_JOBS=2`, cold, 0.1 s | 380 s | 4,817 MB | 3,018 MB | - | `exclusive` |
| the same at default jobs, cold, 0.1 s | 264 s | 5,582 MB | 4,140 MB | - | `exclusive` |
| `cargo build --workspace` (dev), cold, 0.1 s | 87 s | 3,523 MB | 3,496 MB | - | `extension` (compiles `psiv-godot`) |
| `cargo test --workspace --exclude psiv-godot --no-run`, cold, 0.1 s | 255 s | **2,964 MB** | 900 MB | - | `build` (sizes it) |
| `cargo clippy --workspace --all-targets`, cold, 0.1 s | 90 s | 2,560 MB | 1,984 MB | - | `build` |
| `cargo build --release -p psiv-campaign`, cold, 0.1 s | 92 s | 974 MB | 627 MB | - | `build` |
| `cargo test --workspace --no-run`, cold, default jobs (8), #157, 2.8 GHz | 191 s | **4,979 MB** | - | - | `exclusive` in the gate (its peak); `extension` outside it (sizes it) |
| the same, `CARGO_BUILD_JOBS=4`, #157, 2.6 GHz | 216 s | 3,999 MB | - | - | `exclusive` |
| the same, `CARGO_BUILD_JOBS=1`, #157, 2.5 GHz | 583 s | 2,824 MB | - | - | `exclusive` |
| `cargo clippy --workspace --all-targets`, warm | 25 s | 1,762 MB | 525 MB | - | `build` |
| `cargo test -p psiv-data`, built | 5 s | 291 MB | 199 MB | reads | `build` |
| `cargo test -p psiv-core`, built | 2 s | 263 MB | 133 MB | reads | `build` |
| `cargo test -p psiv-runtime`, built | 69 s | 102 MB | 123 MB | reads | `build` |
| `cargo test -p psiv-sound`, built | 1 s | 36 MB | 77 MB | none | `build` |
| `cargo test -p psiv-godot`, built | 5 s | 63 MB | 33 MB | reads | `extension` (compiles `psiv-godot`) |
| `cargo test -p psiv-campaign`, built | 38 s | 149 MB | 118 MB | reads | `build` |
| the workspace `cargo test --test-threads=1`, built (the gate before #157) | 123 s | 292 MB | 199 MB | reads | `exclusive` (the gate) |
| the gate's workspace `cargo test`, built, default threads (8), #157, 2.7 GHz | 47 s | 364 MB | - | reads | `exclusive` (the gate) |
| the same, `--test-threads=4`, #157, 2.8 GHz | 53 s | 305 MB | - | reads | `exclusive` |
| the same, `--test-threads=2`, #157, 2.6 GHz | 78 s | 305 MB | - | reads | `exclusive` |
| the same, `--test-threads=1`, #157, 2.5 GHz | 133 s | 305 MB | - | reads | `exclusive` |
| the Python suite (`unittest discover -s tests`, 1,648 tests) | 229 s | 815 MB | 813 MB | reads | `python` |
| the same, 1,822 tests, one process, #166, 2.5 GHz, load 1.6 | 270 s | 926 MB | - | reads | `python` |
| `tools/py_suite.py`, 2 workers, #166, 2.7 GHz, load 2.2 | 151 s | 1,115 MB | - | reads | `exclusive` (the gate) |
| the same, 3 workers, #166, 2.8 GHz, load 4.9 | 117 s | 1,188 MB | - | reads | `exclusive` |
| the same, 4 workers (the gate's), #166, 2.9 GHz, load 4.2 | 91 s | 1,256 MB | - | reads | `exclusive` |
| the same, 5 workers, #166, 2.8 GHz, load 4.6 | 92 s | 1,347 MB | - | reads | `exclusive` |
| the same, 6 workers, #166, 2.7 GHz, load 5.8 | 93 s | 1,450 MB | - | reads | `exclusive` |
| the same, 8 workers, #166, 2.6 GHz, load 8.5 | 100 s | 1,639 MB | - | reads | `exclusive` |
| the fast tier's Python step before the runner (`build/py-par/base-tools/gate.py --quick-unittest`, 1,593 tests), #166, 2.5 GHz | 154 s | 883 MB | - | reads | `exclusive` (the gate) |
| `py_suite.py --fast`, 4 workers, #166, 2.9 GHz, load 2.9 | 56 s | 1,197 MB | - | reads | `exclusive` (the gate) |
| a full pack build (`python3 -m psiv_tools pack`) | 47 s | 129 MB | 130 MB | writes | `pack` (sizes it) |
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
`psiv-runtime`'s run to 2,214 MB), so a cargo invocation takes `build` for its
memory, unless it can compile `psiv-godot`: then it takes `extension` (above).

Transition: a holder started from a checkout older than the budget takes no
units, so the budget counts only holders running the current
`tools/heavy_lock.py`; exclusion is unaffected, because every class still
holds the lock file itself. Merge main into a lane to bring it onto the budget.

A gate takes the gate lock first and the heavy lock second, as every
deployed gate does; the reverse order deadlocks against them. So never wrap the
gate in `heavy_lock.py run`: the full gate queues first by itself (below), a
priority fast tier is `PSIV_LOCK_PRIORITY=1 python3 tools/gate.py --fast`, and
a gate nested under a heavy holder refuses (exit 2, naming the holder) when the
gate lock is busy instead of waiting.

Waiters queue in arrival order, with two ranks ahead of the rest. The full
gate (`tools/gate.py` without `--fast`), which a pull request waits on to
merge, goes ahead of every other waiter. `--priority` (or `PSIV_LOCK_PRIORITY=1`
for an entry point that takes the lock itself, such as the fast tier) puts a
waiter ahead of every normal waiter, including earlier ones. A holder is never
preempted, so a long run outside the gate (a native replay, a campaign) holds
the machine from every gate until it ends: start one only when no gate is
waiting (`status`). Speedup lanes, whose work makes later runs faster, and the
yield that lets one go ahead use `--priority`; nothing else does. While
waiting, a line names the holders and how many waiters (gates and priority
waiters among them) are ahead, again whenever that changes, and every
`WAIT_REPORT_S` while nothing does, a line says how long it has waited and each
holder's class, age and command line. A gate waiting for the gate lock with
`--wait` reports the same way, so a job runner that stops a job gone quiet
sees it waiting. `python3 tools/heavy_lock.py status` prints the holders and
waiters with each pid's command line.

Transition: a waiter started from a checkout older than the gate rank writes
ranks 0 (priority) and 1 (normal) and ignores rank-2 entries. The current code
reads its priority entries as gates and its normal entries as priority, and an
old waiter may be granted ahead of a current normal waiter. Nobody waits
forever and exclusion is unaffected. Merge main into a lane to bring it onto
the ranks.

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
branch. The full gate runs on its PR's head before the steward merges it
([gate and coverage](#gate-and-coverage)). `verify_candidate.py` runs once per
integration candidate, after every reviewed lane has merged, on the fastest
merged process: speedup lanes merge first, so the others verify on the faster
tools. A failing candidate is bisected by its
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
