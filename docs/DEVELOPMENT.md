# Build, run and verify

[Project overview](../README.md) · [Documentation index](README.md)

For agent-assisted contributions, read [AGENTS.md](../AGENTS.md) and the
[agent workflow](AGENT_WORKFLOW.md) before choosing a verification run.

## Current environment

The committed GDExtension library paths target Linux x86-64. Godot 4.7.1 is
the version used for native verification, with a 1280×800 viewport. Python
project metadata specifies 3.10+; Rust uses edition 2024. Other platforms and
packaged desktop exports remain roadmap work.

The extraction tools use Python's standard library. Pixel-comparison scripts
also use Pillow and NumPy. Native automated checks on Linux use Xvfb.

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

The accepted US ROM SHA-256 is:

```text
511f35cc11f88316f8b8940e28ab298bd75a4da193672a80172884d6eb913b6a
```

The full pack includes all 361 real maps. `pack --map` deliberately restricts
coverage and is useful for development, but is not the full campaign pack.
The ROM, disassembly checkout and generated pack are local inputs/outputs;
they are excluded from Git.

The 2026-09-23 overworld repair requires the resolved `overworld_patches` and
composed base/priority atlas data. Older packs fail with an explicit rebuild
message; rerun the full pack command above. Preserve any pack used by retained
evidence before regeneration. See [map effects](field/MAP_EFFECTS.md#12-native-overworld-page-hook-consumption-2026-09-23).

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

| Input | Action |
| --- | --- |
| Arrow keys | Move / menu selection |
| Enter, Space or Z | Confirm / interact |
| Escape or X | Camp / cancel |

## Run checks

For the complete Python suite, place the supported image at the repository
root as `Phantasy Star IV (USA).md` and prepare the disassembly and full pack.
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
([Gate and coverage](#gate-and-coverage)). Full map/pack tests can take several
minutes. Serial Rust tests avoid the memory pressure seen when many
pack-loading cases run together. Tests requiring local assets may fail or
explicitly skip when their fixtures are absent; report that distinction with
the result.

### Gate and coverage

PSIV has no hosted CI yet ([#15](https://github.com/TusanHomichi/PSIV/issues/15)).
The gate is `python3 tools/gate.py`, run from the root. It runs exactly the five
commands above, in order, with the env prefixes and flags written there, and
writes one log per command plus `receipt.json` under
`build/gate/<UTC stamp>-<short sha>/`: the candidate SHA, whether the tree was
dirty and its `git status --porcelain` paths, and each command's string, UTC
start and end, duration, exit code, log path and parsed counts. Report a gate
result from that receipt, not from a retyped summary. The gate refuses to start
(exit 2, nothing run) while another gate holds `build/gate/.lock` or while a
process has the debug PSIV extension mapped
(`rust/target/debug/libpsiv_godot.so`, which its cargo commands rebuild);
`--list` prints the commands without running them. Reporting a gate result means
those commands; a subset, a different `CARGO_BUILD_JOBS`, parallel test threads
or a per-crate run is a focused check and is reported as one. The gate does not
cover:

- tests skipped for a missing ROM, disassembly or full pack: a checkout without
  them yields a partial gate, and the report says which inputs were absent;
- native Godot drivers (`tools/native/native_*.gd`, `tools/native/verify_native_*.py`) and
  anything visual: their ledgers own those runs;
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

`tools/size_baseline.txt` is the ratchet: one `<lines> <path>` per line, sorted
by path, for the files that were already over the limit when the guard landed.
The guard fails a new over-limit file, a baselined file that grew past its
count, a baselined file now at or under the limit, and a baselined path that is
gone; a baselined file that shrank while staying over the limit passes, with a
suggestion to lower its count. `--write-baseline` seeds that file and may lower
a count; it refuses to record growth, because a file over the limit is
reorganized, not baselined.
[#12](https://github.com/TusanHomichi/PSIV/issues/12) tracks the files still
listed there.

## Native and original-game comparisons

`tools/native/native_*.gd` drives ordinary Godot input and reads runtime observations.
Some drivers use isolated fixtures; others continue a saved campaign. Their
ledgers identify which kind of evidence each run supplies. A driver runs under
Godot's `--script`, so it must name its run directory with `PSIV_SAVE_DIR`; the
extension refuses to resolve one without the variable
([repository instructions](../AGENTS.md#protect-local-inputs-and-evidence)).

- [BioPlant and recovery](campaign/BIOPLANT_NATIVE.md)
- [Post-Rika crossing and current campaign save](field/TRAVEL.md#post-rika-northern-crossing-2026-09-23)
- [Party ORDER](camp/PARTY_ORDER.md)
- [Full native playability ledger](campaign/NATIVE_PLAYABILITY.md)
- [Cartridge oracle setup](../oracle/README.md)

Receipts under `build/` contain local logs, captures and saves; they are not
repository downloads. Keep source saves intact when testing a continuation.
Use the default desktop viewport for visual checks: the small recovery-run
captures clipped camp windows and establish state/persistence, not menu parity.
