# Build, run and verify

[Project overview](../README.md) · [Documentation index](README.md)

This guide is developer setup and the checks. Playing needs less: the
[quick start](../README.md#quick-start-linux) covers the ROM check, the
prerequisites and the commands to build and start the game.

## Current environment

This section owns the toolchain versions. The committed GDExtension library
paths target Linux x86-64, and the Steam Deck build is the same Linux target
(see [package for the Steam Deck](#package-for-the-steam-deck)). The supported
Godot version is **4.7.1** (pinned in `.godot-version`), with a 1280×800
viewport. Python 3.10 or newer; Rust 1.97.1 with edition 2024, pinned in
`rust-toolchain.toml`, which rustup reads and installs on first use. Supported
platforms are a [roadmap](ROADMAP.md) decision.

The extraction tools use Python's standard library. Pixel-comparison scripts
also use Pillow and NumPy, and `tools/check_docs.py` uses PyYAML to parse the
documents' `yaml` blocks. Native automated checks on Linux use Xvfb.

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

Close any running PSIV/Godot instance before rebuilding its loaded extension.

```bash
cargo build --manifest-path rust/Cargo.toml -p psiv-godot
PSIV_SAVE_DIR="$PWD/saves" godot --path godot
```

Here `godot` means your Godot executable. The explicit save directory makes
the example independent of a machine's default save location. Use the title
menu for START/CONTINUE and camp STATE → SAVE for normal persistence.

`godot/run.sh` is a local convenience launcher with build/launch locking; it
needs `rg` and `flock`.

`PSIV_GODOT` names the Godot executable; when it is unset, `godot` on `PATH`
is used. `tools/godot.py` is the one lookup: it prints the executable, or
refuses one that does not report the version pinned in `.godot-version`
(`python3 tools/godot.py` checks your setup). `godot/run.sh` and
`tools/package/deploy-deck.sh`, `tools/certify.py` and
`tools/verify_native_tape.py` ask it. The capture commands in the subsystem
documents write it as `"$PSIV_GODOT"`; set it once in your shell.

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
It needs SSH key access to the Deck, `cargo-zigbuild` under `~/.local/bin`, and
the Godot that `tools/godot.py` resolves. On the Deck, add `~/psiv/psiv.x86_64` as a
non-Steam game.

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
exclusive class; both are one file each for the whole repository, under the main
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
parallel and take no lock); then certify under the shared heavy lock
(`PSIV_HEAVY_LOCK` overrides it); then `gate.py --wait`, with one receipt under
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
it), owned by `tools/heavy_lock.py`. It has two classes:

| Class | Holds | Taken by |
| --- | --- | --- |
| `exclusive` | the whole machine | `tools/gate.py` (both tiers), certify's extension build, and, through `python3 tools/heavy_lock.py run -- <command>`, every other cargo build, test or clippy run, the Python suite and pack builds |
| `godot` | one of `PSIV_GODOT_SLOTS` slots (default 3), about 350 MB each | `tools/certify.py` (one slot per `--jobs` worker), `tools/verify_native_tape.py`, `tools/verify_native_route.py`, and `heavy_lock.py run --class godot -- <command>` for any other single Godot/Xvfb run |

An exclusive holder excludes every godot holder and they exclude it. The
entry points in the table take their class themselves: never wrap them. A
headless `psiv-campaign` run (about 40 MB) takes no lock. `PSIV_GODOT_SLOTS`
must be the same for every process: each holder records its cap in its slot
file, and a process that finds a held slot recording another cap refuses with
exit 2. A gate takes the gate lock first and the heavy lock second, as every
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
nested under it (certify under `verify_candidate`) takes nothing new; an exclusive request under a godot holder exits 2, since that
holder excludes nobody. A raw `flock(1)` wrapper sets no marker: a gate,
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
