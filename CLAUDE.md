# Claude Code in PSIV

Read [AGENTS.md](AGENTS.md) first; every rule there applies. This file adds
only what is specific to the Claude Code host. Codex does not read it.

## Routing under Claude Code

Owner decision, 2026-09-23. In Claude Code sessions, the `AGENTS.md` model
roles map as follows:

- Orchestration (Astra's role): `claude-opus-5-5`. Scoping, briefs,
  architecture and permission calls, review, integration and talking with the
  owner.
- Delivery (Sol/Luna roles): DeepSeek `deepseek-flash` (API id
  `deepseek-v4-flash`) at `max` reasoning, run through
  [`tools/ds-lane`](tools/ds-lane) (Reasonix, one git worktree per lane).

The GPT-6 roster in `AGENTS.md` is unchanged for the Codex host. Record lane
outcomes in the [lane ledger](docs/CLAUDE_LANES.md).

## ds-lane

`ds-lane` is on `PATH` through `~/.local/bin`. Usage is in the script header.

- Launch with `ds-lane start BRIEF.md` in a background shell; the host wakes
  the orchestrator on exit. Runs use a detached supervisor, so the worker and
  its commit survive a reaped shell; `ds-lane wait ID` reattaches. Send
  repairs with `ds-lane resume ID FOLLOWUP.md`, which continues the same
  session. Clean up with `ds-lane rm ID`; receipts stay unless `--purge`.
- Model and effort are pinned: `--effort max` for the worker and
  `subagent_effort = "max"` in `~/.reasonix/config.toml`. Take the route
  from `run.json` or the trajectory. The model's self-description is not
  evidence; it has claimed to be Claude.
- At most three workers run machine-wide; later runs queue. Each gets
  `CARGO_BUILD_JOBS=2`. This follows a 2026-08-16 OOM on this 13 GB machine.
- A lane starts from a committed ref (`--base`, default `HEAD`). Uncommitted
  work in the main tree is invisible to it.
- The Reasonix OS sandbox confines writes to the worktree and makes `.git`
  read-only. `ds-lane` commits each run on `ds/<id>`. Review the receipts in
  `~/.local/state/ds-lane/PSIV/<id>/run-N/` (`result.md`, `diff.patch`,
  `trajectory.jsonl`, `run.json`), then merge or cherry-pick.
- Ignored local inputs (`runtime-pack`, `reference`, `generated`, ...) can be
  symlinked in with `--link PATH`. The worker can read them but not write
  them, so a lane that regenerates packs must write inside its worktree (for
  example with `PSIV_RUNTIME_PACK` pointing there).
- **Brief phrasing law.** Reasonix parses the prompt for constraints.
  Negated-mutation wording outside code fences ("do not edit", "no changes",
  "read-only", ...) bans every write for the whole session. State file
  ownership positively ("touch only X"). Use `--read-only` for investigation
  lanes. `ds-lane` warns when a run hits the ban.
- Each lane has its own `rust/target`, so its first cargo build is cold.
  Restate the relevant `AGENTS.md` safety rules in each brief (GDExtension,
  saves, serialized expensive runs).
- Other agents may commit in the main tree. Keep uncommitted Claude-side
  edits short-lived and scoped, and check `git log` before committing.
