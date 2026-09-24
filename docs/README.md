# Documentation

## Start here

- [Project overview and current status](../README.md)
- [Roadmap and completion criteria](ROADMAP.md)
- [Build, run and verification](DEVELOPMENT.md)
- [Repository instructions for agents](../AGENTS.md)
- [Evidence-driven workflow, authority and handoff](AGENT_WORKFLOW.md)
- [Canonical task graph handoff](ROADMAP.md#task-graph-handoff)
- [Workflow setup evidence and archived graph](WORKFLOW_SETUP.md)
- [Redshirt battle-decision experiment](REDSHIRT_BATTLE.md)
- [MIT License](../LICENSE)

## Current campaign evidence

- [Native playability ledger](NATIVE_PLAYABILITY.md) — connected routes, fixes and verification history.
- [BioPlant checkpoints](BIOPLANT_NATIVE.md) — connected Rika escape, ordinary SAVE/fresh CONTINUE, poison recovery and retained failed attempts.
- [Post-Rika northern crossing](TRAVEL.md#post-rika-northern-crossing-2026-09-23) — current Motavia north-bank save, paid recovery, repaired overworld bridge, restart bytes and bounded visual proof.
- [Party ORDER](PARTY_ORDER.md) — original rules, native input and exact menu-region comparisons.
- [Progression](PROGRESSION.md) — learning, skill uses and saved character growth.

## Gameplay and presentation

- [Battle recovery](BATTLE_RECOVERY.md), [instant death](INSTANT_DEATH.md), [THREAD](THREAD.md), [RIMIT](RIMIT.md), [enemy ability inventory](ENEMY_ABILITIES.md), [enemy POISON](ENEMY_POISON.md).
- [Chests](CHESTS.md), [equipment](EQUIP_SCOUT.md), [travel and pipes](TRAVEL.md), [party status](PARTY_STATUS.md).
- [Scene dialogue](SCENE_DIALOGUE.md), [scene presentation](SCENE_PRESENTATION.md), [battle animations](BATTLE_ANIMATIONS.md).
- [Sound integration](SOUND_INTEGRATION.md), [vehicles](VEHICLES.md), [save format](SAVE_SCOUT.md).

## Architecture and source research

- [Runtime design](RUNTIME_DESIGN.md)
- [ROM extraction reference](EXTRACTION.md)
- [Source notes](../SOURCE_NOTES.md) — the provenance and deviation ledger's index; its records live in:
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

Keep one current work list in the roadmap. Dated measurements remain useful
evidence for their recorded inputs; they do not automatically certify a newer
build. Research and isolated scene fixtures do not establish campaign completion.
