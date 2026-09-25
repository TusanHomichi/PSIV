# Documentation

The top level keeps this index and the documents the project reads first; every
other document sits in the folder for its area.

## Start here

- [Project overview and current status](../README.md)
- [Roadmap and completion criteria](ROADMAP.md)
- [Feature map: a feature's owning paths and focused test](FEATURE_MAP.md)
- [Build, run and verification](DEVELOPMENT.md)
- [Repository instructions for agents](../AGENTS.md)
- [Evidence-driven workflow, authority and handoff](AGENT_WORKFLOW.md)
- [Canonical task graph handoff](ROADMAP.md#task-graph-handoff)
- [Runtime design](RUNTIME_DESIGN.md)
- [ROM extraction reference](EXTRACTION.md)
- [MIT License](../LICENSE)

## battle — battle rules and retail research

- [Battle research](battle/BATTLE_SCOUT.md) and its [worked continuation](battle/BATTLE_SCOUT_CONTINUATION.md).
- [Enemy ability inventory](battle/ENEMY_ABILITIES.md), [enemy damage routes](battle/ENEMY_DAMAGE_ROUTES.md), [enemy POISON](battle/ENEMY_POISON.md).
- [Battle animations](battle/BATTLE_ANIMATIONS.md), [battle geometry](battle/BATTLE_GEOMETRY.md), [battle recovery](battle/BATTLE_RECOVERY.md), [instant death](battle/INSTANT_DEATH.md).
- [THREAD](battle/THREAD.md), [RIMIT](battle/RIMIT.md).

## oracle — cartridge-comparison ledgers

- [Forced battles](oracle/BATTLE_ORACLE_FORCED.md), [replay](oracle/BATTLE_ORACLE_REPLAY.md), [sweep](oracle/BATTLE_ORACLE_SWEEP.md), [battle UI](oracle/BATTLE_ORACLE_UI.md).
- [Results](oracle/RESULTS.md) and [results continued](oracle/RESULTS_CONTINUED.md).
- Method, tapes, harness and captures: the [oracle guide](../oracle/README.md).

## field — field, map and travel behavior

- [Field state](field/FIELD_STATE.md), [map effects](field/MAP_EFFECTS.md), [chests](field/CHESTS.md).
- [Travel and pipes](field/TRAVEL.md), [vehicles](field/VEHICLES.md), [NPC wander](field/NPC_WANDER.md).
- [Camera](field/CAMERA.md), [transitions](field/TRANSITIONS_DECODED.md), [color pipeline](field/COLOR_PIPELINE.md).

## camp — menus, party, shops and saves

- [Camp menu layout](camp/CAMP_MENU_LAYOUT.md), [shop layout](camp/SHOP_LAYOUT_DECODED.md), [shops](camp/SHOPS.md).
- [Party ORDER](camp/PARTY_ORDER.md), [party status](camp/PARTY_STATUS.md), [equipment](camp/EQUIP_SCOUT.md), [progression](camp/PROGRESSION.md).
- [Save format](camp/SAVE_SCOUT.md).

## scenes — scenes, dialogue and boot

- [Scene registry](scenes/README.md) and the numbered scene records beside it.
- [Scene dialogue](scenes/SCENE_DIALOGUE.md), [dialogue actions](scenes/DIALOGUE_ACTIONS.md), [scene presentation](scenes/SCENE_PRESENTATION.md).
- [Event engine scout](scenes/EVENT_ENGINE_SCOUT.md), [title boot](scenes/TITLE_BOOT.md).

## sound — sound extraction and integration

- [Sound scout](sound/SOUND_SCOUT.md), [sound extraction](sound/SOUND_EXTRACTION.md), [sound integration](sound/SOUND_INTEGRATION.md).

## campaign — connected campaign evidence

- [Native playability ledger](campaign/NATIVE_PLAYABILITY.md) — entry point: current checkpoint, owner objective and the segment index.
  - [Foundations and audit](campaign/PLAYABILITY_FOUNDATIONS.md) — audit context, the opening slice's repairs and evidence, and the implementation priorities.
  - [Academy](campaign/PLAYABILITY_ACADEMY.md) — command menus and technique/skill/item combat, the opening resistance repair, the first-boss Fission, dialogue and choices, the connected Academy route, camp restorative rules and battle return reload.
  - [Holt and Tonoe](campaign/PLAYABILITY_HOLT_TONOE.md) — field TECH and SKILL recovery, the connected route through Holt, Acid Breath and attack ailments, map entry and rock removal, field status, the first Zio encounter, field travel and chest input.
  - [Alshline and Zema](campaign/PLAYABILITY_ALSHLINE_ZEMA.md) — the connected Alshline progression and completion, instant-death commands, battle recovery, STATUS and RIMIT, the THREAD correction, the Zema restarts and the BioPlant elevator checkpoint.
- [BioPlant checkpoints](campaign/BIOPLANT_NATIVE.md) — connected Rika escape, ordinary SAVE/fresh CONTINUE, poison recovery and retained failed attempts.
- [Post-Rika northern crossing](field/TRAVEL.md#post-rika-northern-crossing-2026-09-23) — current Motavia north-bank save, paid recovery, repaired overworld bridge, restart bytes and bounded visual proof.
- [Party ORDER](camp/PARTY_ORDER.md) — original rules, native input and exact menu-region comparisons.
- [Progression](camp/PROGRESSION.md) — learning, skill uses and saved character growth.

## records — workflow records and closed experiments

- [Workflow setup evidence and archived graph](records/WORKFLOW_SETUP.md)
- [Claude Code lane ledger](records/CLAUDE_LANES.md) — adoption record and reliability log for the lane routing.
- [Redshirt battle-decision experiment (archived)](records/REDSHIRT_BATTLE.md) — archived 2026-09-24; a record of closed developer-tool experiments, not instructions.
- [Origin conversation (ChatGPT, 2026-08-14)](records/history/2026-08-14-chatgpt-origin-conversation.md) — historical transcript only.

## source-notes — provenance and deviation records

- [Source notes](source-notes/README.md) — the provenance and deviation ledger's index; its records live in:
  - [Cartridge formats](source-notes/formats.md)
  - [Oracle methodology](source-notes/oracle-methodology.md)
  - [Disassembly discrepancies](source-notes/disassembly-discrepancies.md)
  - [Enemy abilities and damage routes](source-notes/battle-enemy-abilities.md)
  - [Party actions, items and battle flow](source-notes/battle-party.md)
  - [Dialogue and field travel](source-notes/field-and-dialogue.md)

## Which document answers which question?

- Current playable scope: the project overview and roadmap.
- Current architecture and commands: runtime design and development setup.
- Retail behavior and measured evidence: subsystem ledgers and scene research.
- Active dependent task graph: the roadmap's handoff section or its single linked owner.
- Workflow adoption, instruction loading and model-route evidence: the setup ledger.
- Obsolete plans and removed status summaries: Git history.

## Rule owners

Each standing rule lives in exactly one document; everything else links to it.
Add a row here when a new rule gets an owner.

| Rule | Owner |
| --- | --- |
| Retail fidelity, integer semantics and layer boundaries | [AGENTS.md](../AGENTS.md#retail-and-layer-boundaries); layers in detail: [runtime design](RUNTIME_DESIGN.md) |
| Retail deviations and provenance | [Source notes](source-notes/README.md) |
| Protected inputs, saves and evidence safety | [AGENTS.md](../AGENTS.md#protect-local-inputs-and-evidence) |
| Evidence types and what a claim needs | [AGENTS.md](../AGENTS.md#verify-the-claim-you-intend-to-make) |
| Roles, delegation, authority and task graphs | [Agent workflow](AGENT_WORKFLOW.md#roles-and-delegation) |
| Correction ladder | [Agent workflow](AGENT_WORKFLOW.md#correction-ladder) |
| One paved path; gaps as issues; guard before cleanup | [Agent workflow](AGENT_WORKFLOW.md#the-codebase-is-memory) |
| Receipts and handoff | [Agent workflow](AGENT_WORKFLOW.md#receipt-and-handoff-template) |
| Gate commands, prerequisites, coverage and repository-owned checks | [Development guide](DEVELOPMENT.md#gate-and-coverage) |
| File size | [Development guide](DEVELOPMENT.md#file-size) |
| Cartridge comparison method and lanes | [Oracle guide](../oracle/README.md) |
| Current scope; work queue and active graph | [Overview](../README.md); [roadmap](ROADMAP.md#task-graph-handoff) |

Keep one current work list in the roadmap. Dated measurements remain useful
evidence for their recorded inputs; they do not automatically certify a newer
build. Research and isolated scene fixtures do not establish campaign completion.
