"""tools/heavy_lock.py and tools/gate.py: a full gate queues first, and a
waiter keeps saying whom it waits on.

    PYTHONPATH=. python3 -m unittest tests.test_heavy_lock_wait -v

On tests/test_heavy_lock.py's fixtures. `GateRankCase` runs real waiters: one
holding the lock with `gate=True` is granted before earlier priority and normal
waiters, and the negative control, the same waiter without it, keeps its place
in arrival order. `GateTierCase` shows `run_gate` queues the full gate at the
gate rank and the fast tier below it. `StillWaitingCase` waits in-process with
a short `WAIT_REPORT_S`: a heavy-lock waiter and a gate waiting for the gate
lock with `--wait` each print lines saying how long they have waited and the
holder's pid, class, age and command line (the holder lets go only once two
have appeared, so a loaded host slows the case down rather than failing it);
with the period longer than the wait (the negative control), neither prints
one. `FlockErrorCase` fails the
blocking flock with ENOLCK: the heavy-lock queue head and the gate waiting for
the gate lock each raise it, leave no descriptor open and no queue entry behind,
instead of waiting forever.
"""

import contextlib
import errno
import fcntl
import io
import os
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from tests.test_heavy_lock import (HOLDER, TIMEOUT, TOOLS, LockProcesses, clean_env, read_until,
                                   stop)
from tools import gate as gate_module
from tools import heavy_lock as heavy_lock_module
from tools.gate import gate_lock, run_gate
from tools.heavy_lock import (ENTRY_RE, GATE_RANK, NORMAL_RANK, gate_lock_path, heavy_lock_path,
                              hold_heavy_lock)

#: Takes `argv[1]` with hold_heavy_lock, as a gate when `argv[4]` is `gate`,
#: then appends `argv[3]` to the order file `argv[2]`.
APPEND_AS_GATE = f"""
import sys
sys.path.insert(0, {str(TOOLS)!r})
from heavy_lock import hold_heavy_lock
with hold_heavy_lock(sys.argv[1], gate=sys.argv[4] == "gate"):
    open(sys.argv[2], "a").write(sys.argv[3] + "\\n")
"""

#: How long a negative control's holder holds after its waiter's first line,
#: and the short report period whose still-waiting lines the positive cases
#: wait for (up to TIMEOUT: a loaded host scans /proc slowly) before letting go.
WAIT_S = 1.5
SHORT_PERIOD_S = 0.4
STILL_LINES = 2


class GateRankCase(LockProcesses):
    """A gate waiter goes ahead of priority and normal waiters queued before it."""

    def order_with_third(self, kind):
        """Holder; waiters `first` (normal), `second` (priority), then `third`
        (`kind`: gate or normal); release; the grant order."""
        holder = self.holder()
        first = self.waiter("first")
        second = self.waiter("second", "--priority")
        third = self.spawn(["-c", APPEND_AS_GATE, str(self.lock), str(self.order), "third", kind])
        self.third_line = read_until(third.stderr, "\n")
        self.assertEqual(self.granted(), [], "a holder is never preempted")
        self.release(holder)
        return self.finish(first, second, third)

    def test_a_gate_waiter_is_granted_before_earlier_priority_and_normal_ones(self):
        self.assertEqual(self.order_with_third("gate"), ["third", "second", "first"])
        self.assertIn("0 waiter(s) ahead", self.third_line)
        self.assertIn("this waiter: exclusive, gate", self.third_line)

    def test_negative_control_the_same_waiter_without_gate_keeps_its_place(self):
        self.assertEqual(self.order_with_third("normal"), ["second", "first", "third"])
        self.assertIn("1 of them priority, 0 gate(s); this waiter: exclusive\n",
                      self.third_line)

    def test_a_normal_waiter_counts_the_gate_ahead_of_it(self):
        holder = self.holder()
        gate = self.spawn(["-c", APPEND_AS_GATE, str(self.lock), str(self.order), "gate", "gate"])
        read_until(gate.stderr, "\n")
        later = self.waiter("later")
        self.assertIn("1 waiter(s) ahead, 0 of them priority, 1 gate(s)", later.first_line)
        self.release(holder)
        self.assertEqual(self.finish(gate, later), ["gate", "later"])


class GateTierCase(unittest.TestCase):
    """`run_gate` ranks the full gate a gate and the fast tier below it."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name).resolve()  # not a git checkout: its own build/
        patcher = mock.patch.dict(os.environ, clean_env(), clear=True)
        patcher.start()
        self.addCleanup(patcher.stop)

    def queued_rank(self, tier):
        """The rank `run_gate` at `tier` queues at while a raw holder has the heavy lock."""
        lock = heavy_lock_path(self.root)
        lock.parent.mkdir(parents=True, exist_ok=True)
        holder = subprocess.Popen([sys.executable, "-c", HOLDER, str(lock)],
                                  stdout=subprocess.PIPE, text=True, env=clean_env())
        self.addCleanup(stop, holder)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        queue = Path(f"{lock.resolve()}.queue")
        receipt_dir = self.root / "build" / "gate" / tier
        thread = threading.Thread(target=run_gate, args=(["true"], self.root, receipt_dir),
                                  kwargs={"tier": tier})
        with contextlib.redirect_stdout(io.StringIO()):
            thread.start()
            deadline = time.monotonic() + TIMEOUT
            entries = []
            while not entries and time.monotonic() < deadline:
                entries = [name for name in (queue.iterdir() if queue.is_dir() else ())
                           if ENTRY_RE.match(name.name)]
                time.sleep(0.05)
            stop(holder)
            thread.join(timeout=TIMEOUT)
        self.assertFalse(thread.is_alive())
        self.assertEqual(len(entries), 1, entries)
        return int(entries[0].name.split("-")[0])

    def test_the_full_gate_queues_at_the_gate_rank(self):
        self.assertEqual(self.queued_rank("full"), GATE_RANK)

    def test_negative_control_the_fast_tier_queues_below_it(self):
        self.assertEqual(self.queued_rank("fast"), NORMAL_RANK)


class StillWaitingCase(unittest.TestCase):
    """A waiter whose wait does not change says so every `WAIT_REPORT_S`."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name).resolve()

    def hold(self, lock):
        """An unrelated holder of `lock`, started and holding."""
        holder = subprocess.Popen([sys.executable, "-c", HOLDER, str(lock)],
                                  stdout=subprocess.PIPE, text=True, env=clean_env())
        self.addCleanup(stop, holder)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        return holder

    def wait_through(self, holder, wait, lines):
        """Run `wait()` in a thread and return what it printed. Once the wait
        has printed its first line, `holder` lets go when it has printed
        `lines` still-waiting lines too, or WAIT_S later when `lines` is 0;
        each wait for output gives up at TIMEOUT."""
        output = io.StringIO()
        thread = threading.Thread(target=wait, args=(output,))
        thread.start()
        deadline = time.monotonic() + TIMEOUT

        def printed(text, count):
            while output.getvalue().count(text) < count and time.monotonic() < deadline:
                time.sleep(0.05)

        printed("waiting for ", 1)
        if lines:
            printed("\nstill waiting", lines)
        else:
            time.sleep(WAIT_S)
        self.assertTrue(thread.is_alive(), "the wait must block while the holder holds")
        stop(holder)
        thread.join(timeout=TIMEOUT)
        self.assertFalse(thread.is_alive())
        return output.getvalue()

    def heavy_wait(self, period, lines):
        lock = self.dir / "build" / "continuation-heavy.lock"
        lock.parent.mkdir()
        holder = self.hold(lock)

        def wait(report):
            with hold_heavy_lock(lock, environ=clean_env(), report=report):
                pass

        with mock.patch.object(heavy_lock_module, "WAIT_REPORT_S", period):
            return lock, holder, self.wait_through(holder, wait, lines)

    def gate_wait(self, period, lines):
        lock = gate_lock_path(self.dir)
        lock.parent.mkdir(parents=True)
        holder = self.hold(lock)

        def wait(report):
            with contextlib.redirect_stdout(report), gate_lock(lock, wait=True):
                pass

        # gate.py imports heavy_lock as a top-level module: patch the one it calls.
        with mock.patch.dict(gate_module.wait_reporting.__globals__, WAIT_REPORT_S=period):
            return lock, holder, self.wait_through(holder, wait, lines)

    def assert_still_lines(self, lock, holder, output):
        lines = [line for line in output.splitlines() if line.startswith("still waiting")]
        self.assertGreaterEqual(len(lines), STILL_LINES, output)
        self.assertIn(f"still waiting for {lock} after ", lines[0])
        self.assertIn(f"held by pid {holder.pid} (exclusive, up ", lines[0])
        self.assertIn(" s): ", lines[0])
        self.assertIn("import fcntl, sys, time", lines[0])  # the holder's command line

    def test_a_heavy_lock_waiter_says_how_long_and_on_whom(self):
        lock, holder, output = self.heavy_wait(SHORT_PERIOD_S, STILL_LINES)
        self.assertIn(f"waiting for {lock} held by pid {holder.pid} (exclusive)", output)
        self.assert_still_lines(lock, holder, output)

    def test_negative_control_a_longer_period_prints_no_still_line(self):
        lock, _, output = self.heavy_wait(TIMEOUT, 0)
        self.assertIn(f"waiting for {lock}", output)
        self.assertNotIn("still waiting", output)

    def test_a_gate_waiting_for_the_gate_lock_says_how_long_and_on_whom(self):
        lock, holder, output = self.gate_wait(SHORT_PERIOD_S, STILL_LINES)
        self.assertIn(f"gate: waiting for {lock} held by pid {holder.pid}", output)
        self.assert_still_lines(lock, holder, output)

    def test_negative_control_the_gate_lock_with_a_longer_period(self):
        lock, _, output = self.gate_wait(TIMEOUT, 0)
        self.assertIn(f"gate: waiting for {lock}", output)
        self.assertNotIn("still waiting", output)

    def test_durations_read_for_a_person(self):
        duration = heavy_lock_module._duration
        self.assertEqual([duration(s) for s in (None, 0.4, 59, 60, 3599, 3600, 7500)],
                         ["?", "0 s", "59 s", "1 min", "59 min", "1 h 00 min", "2 h 05 min"])


def failing_flock(lock):
    """Patch `fcntl.flock` so a blocking lock request on `lock` fails with
    ENOLCK, as an exhausted lock table makes it; every other call is real."""
    real = fcntl.flock

    def flock(fd, operation):
        blocking = operation & (fcntl.LOCK_SH | fcntl.LOCK_EX) and not operation & fcntl.LOCK_NB
        if blocking and os.readlink(f"/proc/self/fd/{fd}") == str(lock):
            raise OSError(errno.ENOLCK, os.strerror(errno.ENOLCK))
        return real(fd, operation)

    return mock.patch.object(fcntl, "flock", flock)


class FlockErrorCase(unittest.TestCase):
    """A blocking flock that fails reaches the waiter as an error, never a wait."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name).resolve()

    def failed(self, lock, wait):
        """Hold `lock` from another process, run `wait()` with the blocking
        flock on it failing, and return the error it raised. The wait must end
        within TIMEOUT with every descriptor it opened closed."""
        holder = subprocess.Popen([sys.executable, "-c", HOLDER, str(lock)],
                                  stdout=subprocess.PIPE, text=True, env=clean_env())
        self.addCleanup(stop, holder)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        fds = set(os.listdir("/proc/self/fd"))
        raised = []

        def run():
            try:
                wait()
            except OSError as exc:
                raised.append(exc)

        with failing_flock(lock), contextlib.redirect_stdout(io.StringIO()):
            thread = threading.Thread(target=run, daemon=True)  # a hang must not hold the run
            thread.start()
            thread.join(timeout=TIMEOUT)
        self.assertFalse(thread.is_alive(), "a failed flock must end the wait")
        self.assertEqual([exc.errno for exc in raised], [errno.ENOLCK])
        self.assertEqual(set(os.listdir("/proc/self/fd")), fds, "a descriptor leaked")
        return raised[0]

    def test_the_heavy_lock_queue_head_raises_and_leaves_the_queue(self):
        lock = self.dir / "build" / "continuation-heavy.lock"
        lock.parent.mkdir()

        def wait():
            with hold_heavy_lock(lock, environ=clean_env(), report=io.StringIO()):
                self.fail("granted the lock with the flock failing")

        self.failed(lock, wait)
        queue = Path(f"{lock}.queue")
        self.assertEqual([name for name in os.listdir(queue) if ENTRY_RE.match(name)], [])

    def test_a_gate_waiting_for_the_gate_lock_raises(self):
        lock = gate_lock_path(self.dir)
        lock.parent.mkdir(parents=True)

        def wait():
            with gate_lock(lock, wait=True):
                self.fail("took the gate lock with the flock failing")

        self.failed(lock, wait)


if __name__ == "__main__":
    unittest.main()
