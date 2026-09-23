# Roadmap

Gameplay checkpoint: September 15, 2026; workflow handoff: September 23, 2026.
The goal is a complete native playthrough with
the original game's behavior and presentation. Work is ordered by what
blocks that playthrough; modding comes afterward.

## Task graph handoff

This is the canonical active graph location unless it links to an owning
issue/PR. The sections below remain the campaign work queue, not a second graph.
The [workflow](AGENT_WORKFLOW.md) defines node states, evidence and authority.

No active implementation graph. The completed [workflow setup graph](WORKFLOW_SETUP.md#setup-graph)
is archived with its checks; setup did not launch campaign work.

**Next action:** when BioPlant continuation is assigned, re-anchor the healthy
`$A7` source save, pack and driver against the [BioPlant ledger](BIOPLANT_NATIVE.md),
then open its bounded graph here. The required connected join/escape and restart
gates remain in milestone 1 below.

## Delivered checkpoints

- [x] Native title, opening, first control and Academy completion.
- [x] Connected early travel through Tonoe, Alshline and the Zema aftermath.
- [x] Persistent rewards, learned abilities, equipment, inventory and saves.
- [x] BioPlant entrance, alarm, doors and elevator traversal to `$A7`.
- [x] STATE/ORDER with pick, undo, cancel, save/reload and original-menu checks.
- [x] A bounded BioPlant battle followed by two ANTI cures, recovery and SAVE/CONTINUE.

These describe the recorded native routes and targeted checks, not complete
coverage of every branch or every mechanic. Details are in the
[playability ledger](NATIVE_PLAYABILITY.md) and [BioPlant ledger](BIOPLANT_NATIVE.md).

## 1. Finish the connected BioPlant route

The current durable checkpoint is `$A7`. The longer Gryz-leading attempt
won six fights and reached `$A9`, but Hahn died. Rika's isolated scene fixture
and exact opening frame are useful evidence; a connected join/escape is still owed.

Next work:

- Exercise the newly verified poison-recovery strategy on the onward route.
- Evaluate ordinary use of Chaz's existing EARTH charges and the healing budget.
- Investigate any reproducible mechanic discrepancy against the cartridge.

**Done when:** ordinary input reaches Rika, joins her to the loaded party,
escapes with the expected story flags, and saves successfully. A fresh process
must CONTINUE that save with party, resources, inventory and flags intact.
Record survival and state checks separately from presentation comparisons.

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
