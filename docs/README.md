# Documentation

These documents describe how PSIV and its tools work now, and what the
cartridge does. Status and active work live elsewhere, under the
[record policy](AGENT_WORKFLOW.md#where-work-and-its-record-live).

## Start here

- [Project overview](../README.md) and [contributing](../CONTRIBUTING.md)
- [Roadmap: the 1.0 bar and what comes after](ROADMAP.md)
- [Build, run and verify](DEVELOPMENT.md)
- [Feature map: a feature's owning paths and focused test](FEATURE_MAP.md)
- [Runtime design: layers and the public runtime API](RUNTIME_DESIGN.md)
- [ROM extraction reference](EXTRACTION.md)
- [Repository instructions for agents](../AGENTS.md) and the [agent workflow](AGENT_WORKFLOW.md)
- [MIT License](../LICENSE)

## battle — battle rules and retail research

- [Battle research](battle/BATTLE_SCOUT.md) and its [worked continuation](battle/BATTLE_SCOUT_CONTINUATION.md).
- [Player abilities](battle/PLAYER_ABILITIES.md).
- [Enemy ability inventory](battle/ENEMY_ABILITIES.md) and its route sets:
  [Zelan to Kuran](battle/ENEMY_ABILITIES_ROUTE.md),
  [the Air Castle stretch](battle/ENEMY_ABILITIES_AIR_CASTLE.md),
  [the Air Castle to the Ending](battle/ENEMY_ABILITIES_ENDGAME.md).
- [Enemy damage routes](battle/ENEMY_DAMAGE_ROUTES.md), [enemy POISON](battle/ENEMY_POISON.md), [enemy status and stat effects](battle/ENEMY_EFFECT_ABILITIES.md), [enemy Fusion](battle/ENEMY_FUSION.md).
- [Battle command windows, status panes and the MetaSlug](battle/BATTLE_COMMAND_UI.md), [battle animations](battle/BATTLE_ANIMATIONS.md), [battle geometry](battle/BATTLE_GEOMETRY.md), [battle recovery](battle/BATTLE_RECOVERY.md), [instant death](battle/INSTANT_DEATH.md).
- [THREAD](battle/THREAD.md), [RIMIT](battle/RIMIT.md).

## oracle — comparing against the cartridge

- Method, tapes, harness and captures: the [oracle guide](../oracle/README.md).
- [Replaying captured battles](oracle/BATTLE_ORACLE_REPLAY.md), [forcing a formation](oracle/BATTLE_ORACLE_FORCED.md), [the divergence sweep](oracle/BATTLE_ORACLE_SWEEP.md), [battle UI](oracle/BATTLE_ORACLE_UI.md).
- Capture sets: [Motavia-arc status abilities](oracle/BATTLE_ORACLE_ARC.md), [Zelan-to-Kuran abilities](oracle/BATTLE_ORACLE_ZELAN.md), [scripted party commands, BARRIER and Psycho Wand](oracle/BATTLE_ORACLE_X86.md).
- [Field measurements](oracle/RESULTS.md) and [their continuation](oracle/RESULTS_CONTINUED.md).

## field — field, map and travel behavior

- [Field state](field/FIELD_STATE.md), [map effects](field/MAP_EFFECTS.md), [per-frame map updates](field/MAP_UPDATES.md), [chests](field/CHESTS.md).
- [Travel and pipes](field/TRAVEL.md), [vehicles](field/VEHICLES.md), [platforms and belts](field/PLATFORMS_AND_BELTS.md), [NPC wander](field/NPC_WANDER.md).
- [Camera](field/CAMERA.md), [transitions](field/TRANSITIONS_DECODED.md), [color pipeline](field/COLOR_PIPELINE.md).

## camp — menus, party, shops and saves

- [Camp menu layout](camp/CAMP_MENU_LAYOUT.md), [shop layout](camp/SHOP_LAYOUT_DECODED.md), [shops](camp/SHOPS.md).
- [Party ORDER](camp/PARTY_ORDER.md), [party status](camp/PARTY_STATUS.md), [equipment](camp/EQUIP_SCOUT.md), [progression](camp/PROGRESSION.md).
- [Save format](camp/SAVE_SCOUT.md).

## scenes — scenes, dialogue and boot

- [Scene registry](scenes/README.md) and the numbered scene transcriptions beside it.
- [Event coverage census](scenes/EVENT_COVERAGE.md), [dialogue callers](scenes/DIALOGUE2_CALLERS.md), [dialogue tree selection](scenes/DIALOGUE_TREE_SELECTION.md).
- [Scene dialogue](scenes/SCENE_DIALOGUE.md), [dialogue actions](scenes/DIALOGUE_ACTIONS.md), [scene presentation](scenes/SCENE_PRESENTATION.md), [field reload](scenes/FIELD_RELOAD.md).
- [Event engine scout](scenes/EVENT_ENGINE_SCOUT.md), [title boot](scenes/TITLE_BOOT.md),
  [revision-conditional op audit](scenes/REVISION_AUDIT.md),
  [live-layout write census](scenes/LIVE_LAYOUT_WRITES.md).

## sound — sound extraction and integration

- [Sound scout](sound/SOUND_SCOUT.md), [sound extraction](sound/SOUND_EXTRACTION.md), [sound integration](sound/SOUND_INTEGRATION.md).

## campaign — the route runner and native replay

- [Campaign runner](campaign/CAMPAIGN_RUNNER.md): route files, the headless runner, and native tape replay.
- [Native drivers](campaign/NATIVE_DRIVERS.md): the Godot drivers that remain beside the tape, and why.

## source-notes — provenance and deviation records

- [Source notes](source-notes/README.md), the provenance and deviation index; its records:
  [cartridge formats](source-notes/formats.md),
  [oracle methodology](source-notes/oracle-methodology.md),
  [disassembly discrepancies](source-notes/disassembly-discrepancies.md),
  [enemy abilities and damage routes](source-notes/battle-enemy-abilities.md),
  [party actions, items and battle flow](source-notes/battle-party.md),
  [dialogue and field travel](source-notes/field-and-dialogue.md).

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
