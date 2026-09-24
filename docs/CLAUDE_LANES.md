# Claude Code lane ledger

Adoption record and reliability log for the Claude Code routing in
[CLAUDE.md](../CLAUDE.md): Opus orchestrates, and DeepSeek `deepseek-flash`
at max reasoning delivers through `tools/ds-lane`.

## Adoption

Owner instruction, 2026-09-23. The owner asked for the route to stay
confined to Claude Code sessions. It first landed in the shared
`AGENTS.md`/workflow docs (swept into PR #2 while uncommitted) and was moved
here and to `CLAUDE.md` on the same day.

Route choice: Reasonix was chosen over Claude Code pointed at DeepSeek's
Anthropic-compatible endpoint. Reasonix validates the effort level
(`disabled|low|high|max` for flash; other values are rejected), provides an
OS sandbox, and reports cost and trajectories. How that endpoint maps Claude
Code's thinking settings to DeepSeek effort was not established, and that
route was not tested.

## Harness verification

2026-09-23, Reasonix v1.38.2. Smoke lanes were removed afterward.

- Headless `reasonix run --effort max` succeeded, and reasoning tokens were
  recorded.
- Sandbox: `write_file` and bash writes outside the worktree and into a
  `--link`ed input were blocked. `git add`/`commit` inside the worktree failed
  on the read-only `.git`. `cargo check -p psiv-core` ran inside the lane.
- A lane committed only its owned file and excluded the link. `resume`
  continued the original session with its context. `--read-only` produced
  constraint blocks and no diff.
- Four simultaneous lanes: three ran and one queued, then ran once a slot
  freed. That lane's waiting foreground process was killed while queued; the
  detached supervisor still ran it and committed. With `CARGO_BUILD_JOBS=8`
  in the caller, the worker saw `2`.
- Harness defects found and fixed:
  - The preamble's negated wording tripped the Reasonix constraint ban.
  - A symlink to an ignored directory was committed; a `dir/` ignore rule
    does not match a symlink.
  - `reasonix run` omits `session_id` from its JSON result.
  - Block counting matched the worker's own prose.
- At max effort, a one-line edit used 10 tool rounds; the cost was
  USD 0.005.

## Reliability log

One row per lane. Include the orchestrator's corrections and any rejected
output.

| Date | Lane | Task | Outcome | Orchestrator corrections | Cost |
| --- | --- | --- | --- | --- | --- |

**Next action:** a bake-off of three to five real PSIV tasks of mixed
difficulty, each logged above, before routing complex retail-parity work to
DeepSeek by default.
