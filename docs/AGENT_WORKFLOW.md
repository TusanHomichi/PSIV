# Evidence-driven agent workflow

[Project instructions](../AGENTS.md) · [Setup and checks](DEVELOPMENT.md) ·
[Roadmap](ROADMAP.md) · [Documentation index](README.md)

This is PSIV's standing default. Durable rules live in `AGENTS.md`, current
playable scope in the project README, and the work queue and canonical graph
pointer in the roadmap. Subsystem ledgers retain measured history. Do not
create a second tracker or copy changing campaign status into instructions.
The [setup ledger](WORKFLOW_SETUP.md) records adoption and host verification.
This procedure is guidance, not an automatic dispatcher, restart service or
enforcement system. No new framework, dashboard or CI is required.

## Start and select work

Inspect the applicable global/project instruction chain, including overrides,
working tree, worktrees and relevant processes. After interruption, also inspect
the exact candidate revision and fresh receipts before restarting a job; a
disconnected monitor does not mean its child exited. Read task-relevant source,
architecture and ledger corrections, not every historical log.

State one bounded outcome, exclusions and an observable acceptance check before
editing. Astra selects a ready authorized task from the canonical graph and
orchestrates its ownership; implementation remains with Sol or Luna. A small
independent change needs only a short plan and receipt, not an Astra exception.
Keep one next action; do not turn a new idea into an implicit milestone or reset
the effort policy.

PSIV currently uses roadmap/subsystem handoffs. The setup inspection found no
GitHub issues, PRs, templates or hosted workflows; that is a dated observation,
not a permanent prohibition. Reuse an established issue/PR if one later owns
the work, and link to it from the roadmap instead of duplicating its graph.

## Models and delegation

| Role | Explicit requested route | Responsibility |
| --- | --- | --- |
| Orchestration | `gpt-6-astra` / `max` | Concise scoping, architecture and permission decisions, graph selection, coordination, review and acceptance; no routine implementation or repeated check loops |
| Complex delivery | `gpt-6-sol` / `max` | Complex implementation/refactoring through relevant checks, debugging/repairs, candidate freeze and evidence handoff |
| Bounded delivery | `gpt-6-luna` / `max` | Bounded exploration/routine implementation, test runs, log triage, receipts and routine docs through checks and closeout |
| Optional helper | Record the actual DeepSeek model, effort and direct route | Narrow supervised work with a concrete benefit; never primary design/integration |

This is the owner's strict PSIV role split. Small routine
tasks go to Luna and complex work to Sol; Astra does not absorb implementation
because a task is small or a check failed. Astra does not routinely write code,
tests, fixtures, harnesses or docs, or run/monitor repeated checks. Exceptions
require an explicit owner instruction or an actually unavailable route, which
must be reported honestly; never silently substitute Astra. Do not create work
merely to exercise a model. Workers own their outcomes through acceptance,
including relevant checks, repair, evidence and closeout. The parent independently
reviews the result and remains accountable for integration, without taking over
implementation or evidence plumbing. Use compact receipts and proportionate
direct diff/raw-artifact spot checks; do not duplicate worker investigations or
rerun passing checks absent a concrete cause. Verify substantive helper results
independently and record corrections/reliability, including rejected output.

Specify model and effort in the host's launch controls. Record requested versus
actual values from host/session metadata; a catalog proves advertisement, not
successful execution. If a route is unavailable, report it and continue only
independent work without silently substituting. A prompt or `AGENTS.md` cannot
switch an already running model. Do not edit global settings to satisfy this
project's role preferences.

Each delegation names exact inputs, acceptance, owned files, forbidden writes,
mutable resources and concurrency constraints. Include a bounded context packet;
workers return compact results and evidence paths. Tell workers to preserve
others' changes. Use disjoint ownership or isolated worktrees when needed. The
assigned worker retains ownership after failed checks and runs the repair loop;
escalate only concrete architectural or permission blockers. Long-running checks
and process monitoring stay worker-owned, without parent busy polling. Preserve
independent review, serialized expensive runs, and safety requirements. The
parent reviews artifacts and raw checks, not just completion messages. After a
failed batch, review all completed workers' receipts before selecting repair
scope without repeating their investigations. Integrate only after writers are
idle. Do not commit a moving shared tree.

## Authority, effort and continuation

Source: the owner's workflow-setup request, repair-budget reply and subsequent
docs PR/merge/cleanup authorization on 2026-09-23, together with the existing
PSIV input-protection rules. Later explicit
PSIV authorization persists; record its source here or in the canonical task.
Another repository's grants, an available tool or a permissive sandbox confer
no project authority.

| Area | Standing record |
| --- | --- |
| Scope | Implement and verify this documentation setup. Future assigned PSIV tasks include necessary local edits, focused checks and scoped repairs; preserve unrelated work. |
| Local setup | Use documented prerequisites or isolated local dependencies before declaring a missing preinstalled tool unavailable. Do not modify unrelated system configuration. |
| Git / remote writes | The owner authorized committing, publishing a PR, merging and owned cleanup for these workflow docs on 2026-09-23. This is not a standing remote-write grant for campaign changes. Reuse explicit task authorization without asking again. Remote inspection is allowed. |
| Cleanup | Remove only owned disposable outputs/processes and, when integration includes it, owned task branches/worktrees. Retain receipts, source saves and other people's work. No broad cleanup. |
| Deployment / publication | Releases, deployment and asset distribution are outside this setup; no standing grant recorded. |
| Paid services | No paid model service is currently authorized in PSIV; the completed Redshirt/TypeSafe/Jev experiments are summarized in their [archived record](REDSHIRT_BATTLE.md). Any future live batch needs a new explicit assignment, and nothing here authorizes unrelated service or credit purchase. |
| Protected inputs | ROM, disassembly, packs, saves and captures remain local/ignored. Copy and hash source saves; set `PSIV_SAVE_DIR`. Never overwrite the only verified checkpoint or force-add protected assets. |
| Effort | Owner: "Continue scoped repairs until it passes, with no fixed cycle limit." Applies to failures of accepted checks inside authorized scope. It does not waive source, permission, resource or input constraints. |
| Continuation | The original setup graph covers local documentation acceptance; the authorized publication follow-through is recorded in the setup ledger. BioPlant continuation was assigned separately and must not be bundled into the docs PR. During an assigned outcome, continue ready authorized nodes through its agreed endpoint without repeated approval requests. |

On 2026-09-23 the owner separately requested committing the pending intentional
PSIV work, creating a PR, merging it and cleaning up owned integration state.
This authorizes the reviewed BioPlant and post-Rika crossing
integration. It is not a standing grant for later campaign work,
deployment or additional paid calls.

An observed failure leads to a scoped repair and rerun of affected checks. A
missing source/input or genuine authority boundary gets a specific blocker,
not an invented result or repeated blind retries. Complete independent
preparation before asking for the blocking decision; any necessary approval
should concern a concrete reviewable result. New tasks cannot silently reset a
budget, weaken a gate, expand scope or grant permissions through graph edits.

## One canonical task graph

For substantive dependent work, put the graph in its existing issue or handoff;
otherwise use the roadmap's **Task graph handoff** section. Link that location
from the roadmap. Common inputs and policy may be inherited. Each node records:

| Field | Required content |
| --- | --- |
| ID / outcome | Stable ID and one concrete observable result |
| Dependencies | IDs whose verified acceptance unlocks this node |
| Owner | End-to-end implementation, relevant checks, repair and evidence/closeout owner; assigned person/model, effort and actual route if different |
| Inputs | Relevant issue/design/source/ledger references and candidate identity |
| Acceptance | Exact command or observation, expected result and evidence type |
| State | `pending`, `ready`, `in_progress`, `blocked` or `verified` |
| Evidence | Actual receipts, failures/skips, review and artifact references |
| Effort | Explicit local policy or inheritance with its source |

Astra reviews graph edits and selects nodes only when dependencies are verified.
`blocked` names the missing evidence/decision. A check failure returns its node
to repair; invalidate dependent acceptance affected by changes. Do not mark a
node verified because its worker stopped or its budget expired. Preserve stable
IDs and failure history. Archive a completed graph in its owning ledger/closed
issue before advancing, remove its active duplicate, and retain one next action.
The [setup graph](WORKFLOW_SETUP.md#setup-graph) is the first real worked example.

## Find the smallest useful verification

All commands below run from the repository root. Prepare local prerequisites
from [DEVELOPMENT.md](DEVELOPMENT.md) before running asset-dependent tests.

| Change | Start with | Extend verification when needed |
| --- | --- | --- |
| ROM decoding / pack | Relevant `tests/test_*.py` file | Full Python suite and Rust pack consumers if the pack schema changes |
| Battle / field rules | Relevant `psiv-core` tests | Runtime integration, original-game comparison and native input |
| Camp / persistence | Relevant `psiv-runtime/tests/` target | Ordinary input, SAVE, fresh process, CONTINUE and state comparison |
| Godot UI / scenes | Build the extension and run the affected interaction | Inspect matching original/native captures; record viewport and region |
| Sound | Relevant `psiv-sound` tests | Register/timing comparisons and playback checks for the affected path |
| Documentation | Verify paths and commands, then `git diff --check` | Code tests only if source also changes |

Examples of selecting existing test targets:

```bash
PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_psiv_tools.py'
CARGO_BUILD_JOBS=1 cargo test --manifest-path rust/Cargo.toml -p psiv-runtime --test camp_order -- --test-threads=1
```

These examples do not cover every change. The full Python/Rust/format/Clippy
commands live in the setup guide. Oracle setup and its fast/full lanes live
in [oracle/README.md](../oracle/README.md). Do not rerun expensive suites after
an unchanged, applicable pass without a new reason.

Run the cheapest useful existing check during development. Add feedback only
when a concrete observation is missing; introduce a new correctness check with
a meaningful negative control. Repair real failures without rewriting evidence.
Review and freeze a candidate (commit, or base commit plus exact patch and
changed-file hashes) before expensive gates. A relevant code, test or gate-input
change invalidates affected acceptance; rerun it and retain the earlier receipt
under its original candidate identity.

Full Python, Rust and oracle jobs stay serialized under PSIV's resource rules.
When PR publication is authorized, independent local and hosted checks may run
concurrently, with isolated outputs, databases, ports and mutable build inputs.
Keep local/default/browser/hosted results separate. A docs check is no gameplay
test, and a hosted pass does not replace native or cartridge evidence.

## Reproduce without damaging the checkpoint

Before a campaign run, identify the source commit and dirty changes, pack
manifest hash, source save hash, driver and executable. Use a separate output
directory under `build/` and a separate `PSIV_SAVE_DIR`. Inspect driver paths
and stop conditions before launch. Record any fixture injection explicitly.

Check that the intended build is actually running: a loaded shared library
can outlive a source edit. Rebuild with the relevant Godot process stopped.
Use the documented desktop viewport for UI checks; tiny screenshots can clip
menus even when state assertions pass.

A failure is useful evidence. Keep its logs and resulting state separately
from a successful save. Do not repair party resources or flags behind the
driver's back and then describe the run as ordinary connected play.

## Receipt and handoff template

Put the concise result in the relevant subsystem ledger. Detailed logs and
ROM-derived artifacts stay local; commit reproduction instructions and hashes.
Use repo-relative paths in shared documentation so another checkout can follow
them. Identify unavailable local inputs instead of assuming they ship in Git.

```text
Task and intended behavior:
Source commit, branch, and relevant dirty changes:
Files changed and why:
Retail basis (symbols/offsets, ledger references):
Environment (tool versions, executable, viewport):
Inputs (pack identity, source save hash, fixture assumptions):
Requested and actual model/effort; ownership and authority source:
Commands actually run, cwd, UTC start/end, elapsed time and exit status:
Results (passed / failed / skipped / not run), raw log paths and hashes:
Evidence type (state / native input / persistence / visual comparison):
Outputs (local paths and hashes, resulting party/map/flags where relevant):
Review (self / independent, reviewer, candidate, findings and corrections):
Known limits and remaining failures:
Next concrete step:
Git state (commit/push status and any remaining edits):
```

For a small docs fix, omit irrelevant gameplay fields. For a campaign milestone,
retain enough input and save provenance to distinguish a continuation from an
isolated fixture. Keep dated historical results intact and add corrections
explicitly when new evidence overturns an earlier interpretation.

Generate revisions, hashes, timing, statuses and receipts from actual artifacts;
do not fill them from recollection. Read raw outputs and read back the archive.
Claim speedups only from comparable completed measurements, with their scope.
ROM-derived logs/captures stay ignored; commit concise provenance and reproduction
commands. Local evidence may be absent from another checkout: say so explicitly.

## Review, integration and procedure changes

Review both diff and behavior, label self-review versus independent review, and
resolve material findings before acceptance. Once permitted, continue through
PR, merge, exact-main verification, evidence archive/read-back and owned cleanup.
Verify affected checks against the integrated main revision; a candidate's old
receipt is not an exact-main pass. Inspect final Git/worktree/process state and
continue the next ready authorized node. If integration was not requested, leave
a verified local result and report that boundary without inventing an approval gate.

Improve this procedure from observed problems: record the problem, smallest
change, review and verification in the relevant existing ledger. Do not weaken
checks or claim automatic dispatch/restart/enforcement without a working executor
and a verification result.

## Instruction loading

Codex loads its global instructions and the project chain at session startup;
`AGENTS.override.md` takes precedence at its directory level. A linked generic
Markdown procedure is not automatically loaded: `AGENTS.md` explicitly routes
agents here. Check configured fallback names and instruction byte limits when
applicable. Verify the effective chain in a fresh session when supported, and
record any unverified host separately. Do not conflate a successful CLI check
with every IDE or app host. See the official
[AGENTS.md guidance](https://learn.chatgpt.com/docs/agent-configuration/agents-md)
and [Astra instruction guidance](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra).
