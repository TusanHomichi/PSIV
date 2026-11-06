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
  one exits 2 and leaves the queue. On the real /proc/locks, a request under a
  parent holding the lock through a hard link exits 2, alone and beside such a
  holder. A queued waiter checks again every `PARENT_RECHECK_S`, leaving out
  its own head handle, so a parent the first check missed is refused after
  joining (the negative control: with no such parent it waits and is
  granted). The check runs beside the wait, so a slow one holds up neither
  the waiting line nor the grant (#384); a lock this process holds itself,
  through a hard link too, is found from its own fdinfo before it queues (the
  controls: an open descriptor alone holds nothing, nor does another file's
  lock, whatever a descriptor's file is by the time it is looked at).
- `OwnMountCase`: that hold is matched on the lock file's own devices, its
  `st_dev` and its own mount's, not its directory's: a file mounted apart
  from its directory, as a single-file bind mount is, and a file whose
  `st_dev` is not the device its lock line names (btrfs) are found (the
  controls: with neither device matching, or no lock, nothing is). A real
  single-file bind mount, in a user and mount namespace of its own, runs
  where `unshare` can make one and is skipped elsewhere.

Temp files and child Pythons only, so the flake's gate-tests check runs it.
"""

import fcntl
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
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

    def test_an_exclusive_request_under_an_unseen_parent_exits_2_and_leaves_the_queue(self):
        """Not left queued behind the shared holder, waiting on its own parent."""
        self.holder("build")
        self.fake_proc_locks(os.getppid(), "READ")
        thread, report, outcome = self.start_waiter()
        thread.join(TIMEOUT)
        self.assertFalse(thread.is_alive(), "waiting on its own parent")
        self.assertEqual(outcome, [EXIT_HELD_BY_PARENT])
        self.assertIn(f"already held by your parent pid {os.getppid()}", report.getvalue())
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

    def test_a_slow_check_holds_up_neither_the_waiting_line_nor_the_grant(self):
        """Each check reads all of /proc/locks, which took seconds under
        another job's 60,000 locks; a waiter that waited for it missed its
        30 s deadlines (#383, #384)."""
        holder = self.holder()
        checking = threading.Event()

        def slow(*args, **kwargs):
            checking.wait(10 * TIMEOUT)  # well past every wait below; cleanup ends it

        self.addCleanup(checking.set)
        with mock.patch.object(heavy_lock_module, "raw_flock_parent", slow):
            thread, report, outcome = self.start_waiter()
            deadline = time.monotonic() + TIMEOUT
            while "waiting for" not in report.getvalue() and time.monotonic() < deadline:
                time.sleep(0.05)
            self.assertIn(f"held by pid {holder.pid} (exclusive)", report.getvalue())
            self.release(holder)
            thread.join(TIMEOUT)
        self.assertFalse(thread.is_alive(), "the grant waited for the check")
        self.assertEqual(outcome, ["granted"])

    def test_this_process_holding_the_lock_through_a_hard_link_is_found(self):
        """A raw flock(1) that exec'd this process holds it on a descriptor of
        its own; a hard link is the same file."""
        self.lock.touch()
        alias = self.lock.parent.parent / "alias"
        os.link(self.lock, alias)
        handle = os.open(alias, os.O_RDONLY)
        self.addCleanup(os.close, handle)
        self.assertFalse(heavy_lock_module.flock_holders.held_here(self.lock),
                         "an open descriptor alone holds nothing")
        fcntl.flock(handle, fcntl.LOCK_SH)
        self.assertTrue(heavy_lock_module.flock_holders.held_here(self.lock))
        thread, report, outcome = self.start_waiter()
        thread.join(TIMEOUT)
        self.assertEqual(outcome, [EXIT_HELD_BY_PARENT])
        self.assertIn(f"already held by your parent pid {os.getpid()}", report.getvalue())
        self.assertEqual(self.queue_entries(), [])

    def test_another_files_lock_is_not_this_one_whatever_a_descriptor_now_is(self):
        """A descriptor can close and be reused between two looks at it, so the
        lock record itself must name the file; and a descriptor's own file is
        never touched, which a hung mount could stall. Here every stat of a
        descriptor claims the lock file, while the only lock held is another
        file's and the lock file's own descriptor holds none."""
        self.lock.touch()
        other = self.lock.parent.parent / "other"
        other.touch()
        locked = os.open(other, os.O_RDONLY)
        self.addCleanup(os.close, locked)
        fcntl.flock(locked, fcntl.LOCK_SH)
        unlocked = os.open(self.lock, os.O_RDONLY)
        self.addCleanup(os.close, unlocked)
        real_stat, statted = os.stat, []

        def stat(path, *args, **kwargs):
            if str(path).startswith("/proc/self/fd/"):
                statted.append(path)
                return real_stat(self.lock)
            return real_stat(path, *args, **kwargs)

        flock_holders = heavy_lock_module.flock_holders
        with mock.patch.object(flock_holders.os, "stat", stat):
            self.assertFalse(flock_holders.held_here(self.lock))
        self.assertEqual(statted, [], "a descriptor's own file was touched")


#: In a user and mount namespace of its own: mounts a tmpfs on `argv[2]`,
#: bind-mounts a file of it over the lock `argv[3]`, and prints the lock's
#: `st_dev` and its directory's, and `held_here` open and then locked.
#: Exits 77 when it cannot mount.
BIND_MOUNTED = """
import fcntl, json, os, subprocess, sys
sys.path.insert(0, sys.argv[1])
import flock_holders
source, lock = sys.argv[2], sys.argv[3]
def mount(*args):
    try:
        if subprocess.run(["mount", *args]).returncode == 0:
            return
    except OSError as error:  # no mount(8) here
        print(error, file=sys.stderr)
    sys.exit(77)
mount("-t", "tmpfs", "tmpfs", source)
open(os.path.join(source, "lock"), "w").close()
open(lock, "w").close()
mount("--bind", os.path.join(source, "lock"), lock)
handle = os.open(lock, os.O_RDONLY)
unlocked = flock_holders.held_here(lock)
fcntl.flock(handle, fcntl.LOCK_SH)
print(json.dumps({"directory": os.stat(os.path.dirname(lock)).st_dev,
                  "file": os.stat(lock).st_dev,
                  "unlocked": unlocked, "locked": flock_holders.held_here(lock)}))
"""

#: A device no file here is on.
FOREIGN = (0xfe, 0xfe)


class OwnMountCase(unittest.TestCase):
    """`flock_holders.held_here` matches a lock record on the lock file's own
    devices, which need not be its directory's."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.base = Path(tmp.name).resolve()
        self.lock = self.base / "build" / "continuation-heavy.lock"
        self.lock.parent.mkdir()
        self.lock.touch()
        self.handle = os.open(self.lock, os.O_RDONLY)
        self.addCleanup(os.close, self.handle)
        device = os.stat(self.lock).st_dev
        self.device = (os.major(device), os.minor(device))
        self.assertNotEqual(self.device, FOREIGN)

    def mountinfo(self, *mounts):
        """A mountinfo file of `(device, mount point)` lines, in order."""
        path = self.base / "mountinfo"
        path.write_text("".join(f"{index} 1 {major}:{minor} / {point} rw - tmpfs tmpfs rw\n"
                                for index, ((major, minor), point) in enumerate(mounts, 30)))
        return path

    def stat_on(self, devices):
        """`os.stat`, but a path in `devices` (path -> (major, minor)) reports
        that `st_dev`."""
        real_stat = os.stat

        def stat(path, *args, **kwargs):
            result = real_stat(path, *args, **kwargs)
            if str(path) in devices:
                fields = list(result[:10])
                fields[2] = os.makedev(*devices[str(path)])
                result = os.stat_result(fields)
            return result
        return mock.patch.object(heavy_lock_module.flock_holders.os, "stat", stat)

    def test_a_file_mounted_apart_from_its_directory_is_found(self):
        """Its directory is on another file system, as under a single-file bind
        mount: the directory's devices are not the file's."""
        flock_holders = heavy_lock_module.flock_holders
        mountinfo = self.mountinfo(((0, 1), "/"), (FOREIGN, self.lock.parent),
                                   (self.device, self.lock))
        fcntl.flock(self.handle, fcntl.LOCK_SH)
        with self.stat_on({str(self.lock.parent): FOREIGN}):
            self.assertEqual(flock_holders.lock_devices(self.lock, mountinfo), {FOREIGN},
                             "the directory's devices name only another file system")
            self.assertTrue(flock_holders.held_here(self.lock, mountinfo))

    def test_a_file_whose_st_dev_is_not_its_lock_lines_device_is_found(self):
        """On btrfs a file's `st_dev` is its subvolume's, while its lock line
        names the mount's device."""
        fcntl.flock(self.handle, fcntl.LOCK_SH)
        mountinfo = self.mountinfo(((0, 1), "/"), (self.device, self.base))
        with self.stat_on({str(self.lock): FOREIGN}):
            self.assertTrue(heavy_lock_module.flock_holders.held_here(self.lock, mountinfo))

    def test_negative_control_neither_device_its_lock_lines_is_not_found(self):
        fcntl.flock(self.handle, fcntl.LOCK_SH)
        mountinfo = self.mountinfo((FOREIGN, "/"))
        with self.stat_on({str(self.lock): FOREIGN}):
            self.assertFalse(heavy_lock_module.flock_holders.held_here(self.lock, mountinfo))

    def test_negative_control_its_own_mount_without_a_lock_is_not_held(self):
        mountinfo = self.mountinfo(((0, 1), "/"), (self.device, self.lock))
        self.assertFalse(heavy_lock_module.flock_holders.held_here(self.lock, mountinfo))

    def test_negative_control_a_mount_named_after_the_file_is_not_its(self):
        """`<lock>.x` is another path; a file that does not exist has no devices."""
        flock_holders = heavy_lock_module.flock_holders
        mountinfo = self.mountinfo(((0, 1), "/"), ((0, 9), f"{self.lock}.x"))
        self.assertEqual(flock_holders.file_devices(self.lock, mountinfo), {self.device, (0, 1)})
        self.assertEqual(flock_holders.file_devices(self.base / "gone", mountinfo), set())

    def test_a_real_single_file_bind_mount_is_found(self):
        unshare = shutil.which("unshare")
        if unshare is None:
            self.skipTest("no unshare here")
        source = self.base / "source"
        source.mkdir()
        lock = self.base / "bound" / "continuation-heavy.lock"
        lock.parent.mkdir()
        run = subprocess.run([unshare, "--user", "--map-root-user", "--mount", sys.executable,
                              "-c", BIND_MOUNTED, str(TOOLS), str(source), str(lock)],
                             capture_output=True, text=True, timeout=TIMEOUT)
        if run.returncode == 77 or (run.returncode and "unshare" in run.stderr):
            self.skipTest(f"no bind mount in a namespace here: {run.stderr.strip()}")
        self.assertEqual(run.returncode, 0, run.stderr)
        found = json.loads(run.stdout)
        self.assertNotEqual(found["file"], found["directory"],
                            "the bound file is on a file system of its own")
        self.assertFalse(found["unlocked"], "an open descriptor alone holds nothing")
        self.assertTrue(found["locked"])


if __name__ == "__main__":
    unittest.main()
