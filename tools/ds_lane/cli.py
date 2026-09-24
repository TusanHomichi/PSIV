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
  ds-lane list [--repo DIR]
  ds-lane show ID [--repo DIR]
  ds-lane rm ID [--repo DIR] [--purge]

Layout:
  entry     tools/ds-lane                       (symlinked as ~/.local/bin/ds-lane)
  package   tools/ds_lane/                      (config, preflight, receipts, lanes,
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

--link PATH symlinks an ignored path from the source repo into the worktree
(e.g. runtime-pack, reference) so repo-relative tooling finds local inputs.
--link PATH=SOURCE links worktree PATH to SOURCE (repo-relative or absolute),
e.g. `--link runtime-pack=build/snap/runtime-pack` to pin a pack snapshot that
other agents cannot rebuild mid-run.
Only gitignored paths may be linked, and links are excluded when the lane
commits, so they never enter the lane diff. The
sandbox resolves real paths, so linked inputs are readable but not writable.

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

`verify ID -- CMD...` runs CMD in the lane worktree (CARGO_BUILD_JOBS=2),
streams its output, saves a header (command, UTC start, lane head, exit code,
duration) plus the output under <state>/verify/NNN.log, and exits with CMD's
code: this is how the orchestrator records its own independent checks as
evidence. `tail ID [-n N]` peeks at the latest run, in flight or finished.
"""
import argparse
import sys

from .config import DEFAULT_STALL_RETRIES, DEFAULT_STALL_TIMEOUT, DEFAULT_TIMEOUT
from .lanes import (cmd_list, cmd_rm, cmd_resume, cmd_show, cmd_start, cmd_stop, cmd_tail,
                    cmd_verify, cmd_wait)
from .supervisor import exec_run


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
