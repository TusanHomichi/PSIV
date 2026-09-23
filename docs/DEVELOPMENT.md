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
python3 -m psiv_tools inspect "/path/to/Phantasy Star IV (USA).md"
python3 -m psiv_tools pack "/path/to/Phantasy Star IV (USA).md" runtime-pack
```

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
evidence before regeneration. See [map effects](MAP_EFFECTS.md#12-native-overworld-page-hook-consumption-2026-09-23).

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
From the root:

```bash
PYTHONPATH=. python3 -m unittest discover -s tests
cargo fmt --manifest-path rust/Cargo.toml --all --check
CARGO_BUILD_JOBS=1 cargo test --manifest-path rust/Cargo.toml --workspace -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
```

Run these sequentially. Full map/pack tests can take several minutes. Serial
Rust tests avoid the memory pressure seen when many pack-loading cases run
together. Tests requiring local assets may fail or explicitly skip when their
fixtures are absent; report that distinction with the result.

### Optional Redshirt adapter

The [battle-decision experiment](REDSHIRT_BATTLE.md#reproduce-the-optional-adapter-checks)
documents the pinned shared checkout, example build and focused Python checks.
It needs Python 3.11+ and a separately built `redshirt_battle` example. Ordinary
test discovery explicitly skips the adapter tests when these optional
prerequisites are absent; the enabled acceptance lane must run every test.
Neither tests nor replay make live model calls. The playable game has no
Redshirt or model-service dependency.

The [survival-pressure fixtures and portable manifest](../tests/fixtures/redshirt_pressure/manifest.json)
are authored synthetic inputs, not extracted assets. Their
[reproduction commands](REDSHIRT_BATTLE.md#reproduce-the-survival-pressure-trial)
use an isolated Rust target so the first experiment's binary stays intact.
The adapter defaults to `--briefing minimal`; `--briefing mechanics-v1` adds
source-verified synthetic battle rules without changing the engine or menu.
The [information-ablation receipt](REDSHIRT_BATTLE.md#mechanics-briefing-results)
keeps both modes, exact source identity and live usage separate.
`--baseline threat` enables the optional visible-state threat/skill heuristic;
the default remains `projected`. The [fresh-case index](../tests/fixtures/redshirt_holdout/index.json)
and four adjacent portable manifests preserve the 24-case follow-up recipe.
Each manifest contains six fresh cases and two explicitly disclosed old
calibration cases. Use `mechanics-v1` for this comparison, preserve the declared
case set, and keep live requests separate from model-free tests and replay.

## Native and original-game comparisons

`tools/native_*.gd` drives ordinary Godot input and reads runtime observations.
Some drivers use isolated fixtures; others continue a saved campaign. Their
ledgers identify which kind of evidence each run supplies.

- [BioPlant and recovery](BIOPLANT_NATIVE.md)
- [Post-Rika crossing and current campaign save](TRAVEL.md#post-rika-northern-crossing-2026-09-23)
- [Party ORDER](PARTY_ORDER.md)
- [Full native playability ledger](NATIVE_PLAYABILITY.md)
- [Cartridge oracle setup](../oracle/README.md)

Receipts under `build/` contain local logs, captures and saves; they are not
repository downloads. Keep source saves intact when testing a continuation.
Use the default desktop viewport for visual checks: the small recovery-run
captures clipped camp windows and establish state/persistence, not menu parity.
