# Roadmap

Gameplay checkpoint and workflow handoff: September 23, 2026.
The goal is a complete native playthrough with
the original game's behavior and presentation. Work is ordered by what
blocks that playthrough; modding comes afterward.

## Task graph handoff

This is the canonical active graph location unless it links to an owning
issue/PR. The sections below remain the campaign work queue, not a second graph.
The [workflow](AGENT_WORKFLOW.md) defines node states, evidence and authority.

The [post-Rika northern-crossing graph](field/TRAVEL.md#archived-northern-crossing-task-graph)
is complete and archived. Ordinary Zema recovery, the Rika-opened bridge,
SAVE and fresh-process CONTINUE pass on the frozen repaired candidate.
There is no active implementation graph. The one next action is in
[section 1](#1-continue-from-the-post-rika-checkpoint).

The [workflow setup graph](records/WORKFLOW_SETUP.md#setup-graph) and
[connected BioPlant graph](campaign/BIOPLANT_NATIVE.md#archived-bioplant-task-graph)
are archived. Workflow docs were merged through PR #1. The verified BioPlant
driver repair and campaign ledger are included in this revision; their raw
saves and captures remain local.

The Redshirt battle-decision experiments closed on 2026-09-23 and are now
[archived](records/REDSHIRT_BATTLE.md); the project moved on from Redshirt and its
TypeSafe/Jev provider, and the campaign queue below is unchanged.

## Delivered checkpoints

- [x] Native title, opening, first control and Academy completion.
- [x] Connected early travel through Tonoe, Alshline and the Zema aftermath.
- [x] Persistent rewards, learned abilities, equipment, inventory and saves.
- [x] BioPlant entrance, alarm, doors and elevator traversal to `$A7`.
- [x] STATE/ORDER with pick, undo, cancel, save/reload and original-menu checks.
- [x] A bounded BioPlant battle followed by two ANTI cures, recovery and SAVE/CONTINUE.
- [x] Connected healthy `$A7` continuation through Rika join/escape, all five alive, ordinary SAVE and fresh-process CONTINUE with byte validation.
- [x] Post-Rika Zema recovery and northern bridge crossing, all five alive, ordinary SAVE/fresh CONTINUE, and exact 32×32 bridge-region comparison.

These describe the recorded native routes and targeted checks, not complete
coverage of every branch or every mechanic. Details are in the
[playability ledger](campaign/NATIVE_PLAYABILITY.md) and [BioPlant ledger](campaign/BIOPLANT_NATIVE.md).

## 1. Continue from the post-Rika checkpoint

The [northern-crossing gate](field/TRAVEL.md#post-rika-northern-crossing-2026-09-23)
is complete. The current campaign source is Motavia `$00 (84,64)`, party
Gryz/Alys/Chaz/Hahn/Rika, all alive with persistent statuses zero, original
inventory/event flags retained and 1129 meseta. Paid Zema recovery, one victory,
the bridge crossing and ordinary SAVE passed. Fresh-process CONTINUE preserved
state; Down to `(84,65)` and SAVE 2 passed byte validation.

Use `build/native-post-rika-20260923/attempt-03/saves/slot_1.sram`, SHA256
`2590e0e97ff3125774951ed2c474eba832892e3644cb79c8cdbb443d6499379e`.
Copy it to a fresh run directory and hash it before use. The prior post-Rika
`(99,83)` source, healthy `$A7` saves and every failed attempt remain protected.
The new code requires the rebuilt full pack; exact candidate, pack, binary and
check receipts are in the travel ledger. The pack, saves and captures remain
local and ignored.

**Next action:** establish the retail-backed ordinary-input route from this
north-bank checkpoint to **Aiedo `$54`**, including its entry/story conditions
and resource budget, then open one bounded graph for that arrival and normal
SAVE/fresh CONTINUE. Do not advance into the Fort or restart archived model
experiments as part of that gate.

The exact 32×32 bridge match does not establish whole-scene visual parity,
retail/native save-coordinate interchange, or later campaign progression.

## 2. Complete gameplay coverage

- Remaining character techniques and skills, including SEALS/FEEVE/AROWS.
- Remaining enemy abilities and conditional AI. Unsupported regular abilities
  currently can fall back to physical attacks; this is an explicit fidelity gap.
- Battle macros/combinations and the remaining camp commands, including MUMBL/MACRO.
- Later bosses, party changes, vehicles, story branches and special field behavior.

**Done when:** each implemented mechanic has a cartridge-backed rule, a
meaningful regression and a native input check where applicable. Data extraction
or animation coverage alone does not close a gameplay item.

## 3. Finish presentation fidelity

- Whole-scene staging, camera, palettes and temporary objects.
- Original ability animations and timings, including CROSSCUT's separate hits.
- Remaining Rika palette writes and scene-wide comparisons.
- Camp summaries, child windows and small-viewport clipping.
- Results screens, menus, audio timing and controller behavior.

**Done when:** comparisons name the exact scene/frame or window region, use
the matching original state, and retain reproducible captures. A matching
opening frame does not certify the rest of its scene.

## 4. Package and prove a complete game

- Reproducible build and installation instructions beyond the current Linux setup.
- Desktop exports, pack discovery and save-directory handling on each supported platform.
- A full campaign using ordinary controls, including restarts and recovery.

**Done when:** a fresh installation can complete the campaign without an
emulator, debug state injection or development-machine paths at play time.
Required local ROM/pack preparation must be documented and reproducible.

## 5. Add modding tools

After the base game works end to end, add authoring and validation tools.
Keep the extracted retail pack separate from mods, and keep the unmodified
game usable without an editor or agent service.

## Recording progress

For each checkpoint, record the source commit, pack identity, commands,
checks, native route, save hashes and known limits. Keep unsuccessful attempts
distinct from verified saves. Update the README and this roadmap when a
milestone closes; preserve detailed evidence in the relevant subsystem ledger.
