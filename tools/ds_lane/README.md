# ds-lane

`ds-lane` runs delivery lanes: one worker per git worktree, under a detached
supervisor that commits each run and writes reviewable receipts. It drives the
Reasonix CLI with DeepSeek `deepseek-flash` (API id `deepseek-v4-flash`). Which
host uses it, and for which role, is host-local configuration; outcomes are
recorded in the [lane ledger](../../docs/records/CLAUDE_LANES.md).

## Reference

`ds-lane` is on `PATH` through `~/.local/bin`. `tools/ds-lane` is the
executable entry point (a shim that resolves its own symlink) for the
stdlib-only `tools/ds_lane/` package (`config`, `preflight`, `trajectory`,
`receipts`, `evidence`, `compaction`, `report`, `lanes`, `verify`,
`supervisor`, `cli`). Usage: `ds-lane --help`, or the `cli` module docstring.

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
- **The worker's environment is an allowlist.** The supervisor starts the
  worker with `worker_env()` (`config.py`): `WORKER_ENV_ALLOW` (path, home,
  user, locale, temp and XDG directories, cargo/rustup), the `LC_*` variables,
  a capped `CARGO_BUILD_JOBS`, and any name listed in
  `DS_LANE_WORKER_ENV_PASS` - nothing else. A copy of the caller's environment
  sent two GitHub tokens to the model provider when lane `re-B-tools` printed
  its environment while debugging (2026-09-25); it also exposed
  `SSH_AUTH_SOCK`, the display and bus sockets and every other exported
  secret. Reasonix reads its own API key from its config file. The sandbox
  still confines only writes, so files a worker can read (credentials under
  `HOME`) remain a known gap, tracked in its own issue.
- **The prompt travels on stdin.** The supervisor writes the run's `prompt.md`
  to the worker's stdin and puts nothing of it on the command line: a brief in
  argv is text the worker's own process matching can hit, and lane
  `sw-S1-motavia` killed its own worker with a `pkill -f` pattern that sat in
  its brief (2026-09-24). `command.json` records `<stdin: prompt.md>` in the
  prompt's place, the run's `prompt.md` is what the worker read byte for byte,
  and the worker preamble states the rule in positive words (stop a process
  only by the PID you recorded or the job id your tools returned).
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
- **Exclusions name only paths git does not already ignore.** A path the
  worktree's rules cover needs no `:(exclude)` pathspec (`git add -A` skips it
  by itself), and naming one makes git refuse the whole add with "The following
  paths are ignored by one of your .gitignore files": that is how lane
  `or-O1`'s finalize died on a linked ROM matching `Phantasy Star IV*.md`. A
  file link's own name is what such a pattern matches, while a directory link
  is a symlink (mode 120000), which a `dir/` rule does not match - so the
  directory link still needs its exclusion. Each path is tested with
  `git check-ignore -q --no-index` before it is named. A git step that fails
  anyway is recorded as `finalize_error` in `run.json` and printed in the
  summary, with the worker's exit code, result and receipt preserved and its
  files left in the worktree, instead of a bare supervisor error (2026-09-24).
- **Size rule.** `ds-lane` flags the project's
  [file-size rule](../../docs/DEVELOPMENT.md#file-size) the way it flags a
  write-set violation. After each run, every path changed between the lane's
  base and the new head that still exists and is text (no NUL byte in its
  first 8 KiB) is counted at the new head; each one over `MAX_FILE_LINES`
  (`tools/ds_lane/config.py`, default 1,000, movable with
  `DS_LANE_MAX_FILE_LINES`) is recorded in `run.json` as `oversize_files`
  (`{path, lines, base_lines}`, where `base_lines` is the count at the base and
  null for a file that is new there) and printed as
  `WARNING: over 1000 lines: <path> (<lines>, was <base_lines>)` (`was new` for
  a new file). Every path in a commit is counted, whatever the brief's write
  set says; the `--link`ed inputs and the worker's host state never reach a
  commit, so they are never counted. **Generated and data files are exempt**
  (owner decision 2026-09-24): the rule is for source text a worker edits,
  while a manifest or a replay transcript grows with its content, so paths
  matching `DEFAULT_SIZE_EXEMPT` in `config.py` (`*.json`, `*.tsv`, `*.csv`,
  `*.lock`, `**/replay_fixtures/**`) are skipped, and `DS_LANE_SIZE_EXEMPT`
  replaces that list with a comma-separated one (an empty value exempts
  nothing). The list a run used is recorded as `size_exempt` in its `run.json`,
  beside `max_file_lines`. The preamble carries the rule to the worker in
  positive words (keep every file you touch under 1,000 lines; reorganize a
  file into cohesive modules when a change would take it over). Why it exists:
  lane ab-M grew `rust/psiv-core/src/battle/enemy_damage_tests.rs` to 1,504
  lines and review missed it, because nothing flagged it (2026-09-24).
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
- **Run numbering and crashed runs.** A run's number is one more than the
  highest `run-N` directory in the lane's receipts, not the length of
  `lane.json`'s `runs`: a run whose supervisor died before it recorded itself
  leaves its directory behind with no entry there, and counting from the
  record made `resume` try to create that same directory again
  (`FileExistsError`), which left the lane stuck until an orchestrator cleared
  it by hand (2026-09-24). A supervisor that raises - inside the run, or inside
  finalize before `run.json` landed - is recorded in `lane.json` anyway, with
  the `error`, the session id read from its trajectory, and its turn error,
  exit code and cost when the worker's result arrived. Such a run gets
  `outcome` `error`, keeps no `run.json` (that file's presence is what marks a
  run complete) and is marked `failed`, so `wait ID` reports it as failed and
  prints the error. `resume ID` continues the session that record holds, and
  falls back to the newest leftover run's trajectory when a supervisor was
  killed outright - a host crash, an OOM kill - and could record nothing at
  all.
- **Deduplication and compression of receipts.** Every run's receipt keeps its
  own copy of `build/lane-evidence/`, so a lane's receipts repeat each other
  and the raw captures in them are large repetitive text: one sweep lane's
  receipts reached 8.7 GB, most of it a later run's re-copy of an earlier one's
  (owner decision 2026-09-24: deduplicate and compress; pruning is a later
  lane). The finalize step therefore compacts the run it has just finished,
  after that run's record has landed and its machine-wide slot has been
  released, so a pass that takes minutes never delays another lane's start. Two
  passes, oldest run first: a file byte-identical to one an earlier run of the
  same lane keeps becomes a hardlink to it (same storage, every path readable
  exactly as before; a link that fails - across filesystems, say - falls back
  to a copy), and a file at or over `DS_LANE_COMPRESS_MIN_BYTES` (default
  1 MiB) whose extension is in `DS_LANE_COMPRESS_EXTS` (default csv, log,
  jsonl, txt, tsv) is stored as `name.ext.xz` with lzma preset 6 and the
  original removed. Files already compressed, binaries, files under the
  threshold and files another run links to are left alone; a file an earlier
  run already keeps compressed is hardlinked rather than compressed again,
  nothing is compressed that a link can replace, and an interrupted pass leaves
  the original, the finished `.xz` or both - never neither. Each run's
  `evidence/INDEX.json` records every file's path, the size and sha256 of its
  content as captured, whether it is stored `plain`, `xz` or as a `hardlink` to
  an earlier run (and where that storage comes from), so a decompressed file
  can be checked against it: `xz -dc name.ext.xz`, or Python's `lzma.open`. A
  failure is in the run's `compaction_error`, and never disturbs the rest of
  its receipt. `ds-lane compact ID` applies the same passes to a lane's
  existing receipts, including lanes finalized before this existed, oldest run
  first; it is idempotent, refuses a lane with a run in flight, and `--dry-run`
  reports what it would do and the bytes it would save - it streams each
  candidate through the compressor to measure it, so on a lane never compacted
  it costs the CPU of a real pass. There is deliberately no `--all`: the
  orchestrator decides which lanes are finished with their evidence. The
  modules are `evidence.py` (bytes, index, scratch names), `compaction.py` (the
  pass) and `report.py` (the lines it prints and the block it records).
- **Independent checks.** `ds-lane verify ID -- CMD...` runs CMD in the lane
  worktree with `CARGO_BUILD_JOBS=2`, streams its output, saves it with the
  command, UTC start, lane head, exit code and duration under
  `<state>/verify/NNN.log`, and exits with CMD's code. Use it for the
  orchestrator's own checks instead of "the worker says it passed".
- `DS_LANE_HOME` relocates worktrees/receipts (tests), `DS_LANE_REASONIX`
  swaps the worker binary, `DS_LANE_MAX_LANES` caps concurrency,
  `DS_LANE_STALL_POLL` shortens the 15 s stall sampling interval,
  `DS_LANE_STALL_CPU_PCT` moves the work threshold (both are test seams; the
  defaults are 15 s and 1.0% of a core), `DS_LANE_MAX_FILE_LINES` moves the
  1,000-line limit and `DS_LANE_SIZE_EXEMPT` replaces the globs that limit
  skips (both are test seams too; the defaults are 1,000 lines and the
  data-file list above), and `DS_LANE_COMPRESS_MIN_BYTES` /
  `DS_LANE_COMPRESS_EXTS` move what a receipt's pass compresses (test seams as
  well; the defaults are 1 MiB and the five extensions above).
  `PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v`
  covers the harness hermetically in under a minute (its stall cases run with a
  lowered poll, its size cases against the real limit and its compaction cases
  against the real 1 MiB threshold; the suite must stay under 90 s). The cases
  are split by cohesion - `tests/ds_lane_support.py` (the fake worker, the repo
  and home fixtures, the CLI helpers), `test_ds_lane_unit.py`,
  `test_ds_lane_lanes.py`, `test_ds_lane_size.py`,
  `test_ds_lane_compaction.py`, `test_ds_lane_finalize.py`,
  `test_ds_lane_crash.py` and `test_ds_lane_supervisor.py` - each under the
  line cap that applies to it (500 lines for a test module, 400 for a package
  module).
- A lane that builds the whole workspace needs `--link oracle/gpgx-src`:
  `psiv-sound`'s build script compiles the ignored core sources under it.
- Each lane has its own `rust/target`, so its first cargo build is cold.
  Restate the relevant `AGENTS.md` safety rules in each brief (GDExtension,
  saves, serialized expensive runs).
- Other agents may commit in the main tree. Keep uncommitted Claude-side
  edits short-lived and scoped, and check `git log` before committing.
