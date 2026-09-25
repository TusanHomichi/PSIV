# ds-lane

`ds-lane` runs delivery lanes: one worker per git worktree, under a detached
supervisor that commits each run and writes reviewable receipts. It drives the
Reasonix CLI with DeepSeek `deepseek-flash` (API id `deepseek-v4-flash`). Which
host uses it, and for which role, is host-local configuration; outcomes are
recorded in the [lane ledger](../../docs/records/CLAUDE_LANES.md).

## Reference

`ds-lane` is on `PATH` through `~/.local/bin`. `tools/ds-lane` is the
executable entry point (a shim that resolves its own symlink) for the
stdlib-only `tools/ds_lane/` package (`config`, `confine`, `preflight`,
`trajectory`, `receipts`, `evidence`, `compaction`, `report`, `lanes`,
`verify`, `supervisor`, `cli`). Usage: `ds-lane --help`, or the `cli` module
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
- **The worker's environment is an allowlist.** The supervisor starts the
  worker with `worker_env()` (`config.py`): `WORKER_ENV_ALLOW` (path, home,
  user, locale, temp and XDG directories, cargo/rustup/nvm), the `LC_*`
  variables,
  a capped `CARGO_BUILD_JOBS`, and any name listed in
  `DS_LANE_WORKER_ENV_PASS` - nothing else. A copy of the caller's environment
  sent two GitHub tokens to the model provider when lane `re-B-tools` printed
  its environment while debugging (2026-09-25); it also exposed
  `SSH_AUTH_SOCK`, the display and bus sockets and every other exported
  secret. Reasonix reads its own API key from its config file.
- **The worker's filesystem is an allowlist too** (the read boundary below).
  The environment was only half of it: a worker could still read any file under
  the owner's home - `~/.ssh/id_*`, `~/.config/gh/hosts.yml`, another project's
  session - and what it reads reaches the provider through its trajectory. Lane
  `h1-confine` closed that with bubblewrap; issue #26 was the record.
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
- **Investigation briefs bound what may be inspected.** A brief that asks a
  worker to find out how a tool behaves names the method: list names, check
  existence, run with synthetic inputs. It never leaves the worker free to open
  a file that may hold a credential or to dump the environment. Anything a
  worker reads enters its trajectory and reaches the model provider. Lane
  `h1-confine` (2026-09-25) was told to establish what Reasonix reads under
  `~/.reasonix`; it ran `cat ~/.reasonix/config.json`, and the provider key
  left the machine. The read boundary now masks the owner's home, but the
  lane's own Reasonix home still holds its key, so the rule stands.
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
  swaps the worker binary, `DS_LANE_MAX_LANES` caps concurrency,  `DS_LANE_STALL_POLL` shortens the 15 s stall sampling interval,
  `DS_LANE_STALL_CPU_PCT` moves the work threshold (both are test seams; the
  defaults are 15 s and 1.0% of a core), `DS_LANE_MAX_FILE_LINES` moves the
  1,000-line limit and `DS_LANE_SIZE_EXEMPT` replaces the globs that limit
  skips (both are test seams too; the defaults are 1,000 lines and the
  data-file list above), and `DS_LANE_COMPRESS_MIN_BYTES` /
  `DS_LANE_COMPRESS_EXTS` move what a receipt's pass compresses (test seams as
  well; the defaults are 1 MiB and the five extensions above).
  `DS_LANE_BWRAP` points at another bubblewrap - a value that does not resolve
  is a refusal, never a way to run unconfined - and `DS_LANE_CONFINE_HOME`
  names the home the boundary masks (the test seam; the default is the worker's
  own `HOME`).
  `PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v`
  covers the harness hermetically in under a minute and a half (its stall cases
  run with a lowered poll, its size cases against the real limit and its
  compaction cases against the real 1 MiB threshold; the suite must stay under
  90 s). The cases
  are split by cohesion - `tests/ds_lane_support.py` (the fake worker, the repo
  and home fixtures, the CLI helpers), `test_ds_lane_unit.py`,
  `test_ds_lane_lanes.py`, `test_ds_lane_size.py`,
  `test_ds_lane_compaction.py`, `test_ds_lane_finalize.py`,
  `test_ds_lane_crash.py`, `test_ds_lane_supervisor.py` and
  `test_ds_lane_confine.py` (the read boundary: the generated argv, the masked
  home and a canary the worker cannot reach) - each under the
  line cap that applies to it (500 lines for a test module, 400 for a package
  module). The confinement cases run the real `bwrap`, so a host that cannot
  create a user namespace cannot run the whole suite; the rest of the suite
  needs it too, because every run in the suite is confined.
- A lane that builds the whole workspace needs `--link oracle/gpgx-src`:
  `psiv-sound`'s build script compiles the ignored core sources under it.
- Each lane has its own `rust/target`, so its first cargo build is cold.
  Restate the relevant `AGENTS.md` safety rules in each brief (GDExtension,
  saves, serialized expensive runs).
- Other agents may commit in the main tree. Keep uncommitted Claude-side
  edits short-lived and scoped, and check `git log` before committing.

## The read boundary

`ds-lane` owns what a worker can read, the way it owns what a worker can write.
The supervisor starts every worker under bubblewrap (`confine.py`, called from
`supervisor.run_worker`), and the boundary applies to the worker and everything
it spawns - Reasonix, its bash tool, cargo, Godot. There is no switch that turns
it off: `lanes.prepare_run` refuses to launch (`start`, `resume`, and the
watchdog's automatic resume) when bubblewrap is missing, so a run either happens
under the boundary or does not happen at all. Each run's `supervisor.log`
records the boundary it used: `confined: masked <paths>; readable <n>: ...;
writable: ...; seeded lane home: <files>`.

**Visible.** The root filesystem, read-only, minus the two masks below. Inside
the sandbox the worker sees the toolchain and its own inputs, all read-only:

- `~/.cargo`, `~/.rustup`, `~/.local/bin` and the resolved target of every
  symlink in it (`local_bin_targets`: `ds-lane`, `claude`, a Godot build);
- the Node that runs Reasonix (`worker_roots`): `reasonix` on this host is the
  nvm shim `~/.local/bin/reasonix`, which sources `$NVM_DIR/nvm.sh`, resolves
  `$NVM_DIR/alias/default` and execs `$NVM_DIR/versions/node/<v>/bin/reasonix`,
  so those three pieces are bound - not the rest of `$NVM_DIR`, which also
  holds nvm's own checkout, a tarball cache and a `.npmrc`. A worker binary
  that is already a Node install's entry point resolves to its prefix instead,
  and the directory holding the binary is always readable;
- the main repository's `.git`: a linked worktree keeps its objects there, and
  it stays read-only, as it is today;
- the resolved sources of every `--link` and `--add-dir`.

**Masked.** The worker's home is replaced by a tmpfs (`--tmpfs "$HOME"`, or
`DS_LANE_CONFINE_HOME`), and `/tmp` is a private tmpfs of its own. Nothing
under either is in view: `~/.ssh`, `~/.config/gh/hosts.yml`, `~/.claude`
(including its credentials), `~/.gnupg`, `~/.gitconfig` and any credential
helper it points at, `~/.npmrc`, the rest of `~/.nvm`, `~/.cache/reasonix`,
and - the case that started this - the owner's own `~/.reasonix`. The home is
writable as a tmpfs, so a program that writes `$HOME/.cache` or a session tmp
directory still works; what it writes there is private to the run and gone
when it ends. A write aimed at the masked home never reaches the owner's disk,
which is the `test_canary_is_unreadable_and_absent_from_the_workers_view` case.

**Writable, and only these three.** The lane worktree, the run directory
(trajectory, metrics, logs) and the lane's own Reasonix home. Everything else
in the sandbox is read-only, so the worker's toolchain and its linked inputs
cannot be modified either.

**The per-lane Reasonix home.** Each lane gets `<state>/reasonix-home`, bound
at `~/.reasonix` for every run of that lane, so a `resume` finds the session
the previous run left and no other project's sessions are in view. It is seeded
once (and refreshed on later runs) with the config Reasonix needs, and with
nothing else:

- `.env`, copied as-is when it is a regular file: this is the credential store
  the worker authenticates from, because a `config.toml` provider table names
  each key with `api_key_env` and does not fall back to the legacy store. The
  real CLI was measured on this host (2026-09-25): with the owner's
  `config.toml` shape seeded and no `.env`, a run fails at once with
  `missing_credential`; with `.env` seeded it reaches the provider API. It is
  also created empty when there is nothing to copy, because Reasonix's own
  sandbox binds `/dev/null` over it on every bash call and bubblewrap cannot
  create that destination under a read-only tree;
- `config.toml`, what the lanes depend on: `sandbox.bash = "enforce"` is what
  confines the worker's *own* bash tool, `subagent_effort = "max"` is pinned
  there, and the provider table is what resolves `--model deepseek-flash`;
- `config.json`, the legacy config and settings store.

Every seeded copy is mode 0600, like the files it comes from: the supervisor
reads the real home from outside the sandbox, and the copies live in the lane's
state directory, never in the worktree or a receipt. Deliberately not seeded:
global skills, another project's sessions and stats, MCP state, and the rest of
the toolchain state under the home. A lane that needs any of it should get it
bound explicitly, in `confine.read_paths`.

A run that predates this boundary keeps its session in the real home, so a
later `resume` of such a lane fails at once - `error: no session matches
"<id>"`, measured with the real CLI on 2026-09-25, and no API call is made -
instead of continuing that conversation. Nothing needs migrating before the
change lands: finish such a lane under the old code, or start a new lane, or
copy its session out of the real home into `<state>/reasonix-home` first.

**Nesting.** Reasonix runs its own bash tool under bubblewrap, and that works
inside this boundary (user namespaces nest on this host; the case
`test_reasonix_sandbox_nests_inside_the_boundary` re-runs the real inner argv
inside the outer sandbox and requires `nested-ok`). Two things make it work,
and both were found the hard way: the per-lane home must contain `.env` (see
above), and the outer sandbox must bind the worktree writable, because the
inner one re-binds it and fails on a read-only source.

**What it does not cover.** The boundary masks the home and `/tmp`, not the
whole filesystem: anything readable outside them - `/etc`, `/var`, another
user's directories - is still readable, read-only. The three writable paths are
the only ones the supervisor grants, but a lane worktree is shared with the
worker's own runtime, so a worker can still write what a lane normally
produces. The network stays shared: the worker needs its provider API, and
nothing here limits what it can reach.

**Process lifetime.** The worker runs with `--die-with-parent`: if the
supervisor dies outright (a host crash, an OOM kill, a SIGKILL), the worker and
its children go down with it instead of lingering as orphans - the `stop`,
timeout and stall paths still kill the whole process group, and bubblewrap
passes the worker's exit code through (`124`, `125`, `137`, `143` are the
values the receipts record).
