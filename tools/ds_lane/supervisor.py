"""The detached supervisor: one process per run, owning its worker and its slot.

It waits for a machine-wide slot, runs the worker in its own process group
under a wall-clock timeout and a stall watchdog, kills the group on `stop`,
timeout or stall, and hands the result to the receipts. Its last act can be to
start the next run of the lane itself when a stall costs the worker its turn -
a host suspend leaves the process alive with a dead stream, and the run would
otherwise hold a slot for hours (observed 2026-09-24).
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
from .config import (CARGO_JOBS, KILL_GRACE, STALL_EXIT, STALL_POLL, STOPPED_EXIT, TIMEOUT_EXIT,
                     load_receipt, max_lanes, now, pid_alive, stall_poll, state_root)
from .receipts import clear_stall_resume, finalize_run

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
    """(process group id, utime+stime in seconds) for a live pid, else None.

    Fields are read from after the comm, which can itself hold spaces and
    parentheses: state(0) ppid(1) pgrp(2) ... utime(11) stime(12).
    """
    try:
        raw = Path(f"/proc/{pid}/stat").read_text()
    except (OSError, ValueError):
        return None
    fields = raw.rsplit(")", 1)[-1].split()
    try:
        return int(fields[2]), (int(fields[11]) + int(fields[12])) / CLK_TCK
    except (IndexError, ValueError):
        return None


def group_cpu(pgid):
    """{pid: cpu seconds} for every live member of process group `pgid`.

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

    A stall is a whole window in which the trajectory has not grown AND no
    member of the worker's process group has gained CPU time: the worker is
    alive, holds a slot, and nothing is happening to it. CPU time is summed
    from /proc/<pid>/stat, so a long silent `cargo build` (or any other busy
    child) keeps the run alive. A process that exits cannot subtract from the
    window: each pid keeps the high-water mark of its own CPU time.
    """

    def __init__(self, pgid, trajectory, timeout, poll=STALL_POLL, clock=time.monotonic,
                 groups=group_cpu):
        self.pgid, self.trajectory = pgid, Path(trajectory)
        self.timeout, self.poll = timeout, poll
        self.clock, self.groups = clock, groups
        self.cpu = {}  # pid -> highest utime+stime seen for that pid
        self.size = self._size()
        self.since = self.clock()

    def _size(self):
        try:
            return self.trajectory.stat().st_size
        except OSError:
            return 0

    def sample(self):
        """True when the run has made no progress for a whole stall window."""
        stamp = self.clock()
        progress = False
        for pid, cpu in self.groups(self.pgid).items():
            if cpu > self.cpu.get(pid, 0.0):
                self.cpu[pid] = cpu
                progress = True
        size = self._size()
        if size != self.size:
            self.size, progress = size, True
        if progress:
            self.since = stamp
        return bool(self.timeout) and stamp - self.since >= self.timeout


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
        watcher = StallWatcher(p.pid, run_dir / "trajectory.jsonl", spec.get("stall_timeout"), poll) \
            if spec.get("stall_timeout") else None
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
                if watcher.sample():
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
        (run_dir / "summary.txt").write_text(f"lane {lane['id']} run-{n}: SUPERVISOR ERROR {e!r}\n"
                                             f"see {run_dir}/supervisor.log\n")
        (run_dir / "failed").write_text(repr(e) + "\n")
        raise
