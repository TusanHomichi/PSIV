"""ds-lane: run a DeepSeek worker (via Reasonix) in an isolated git worktree lane.

The orchestrator (Claude) writes a brief; ds-lane runs it headless in a fresh
worktree on branch ds/<id>, commits whatever the worker changed (the worker
itself cannot write .git — the Reasonix OS sandbox confines writes to the
worktree), and leaves a receipt directory with the brief, final result, full
trajectory (tool calls + reasoning), metrics/cost and the diff.

Source: PSIV tools/ds-lane, symlinked from ~/.local/bin/ds-lane. That file is
the executable entry point; the implementation is the stdlib-only package
tools/ds_lane/ next to it. Claude Code host only; see CLAUDE.md.

Pinned, not configurable: model deepseek-flash, reasoning effort max
(owner decision 2026-09-23). Subagents are pinned to max in
~/.reasonix/config.toml (subagent_effort = "max").

Usage:
  ds-lane start BRIEF.md [--repo DIR] [--base REF] [--id NAME] [--max-steps N]
                         [--timeout SECONDS] [--stall-timeout SECONDS]
                         [--stall-retries N] [--link PATH[=SOURCE]]... [--add-dir DIR]...
                         [--read-only] [--allow-phrasing] [--no-wait]
  ds-lane resume ID FOLLOWUP.md [--max-steps N] [--timeout SECONDS]
                                [--stall-timeout SECONDS] [--stall-retries N]
                                [--allow-phrasing] [--no-wait]
  ds-lane wait ID
  ds-lane stop ID
  ds-lane tail ID [-n N]
  ds-lane verify ID -- CMD...
  ds-lane compact ID [--repo DIR] [--dry-run]
  ds-lane list [--repo DIR]
  ds-lane show ID [--repo DIR]
  ds-lane rm ID [--repo DIR] [--purge]

Layout:
  entry     tools/ds-lane                       (symlinked as ~/.local/bin/ds-lane)
  package   tools/ds_lane/                      (config, preflight, trajectory, receipts,
                                                 evidence, compaction, report, lanes, verify,
                                                 supervisor, cli)
  worktree  $DS_LANE_HOME/wt/<repo>/<id>        (branch ds/<id>)
  receipts  $DS_LANE_HOME/state/<repo>/<id>/    (run-N/ per worker run)

Environment (read per call, so tests can set them per case):
  DS_LANE_HOME       root for lanes: worktrees <home>/wt, receipts <home>/state
                     (default ~/.cache/ds-lane/wt and ~/.local/state/ds-lane)
  DS_LANE_REASONIX   worker binary (default `reasonix`), used for the run and --version
  DS_LANE_MAX_LANES  machine-wide concurrent workers (default 3)
  DS_LANE_STALL_POLL seconds between stall samples (default 15; the test suite
                     lowers it so a stall case does not cost a whole window)
  DS_LANE_STALL_CPU_PCT  percent of one core a stall window needs to count as
                     work (default 1.0)
  DS_LANE_MAX_FILE_LINES  line count above which a changed file is reported
                     (default 1000: the owner's rule that a file over roughly
                     1,000 lines is reorganized when touched)
  DS_LANE_SIZE_EXEMPT  comma-separated globs the line rule skips, replacing
                     DEFAULT_SIZE_EXEMPT (default *.json, *.tsv, *.csv, *.lock,
                     **/replay_fixtures/**: generated and data files)
  DS_LANE_COMPRESS_MIN_BYTES  size at which a receipt's evidence file is stored
                     compressed (default 1048576, 1 MiB)
  DS_LANE_COMPRESS_EXTS  comma-separated extensions that compression applies to,
                     replacing DEFAULT_COMPRESS_EXTS (default csv, log, jsonl,
                     txt, tsv); a leading dot is optional and an empty value
                     leaves nothing to compress

--link PATH symlinks an ignored path from the source repo into the worktree
(e.g. runtime-pack, reference) so repo-relative tooling finds local inputs.
--link PATH=SOURCE links worktree PATH to SOURCE (repo-relative or absolute),
e.g. `--link runtime-pack=build/snap/runtime-pack` to pin a pack snapshot that
other agents cannot rebuild mid-run.
Only gitignored paths may be linked, and links are excluded when the lane
commits, so they never enter the lane diff. A linked path that git already
ignores is left unnamed in that exclusion: naming one makes git refuse the whole
add ("The following paths are ignored..."), while a linked directory is a
symlink - mode 120000, which a `dir/` rule does not match - and so still needs
its exclusion. A git step that fails during finalize is recorded as
`finalize_error` in run.json and in the summary, with the worker's exit code,
result and receipt kept and the run's files left in the worktree. The sandbox
resolves real paths, so linked inputs are readable but not writable.

Each run executes under a detached supervisor (own session), so the worker and
its commit step survive the calling shell being killed; `start`/`resume` wait
for the result unless --no-wait, and `wait ID` reattaches. At most MAX_LANES
workers run machine-wide (later runs queue) and each gets CARGO_BUILD_JOBS<=2.
Every run has a wall-clock budget (--timeout, default 5400 s): on expiry the
supervisor SIGTERMs the worker's process group, SIGKILLs it 30 s later if it
survives, and the run still commits and finalizes with exit 124 and turn_error
"timeout after N s". A final SIGKILL always follows, so children the worker
spawned (a `cargo test`, say) cannot outlive the run and its released slot.

Every run also gets a stall watchdog (--stall-timeout SECONDS, default 900; 0
disables it). The supervisor samples every 15 s and judges each whole window:
the run is stalled when the window saw no trajectory.jsonl growth AND the
worker's process group used less than DS_LANE_STALL_CPU_PCT percent of one
core across it (default 1.0; utime+stime deltas from /proc/<pid>/stat over
SC_CLK_TCK). A rate, not any gain, is what tells work from a dead stream: a
silent `cargo build` burns far more than a percent of a core, while a worker
left holding a dead stream gains a tick every half minute (0.03%), which is
what a hung run looks like from here. A stalled run is stopped through the
same group kill as a timeout and finalizes with exit_code 125 and turn_error
"stalled: no progress for N s"; the measured rate and the threshold it was
under are recorded in the run's supervisor.log. With
--stall-retries N (default 1) the supervisor then starts the next run of the
lane itself, on the same Reasonix session: that run's follow-up says the
previous run stalled (a host suspend leaves the worker alive with a dead
stream: observed 2026-09-24), that its session context is intact, and to
continue the brief through acceptance and the Receipt. It keeps the timeout,
the stall timeout and the retries left. `stall_retries_left` and
`resumed_after_stall` are recorded in the run spec and in run.json, and
`wait ID` follows the chain to the lane's final run, so a waiter that attached
before the stall still returns the final outcome.

`stop ID` ends a lane's in-flight run cleanly: it SIGTERMs the supervisor
recorded in the run directory (never a pattern match over command lines), which
turns that into the same group kill and finalizes the run - committing whatever
the worker left, with exit_code 143 and turn_error "stopped by orchestrator". A
run still queued for a slot stops immediately with nothing run. `stop` waits up
to 60 s for finalization and prints the summary; it exits 0 when the run is
stopped (or when nothing was in flight) and 1 if finalization did not finish.

Lane rules for the worker are prepended to every brief (see PREAMBLE).

BRIEF PHRASING LAW: Reasonix bans all writes for the session when the prompt
contains negated-mutation wording outside code fences ("do not edit",
"don't modify", "no changes", "read-only"...). ds-lane preflights the brief
(and the follow-up) and refuses to launch on such wording, printing the
offending lines; rephrase positively ("touch only X") or keep quoted wording
inside a fenced block or inline code. `--read-only` skips the preflight and
uses the ban on purpose; `--allow-phrasing` skips it for a lane that must
carry such wording. Runs that still hit the ban are reported in the summary.

WRITE SET: a brief may carry one fenced block whose info string is `write-set`,
listing one path or glob per line (fnmatch semantics, `**` matching across
directories). After each run, the paths changed between the lane base and the
new head are compared with it; violations are recorded in run.json as
`write_set_violations` and printed as `WARNING: outside write set: ...`. A
follow-up passed to `resume` may carry its own block: it replaces the lane's
write set from that run on (lane.json records the current set, and every run's
run.json records the `write_set` it was actually checked against). A follow-up
without a block inherits the set the brief or an earlier follow-up declared.

SIZE RULE: the owner's rule is that a file over roughly 1,000 lines is
reorganized when touched, so `ds-lane` flags it the way it flags a write-set
violation. After each run, every path changed between the lane base and the new
head that still exists and is text (no NUL byte in its first 8 KiB) is counted
at the new head; each one over DS_LANE_MAX_FILE_LINES (default 1000, the
constant MAX_FILE_LINES in config.py) is recorded in run.json as
`oversize_files` - `{path, lines, base_lines}`, where `base_lines` is the count
at the lane base and null for a file that is new there - and printed as
`WARNING: over 1000 lines: <path> (<lines>, was <base_lines>)` (`was new` for a
new file). Every path in a commit is counted, whatever the brief's write set
says; linked inputs and worker host state are excluded from the commit, so they
are never counted. Generated and data files are exempt (owner decision): a path
matching the globs in DEFAULT_SIZE_EXEMPT (`*.json`, `*.tsv`, `*.csv`, `*.lock`,
`**/replay_fixtures/**`, config.py) is skipped, and DS_LANE_SIZE_EXEMPT replaces
that list with a comma-separated one (an empty value leaves nothing exempt). The
rule is for source text a worker edits; data grows with its content. The list a
run used is recorded as `size_exempt` in run.json, beside `max_file_lines`. The
worker preamble states the rule positively.

RUN NUMBERING: a run's number is one more than the highest `run-N` directory in
the lane's receipts, never the length of lane.json's `runs`. A run whose
supervisor died before it recorded itself leaves its directory behind with no
entry there, and counting from the record made `resume` try to create that same
directory again (FileExistsError), which left the lane stuck until an
orchestrator cleared it by hand. A supervisor that raises - inside the run, or
inside finalize before run.json landed - is still recorded in lane.json with
what is known of it: the `error`, the session id read from its trajectory, and
its turn error, exit code and cost when the worker's result arrived. Such a run
is marked `failed`, never run.json, so `wait ID` reports it as failed and prints
the error, and `resume ID` continues the session its trajectory holds - which
is also what a `resume` falls back to when a supervisor was killed outright (a
host crash, an OOM kill) and could record nothing at all.

`verify ID -- CMD...` runs CMD in the lane worktree (CARGO_BUILD_JOBS=2),
streams its output, saves a header (command, UTC start, lane head, exit code,
duration) plus the output under <state>/verify/NNN.log, and exits with CMD's
code: this is how the orchestrator records its own independent checks as
evidence. `tail ID [-n N]` peeks at the latest run, in flight or finished.

COMPACTION: every run's receipt keeps a copy of the worktree's
`build/lane-evidence/`, so a lane's receipts repeat each other and the raw
captures in them are large repetitive text. After a run's record has landed and
its machine-wide slot is released - so it never delays another lane's start -
that run's evidence is deduplicated against the lane's earlier runs (a file
byte-identical to one they already keep becomes a hardlink to it, so every path
stays readable exactly as before) and files at or over
DS_LANE_COMPRESS_MIN_BYTES whose extension is in DS_LANE_COMPRESS_EXTS are
stored as `name.ext.xz` with lzma preset 6, the original removed. A compressed
file an earlier run already keeps is hardlinked too, and nothing is compressed
that an earlier run can share instead. `evidence/INDEX.json` in each run
records every file's path, the size and sha256 of its content as captured,
whether it is stored plain, as `.xz` or as a hardlink (and where that storage
comes from), so a reader can verify a decompressed file: `xz -dc name.ext.xz`,
or Python's `lzma.open`. Failures land in `run.json` as `compaction_error`; the
evidence stays readable either way. `ds-lane compact ID` applies the same
passes to a lane's existing receipts, including lanes finalized before
compaction existed, oldest run first; it is idempotent, refuses a lane with a
run in flight, and `--dry-run` reports what it would do and the bytes it would
save without touching a file. There is deliberately no `--all`: which lanes are
finished with their evidence is the orchestrator's call.
"""
import argparse
import sys

from .config import DEFAULT_STALL_RETRIES, DEFAULT_STALL_TIMEOUT, DEFAULT_TIMEOUT
from .lanes import (cmd_compact, cmd_list, cmd_rm, cmd_resume, cmd_show, cmd_start, cmd_stop,
                    cmd_tail, cmd_wait)
from .supervisor import exec_run
from .verify import cmd_verify


def main():
    p = argparse.ArgumentParser(prog="ds-lane", description=__doc__.split("\n\n")[0])
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("start")
    s.add_argument("brief")
    s.add_argument("--repo", default=".")
    s.add_argument("--base", default="HEAD")
    s.add_argument("--id")
    s.add_argument("--max-steps", type=int, default=0)
    s.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT,
                   help="wall-clock budget for the run in seconds (default 5400)")
    s.add_argument("--stall-timeout", type=float, default=DEFAULT_STALL_TIMEOUT,
                   help="no trajectory growth and no CPU for this many seconds is a stall "
                        "(default 900; 0 disables the watchdog)")
    s.add_argument("--stall-retries", type=int, default=DEFAULT_STALL_RETRIES,
                   dest="stall_retries_left",  # one name for the flag, the spec and run.json
                   metavar="STALL_RETRIES",
                   help="runs the supervisor starts by itself after a stall (default 1)")
    s.add_argument("--link", action="append", default=[])
    s.add_argument("--add-dir", action="append", default=[])
    s.add_argument("--read-only", action="store_true",
                   help="investigation lane: deliberately trigger Reasonix's session-wide write ban")
    s.add_argument("--allow-phrasing", action="store_true",
                   help="skip the brief phrasing preflight (negated-mutation wording)")
    s.add_argument("--no-wait", action="store_true", help="return after launch; reattach with `wait`")
    s.set_defaults(fn=cmd_start)
    r = sub.add_parser("resume")
    r.add_argument("id")
    r.add_argument("followup", help="follow-up brief; its own write-set block replaces the lane's")
    r.add_argument("--repo", default=".")
    r.add_argument("--max-steps", type=int, default=0)
    r.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    r.add_argument("--stall-timeout", type=float, default=DEFAULT_STALL_TIMEOUT)
    r.add_argument("--stall-retries", type=int, default=DEFAULT_STALL_RETRIES,
                   dest="stall_retries_left", metavar="STALL_RETRIES")
    r.add_argument("--allow-phrasing", action="store_true")
    r.add_argument("--no-wait", action="store_true")
    r.set_defaults(fn=cmd_resume)
    w = sub.add_parser("wait")
    w.add_argument("id")
    w.add_argument("--repo", default=".")
    w.set_defaults(fn=cmd_wait)
    sp = sub.add_parser("stop")
    sp.add_argument("id")
    sp.add_argument("--repo", default=".")
    sp.set_defaults(fn=cmd_stop)
    tl = sub.add_parser("tail")
    tl.add_argument("id")
    tl.add_argument("-n", type=int, default=10, help="how many recent tool calls to show")
    tl.add_argument("--repo", default=".")
    tl.set_defaults(fn=cmd_tail)
    v = sub.add_parser("verify")
    v.add_argument("id")
    v.add_argument("cmd", nargs="*", help="command to run in the lane worktree, after `--`")
    v.add_argument("--repo", default=".")
    v.set_defaults(fn=cmd_verify)
    cp = sub.add_parser("compact")
    cp.add_argument("id")
    cp.add_argument("--repo", default=".")
    cp.add_argument("--dry-run", action="store_true",
                    help="report what a pass would deduplicate and compress, and change nothing")
    cp.set_defaults(fn=cmd_compact)
    x = sub.add_parser("_exec")  # internal: detached supervisor
    x.add_argument("state")
    x.add_argument("n", type=int)
    x.set_defaults(fn=lambda a: exec_run(a.state, a.n))
    for name, fn in (("list", cmd_list), ("show", cmd_show), ("rm", cmd_rm)):
        c = sub.add_parser(name)
        if name != "list":
            c.add_argument("id")
        c.add_argument("--repo", default=".")
        if name == "rm":
            c.add_argument("--purge", action="store_true")
        c.set_defaults(fn=fn)
    a = p.parse_args()
    sys.exit(a.fn(a) or 0)


if __name__ == "__main__":
    main()
