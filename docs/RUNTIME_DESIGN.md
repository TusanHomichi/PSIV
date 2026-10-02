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
| [psiv-godot](../rust/psiv-godot/) and [godot](../godot/) | Desktop bridge: rendering, pad input and audio output. Title, game over and battle are moving into the runtime `Session`, which already runs the field, scenes, dialogue, shops and camp ([campaign runner](campaign/CAMPAIGN_RUNNER.md)) |

Game rules belong in the core; the runtime coordinates them, and Godot presents
results and sends input. Keep ROM decoding in Python. Pack schema changes need
matching extraction and consumer validation. ROM-derived packs remain local
and excluded from Git; see [extraction](EXTRACTION.md) and [setup](DEVELOPMENT.md).

## The session frame

One `Session` (`rust/psiv-runtime/src/session/mod.rs`) owns the runtime and
turns a joypad byte into a frame of game: `Session::frame(pad)` is the one call
per 60 Hz frame. A window that is up owns the frame; otherwise an open shop or
inn window (`session/shop.rs`) or camp menu and chest window (`session/camp/`)
does, and the Camp button or a chest the field opened starts them. Everything
else is the field's frame: the dialogue window's input half, a pending `$F6`,
then the field or the scene, then the events that open windows (a talk, a
counter, a scene's own line, a choice, "nothing here", applied in
`session/route.rs`), then the window's own half — the cartridge's node order, so
the box takes its first open-animation step on the frame it opens. The `Frame`
carries the runtime events left for the shell, what the session opened
(`Frame::routed`), the dialogue signals, the scene a `$F6` started, the field
input it resolved and, for a menu frame, the camp's events, sound and save
request. The menus' windows are views the shell draws (`ShopView`, `CampView`):
pages, cursors, the roster snapshot and the line a command answered with. The
shop catalog (counters, stock, inn rates) is `shops.json` read through
`psiv-data`; equip against unequip is decided from the equipment bytes, and a
sale pays half the item record's price word (`ps4.asm:135570`) for any item.
The directions and the talk button resolve in the cartridge's order
(`Pad::field_input`, `rust/psiv-runtime/src/pad.rs`: `FieldObj_MovementsTbl`'s
sixteen d-pad masks — an opposing pair cancels, a horizontal beats a vertical —
and `FieldControls_GetInput`'s talk press taking the frame); the menus read
presses as edges against the previous frame's pad. Godot sends the pad and
presents the frame; the modes it still owns (title and game over) are checked
in front of the call, and the [campaign runner](campaign/CAMPAIGN_RUNNER.md)
node S5 moves them in.

## The battle frame

A battle is a mode of the same session, not a script the shell plays
(`rust/psiv-runtime/src/session/battle/`). `EncounterRolled` or a scene's
`SceneBattleStarted` starts the runtime's own battle inside the field frame, and
from the next frame `Session::frame(pad)` runs the battle loop instead of the
field: the shared vblank/RNG tick, the command menu (the main options, the
per-character window, the target lists, the mounted skill window), beat playback
with the retail dwell of `12 * (Battle_Speed + 1)` frames and the post-battle
pages' confirm waits, the epilogue that pays the pools and levels the party, and
the frame the map refresh returns on. `Frame::battle` carries the loop's
read-only `BattleView` — the menu and its cursor, the narration window, the
damage block, the party strip, the enemies with their visibility, the beat and
its progress, plus the retail sound cues the frame raised — and Godot draws it,
deciding nothing. The buttons are the cartridge's own where its routines name
them (`Battle_MainOptions`, `Battle_CharCommand`, the tech/skill/item windows,
`Battle_VehSkills`; `ps4.asm:1123` onward), and the per-character window's
one-list layout keeps the shell's four-direction mapping with that difference
recorded in `session/battle/menu/`.

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
[docs/source-notes/README.md](source-notes/README.md).

Triggers select retail scene transcriptions. The core's `SceneOp` interpreter
updates state and emits effects; the runtime owns the dialogue continuation —
the byte walk, the yes/no answers, the `$FA`/`$F2`/`$F6`/`$F7` codes and every
frame count that gates them — while Godot handles actor motion and pixels.
The runtime's `DialogueRunner` (`rust/psiv-runtime/src/dialogue/`) owns the
message box: the open animation (nine frames, derived from the pack's window
geometry), the typewriter (one glyph per three frames, one per frame while
Speak is held), the choices, and the branches, which read the live `GameState`
at the moment they are evaluated. It runs in two halves per frame — the pad's
presses before the field tick, the `$F2` actions and the typewriter after it —
because that is the cartridge's node order, and the split is what keeps a flag
write on the correct side of a field tick. Its input is one `Pad` (the joypad
byte of `ps4.constants.asm:1877-1884`); its outputs are presentation signals and
a `DialogueView` snapshot the window node draws. The pack it walks is part of the
loaded data (`GameData::dialogue`, read by `GameData::load` with the maps and the
sound) and every runtime constructor installs it, so a runtime whose message box
cannot resolve an entry is not a state a caller can build. Fork-rewritten scene bodies
require validation against the retail bytes. See [scene research](scenes/README.md),
[dialogue](scenes/SCENE_DIALOGUE.md) and [presentation](scenes/SCENE_PRESENTATION.md).

Save serialization uses the retail SRAM layout with three local slot files.
Normal title CONTINUE and camp SAVE are implemented. See [save format](camp/SAVE_SCOUT.md),
[field state](field/FIELD_STATE.md), [party ORDER](camp/PARTY_ORDER.md) and the
[BioPlant ledger](campaign/BIOPLANT_NATIVE.md) for state and restart evidence.

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
and a discrepancy record in [docs/source-notes/README.md](source-notes/README.md). Existing
fixes do not authorize arbitrary balance changes or guessed ability effects.

Presentation uses extracted art and decoded layouts. The current desktop
viewport is 1280×800; selected comparisons use the original 320×224 surface.
Viewport, camera, timing and matched game state are part of a visual result.
See [camera](field/CAMERA.md), [color pipeline](field/COLOR_PIPELINE.md),
[battle UI](oracle/BATTLE_ORACLE_UI.md) and [sound integration](sound/SOUND_INTEGRATION.md).

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
