# PSIV runtime architecture

Updated September 15, 2026. This describes the current implementation structure
and fidelity decisions. [README](../README.md) records playable scope;
[ROADMAP.md](ROADMAP.md) is the current work list. Detailed measurements belong
in the subsystem ledgers linked below.

## Data and runtime layers

Python extracts the verified US ROM into a local pack. The native game loads
that pack; it does not run the emulator. The Cargo workspace has five crates:

| Component | Role |
| --- | --- |
| [psiv_tools](../psiv_tools/) | ROM decoding, extraction and pack generation in Python |
| [psiv-data](../rust/psiv-data/) | Serde types, pack loading and validation |
| [psiv-core](../rust/psiv-core/) | Deterministic field, battle, event, party and save rules; no dependencies or engine types |
| [psiv-runtime](../rust/psiv-runtime/) | Pack-to-core conversion, game orchestration, persistence integration and presentation snapshots |
| [psiv-sound](../rust/psiv-sound/) | Live PSIV driver, FM/PSG/DAC playback and register traces |
| [psiv-godot](../rust/psiv-godot/) and [godot](../godot/) | Desktop bridge, rendering, menus, input and audio output |

Game rules belong in the core; the runtime coordinates them, and Godot presents
results and sends input. Keep ROM decoding in Python. Pack schema changes need
matching extraction and consumer validation. ROM-derived packs remain local
and excluded from Git; see [extraction](EXTRACTION.md) and [setup](DEVELOPMENT.md).

## State, events and persistence

The field uses 16-pixel collision cells with integer step progress. Map data,
transition predicates and object occupancy determine movement; the runtime
applies story-dependent map changes. The persistent roster, inventory and flags
survive battles and map changes.

The final flag model is four banks, with chest flags sharing extended events:

| Bank | Retail RAM | Meaning |
| --- | --- | --- |
| Event | `$F100` | Event IDs `$00–$FF` |
| Extended event / chest | `$F120` | Event IDs `$100–$1FF`; chest `n` is event `0x100+n` |
| Temporary event | `$F140` | Explicitly managed temporary flags |
| Town | `$F160` | Town flags |

There is no separate `$F156` flag bank. Earlier disassembly-label readings were
wrong; the correction and measured evidence remain in
[SOURCE_NOTES.md](../SOURCE_NOTES.md).

Triggers select retail scene transcriptions. The core's `SceneOp` interpreter
updates state and emits effects; runtime/Godot handle dialogue continuation,
actor motion and presentation. Fork-rewritten scene bodies require validation
against the retail bytes. See [scene research](scenes/README.md),
[dialogue](SCENE_DIALOGUE.md) and [presentation](SCENE_PRESENTATION.md).

Save serialization uses the retail SRAM layout with three local slot files.
Normal title CONTINUE and camp SAVE are implemented. See [save format](SAVE_SCOUT.md),
[field state](FIELD_STATE.md), [party ORDER](PARTY_ORDER.md) and the
[BioPlant ledger](BIOPLANT_NATIVE.md) for state and restart evidence.

## RNG design

The field LCG and battle mixer use one shared 32-bit seed. Preserve the original
integer operations, draw ordering and frame scheduling. The battle mixer
replaces the hardware VDP H/V counter read with a deterministic surrogate;
see [the implementation](../rust/psiv-core/src/battle/rng.rs).

This preserves the transcribed battle formulas while changing the hardware
random stream. Battle comparison must account for that difference instead of
claiming identical per-frame battle rolls. Field replay and battle formula
checks have distinct oracle fixtures.

## Fidelity and original bugs

The retail cartridge is the behavioral reference. Preserve intentional quirks;
deliberate fixes to demonstrable original bugs need explicit source evidence
and a discrepancy record in [SOURCE_NOTES.md](../SOURCE_NOTES.md). Existing
fixes do not authorize arbitrary balance changes or guessed ability effects.

Presentation uses extracted art and decoded layouts. The current desktop
viewport is 1280×800; selected comparisons use the original 320×224 surface.
Viewport, camera, timing and matched game state are part of a visual result.
See [camera](CAMERA.md), [color pipeline](COLOR_PIPELINE.md),
[battle UI](BATTLE_ORACLE_UI.md) and [sound integration](SOUND_INTEGRATION.md).

## Verification

The original-game oracle is a headless Genesis Plus GX libretro host in
[oracle/](../oracle/README.md). It is implemented and used for development;
it is not a runtime dependency. Native drivers in `tools/` provide separate
input and SAVE/CONTINUE checks.

Use [DEVELOPMENT.md](DEVELOPMENT.md) for current commands and
[AGENT_WORKFLOW.md](AGENT_WORKFLOW.md) for evidence and handoffs. A scene
transcription, a passing state test, a connected route and a matching screenshot
each establish different facts. Follow the roadmap for campaign progress
and remaining work.
