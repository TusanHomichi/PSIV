# Workflow setup ledger

Date: 2026-09-23. Scope: documentation and instruction setup only.
Base: `e03700be2d71fcb298a3a877c3ebdf1bfcf85aad` on `main`.

## Outcome and acceptance

Adopt the owner's evidence-driven workflow using PSIV's existing instructions,
README, roadmap and subsystem-ledger conventions. Preserve retail rules,
protected inputs, serial heavy checks and distinct evidence types. Keep the
three requested GPT roles explicit, record actual authority/effort, and walk
setup through the canonical graph. No gameplay change, campaign run, framework,
CI, release or remote write is included.

Acceptance, stated before editing: concise standing instructions; one workflow
and graph; a current-state entry point; correct local links and command targets;
`git diff --check`; fresh-session instruction verification when supported;
reviewed diff and a read-back of actual receipts and archived graph.

## Inspection

- Clean `main` at the base revision; one worktree, no PSIV Godot/build/oracle
  process. The active Codex process belongs to this session; unrelated host
  processes were left alone.
- Applicable files: `~/.codex/AGENTS.md` and repository `AGENTS.md`. No applicable
  `AGENTS.override.md` or nested documentation instructions were found; no
  configured fallback filenames or nondefault instruction byte limit.
- Read the overview, roadmap, workflow, development guide, documentation index,
  runtime architecture and relevant BioPlant/sound checkpoint sections.
- `gh repo view`, `gh issue list`, `gh pr list` and `gh workflow list --all`
  succeeded: default branch `main`, issues enabled, no open issues, no PRs in
  the returned all-state list, no hosted workflows. No tracked `.github/`
  templates or CI configuration. The existing roadmap/handoff is reused.
- Live parent turn metadata reports `gpt-6-astra` / `max`; CLI version
  `codex-cli 0.156.1`. The local host catalog advertises Astra, Sol and Luna with
  `max`. A catalog entry alone is not an execution test.
- Permissions and the owner's explicit no-fixed-cycle-limit repair reply are
  recorded in [the workflow](AGENT_WORKFLOW.md#authority-effort-and-continuation).
  No other repository's permissions were imported.

## Changes and review

The existing short handoff guide lacked explicit model roles, permission/effort
records and dependent-task ownership. The smallest project-local adaptation was:

| File | Change |
| --- | --- |
| [AGENTS.md](../AGENTS.md) | Durable roles, boundaries, checks and completion; route detailed architecture/procedure reads |
| [Agent workflow](AGENT_WORKFLOW.md) | Working loop, authority/effort, graph fields, evidence and instruction-loading procedure |
| [Project README](../README.md) | Concise current-state/next-gate pointers; preserve historical gameplay context |
| [Roadmap](ROADMAP.md#task-graph-handoff) | Single active-graph location and completed-setup archive pointer |
| [Documentation index](README.md) | Route readers to the workflow, canonical handoff and this ledger |
| This ledger | Actual setup graph, checks, corrections and limits |

Primary author/integrator: `gpt-6-astra` / `max`, verified in the active turn
metadata. The parent handled this bounded documentation integration directly.
Parent diff review is **self-review**; the separate fresh `gpt-6-luna` / `max`
session performed a bounded read-only instruction/documentation check, not a
full independent architecture or gameplay review. No material policy conflict,
lost PSIV boundary or incorrect local command/path was found.

The bounded check flagged two changed-file hashes newer than the initial receipt:
the roadmap's node states and this ledger's progress entry had changed. Correction:
retain the initial receipt as historical and generate final exact-file acceptance
after archive/current-state edits. This exercises the invalidation rule without
rewriting a prior result. The parent also checks this new untracked ledger
separately for whitespace; ordinary `git diff --check` does not include it.

## Verification

Initial candidate check: `python3 build/workflow-setup-20260923/record_checks.py candidate`
passed. The local receipt contains the exact base, changed-file and patch hashes,
commands, UTC timestamps, elapsed time, exit status and raw log hashes. The
six-document link check resolved 68 local links/anchors; its missing-file and
missing-anchor negative controls were rejected. Four full-gate command strings
and four example targets were checked against the development guide/files.
`git diff --check` passed. These are documentation checks, not command execution
of the referenced gameplay suites. The candidate receipt's SHA-256 is
`f56507e5674581563bf37c8b8e64ebbecd97863cfa196d411dc5634e9dd31f44`.

Fresh CLI check: `python3 build/workflow-setup-20260923/run_fresh.py` launched
`codex exec --model gpt-6-luna -c 'model_reasoning_effort="max"' --sandbox read-only`
in this repository, with the complete command/prompt retained in the local
receipt. It exited 0 after 458.726490 seconds. Its checks independently reran
the 68-link/command check, `git diff --check` and a six-document whitespace scan.
The session's `turn.completed` event was read after exit.

Parent inspection of that fresh session's actual startup transcript and
`turn_context` confirmed `gpt-6-luna` / `max` and exact inclusion of both the
global and updated project instruction texts. The child correctly identified
the roles, repair waiver, save protection, serial Rust command and graph/workflow
pointers from its startup context. This verifies the local CLI instruction chain;
it does not certify fresh loading in an IDE or desktop app.

| Requested route | Observed in this setup |
| --- | --- |
| Astra / max | Parent live turn metadata; authored, reviewed and integrated docs |
| Sol / max | Advertised by the host catalog; not invoked because no complex implementation was needed |
| Luna / max | Fresh CLI turn metadata and successful bounded check |
| DeepSeek | Not invoked; no routing or reliability claim |

Final documentation gate, after all archive edits:

```bash
python3 build/workflow-setup-20260923/record_checks.py final
```

The runner records local-link/anchor checks, the two negative controls, command
reference checks, `git diff --check`, a separate new-ledger whitespace check,
and final status. The new-file command is
`git diff --no-index --check /dev/null docs/WORKFLOW_SETUP.md`: empty output and
exit 1 are expected for a whitespace-clean added-file difference, not a failed
gate. The final receipt records each actual/expected exit code. All documentation
checks passed; final file/log hashes were read back against their artifacts.

Local evidence under `build/workflow-setup-20260923/`:

- `candidate-receipt.json`, candidate patch and logs: initial WF-02 acceptance.
- `review-candidate.json`, `self-review.json`: exact reviewed files and parent findings.
- `fresh-prompt.txt`, `fresh-events.jsonl`, `fresh-instructions.md`,
  `fresh-receipt.json`: delegated input, raw work, report and timing/exit receipt.
- `fresh-routing-and-loading.json`: actual route and exact startup-text checks.
- `fresh-stderr.log`: preserved non-gating RMCP startup transport failure,
  HTTP 526. It did not prevent local reads or CLI completion; unrelated MCP
  service health was not repaired or certified.
- `final-receipt.json`, final patch and logs: final base, all six file hashes,
  checker/runner hashes, UTC timing, exit statuses and log hashes.
- `readback.json`: final archive, hash and owned-process verification.

These are ignored local artifacts, not files shipped in Git. The two small
Python helpers are one-off documentation-check receipts, not a project agent
framework or a new required CI dependency. Future docs changes may use an
equivalent local-link/command review plus the documented whitespace gate.

Runtime, native-input, persistence, visual and hosted checks are not run for
this documentation-only change. Historical gameplay receipts remain attached
to their original revisions.

## Setup graph

Archived after verified setup. This is the sole retained graph record; the
roadmap links here and contains no active duplicate. WF-01's inspection unlocked
WF-02; its initial docs checks unlocked WF-03; the fresh check, parent review,
final documentation gates and archive read-back closed WF-03. No gameplay node
was opened.

```yaml
outcome: "Adopt PSIV's evidence-driven workflow and verify a real setup task"
canonical_record: "docs/WORKFLOW_SETUP.md#setup-graph"
archive_state: "completed"
base_revision: "e03700be2d71fcb298a3a877c3ebdf1bfcf85aad"
scope: "Documentation only; no campaign run, commit or remote write"
effort_policy: "Owner 2026-09-23: scoped repairs until acceptance passes; no fixed cycle limit"
next_action: "When BioPlant continuation is assigned, re-anchor the healthy A7 save, pack and driver"
nodes:
  - id: WF-01
    outcome: "Establish repository conventions, authority and observable setup acceptance"
    depends_on: []
    owner: "gpt-6-astra / max"
    inputs: ["owner setup request and repair-budget reply", "AGENTS.md", "docs/DEVELOPMENT.md"]
    acceptance: "Inspect instructions, clean base, relevant architecture, tracker/CI and live model metadata"
    state: verified
    evidence: ["docs/WORKFLOW_SETUP.md#inspection"]
    effort_policy: inherited
  - id: WF-02
    outcome: "Implement the concise instructions, workflow, current-state pointers and graph"
    depends_on: [WF-01]
    owner: "gpt-6-astra / max (parent authors this bounded documentation integration)"
    inputs: ["WF-01", "docs/AGENT_WORKFLOW.md", "README.md", "docs/README.md"]
    acceptance: "Changed-document links and commands resolve; git diff --check passes; protected rules retained"
    state: verified
    evidence: ["build/workflow-setup-20260923/candidate-receipt.json", "docs/WORKFLOW_SETUP.md#verification", "build/workflow-setup-20260923/final-receipt.json"]
    effort_policy: inherited
  - id: WF-03
    outcome: "Verify instruction loading, review the candidate and archive the completed graph"
    depends_on: [WF-02]
    owner: "gpt-6-astra / max; fresh bounded verification by gpt-6-luna / max"
    inputs: ["WF-02 candidate and raw checks", "docs/WORKFLOW_SETUP.md"]
    acceptance: "Fresh-session instruction check, parent diff review, final docs gates and archive read-back recorded"
    state: verified
    evidence: ["build/workflow-setup-20260923/fresh-receipt.json", "build/workflow-setup-20260923/fresh-routing-and-loading.json", "build/workflow-setup-20260923/final-receipt.json", "build/workflow-setup-20260923/readback.json"]
    effort_policy: inherited
```

## Limits and next action

At the original local-setup handoff, HEAD remained the base revision: five
tracked documents were modified and this new ledger was untracked. No task
branch/worktree or game/build process had been created; the owned CLI child
exited, and local receipts were retained. No commit, push, PR, merge or campaign
run was included in that original setup acceptance. The later publication
authorization below does not rewrite the archived graph's scope or receipts.

Sol execution, DeepSeek and fresh IDE/app instruction loading remain untested.
No full gameplay/native/persistence/visual or hosted acceptance was refreshed.
No automatic dispatcher or enforcement mechanism was implemented.

## Publication follow-through

After accepting the local setup, the owner authorized: "create a pr for the new
docs / merge / clean up as needed" on 2026-09-23. This covers the six workflow
documents only. Campaign investigation and implementation remain a separate
assignment. Parent route: `gpt-6-astra` / `max`.

Short plan and acceptance:

1. Record the new authority, review and check exactly the six docs, and commit
   them on an owned documentation branch with all writers idle.
2. Publish the PR, inspect its actual diff/check status, and merge the verified
   head. Do not claim hosted checks where none are configured.
3. Fast-forward local `main` to the merged revision, rerun the documentation
   checks against that exact tree, read back receipts, and remove only the
   owned task branch. Retain local evidence and unrelated work.

The PR and its merge metadata are the canonical publication record. Local
candidate and exact-main receipts are retained under
`build/workflow-setup-20260923/`; the original `final-receipt.json` remains the
historical local-setup result. A new receipt identifies each publication stage.
This section records the authorized procedure, not a preclaimed merge or CI pass.

**Next action after publication:** re-anchor the healthy `$A7` source save, pack
and driver against [the BioPlant ledger](BIOPLANT_NATIVE.md), then open its
single active graph in the roadmap before running the assigned connected route.
