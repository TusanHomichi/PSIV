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

`ds-lane` is on `PATH` through `~/.local/bin`. `tools/ds-lane` is the
executable entry point (a shim that resolves its own symlink) for the
stdlib-only `tools/ds_lane/` package (`config`, `preflight`, `receipts`,
`lanes`, `supervisor`, `cli`). Usage: `ds-lane --help`, or the `cli` module
docstring.

- Launch with `ds-lane start BRIEF.md` in a background shell; the host wakes
  the orchestrator on exit. Runs use a detached supervisor, so the worker and
  its commit survive a reaped shell; `ds-lane wait ID` reattaches. Send
  repairs with `ds-lane resume ID FOLLOWUP.md`, which continues the same
  session. Peek at a run in flight with `ds-lane tail ID [-n N]` (state,
  elapsed, recent tool calls). End a run early with `ds-lane stop ID`: it
  SIGTERMs the supervisor recorded in the run directory (PIDs only, never a
  command-line pattern), which kills the worker's process group and finalizes
  the run — committing what the worker left, with `exit_code` 143 and
  `turn_error` `stopped by orchestrator`. A run still queued stops with
  nothing run. `stop` waits up to 60 s, prints the summary, and exits 0 when
  the run is stopped or idle (1 if it is still finalizing). Clean up with
  `ds-lane rm ID`; receipts stay unless `--purge`.
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
  `trajectory.jsonl`, `run.json`), then merge or cherry-pick. `summary.txt`
  ends with the worker's `## Receipt` block.
- Ignored local inputs (`runtime-pack`, `reference`, `generated`, ...) can be
  symlinked in with `--link PATH` (or `--link PATH=SOURCE`). The worker can
  read them but not write them, so a lane that regenerates packs must write
  inside its worktree (for example with `PSIV_RUNTIME_PACK` pointing there).
- **Brief phrasing law.** Reasonix parses the prompt for constraints.
  Negated-mutation wording outside code fences ("do not edit", "no changes",
  "read-only", ...) bans every write for the whole session. State file
  ownership positively ("touch only X"), and keep quoted wording inside a
  fenced block or backticks. `start` and `resume` now preflight for it and
  refuse to launch, naming the offending lines; `--read-only` (investigation
  lanes) and `--allow-phrasing` skip that check. Runs that still hit the ban
  are reported in the summary.
- **Write set.** Give each brief one fenced block whose info string is
  `write-set`, listing one path or glob per line (`fnmatch`, `**` crosses
  directories). `ds-lane` records paths changed between the base and the new
  head that fall outside it as `write_set_violations` in `run.json` and warns
  in the summary. A brief without the block is unenforced. A follow-up passed
  to `resume` may carry its own block: `resume` adopts it as the lane's write
  set from that run on (the decision after lane ab-H run-3 was checked against
  the brief's set, not its follow-up's, and flagged both new paths), so
  `lane.json` always holds the current set and every `run.json` the `write_set`
  that run was checked against. A follow-up with no block inherits.
- **Host state.** The worker's own agent runtime writes its task state into the
  worktree (`.reasonix/tasks/<id>/events.jsonl`); that is host state, not lane
  output. The finalize commit excludes every path in `HOST_STATE_PATHS`
  (`tools/ds_lane/config.py`) alongside the `--link`ed inputs, so it never
  reaches a lane commit or the write-set check - the plain `git add -A` swept
  it into lane ab-J's commit and tripped that check (2026-09-24) - and each
  run's receipt copies what it held to `host-state/<name>/`, reviewable like
  `evidence/`.
- **Timeout.** Each run gets `--timeout SECONDS` (default 5400). On expiry the
  worker's process group is SIGTERM'd, then SIGKILL'd after 30 s; the run
  still commits and reports `exit_code` 124 with `turn_error`
  `timeout after N s`. A final SIGKILL always follows the parent's exit, so
  children the worker spawned (a `cargo test`, say) cannot outlive the run and
  its released slot. `run.json` records `outcome`:
  `completed` | `timeout` | `stopped` | `stalled`.
- **Stall watchdog.** Each run gets `--stall-timeout SECONDS` (default 900; 0
  disables it; in the run spec). The supervisor samples every 15 s and judges
  each whole window: the run is **stalled** when the window saw no
  `trajectory.jsonl` growth *and* the worker's process group used less than
  `DS_LANE_STALL_CPU_PCT` percent of one core across it (default 1.0;
  `utime+stime` deltas from `/proc/<pid>/stat` over `SC_CLK_TCK`). The rate,
  not any gain, is what separates work from a corpse: a long silent
  `cargo build` burns far more than a percent of a core and a live Reasonix
  worker streaming a turn measured 5.6%, while a worker whose stream died and
  whose wrapper then idles on a timer gains a tick every half minute - 0.03%
  (measured 2026-09-24). A stalled run is killed through the same group path
  and finalizes with `exit_code` 125 and `turn_error`
  `stalled: no progress for N s`; the measured rate and the threshold it fell
  under are recorded in that run's `supervisor.log`. `--stall-retries N`
  (default 1) then has the supervisor start the next run of the lane itself,
  on the same Reasonix session, through the `resume` code path: that follow-up
  says the previous
  run stalled (a host suspend leaves the worker alive with a dead stream -
  observed 2026-09-24), that the session context is intact, and to continue
  through acceptance and the Receipt; it keeps the timeout, the stall timeout
  and the retries left. `stall_retries_left` and `resumed_after_stall` are in
  the run spec and in `run.json`, and `wait ID` follows the chain to the
  lane's final run, so a waiter that attached before the stall still returns
  the final outcome. Stopping the resumed run is an ordinary `stop ID`.
- **Independent checks.** `ds-lane verify ID -- CMD...` runs CMD in the lane
  worktree with `CARGO_BUILD_JOBS=2`, streams its output, saves it with the
  command, UTC start, lane head, exit code and duration under
  `<state>/verify/NNN.log`, and exits with CMD's code. Use it for the
  orchestrator's own checks instead of "the worker says it passed".
- `DS_LANE_HOME` relocates worktrees/receipts (tests), `DS_LANE_REASONIX`
  swaps the worker binary, `DS_LANE_MAX_LANES` caps concurrency,
  `DS_LANE_STALL_POLL` shortens the 15 s stall sampling interval and
  `DS_LANE_STALL_CPU_PCT` moves the work threshold (both are test seams; the
  defaults are 15 s and 1.0% of a core).
  `PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v`
  covers the harness hermetically in under a minute (its stall cases run with a
  lowered poll; the suite must stay under 90 s). The cases are split by
  cohesion - `tests/ds_lane_support.py` (the fake worker, the repo and home
  fixtures, the CLI helpers), `test_ds_lane_unit.py`, `test_ds_lane_lanes.py`
  and `test_ds_lane_supervisor.py` - each under the line cap that applies to it
  (500 lines for a test module, 400 for a package module).
- A lane that builds the whole workspace needs `--link oracle/gpgx-src`:
  `psiv-sound`'s build script compiles the ignored core sources under it.
- Each lane has its own `rust/target`, so its first cargo build is cold.
  Restate the relevant `AGENTS.md` safety rules in each brief (GDExtension,
  saves, serialized expensive runs).
- Other agents may commit in the main tree. Keep uncommitted Claude-side
  edits short-lived and scoped, and check `git log` before committing.
