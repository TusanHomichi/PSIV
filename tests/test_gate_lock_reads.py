"""tools/heavy_lock.py, the lock the gate queues for: when a waiter reads
/proc/locks, and that its raw `flock(1)` parent check never skips it.

    PYTHONPATH=. python3 -m unittest tests.test_gate_lock_reads -v

A waiter names its holders again and again, and /proc/locks takes ever longer
to read the more locks the machine holds (#383: 3 s a read under another job's
60,000). On tests/test_heavy_lock.py's fixtures (real holders on a temp lock),
mostly under a fake /proc/locks (`flock_holders.PROC_LOCKS`) that names a pid
fdinfo can't see, as another user's would be:

- `UnseenHolderCase`: the line a waiter prints on every change never reads it;
  the line it repeats reads it only when fdinfo shows nobody else holding the
  lock; `status` reads it always (the negative control); the holder line is
  built outside the queue mutex.
- `RawParentCase`: the raw-parent check always reads it, so a parent fdinfo
  can't see is found alone and beside a shared holder fdinfo does see (the
  control: a pid that is no ancestor is not), and an exclusive request under
  one exits 2 before it queues. On the real /proc/locks, a request under a
  parent holding the lock through a hard link exits 2, alone and beside such a
  holder. A queued waiter checks again every `PARENT_RECHECK_S`, leaving out
  its own head handle, so a parent the first check missed is refused after
  joining (the negative control: with no such parent it waits and is
  granted); the check runs outside the queue mutex.

Temp files and child Pythons only, so the flake's gate-tests check runs it.
"""

import fcntl
import io
import os
import subprocess
import sys
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from tests.test_heavy_lock import TIMEOUT, TOOLS, LockProcesses
from tools import heavy_lock as heavy_lock_module
from tools.heavy_lock import EXIT_HELD_BY_PARENT, NORMAL_RANK, hold_heavy_lock

#: A pid no process here has, which only the fake /proc/locks names.
UNSEEN_PID = 4194000

#: Holds `argv[1]` (a hard link to the lock) shared, prints its pid, then runs
#: the probe `argv[2]` on the lock `argv[3]` as its child.
ALIAS_PARENT = """
import fcntl, os, subprocess, sys
held = os.open(sys.argv[1], os.O_RDONLY)
fcntl.flock(held, fcntl.LOCK_SH)
print(os.getpid(), flush=True)
sys.exit(subprocess.run([sys.executable, "-c", sys.argv[2], sys.argv[3]]).returncode)
"""

#: A short recheck period, so a queued waiter's second check comes at once.
RECHECK_S = 0.2

#: Requests `argv[1]` exclusive, checking for a raw parent every RECHECK_S
#: while queued; prints `granted` if it ever gets it.
REQUEST = f"""
import sys
sys.path.insert(0, {str(TOOLS)!r})
import heavy_lock
heavy_lock.PARENT_RECHECK_S = {RECHECK_S}
with heavy_lock.hold_heavy_lock(sys.argv[1], environ={{}}):
    print("granted", flush=True)
"""


class FakeProcLocks(LockProcesses):

    def fake_proc_locks(self, pid, kind="WRITE"):
        """A /proc/locks of one `kind` lock by `pid` on the lock file, which
        `flock_holders` reads for the rest of the case (call again to change
        it)."""
        device = os.stat(self.lock.parent).st_dev
        fake = self.lock.parent / "proc-locks"
        fake.write_text(f"1: FLOCK  ADVISORY  {kind} {pid} {os.major(device):02x}:"
                        f"{os.minor(device):02x}:{os.stat(self.lock).st_ino} 0 EOF\n")
        if not getattr(self, "_faked", False):
            patcher = mock.patch.object(heavy_lock_module.flock_holders, "PROC_LOCKS", fake)
            patcher.start()
            self.addCleanup(patcher.stop)
            self._faked = True

    def queue_entries(self):
        queue = Path(f"{self.lock}.queue")
        return sorted(queue.iterdir()) if queue.exists() else []


class UnseenHolderCase(FakeProcLocks):
    """The waiting lines; see the module docstring."""

    def waiting_lines(self):
        lock = str(self.lock)
        return (heavy_lock_module._wait_line(lock, "exclusive", NORMAL_RANK, []),
                heavy_lock_module._still_line(lock, 5))

    def test_a_holder_fdinfo_sees_is_named_without_reading_proc_locks(self):
        holder = self.holder()
        self.fake_proc_locks(UNSEEN_PID)
        line, still = self.waiting_lines()
        self.assertIn(f"held by pid {holder.pid} (exclusive); 0 waiter(s)", line)
        self.assertIn(f"held by pid {holder.pid} (exclusive, up ", still)
        self.assertNotIn(str(UNSEEN_PID), line + still)

    def test_negative_control_status_still_reads_proc_locks(self):
        holder = self.holder()
        self.fake_proc_locks(UNSEEN_PID)
        out = io.StringIO()
        heavy_lock_module.status(self.lock, out)
        self.assertIn(f"  {holder.pid} exclusive: ", out.getvalue())
        self.assertIn(f"  {UNSEEN_PID} exclusive: ", out.getvalue())

    def test_with_no_holder_fdinfo_sees_the_repeated_line_names_it(self):
        """Not the line printed on every change, which a handoff, when fdinfo
        sees nobody, would otherwise slow."""
        self.lock.touch()
        self.fake_proc_locks(UNSEEN_PID)
        line, still = self.waiting_lines()
        self.assertIn(" held by pid none; ", line)
        self.assertIn(f"held by pid {UNSEEN_PID} (exclusive, up ", still)

    def test_a_waiter_names_its_holders_outside_the_queue_mutex(self):
        """tests/test_heavy_lock.py's aborted-wait case reads the mutex's
        holders the same way and finds this process holding it."""
        self.holder()
        mutex = f"{self.lock}.mutex"
        under_mutex = []

        def named(*args):
            under_mutex.append(os.getpid() in heavy_lock_module.lock_holders(mutex))
            raise RuntimeError("stop waiting")

        with mock.patch.object(heavy_lock_module, "_wait_line", named), \
                self.assertRaises(RuntimeError):
            with hold_heavy_lock(self.lock, environ={}, report=io.StringIO()):
                self.fail("the lock is held by another process")
        self.assertEqual(under_mutex, [False])


class RawParentCase(FakeProcLocks):
    """The raw `flock(1)` parent check; see the module docstring."""

    def test_an_unseen_parent_alone_is_refused(self):
        """Another user's `flock(1)` above this process, say."""
        self.lock.touch()
        self.fake_proc_locks(os.getppid())
        self.assertEqual(heavy_lock_module.raw_flock_parent(self.lock, environ={}),
                         os.getppid())

    def test_an_unseen_parent_beside_a_visible_shared_holder_is_refused(self):
        holder = self.holder("build")
        self.assertIn(holder.pid, heavy_lock_module.lock_holders(
            self.lock, read=heavy_lock_module.flock_holders.NEVER), "fdinfo sees it")
        self.fake_proc_locks(os.getppid(), "READ")
        self.assertEqual(heavy_lock_module.raw_flock_parent(self.lock, environ={}),
                         os.getppid())

    def test_a_visible_holder_that_is_no_ancestor_is_no_parent(self):
        """The control for the case above: the holder fdinfo sees, and a pid
        /proc/locks names that is not an ancestor, are neither refused."""
        self.holder("build")
        self.fake_proc_locks(UNSEEN_PID, "READ")
        self.assertIsNone(heavy_lock_module.raw_flock_parent(self.lock, environ={}))

    def test_a_queued_waiter_leaves_itself_out(self):
        """Its own head handle holds the lock once granted, which is no parent;
        before it queues, a lock this process holds still counts."""
        handle = os.open(self.lock, os.O_RDWR | os.O_CREAT, 0o644)
        self.addCleanup(os.close, handle)
        fcntl.flock(handle, fcntl.LOCK_EX)
        self.assertIsNone(heavy_lock_module.raw_flock_parent(self.lock, environ={},
                                                             queued=True))
        self.assertEqual(heavy_lock_module.raw_flock_parent(self.lock, environ={}),
                         os.getpid())

    def alias_parent(self):
        """An exclusive request (REQUEST) under a parent holding the lock shared
        through a hard link, on the real /proc/locks: (the parent's pid, the
        request's exit code, its stderr). One read of /proc/locks can miss the
        parent while the machine's locks churn; the checks while queued do
        not, so the request is refused within TIMEOUT."""
        alias = self.lock.parent.parent / "alias"
        os.link(self.lock, alias)
        done = subprocess.run([sys.executable, "-c", ALIAS_PARENT, str(alias), REQUEST,
                               str(self.lock)], capture_output=True, text=True,
                              timeout=TIMEOUT, env=self.env)
        parent = done.stdout.split()[0]
        self.assertNotIn("granted", done.stdout)
        return parent, done.returncode, done.stderr

    def test_a_parent_holding_a_hard_link_is_refused_alone(self):
        self.lock.touch()
        parent, code, stderr = self.alias_parent()
        self.assertEqual(code, EXIT_HELD_BY_PARENT, stderr)
        self.assertIn(f"already held by your parent pid {parent}", stderr)

    def test_a_parent_holding_a_hard_link_is_refused_beside_a_visible_shared_holder(self):
        self.holder("build")
        parent, code, stderr = self.alias_parent()
        self.assertEqual(code, EXIT_HELD_BY_PARENT, stderr)
        self.assertIn(f"already held by your parent pid {parent}", stderr)

    def start_waiter(self):
        """An in-process exclusive request in a thread, so one that waits
        forever fails the case instead of hanging it: (thread, report,
        outcome), outcome getting `granted` or the exit code."""
        report, outcome = io.StringIO(), []

        def wait():
            try:
                with hold_heavy_lock(self.lock, environ={}, report=report):
                    outcome.append("granted")
            except SystemExit as refused:
                outcome.append(refused.code)

        thread = threading.Thread(target=wait, daemon=True)
        thread.start()
        return thread, report, outcome

    def test_an_exclusive_request_under_an_unseen_parent_exits_2_before_queueing(self):
        """Not queued behind the shared holder, waiting on its own parent."""
        self.holder("build")
        self.fake_proc_locks(os.getppid(), "READ")
        thread, report, outcome = self.start_waiter()
        thread.join(TIMEOUT)
        self.assertFalse(thread.is_alive(), "waiting on its own parent")
        self.assertEqual(outcome, [EXIT_HELD_BY_PARENT])
        self.assertIn(f"already held by your parent pid {os.getppid()}", report.getvalue())
        self.assertNotIn("waiting for", report.getvalue())
        self.assertEqual(self.queue_entries(), [])

    def queued_waiter(self):
        """`start_waiter` behind a holder, checking for a raw parent every
        RECHECK_S, once it has joined the queue: (holder, thread, report,
        outcome)."""
        holder = self.holder()
        self.fake_proc_locks(UNSEEN_PID)
        patcher = mock.patch.object(heavy_lock_module, "PARENT_RECHECK_S", RECHECK_S)
        patcher.start()
        self.addCleanup(patcher.stop)
        thread, report, outcome = self.start_waiter()
        deadline = time.monotonic() + TIMEOUT
        while "waiting for" not in report.getvalue() and time.monotonic() < deadline:
            time.sleep(0.05)
        self.assertIn("waiting for", report.getvalue(), "the waiter never queued")
        self.assertEqual(len(self.queue_entries()), 1)
        return holder, thread, report, outcome

    def test_a_parent_the_first_check_missed_is_refused_after_queueing(self):
        _, thread, report, outcome = self.queued_waiter()
        self.fake_proc_locks(os.getppid())
        thread.join(TIMEOUT)
        self.assertFalse(thread.is_alive(), "still waiting on its own parent")
        self.assertEqual(outcome, [EXIT_HELD_BY_PARENT])
        self.assertIn(f"already held by your parent pid {os.getppid()}", report.getvalue())
        self.assertEqual(self.queue_entries(), [])

    def test_negative_control_without_a_parent_it_waits_and_is_granted(self):
        holder, thread, _, outcome = self.queued_waiter()
        time.sleep(5 * RECHECK_S)
        self.assertTrue(thread.is_alive())
        self.release(holder)
        thread.join(TIMEOUT)
        self.assertEqual(outcome, ["granted"])

    def test_the_check_runs_outside_the_queue_mutex(self):
        self.holder()
        mutex = f"{self.lock}.mutex"
        under_mutex = []

        def check(*args, **kwargs):
            under_mutex.append(os.getpid() in heavy_lock_module.lock_holders(mutex))
            raise RuntimeError("stop waiting")

        with mock.patch.object(heavy_lock_module, "raw_flock_parent", check), \
                self.assertRaises(RuntimeError):
            with hold_heavy_lock(self.lock, environ={}, report=io.StringIO()):
                self.fail("the lock is held by another process")
        self.assertEqual(under_mutex, [False])


if __name__ == "__main__":
    unittest.main()
