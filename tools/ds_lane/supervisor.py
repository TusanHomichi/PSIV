"""The detached supervisor: one process per run, owning its worker and its slot.

It waits for a machine-wide slot, runs the worker in its own process group
under a wall-clock timeout and a stall watchdog, kills the group on `stop`,
timeout or stall, and hands the result to the receipts. Its last act can be to
start the next run of the lane itself when a stall costs the worker its turn -
a host suspend leaves the process alive with a dead stream, and the run would
otherwise hold a slot for hours (observed 2026-09-24). A run that raises before
its record lands is recorded anyway (receipts.record_crashed_run) and then
re-raised, so the lane keeps its numbering and the session a resume continues.
"""
import fcntl
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

from . import lanes
from .config import (CARGO_JOBS, DEFAULT_STALL_CPU_PCT, KILL_GRACE, STALL_EXIT, STALL_POLL,
                     STOPPED_EXIT, TIMEOUT_EXIT, load_receipt, max_lanes, now, pid_alive,
                     stall_cpu_pct, stall_poll, state_root)
from .receipts import clear_stall_resume, finalize_run, record_crashed_run

# ----------------------------------------------------------- stop requests

# `ds-lane stop ID` SIGTERMs the supervisor; the handler only raises a flag so
# the running worker (or the queue wait) can be unwound on a normal code path
# and the run still commits and finalizes.
STOP_REQUESTED = False


def install_stop_handler():
    def request_stop(signum, frame):
        global STOP_REQUESTED
        STOP_REQUESTED = True
    signal.signal(signal.SIGTERM, request_stop)


# ------------------------------------------------------------------- slot

def acquire_slot(run_dir):
    """Block until fewer than MAX_LANES workers run machine-wide, then claim a slot.

    Returns False when a `stop` request arrives while the run is still queued.
    """
    root = state_root()
    root.mkdir(parents=True, exist_ok=True)
    announced = False
    t0 = time.monotonic()
    while True:
        if STOP_REQUESTED:
            return False
        with open(root / ".slots.lock", "w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            live = [f for f in root.glob("*/*/run-*/worker.slot") if pid_alive(f)]
            if len(live) < max_lanes():
                if STOP_REQUESTED:
                    return False
                (run_dir / "worker.slot").write_text(str(os.getpid()))
                return True
        if not announced:
            print(f"{now()} queued: {len(live)} lanes running (cap {max_lanes()})", flush=True)
            announced = True
        time.sleep(1 if time.monotonic() - t0 < 30 else 10)


# ------------------------------------------------- worker process group

def kill_group(pgid, sig):
    """Signal a process group. The worker's pgid equals its pid (start_new_session)."""
    try:
        os.killpg(pgid, sig)
    except OSError:  # ProcessLookupError: the group is already gone
        pass


def stop_group(p):
    """Terminate a worker's whole process group, then always finish it off.

    SIGTERM first, SIGKILL after KILL_GRACE seconds, and a final SIGKILL even
    when the parent exited promptly: children it spawned (a `cargo test`, say)
    would otherwise outlive the run and the slot it released. Signals go to
    p.pid as the group id — os.getpgid() fails once the parent is reaped.
    """
    kill_group(p.pid, signal.SIGTERM)
    try:
        p.wait(timeout=KILL_GRACE)
    except subprocess.TimeoutExpired:
        pass
    kill_group(p.pid, signal.SIGKILL)
    try:
        p.wait(timeout=KILL_GRACE)
    except subprocess.TimeoutExpired:  # paranoia: an unkillable parent
        p.kill()
        p.wait()


# ------------------------------------------------------------ stall watchdog

CLK_TCK = os.sysconf("SC_CLK_TCK")


def proc_stat(pid):
    """(process group id, utime+stime in clock ticks) for a live pid, else None.

    Fields are read from after the comm, which can itself hold spaces and
    parentheses: state(0) ppid(1) pgrp(2) ... utime(11) stime(12).
    """
    try:
        raw = Path(f"/proc/{pid}/stat").read_text()
    except (OSError, ValueError):
        return None
    fields = raw.rsplit(")", 1)[-1].split()
    try:
        return int(fields[2]), int(fields[11]) + int(fields[12])
    except (IndexError, ValueError):
        return None


def group_ticks(pgid):
    """{pid: cpu ticks} for every live member of process group `pgid`.

    Group membership comes from each process's own stat, so a child that
    re-parented onto init still counts while it lives.
    """
    out = {}
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        st = proc_stat(int(entry))
        if st and st[0] == pgid:
            out[int(entry)] = st[1]
    return out


class StallWatcher:
    """Decide whether a run has stopped making progress.

    Time is cut into windows of `timeout` seconds. A window saw work when the
    trajectory grew during it, or when the worker's process group used at
    least `cpu_pct` percent of one core across it. The rate - utime+stime
    deltas from /proc/<pid>/stat over SC_CLK_TCK - is what tells work apart
    from a process that is merely alive: a silent `cargo build` burns far
    more than 1% of a core, while a worker whose stream died and whose wrapper
    then idles on a timer gains a tick every half minute, 0.03% (measured
    2026-09-24). Any gain at all is not enough to call a run alive.
    """

    def __init__(self, pgid, trajectory, timeout, poll=STALL_POLL,
                 cpu_pct=DEFAULT_STALL_CPU_PCT, clock=time.monotonic, groups=group_ticks):
        self.pgid, self.trajectory = pgid, Path(trajectory)
        self.timeout, self.poll, self.cpu_pct = timeout, poll, cpu_pct
        self.clock, self.groups = clock, groups
        self.cpu = {}  # pid -> highest utime+stime seen for it, in clock ticks
        self.open_window()

    def open_window(self):
        """Start a window: the trajectory size and group CPU it begins from."""
        self.window_at = self.clock()
        self.size_at = self._size()
        self.cpu_at = self.group_ticks()

    def _size(self):
        try:
            return self.trajectory.stat().st_size
        except OSError:
            return 0

    def group_ticks(self):
        """Cumulative CPU ticks of the worker's process group.

        Each pid keeps the highest value seen for it, so a member that exits
        cannot make the group look like it lost CPU time, and a pid the kernel
        hands out again cannot subtract from the window's delta.
        """
        for pid, ticks in self.groups(self.pgid).items():
            self.cpu[pid] = max(ticks, self.cpu.get(pid, 0))
        return sum(self.cpu.values())

    def rate(self):
        """(percent of one core, trajectory bytes grown) since the window opened."""
        ticks = self.group_ticks() - self.cpu_at
        return ticks / CLK_TCK / (self.clock() - self.window_at) * 100, self._size() - self.size_at

    def verdict(self):
        """None while a window is open or when it saw work, else the stall.

        A stall is a whole window with no trajectory growth and a group CPU
        rate under `cpu_pct` percent of one core; the verdict then carries
        what was measured, for the supervisor log. The next window opens
        either way.
        """
        if not self.timeout or self.clock() - self.window_at < self.timeout:
            return None
        rate, grew = self.rate()
        self.open_window()
        return (rate, grew) if grew <= 0 and rate < self.cpu_pct else None


def run_worker(run_dir, spec, wt, env):
    """Run the worker in its own process group under a wall-clock budget.

    Returns (exit code, started, duration, outcome), where outcome is None,
    "timeout", "stopped" or "stalled". A `stop` request (SIGTERM to the
    supervisor), a budget expiry or the stall watchdog kills the group via
    stop_group(); the run still finalizes.
    """
    timeout = spec.get("timeout") or 0
    started, t0 = now(), time.monotonic()
    if STOP_REQUESTED:  # stopped while queued, before the worker was spawned
        return STOPPED_EXIT, started, 0.0, "stopped"
    print(f"{started} worker start", flush=True)
    outcome = None
    prompt = (run_dir / "prompt.md").read_text()
    with open(run_dir / "stdout.log", "w") as out, open(run_dir / "stderr.log", "w") as err:
        p = subprocess.Popen(spec["cmd"] + [prompt], cwd=wt, stdout=out, stderr=err,
                             text=True, env=env, start_new_session=True)
        poll = stall_poll()
        watcher = StallWatcher(p.pid, run_dir / "trajectory.jsonl", spec.get("stall_timeout"), poll,
                               stall_cpu_pct()) if spec.get("stall_timeout") else None
        next_sample = t0 + poll
        while True:  # short waits so a stop request lands promptly
            try:
                rc = p.wait(timeout=min(0.25, poll))
                break
            except subprocess.TimeoutExpired:
                pass
            if STOP_REQUESTED:
                outcome, rc = "stopped", STOPPED_EXIT
                break
            if timeout and time.monotonic() - t0 >= timeout:
                outcome, rc = "timeout", TIMEOUT_EXIT
                break
            if watcher and time.monotonic() >= next_sample:
                next_sample = time.monotonic() + poll
                stall = watcher.verdict()
                if stall:
                    rate, grew = stall
                    print(f"{now()} stalled: no progress for {spec.get('stall_timeout'):g} s "
                          f"(group CPU {rate:.3f}% of a core, under {watcher.cpu_pct:g}%, "
                          f"trajectory +{grew} bytes over the window)", flush=True)
                    outcome, rc = "stalled", STALL_EXIT
                    break
        if outcome:
            stop_group(p)
    duration = round(time.monotonic() - t0, 1)
    print(f"{now()} worker exit {rc}{f' ({outcome})' if outcome else ''}", flush=True)
    return rc, started, duration, outcome


# --------------------------------------------------------- run orchestration

STALL_FOLLOWUP = """\
# Automatic resume after a stall

The previous run of this lane stalled: for {timeout} s its trajectory grew by
nothing and no member of its process group used any CPU, which is what a host
suspend looks like from here (the worker process stays alive with a dead
stream; observed 2026-09-24). The harness stopped that run and started this one
in its place.

Your session context is intact. Continue the brief from where it stopped and
finish it: run the acceptance checks the brief asks for and end with the
`## Receipt` block.
"""


def stall_resume(lane, run_dir, spec, successor):
    """Start the next run of a stalled lane, on the same Reasonix session.

    The resumed run keeps the wall clock, the stall window and the retries
    that are left; a failure here only costs the retry, so this never raises.
    """
    try:
        session = json.loads((run_dir / "run.json").read_text()).get("session_id")
        print(f"{now()} stalled: resuming as run-{successor} (session {session or 'unknown'})",
              flush=True)
        lanes.launch_resume(
            lane, STALL_FOLLOWUP.format(timeout=f"{spec.get('stall_timeout'):g}"), session,
            max_steps=spec.get("max_steps") or 0, timeout=spec.get("timeout"),
            stall_timeout=spec.get("stall_timeout"),
            stall_retries_left=(spec.get("stall_retries_left") or 0) - 1,
            run_number=successor, resumed_after_stall=True)
    except BaseException as e:
        print(f"{now()} stall resume failed: {e!r}", flush=True)
        clear_stall_resume(run_dir, repr(e))


def exec_run(state, n):
    state = Path(state)
    run_dir = state / f"run-{n}"
    (run_dir / "supervisor.pid").write_text(str(os.getpid()))
    lane = load_receipt(state)
    wt = Path(lane["worktree"])
    spec = json.loads((run_dir / "spec.json").read_text())
    install_stop_handler()
    rc, started, duration, outcome = None, None, None, None  # what a crash still gets to report
    try:
        if not acquire_slot(run_dir):  # `ds-lane stop` while still queued
            finalize_run(lane, run_dir, spec, STOPPED_EXIT, now(), 0.0, outcome="stopped")
            return
        try:
            env = dict(os.environ)
            env["CARGO_BUILD_JOBS"] = str(min(int(env.get("CARGO_BUILD_JOBS", CARGO_JOBS)), CARGO_JOBS))
            rc, started, duration, outcome = run_worker(run_dir, spec, wt, env)
        finally:  # the machine-wide slot is released on every path
            (run_dir / "worker.slot").unlink(missing_ok=True)
        successor = None  # a stalled run continues the lane by itself, retries permitting
        if outcome == "stalled" and (spec.get("stall_retries_left") or 0) > 0:
            successor = spec["run"] + 1
        # The pointer lands in run.json first, so a waiter never sees this run
        # complete without learning what follows; the successor starts only
        # afterwards, so it reads a lane.json that already lists this run.
        finalize_run(lane, run_dir, spec, rc, started, duration, outcome, stall_resume=successor)
        if successor:
            stall_resume(lane, run_dir, spec, successor)
    except BaseException as e:  # leave a terminal record whatever happens
        (run_dir / "worker.slot").unlink(missing_ok=True)
        try:
            record_crashed_run(lane, run_dir, spec, repr(e), started=started, duration=duration,
                               exit_code=rc)
        except BaseException as record_error:  # best effort: the run failed either way
            (run_dir / "summary.txt").write_text(
                f"lane {lane['id']} run-{n}: SUPERVISOR ERROR {e!r}\n"
                f"recording the run in lane.json failed too: {record_error!r}\n"
                f"see {run_dir / 'supervisor.log'}\n")
        # Last, so a reader that sees it finds the record and the summary already there.
        (run_dir / "failed").write_text(repr(e) + "\n")
        raise
