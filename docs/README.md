# Documentation

These documents describe how PSIV works now and what the cartridge does. They
are grouped by what a reader came to do. Status and active work are GitHub
issues, not documents.

## Play

- [Project overview and quick start](../README.md): what PSIV is, how far it
  plays, how to build and start it from source, and the known differences.
- [Player's guide](PLAYING.md): the ROM, the downloads and their first run,
  where the pack and saves live, and the controls.
- [Roadmap](ROADMAP.md): the 1.0 bar, the platforms, and what comes after.
- [Save format](camp/SAVE_FORMAT.md): where saves go and what they hold.

## Mod and contribute

- [Modding guide](MODDING.md): which pack file holds what, and two worked
  edits.
- [Contributing](../CONTRIBUTING.md): issues, labels, pull requests and
  where questions go.
- [Build, run and verify](DEVELOPMENT.md): developer setup, the checks and
  the gate.
- [Feature map](FEATURE_MAP.md): a feature's owning paths and focused test.
- [Runtime design](RUNTIME_DESIGN.md): the layers and the public runtime API.
- [ROM extraction](EXTRACTION.md): what the extractor reads and writes.
- [MIT License](../LICENSE).

## Retail reference

What the cartridge does, read from the disassembly and measured on the
hardware's behavior through the oracle.

**Battle**

- [Battle rules](battle/BATTLE_RULES.md), [a worked basic attack](battle/BATTLE_ATTACK_EXAMPLE.md)
  and [the rules measured on the cartridge](battle/BATTLE_MEASUREMENTS.md).
- [Player abilities](battle/PLAYER_ABILITIES.md), [RIMIT](battle/RIMIT.md),
  [instant death](battle/INSTANT_DEATH.md), [battle recovery](battle/BATTLE_RECOVERY.md).
- [Enemy ability inventory](battle/ENEMY_ABILITIES.md) and its stretches:
  [Zelan to Kuran](battle/ENEMY_ABILITIES_ZELAN_TO_KURAN.md),
  [the Air Castle](battle/ENEMY_ABILITIES_AIR_CASTLE.md),
  [the Air Castle to the Ending](battle/ENEMY_ABILITIES_ENDGAME.md).
- [Enemy damage routes](battle/ENEMY_DAMAGE_ROUTES.md),
  [enemy status and stat effects](battle/ENEMY_EFFECT_ABILITIES.md),
  [Fusion, COMBINE and reloads](battle/ENEMY_FUSION.md).
- [Battle command windows and status panes](battle/BATTLE_COMMAND_UI.md),
  [battle geometry](battle/BATTLE_GEOMETRY.md), [battle animations](battle/BATTLE_ANIMATIONS.md).

**Field**

- [Walking and collision](field/MOVEMENT.md), [the camera](field/CAMERA.md),
  [NPC wander](field/NPC_WANDER.md), [transitions](field/TRANSITIONS_DECODED.md).
- [Field state and flags](field/FIELD_STATE.md), [chests](field/CHESTS.md),
  [map effects](field/MAP_EFFECTS.md), [per-frame map updates](field/MAP_UPDATES.md).
- [Travel and pipes](field/TRAVEL.md), [vehicles](field/VEHICLES.md),
  [platforms and belts](field/PLATFORMS_AND_BELTS.md), [the color pipeline](field/COLOR_PIPELINE.md).

**Camp, shops and saves**

- [Camp menu layout](camp/CAMP_MENU_LAYOUT.md), [party ORDER](camp/PARTY_ORDER.md),
  [party status](camp/PARTY_STATUS.md), [equipment](camp/EQUIPMENT.md),
  [progression](camp/PROGRESSION.md).
- [Shops and inns](camp/SHOPS.md), [shop layout](camp/SHOP_LAYOUT_DECODED.md);
  the save format is under [Play](#play).

**Scenes and dialogue**

- [The scene registry](scenes/README.md) and the numbered scene transcriptions
  beside it; [the event engine](scenes/EVENT_ENGINE.md),
  [the trigger census](scenes/TRIGGER_CENSUS.md),
  [event coverage](scenes/EVENT_COVERAGE.md).
- [Scene dialogue](scenes/SCENE_DIALOGUE.md), [dialogue tree selection](scenes/DIALOGUE_TREE_SELECTION.md),
  [dialogue actions](scenes/DIALOGUE_ACTIONS.md), [the second dialogue routine's callers](scenes/DIALOGUE2_CALLERS.md).
- [Scene presentation](scenes/SCENE_PRESENTATION.md), [the field reload](scenes/FIELD_RELOAD.md),
  [live layout writes](scenes/LIVE_LAYOUT_WRITES.md), [revision-conditional ops](scenes/REVISION_AUDIT.md),
  [the title and boot](scenes/TITLE_BOOT.md).

**Sound**

- [The sound driver](sound/SOUND_DRIVER.md), [sound extraction](sound/SOUND_EXTRACTION.md),
  [sound integration](sound/SOUND_INTEGRATION.md).

**Provenance and deliberate deviations**

- [Source notes](source-notes/README.md), with the deviation record; its
  topic files: [cartridge formats](source-notes/formats.md),
  [disassembly discrepancies](source-notes/disassembly-discrepancies.md),
  [enemy abilities and damage routes](source-notes/battle-enemy-abilities.md),
  [party actions, items and battle flow](source-notes/battle-party.md).

## Tooling

- [The oracle](../oracle/README.md): the emulator harness, tapes, the RAM map
  and where each measurement lives; [oracle methodology](source-notes/oracle-methodology.md).
- [Replaying captured battles](oracle/BATTLE_ORACLE_REPLAY.md),
  [forcing a formation](oracle/BATTLE_ORACLE_FORCED.md),
  [sweeping a region](oracle/BATTLE_ORACLE_SWEEP.md).
- [The campaign runner](campaign/CAMPAIGN_RUNNER.md): route files, the
  headless runner and native tape replay; [native drivers](campaign/NATIVE_DRIVERS.md).
- [The sound core](../rust/psiv-sound/README.md), [oracle core patches](../oracle/patches/README.md).

## Working with agents

- [Repository instructions for agents](../AGENTS.md) and the
  [agent workflow](AGENT_WORKFLOW.md): roles, authority, verification,
  receipts and the correction ladder.

## Rule owners

Each standing rule lives in exactly one document; everything else links to it.
Add a row here when a new rule gets an owner.

| Rule | Owner |
| --- | --- |
| Release bar and platforms | [Roadmap](ROADMAP.md#decisions-owner-2026-10-14) |
| Retail fidelity and integer semantics | [AGENTS.md](../AGENTS.md#retail-and-layer-boundaries) |
| Layer boundaries; Godot presents and sends input | [Runtime design](RUNTIME_DESIGN.md#data-and-runtime-layers) |
| Retail deviations and provenance | [Source notes](source-notes/README.md) |
| Protected inputs, local assets out of Git, saves and evidence safety | [AGENTS.md](../AGENTS.md#protect-local-inputs-and-evidence) |
| Evidence types and what a claim needs | [AGENTS.md](../AGENTS.md#verify-the-claim-you-intend-to-make) |
| Record policy: where work and its record live (issues, PRs, commits; no ledgers, logs or graphs in docs) | [Agent workflow](AGENT_WORKFLOW.md#where-work-and-its-record-live) |
| Roles and delegation | [Agent workflow](AGENT_WORKFLOW.md#roles-and-delegation) |
| Authority, permissions and the effort policy | [Agent workflow](AGENT_WORKFLOW.md#authority-effort-and-continuation) |
| Correction ladder | [Agent workflow](AGENT_WORKFLOW.md#correction-ladder) |
| One paved path; gaps as issues; guard before cleanup | [Agent workflow](AGENT_WORKFLOW.md#the-codebase-is-memory) |
| Receipts and handoff | [Agent workflow](AGENT_WORKFLOW.md#receipt-and-handoff) |
| Toolchain and Godot version; build, pack and launch commands | [Development guide](DEVELOPMENT.md#current-environment) |
| Gate commands, the heavy-run lock, coverage and repository-owned checks | [Development guide](DEVELOPMENT.md#gate-and-coverage) |
| File size | [Development guide](DEVELOPMENT.md#file-size) |
| Cartridge comparison method and lanes | [Oracle guide](../oracle/README.md) |
