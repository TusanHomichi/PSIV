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
| [psiv-godot](../rust/psiv-godot/) and [godot](../godot/) | Desktop bridge: rendering, pad input and audio output. The runtime `Session` owns every game mode — field, scenes, dialogue, shops, camp, battle, title and game over ([campaign runner](campaign/CAMPAIGN_RUNNER.md)) |

Game rules belong in the core; the runtime coordinates them, and Godot presents
results and sends input. Keep ROM decoding in Python. Pack schema changes need
matching extraction and consumer validation. ROM-derived packs remain local
and excluded from Git; see [extraction](EXTRACTION.md) and [setup](DEVELOPMENT.md).

## The public runtime API

A game is a `Session` (`rust/psiv-runtime/src/session/mod.rs`). After the S6
node that is the whole of the runtime's public surface: `Session::frame(pad)` is
a frame of game, `Session::runtime()` hands out the read-only runtime for
presentation, the frame carries the views a shell draws (`Frame::battle`,
`Frame::title`, `Frame::game_over`, `Frame::notice` and the open menus'
`ShopView`/`CampView`), and `Session::start(data)` is the only way to build one
(`rust/psiv-runtime/src/session/start.rs`):

| Start | What it is |
| --- | --- |
| `with_battles(files)` | the run's battle pack: without it no encounter rolls |
| `with_saves(store)` | the run's save directory, as a `SaveStore` |
| `with_step_frames(frames)` | a harness that walks at a different pace |
| `power_on()` | retail's boot: the pack's own first control, at the title |
| `field()` | the same boot with no front door (the debug selectors) |
| `new_game()` | the title's START: the initializer, battles armed, the opening fired |
| `continue_slot(n)` | the title's CONTINUE: slot `n` through the store |
| `from_slot_bytes(bytes, slot)` | a slot file's bytes, without a directory |
| `from_save(save)` | one decoded save: the fixtures' and tests' hand-built state |

The certification fixtures are the other documented way in
(`rust/psiv-runtime/src/session/debug.rs` plus the `Session::debug_*`
selectors), and they hand back sessions too.

`Runtime` — the engine's state — is crate-private in both directions: no
constructor and no method that takes `&mut self` is `pub`. A shell that tries to
build one, or to call a mutator through `Session::runtime()`, fails to compile
for the right reason, and the crate carries the proof: a `compile_fail` doctest
on `Runtime` (`rust/psiv-runtime/src/lib.rs`), the two on `Session`'s module,
and a source check (`rust/psiv-runtime/src/suites/visibility.rs`,
`no_runtime_mutator_is_public`) that fails when a new `pub fn (&mut self)` lands
in an `impl Runtime` block. The runtime's own rule tests moved inside the crate
with the mutators (`rust/psiv-runtime/src/suites/`, formerly
`rust/psiv-runtime/tests/`), because a gameplay rule that ticks the field with
an `Input` is not a pad-driven session; the pad-driven ones stayed integration
tests.

Two entry points in the crate are deliberately not games
(`rust/psiv-runtime/src/tools/`): the oracle-tape replay driver
(`psiv-runtime/src/bin/psiv-replay.rs`) and the legacy-save repair
(`psiv-runtime/examples/repair_progression.rs`). Both need the seams above to do
one whole documented job — replay a recorded tape and hand back a row per frame,
repair a copied slot in and a repaired slot out — and neither hands out a
`&mut Runtime`.

What remains of the shell is pictures, input mapping and the directory policy:
`rust/psiv-godot/src/save_dir.rs` resolves `PSIV_SAVE_DIR` (refusing a scripted
run that names none) and hands `psiv_runtime::SaveStore` to the constructor,
which owns every slot read, write and erase from then on.

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
input it resolved and, for a menu frame, the camp's events, sound and the
failure of a SAVE the session itself wrote (`Frame::camp_save_error`). The menus' windows are views the shell draws (`ShopView`, `CampView`):
pages, cursors, the roster snapshot and the line a command answered with. The
shop catalog (counters, stock, inn rates) is `shops.json` read through
`psiv-data`; equip against unequip is decided from the equipment bytes, and a
sale pays half the item record's price word (`ps4.asm:135570`) for any item.
Some menu commands do not answer inside their own window: they destroy it and
hand the field to an event, exactly as the cartridge's `Field_MenuExit` does
(`ps4.asm:117150-117169`). `session/menu_scene.rs` owns that hand-off for both —
the ITEM menu's vehicle actions, which leave the menu closed, and Aiedo's inn,
which rebuilds its window and charges the bill when the scene returns
(`docs/camp/SHOPS.md`, "Finding 3").
START and CONTINUE replace the runtime and clear the per-frame menu-scene
handoff with the other menu/window latches, so an old inn bill cannot consume
an unrelated later `SceneEnded`. Vehicle ITEM scenes snap the live party via
one core boarding rule and wait for the existing runtime camera glide only
when the retail original-coordinate bit test requires it; the general scene
`MoveCamera` timing and #59 camera-gate refresh remain separate work.
The persistent `Saved_Sound_Index` byte lives in runtime: START clears it,
CONTINUE/field entry seed it from the loaded map, ordinary field loads update
it, and scene writes update or compare it before Godot receives a chosen
sound op. Real-session Godot restores read that runtime word without consuming
it, so repeated battle returns retain the selected track. Boarding also tells
Godot to retain that sound through scene end without a synthetic restart. A
one-shot cue survives only for runtime-less debug fixtures. Exact map-load
replay timing still needs the runtime's `$ECED` music-change edge; the current
shell can issue a redundant restore when a map transition keeps the track.
For a still-zero word at a generic restore edge, the shell's existing map
fallback remains a playback policy, not a claimed cartridge zero/stop rule.
The directions and the talk button resolve in the cartridge's order
(`Pad::field_input`, `rust/psiv-runtime/src/pad.rs`: `FieldObj_MovementsTbl`'s
sixteen d-pad masks — an opposing pair cancels, a horizontal beats a vertical —
and `FieldControls_GetInput`'s talk press taking the frame); the menus read
presses as edges against the previous frame's pad. Godot sends the pad and
presents the frame; there is no mode left in front of the call.

## The front door and the end of play

The title is a session mode like any other
(`rust/psiv-runtime/src/session/title.rs`): the phases and their frame counts
(the Sega hold, the reveal transfer, the `#$233` Press Start hold,
`ps4.asm:86926`), the option window CONTINUE/START/ERASE DATA, the three-row
slot lists and the ARE YOU SURE? confirmation, all driven by the pad and
reported as a `TitleView` the shell draws. START builds the retail initializer
inside the session, CONTINUE loads a slot through the session's `SaveStore`, and
ERASE DATA zeroes the picked slot's payload — no shell answers a request.
A defeat is the other boundary (`session/game_over.rs`): the field-status
windows (`session/notices.rs`) are acknowledged by the pad, and the fourteen
frames that follow gate the title's return, exactly as the shell's own fade
counted them. `Session::start_title()` puts a session at the front door; the
shell calls it on a power-on boot and the fade calls it on a defeat.

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

Save serialization uses the retail SRAM layout with three local slot files, and
every slot operation (the title's slot list, CONTINUE, ERASE DATA and the camp's
STATE > SAVE) runs inside the session's `SaveStore`. See [save format](camp/SAVE_SCOUT.md),
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

## Map-music transition boundary

The remaining music transition and zero saved-sound restore policy are tracked
in [#73](https://github.com/TusanHomichi/PSIV/issues/73). In accepted F2 candidate
`57f15904084e97317fa29c39e6d58ae9ff2d8c92`, runtime owns the persistent `$ECEC`
saved byte; the retail `$ECED` change edge is still absent, and a zero saved byte
at a generic shell restore can fall back to raw map music. The US image sets
the edge at `$051812`, consumes it at `$051958`, and restores the saved byte at
`$05197A`; the separate battle-return copy is at `$033E0C`. The issue owns the
source references, paths and command/timing acceptance, including verification
of the driver's zero-byte behavior. This records an unfixed source boundary;
no native audio parity or connected audio route is established here.
