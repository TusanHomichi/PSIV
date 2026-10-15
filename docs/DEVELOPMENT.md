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
once, and `--list` prints the commands without running them. The gate lock and
the heavy lock are one file each for the whole repository, under the main
checkout's `build/` (`build/gate/.lock` and `build/continuation-heavy.lock`),
and `tools/heavy_lock.py` owns both. The gate takes the heavy lock itself, as
`tools/verify_candidate.py` does for its certify step, so gates and heavy runs
in different worktrees serialize; never wrap either of them. Every other heavy
command runs as `python3 tools/heavy_lock.py run -- <command>`: the holder
exports `PSIV_HEAVY_LOCK_HELD`, so the gate or a driver nested under it does not
wait on its own parent. A raw `flock(1)` wrapper sets no marker; a gate or
candidate run under one exits 2 naming the parent instead of waiting forever.
Presentation certification
is a separate entry point, `python3 tools/certify.py` (Godot, Xvfb and
the local oracle frames required; pairs and doctrine in
[scene presentation](scenes/SCENE_PRESENTATION.md)); run it for any change to
Godot-visible paths and report it apart from the gate. An integration candidate
is verified as a whole by `python3 tools/verify_candidate.py --expect-digest <hex>
--expect-chapters <n>` on a clean tree: the release campaign route against the
expected digest and chapter count, then certify under the shared heavy lock
(`PSIV_HEAVY_LOCK` overrides it), then `gate.py --wait`, with
one receipt under `build/candidate/`. Reporting a gate result means
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
