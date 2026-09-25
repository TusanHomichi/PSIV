_Foundations and audit — part of the [native playability ledger](NATIVE_PLAYABILITY.md); see the [documentation index](../README.md)._

## Audit context

The earlier audit established that selected matching reference screens did
not prove complete gameplay. Its obsolete coverage table and task ordering
have been removed; use [the roadmap](../ROADMAP.md) for current gaps. The dated
implementation and failure records below retain their original evidence.

The audit's extracted enemy data contains 100 of 153 enemies with nonzero
regular abilities and 46 with nonzero condition IDs. Complete animation
coverage does not implement those gameplay effects.

Code anchors:

- `rust/psiv-core/src/battle/engine.rs`: `Command`, `roll_enemy_ability`.
- `rust/psiv-godot/src/battle/ui_input.rs`: `take_command`;
  `commands.rs`: pure menu state and target selection.
- `rust/psiv-godot/src/camp/mod.rs`: `confirm_root`, STATE/ORDER.
- `rust/psiv-runtime/src/scene_runtime.rs`: `DialogueResume`, `ChoiceRequested`.
- `rust/psiv-runtime/src/camp.rs` and `camp/abilities.rs`: field recovery and confirmation costs.
- `rust/psiv-runtime/tests/next_arc.rs`: the campaign test explicitly
  relocates between maps and directly starts scenes. It is useful script
  coverage, but does not prove a player can traverse the whole campaign.

## Repairs in this slice

START previously reused `debug_scene_runtime`: an empty `GameState` with
two characters and a location. That discarded the retail initial 500
meseta, eleven preloaded extended flags, and three town flags. The
first-control constructor also omitted the starting money.

`psiv-data` now reads the existing `game_start.json` initializer separately
from the manifest's post-opening summary. `Runtime::new_game` supplies the
pre-opening party, money, settings and flags; START uses that constructor.
The first-control constructor shares the initializer and then applies the
post-opening party/flags. Loaded saves remain a separate path.

The uninterrupted opening test exposed a second bug: triggers were only
checked after a player step. Returning from the opening therefore left
Chaz's first line waiting for movement. The runtime now checks map events
when control returns from a scene, before accepting movement. Retail's
`FieldRoutine_Controls` runs `RunEvents` before `FieldControls_GetInput`;
see `reference/ps4disasm/ps4.asm:116755`.

New regression coverage:

- The exact constructor called by Godot START retains the initial state,
  with the opening's two progression flags still clear.
- A single runtime plays the opening with real map loads, receives Chaz's
  first line without movement, and reaches the first-control state. Dialogue
  is acknowledged automatically in this test; text pacing is a separate
  presentation check.
- The resulting flag banks, party, money, roster, settings and inventory
  match the retail starting state; directional input then moves Chaz.
- Saving and continuing that state preserves the snapshot and position,
  including when battle data is enabled again.

`PSIV_DEBUG_STATE=<path.json>` records the Godot-owned runtime at
`PSIV_DEBUG_SHOT_FRAME` (default 180). It includes map, cell, scene-active
state, party, money and flags. This makes a native launch inspectable
without inferring state from a screenshot or a separate fixture.

## Evidence

Local receipts for this audit are under `/tmp/psiv-audit-20260912/`; this
directory is temporary and contains derived game data, so it is not tracked.

- `boot-before.txt`: regression fails with 0 meseta, expected 500.
- `boot-after.txt`: the same constructor regression passes after repair.
- `opening-before.txt`: missing Chaz dialogue before movement.
- `opening-after.txt`: uninterrupted opening regression passes.
- `retail-newgame.csv`: fresh power-on oracle tape 01; frame 6456 has
  map `$13`, character pixels `(768,288)`, Chaz alone, 500 meseta,
  and town flags `80008040`.
- `opening.png`: inspected pre-repair native field after the opening.
- `rust-after.txt`: 743 Rust tests passed; `python-tests.txt`: 918 Python
  tests passed. `clippy.txt`: all workspace targets pass with warnings denied.
  `runtime-final.txt` repeats the affected runtime tests after final ordering
  cleanup.
- `native-after.log`, `native-state.json`, `native-after.png`: rendered
  Godot execution at tick 3360. START played the opening, automatically
  dispatched trigger 124 / Chaz's `$6D` line, and accepted an eight-frame Up
  input. State is map `$13`, cell `(48,18)`, Chaz alone, no active scene,
  500 meseta, and the exact expected event/town banks. The screenshot was
  inspected. This uses auto-acknowledged dialogue and the dummy audio driver;
  it does not certify dialogue pacing, sound or hardware controller input.

The saved/native control tests establish this opening slice, not completion
of the game. Missing battle mechanics remain the largest known obstacle.

Reproduce the rendered opening smoke test from the repository root (requires
the local pack, Godot, and Xvfb):

```sh
cargo build -p psiv-godot --manifest-path rust/Cargo.toml
mkdir -p build/native-opening
xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 \
  PSIV_SAVE_DIR="$PWD/build/native-opening/saves" \
  PSIV_DEBUG_TITLE_SHOT=1 PSIV_DEBUG_TITLE_AUTOSTART=1 \
  PSIV_DEBUG_AUTOCLOSE_SCENE=1 PSIV_DEBUG_SCENE_TICKS=1 PSIV_DEBUG_INPUT=1 \
  PSIV_DEBUG_STATE="$PWD/build/native-opening/state.json" \
  PSIV_DEBUG_SHOT="$PWD/build/native-opening/field.png" \
  PSIV_DEBUG_SHOT_FRAME=3360 \
  "$HOME/.local/bin/psiv-godot-4.7.1" --path godot \
  --script "$PWD/tools/native/native_opening.gd" --display-driver x11 \
  --rendering-method gl_compatibility --audio-driver Dummy \
  --fixed-fps 60 --quit-after 3370 \
  --log-file "$PWD/build/native-opening/godot.log"
```

Run with other `PSIV_DEBUG_*` selectors and `PSIV_LOAD_SLOT` unset. The
expected state is specified above; a process exiting successfully alone is
not a passing receipt. Headless opening/save coverage is
`cargo test -p psiv-runtime --test new_game` from `rust/`.

## Current implementation priorities

See [ROADMAP.md](../ROADMAP.md). The earlier Tonoe/ORDER work list has been
superseded by the verified BioPlant checkpoint and later entries below.
