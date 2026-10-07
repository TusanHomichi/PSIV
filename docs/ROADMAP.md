# Roadmap

Historical native gameplay checkpoint: September 23, 2026.
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

The active graph is the [campaign runner graph](campaign/CAMPAIGN_RUNNER.md#task-graph),
opened 2026-10-01 when the owner replaced per-segment native drivers with one
headless runner over a runtime-owned `Session`. It supersedes the
[Aiedo arrival graph](campaign/AIEDO.md#5-aiedo-arrival-task-graph), which never
started. The current combined candidate has an 11/11 zero-RMSE certificate.
The sole five-stage gate on clean `5ec096f` passed docs, formatting, Rust tests
and Clippy; Python failed on a missing ignored oracle fixture and a file-list
authority guard. That failed receipt remains immutable. The scoped repair on
clean `e9372bc` passed 1,241 Python tests (one skip) and docs (zero problems)
through one affected recheck. Its composed five-row matrix retains the earlier
fmt/Rust/Clippy passes because all 558 Rust/config inputs match; it is not a
second full gate. The [R2 native receipts](campaign/CAMPAIGN_RUNNER.md#r2-native-tape-replay-in-progress)
separately cover New Game-to-initial-Aiedo and earned-checkpoint Zio SAVE/fresh
CONTINUE. A primary default-pack 180-pad CONTINUE also passed with no
`PSIV_RUNTIME_PACK` override and a visible 1280×800 field. The isolated
[#70 headless receipt](campaign/RUNNER_LOG.md#70-frozen-headless-acceptance)
remains historical, not the combined Godot check.

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
- [x] Bounded post-Zio checkpoint: the 29-chapter headless New Game route, ordinary final SAVE and fresh Session load/replay; separate native earned-save Zio SAVE and fresh CONTINUE, plus primary default-pack CONTINUE. See the campaign graph for evidence boundaries.

These describe the recorded native routes and targeted checks, not complete
coverage of every branch or every mechanic. Details are in the
[playability ledger](campaign/NATIVE_PLAYABILITY.md) and [BioPlant ledger](campaign/BIOPLANT_NATIVE.md).

The combined headless `Session` candidate on 2026-10-03 plays all 29 chapters
from New Game through `Cutscene_ZioDefeated` twice, uses an ordinary camp SAVE,
loads it in a fresh Session and replays both identical tapes. The
[runner receipt](campaign/RUNNER_LOG.md#combined-p1f2r2-headless-integration) records
its source, hashes and limits. Generic native tape replay has reached the
initial Aiedo endpoint from New Game; an earned-save Zio segment also passes
ordinary SAVE and fresh CONTINUE. They are distinct input chains, with their
raw evidence in [R2](campaign/CAMPAIGN_RUNNER.md#r2-native-tape-replay-in-progress).
The 11-pair static certification passed. The composed applicable-check matrix
is green on clean `e9372bc`; it joins one affected Python/docs recheck to the
unchanged Rust/fmt/Clippy rows, without changing the original failed `5ec096f`
receipt. The primary default-pack 180-pad ordinary CONTINUE passed from a
copied, hash-verified native-written save with no override; that copy is not a
new SAVE. The campaign graph marks C70 and CZ verified as a bounded post-Zio
checkpoint, while full R2 remains in progress on bespoke-driver retirement.
Whole-scene/event-oracle/audio parity and later-arc play remain open.

**Next action:** integrate the Air Castle enemy abilities (lane A5, committed on
its branch; it needs a pack rebuild for the decoded inline formation), then restore
the Xe-A-Thoul chapter and play on to Lashiec and Dark Force 2. The route plays New
Game to the Air Castle's arrival in 50 chapters with every player technique and skill
cartridge-verified ([run log](campaign/RUNNER_LOG.md)).

## 1. Continue from the post-Rika checkpoint

The [northern-crossing gate](field/TRAVEL.md#post-rika-northern-crossing-2026-09-23)
is complete. This section preserves the older replay source; the current
single next route action is post-Zio, above. The post-Rika source is Motavia
`$00 (84,64)`, party Gryz/Alys/Chaz/Hahn/Rika, all alive with persistent statuses zero, original
inventory/event flags retained and 1129 meseta. Paid Zema recovery, one victory,
the bridge crossing and ordinary SAVE passed. Fresh-process CONTINUE preserved
state; Down to `(84,65)` and SAVE 2 passed byte validation.

For that historical replay, use
`build/native-post-rika-20260923/attempt-03/saves/slot_1.sram`, SHA256
`2590e0e97ff3125774951ed2c474eba832892e3644cb79c8cdbb443d6499379e`.
Copy it to a fresh run directory and hash it before use. The prior post-Rika
`(99,83)` source, healthy `$A7` saves and every failed attempt remain protected.
The new code requires the rebuilt full pack; exact candidate, pack, binary and
check receipts are in the travel ledger. The pack, saves and captures remain
local and ignored.

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
