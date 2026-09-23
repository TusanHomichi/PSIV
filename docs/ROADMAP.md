# Roadmap

Gameplay checkpoint and workflow handoff: September 23, 2026.
The goal is a complete native playthrough with
the original game's behavior and presentation. Work is ordered by what
blocks that playthrough; modding comes afterward.

## Task graph handoff

This is the canonical active graph location unless it links to an owning
issue/PR. The sections below remain the campaign work queue, not a second graph.
The [workflow](AGENT_WORKFLOW.md) defines node states, evidence and authority.

The [post-Rika northern-crossing graph](TRAVEL.md#archived-northern-crossing-task-graph)
is complete and archived. Ordinary Zema recovery, the Rika-opened bridge,
SAVE and fresh-process CONTINUE pass on the frozen repaired candidate.
There is no active implementation graph. The one next action is in
[section 1](#1-continue-from-the-post-rika-checkpoint).

The [workflow setup graph](WORKFLOW_SETUP.md#setup-graph) and
[connected BioPlant graph](BIOPLANT_NATIVE.md#archived-bioplant-task-graph)
are archived. Workflow docs were merged through PR #1. The verified BioPlant
driver repair and campaign ledger are included in this revision; their raw
saves and captures remain local.

The assigned Redshirt proof is complete; its [archived graph and receipts](REDSHIRT_BATTLE.md#archived-task-graph)
keep synthetic decisions separate from campaign and native-menu evidence.
Both policies won eight frozen cases; Jev made 34 calls and only attacked.
A separately labelled post-hoc attack-only rule reproduced every Jev result,
so this trial establishes no incremental model value. The shared replay repair
was merged through Redshirt PR #33. PSIV's adapter and campaign source are
included in this revision; protected inputs and earlier work are preserved.

The follow-up [survival-pressure trial](REDSHIRT_BATTLE.md#survival-pressure-results)
is also complete: attack-only wins 0/8; Jev and the projected policy each
win 3/8 in all three live passes, with different victory sets. A post-hoc
threat-first rule explains Jev's extra victory. The [completed graph](REDSHIRT_BATTLE.md#archived-survival-pressure-graph)
retains the protocol, 82-call result, checks and scoped limitations. No native
adaptive controller or production game rule changed. Raw experiment evidence
remains local.

The [mechanics-briefing follow-up](REDSHIRT_BATTLE.md#mechanics-briefing-results)
is complete. With engine, cases, objective and menu fixed, Jev won 8/8 with
source-verified rules versus 3/8 with sparse information in all three paired
passes. The unchanged fixed policy remained 3/8. The [archived graph](REDSHIRT_BATTLE.md#archived-mechanics-briefing-graph)
retains the frozen briefing, 158 accepted calls and exact replay evidence.
The earlier rejection of Jev as a useful selector was premature given the
missing mechanics; native integration remains unproven and unchanged.

The [fresh-case comparison is archived](REDSHIRT_BATTLE.md#archived-fresh-case-graph):
the threat-aware rule wins 19/24 each pass; Jev wins 18/24, 18/24 and 17/24,
with all four extra non-wins caused by provider timeouts. No Jev-only victory
was observed. Shared-win TP savings came with lower final HP; all concrete
traces replayed and independent raw review confirmed the results.

**Direct-model comparison archived:** the [results](REDSHIRT_BATTLE.md#direct-model-results)
cover Jev and DeepSeek Flash on 24 known cases repeated three times. Jev won
47/72 and DeepSeek 50/72; the fresh deterministic reference won 19/24 once.
Jev's median first selection was 2.52 times faster, but its timeout tail gave
DeepSeek lower total episode time per win. Known charges were USD 0.0204 versus
0.0338; 12 Jev calls have unknown usage. Luna is excluded from this API run:
the owner's subscription workflow is a separate unmeasured comparison.
All 144 model traces replayed exactly, and independent review matched the raw
results. The [archived graph](REDSHIRT_BATTLE.md#archived-direct-model-task-graph)
retains scope, limitations and closure receipts. Keep the model-free battle path.
Reusable API support is merged through
[Redshirt PR #39](https://github.com/FieldmouseWorks/redshirt/pull/39).
The campaign queue below is unchanged. PSIV's source and ledgers are included
in this revision; raw experiment evidence remains local.

## Redshirt fit assessment (2026-09-23)

The inspected shared source is
[Redshirt `47c5537`](https://github.com/FieldmouseWorks/redshirt/tree/47c5537d3866b0e34fc46472b6643819320cb7db).
It advanced from `f7338da` during this assessment when PR #27 merged; only
`docs/PROGRESS.md` changed. The merged
[Rust adapter contract](https://github.com/FieldmouseWorks/redshirt/blob/47c5537d3866b0e34fc46472b6643819320cb7db/docs/RUST-ADAPTER.md)
already supplies bounded execution, candidate freshness, independent evaluation,
evidence and model-free replay. A thin PSIV adapter supplies observations,
legal actions, reset, effects and cleanup. No duplicate controller is needed.

The useful first hypothesis is resource-aware action selection in one small,
resettable battle decision window, compared with the unchanged fixed policy.
TypeSafe's [typed judgments](https://docs.typesafe.ai/primitives) fit a focused
choice among supplied options; long-horizon campaign planning is outside this
proof. Exact ROM/rule/save checks remain deterministic. The full BioPlant run
exceeds the merged runner's 24-input/decision and 180-second episode ceilings;
do not omit inputs or raise shared limits merely to fit that route.

First-proof acceptance:

1. Pin source, pack/fixture identity, reset state, legal menu and budgets. Use
   a small decision-ready fixture; preserve exact freshness/replay semantics
   without normalizing a changing clock to manufacture agreement.
2. Run provider-free scripted selection and the existing fixed policy. Verify
   actual effects independently, reject stale/unavailable actions with negative
   controls, preserve failures, and replay concrete operations without a model.
3. Prove ordinary menu input separately from any headless contract test. Keep
   the Rust runner's zero-capture evidence boundary; native captures stay in
   PSIV's local evidence and do not become provider input.
4. Before a live comparison, freeze calibration/held-out cases, a minimal
   reviewed observation schema and explicit owner allowance. Measure survival,
   progress, TP/item use, refusals and elapsed time against the same baseline.
   A provider usefulness or efficiency claim needs completed comparable results.

The [Jev provider](https://github.com/FieldmouseWorks/redshirt/blob/47c5537d3866b0e34fc46472b6643819320cb7db/docs/JEV-RUST.md)
is optional. [Issue #26](https://github.com/FieldmouseWorks/redshirt/issues/26)
already records a live Choice/max-probability disagreement rejected before
dispatch; retain that validator and the failed evidence. PRs #23/#25 were still
unmerged at this inspection; their context-selection work is not a shipped
PSIV adapter. No new shared-runtime defect was demonstrated, so no duplicate
upstream issue or PR was created. Concrete reusable needs can get their own
scoped public records with generic/sanitized evidence. Protected local inputs
remain excluded; see [Redshirt usage guidance](AGENT_WORKFLOW.md#shared-redshirt-experiments).

This was source/contract review only: no Redshirt build, adapter execution,
model call, external asset upload or remote write. Detailed reviewed notes
are local at `build/native-bioplant-20260923/redshirt-assessment.md`.

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
[playability ledger](NATIVE_PLAYABILITY.md) and [BioPlant ledger](BIOPLANT_NATIVE.md).

## 1. Continue from the post-Rika checkpoint

The [northern-crossing gate](TRAVEL.md#post-rika-northern-crossing-2026-09-23)
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
