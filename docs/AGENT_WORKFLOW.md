# Agent workflow

[Project instructions](../AGENTS.md) · [Setup and checks](DEVELOPMENT.md) ·
[Roadmap](ROADMAP.md) · [Documentation index](README.md)

This is PSIV's standing process for planning, delegation, authority, evidence
and handoff. Each standing rule has one owner, listed in the
[documentation index](README.md#rule-owners). This procedure is guidance, not
an automatic dispatcher or enforcement system; enforcement belongs to the
[gate](DEVELOPMENT.md#gate-and-coverage).

## Where work and its record live

Owner decision, 2026-10-14. Active work is a GitHub issue carrying one of the
labels `v1.0-blocker`, `gameplay-fidelity`, `presentation`, `tooling` or
`post-release`. Planning and acceptance live in that issue or in
the pull request that closes it: the observable outcome, its exclusions, the
acceptance check and, for dependent work, the order of steps. Changes land on
`main` through a branch and a pull request, and the PR description is the
receipt. Commits, issues and PRs are the project's record; the docs hold no
task graphs, run logs or status ledgers (git history keeps the old ones;
`tools/check_docs.py` refuses new ones), and
are updated only when the behavior, commands or retail findings they describe
change.

## Start and select work

Inspect the applicable instruction chain (including overrides), the working
tree, worktrees and relevant processes. After an interruption, also inspect the
exact candidate revision and fresh results before restarting a job; a
disconnected monitor does not mean its child exited. Read the task-relevant
source, design and recorded corrections, not every old log.

State one bounded outcome, exclusions and an observable acceptance check before
editing. The orchestrator selects a ready authorized issue and assigns it; a
delivery worker implements it. A small independent change needs only a short
plan and receipt. Do not turn a new idea into an implicit milestone or reset
the effort policy; file it as an issue.

## Roles and delegation

| Role | Responsibility |
| --- | --- |
| Orchestrator | Scoping, architecture and permission decisions, work selection, coordination, review and acceptance; no routine implementation or repeated check loops |
| Complex delivery | Complex implementation and refactoring through relevant checks, debugging and repairs, candidate freeze and evidence handoff |
| Bounded delivery | Bounded exploration and routine implementation, test runs, log triage, receipts and routine docs through checks and closeout |
| Optional helper | Narrow supervised work with a concrete benefit; never primary design or integration |

Small routine tasks go to bounded delivery and complex work to complex
delivery; the orchestrator does not absorb implementation because a task is
small or a check failed. Exceptions require an explicit owner instruction or an
actually unavailable route, reported honestly. Workers own their outcomes
through acceptance, including relevant checks, repair, evidence and closeout.
The orchestrator independently reviews the result and remains accountable for
integration, without taking over implementation. Review compact receipts with
proportionate spot checks of the diff and raw artifacts; do not duplicate a
worker's investigation or rerun a passing check without a concrete cause.
Verify substantive helper results independently, including rejected output.

Which model, tool and effort fill each role is host-local configuration in
untracked files, never tracked guidance; the same goes for personal and
machine-specific preferences. Record requested versus actual values from host
or session metadata; a catalog proves advertisement, not execution. If a route
is unavailable, report it and continue only independent work. A prompt or
`AGENTS.md` cannot switch an already running model.

Each delegation names exact inputs, acceptance, owned files, forbidden writes,
mutable resources and concurrency constraints, with a bounded context packet;
workers return compact results and evidence paths. Use disjoint ownership or
isolated worktrees when needed. The assigned worker keeps ownership after a
failed check and runs the repair loop; escalate only concrete architectural or
permission blockers. Long-running checks and monitoring stay with the worker.
Serialize expensive runs, integrate only after writers are idle, and do not
commit a moving shared tree.

## Authority, effort and continuation

This is the permission record. Explicit PSIV authorization persists; record a
new grant here. Another repository's grants, an available tool or a permissive
sandbox confer no project authority.

| Area | Standing record |
| --- | --- |
| Scope | Assigned PSIV tasks include necessary local edits, focused checks and scoped repairs; preserve unrelated work. |
| Local setup | Use documented prerequisites or isolated local dependencies before declaring a tool unavailable. Do not modify unrelated system configuration. |
| Git and remote writes | On 2026-10-01 the owner granted committing, pushing branches, opening PRs and issue filing; reuse it without asking again. Remote inspection is allowed. Since 2026-10-26 (owner, moving PSIV into the shared pipeline) merging is the steward's: an agent opens the PR and leaves it to the steward ([AGENTS.md](../AGENTS.md#pull-requests)), and never merges itself. |
| Cleanup | Remove only owned disposable outputs and processes and, when integration includes it, owned task branches and worktrees. Retain evidence, source saves and other people's work. No broad cleanup. |
| Deployment and publication | Releases, deployment and asset distribution have no standing grant. |
| Paid services | The review bots the steward asks on every PR (owner, 2026-10-26). Nothing else: a live batch needs a new explicit assignment. |
| Protected inputs | Owned by [AGENTS.md](../AGENTS.md#protect-local-inputs-and-evidence); no task grant relaxes them. |
| Effort | Owner, 2026-09-23: "Continue scoped repairs until it passes, with no fixed cycle limit." This applies to failures of accepted checks inside authorized scope. It does not waive source, permission, resource or input constraints, expand scope, or grant publication or spending authority. |
| Continuation | During an assigned outcome, continue ready authorized steps through its agreed endpoint without repeated approval requests. |

An observed failure leads to a scoped repair and a rerun of the affected
checks. A missing source or input, or a genuine authority boundary, gets a
specific blocker, not an invented result or blind retries. Complete independent
preparation before asking for the blocking decision. A new task cannot silently
reset a budget, weaken a gate, expand scope or grant permissions.

## Find the smallest useful verification

All commands run from the repository root. Prepare local prerequisites from
[DEVELOPMENT.md](DEVELOPMENT.md) before running asset-dependent tests.

| Change | Start with | Extend verification when needed |
| --- | --- | --- |
| ROM decoding / pack | Relevant `tests/test_*.py` file | Full Python suite and Rust pack consumers if the pack schema changes |
| Battle / field rules | Relevant `psiv-core` tests | Runtime integration, original-game comparison and native input |
| Camp / persistence | Relevant `psiv-runtime/tests/` target | Ordinary input, SAVE, fresh process, CONTINUE and state comparison |
| Godot UI / scenes | Build the extension and run the affected interaction | Inspect matching original/native captures; record viewport and region |
| Sound | Relevant `psiv-sound` tests | Register/timing comparisons and playback checks for the affected path |
| Documentation | `python3 tools/check_docs.py` and `git diff --check` | Code tests only if source also changes |

```bash
PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_psiv_tools.py'
cargo test --manifest-path rust/Cargo.toml -p psiv-runtime --lib suites::camp_order::
```

The full commands and the heavy-run lock are in the
[development guide](DEVELOPMENT.md#run-checks); oracle lanes are in
[oracle/README.md](../oracle/README.md). What each kind of evidence proves is
owned by [AGENTS.md](../AGENTS.md#verify-the-claim-you-intend-to-make).

Run the cheapest useful check during development. Add feedback only when a
concrete observation is missing, and give a new correctness check a meaningful
negative control. Repair real failures without rewriting evidence. Freeze a
reviewed candidate (a commit, or a base commit plus an exact patch) before
expensive gates; a relevant code, test or gate-input change invalidates the
affected results.

## Reproduce without damaging a save

Before a native run, identify the source commit and dirty changes, the pack
manifest hash, the source save hash, the driver and the executable. Use a
separate output directory under `build/` and a separate `PSIV_SAVE_DIR`.
Inspect driver paths and stop conditions before launch, and record any fixture
injection explicitly. Check that the intended build is the one running: a
loaded shared library can outlive a source edit. Keep a failed run's logs and
state apart from a successful save, and never repair party resources or flags
behind the driver's back and then call the run ordinary play.

## Receipt and handoff

The PR description (or the worker's report to the orchestrator) carries the
receipt. Detailed logs and ROM-derived artifacts stay local; the receipt gives
repo-relative paths and says when local evidence is absent from another
checkout.

```text
Task and intended behavior:
Source commit, branch, and relevant dirty changes:
Files changed and why:
Retail basis (symbols/offsets, references):
Environment (tool versions, executable, viewport):
Inputs (pack identity, source save hash, fixture assumptions):
Requested and actual model/effort; ownership and authority source:
Commands actually run, exit status and timing:
Results (passed / failed / skipped / not run) and raw log paths:
Evidence type (state / native input / persistence / visual comparison):
Review (self / independent, findings and corrections):
Correction-ladder rung:
Known limits, remaining failures and the issues filed for them:
Next concrete step:
Git state:
```

Omit irrelevant gameplay fields for a small docs fix. Generate revisions,
hashes, timing and statuses from actual artifacts, never from recollection.
Claim a speedup only from comparable completed measurements.

## Review, integration and procedure changes

Review both the diff and the behavior, label self-review versus independent
review, and resolve material findings before acceptance. A PR's independent
review is the steward's review bots; fix or answer every finding
([AGENTS.md](../AGENTS.md#pull-requests)). Once permitted, continue through the
PR, verification on the integrated `main` revision once the steward has merged
it, and owned cleanup; a candidate's earlier result is not a pass on `main`. If
integration was not requested, leave a verified local result and report that
boundary.

Improve this procedure from observed problems the same way code is corrected:
climb the [correction ladder](#correction-ladder) and record the change in its
PR. Do not weaken checks or claim automatic enforcement without a working
executor.

## Correction ladder

When a defect, review finding or correction exposes a recurring class of
mistake, fix the class, not the instance, at the strongest rung that applies:

1. **Make it unrepresentable.** Types, exhaustive matches without catch-all
   arms, one owning API or module, a data structure that cannot hold the bad
   state. The pack's explicit rebuild error for stale schemas is an example.
2. **Enforce it statically.** Compiler or lint settings (the crates'
   `forbid(unsafe_code)` and `deny(clippy::float_arithmetic)`), strict Clippy,
   or a guard test or script that the [gate](DEVELOPMENT.md#gate-and-coverage)
   runs.
3. **Guidance.** `AGENTS.md`, module docs or the routing docs, placed where the
   next contributor will read it before repeating the mistake.
4. **Review only.** A last resort and a known gap: file an issue for the guard
   that would close it.

The PR description states which rung the fix used and, below rung 1, why no
stronger rung applied. A rung-3 or rung-4 fix for a mistake that has already
recurred needs a linked issue for the stronger rung.

## The codebase is memory

Contributors and agents extend whatever patterns they read, so the tree must
only contain patterns worth copying.

- One paved path per concern: one owning module or API, reached through the
  layer that owns it in the [runtime design](RUNTIME_DESIGN.md). A second
  implementation of the same concern is a defect, even when both agree today.
- No workarounds, duplicate code paths, or comments that justify a deviation.
  A known gap is a GitHub issue link. Comments state contracts and invariants;
  retail deviations are recorded in [the source notes](source-notes/README.md).
- An anti-pattern that cannot be removed in the current change is first stopped
  from spreading: add a guard (lint setting or guard test, with a baseline of
  the existing instances if needed) that fails on new ones, then file the
  cleanup as an issue.

## Instruction loading

`AGENTS.md` is the only always-loaded project instruction file. It holds policy
and pointers; detailed knowledge lives in owned documents that it and the
[documentation index](README.md) route to, and linked documents are not loaded
automatically. A host that reads a different entry file gets a tracked file that
only imports `AGENTS.md`; host-local routing and preferences live in untracked
files (see [roles and delegation](#roles-and-delegation)). `AGENTS.override.md`,
where a host supports it, replaces `AGENTS.md` at its directory level rather
than extending it. Verify the effective chain in a fresh session when
supported.
