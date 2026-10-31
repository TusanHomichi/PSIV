"""tools/heavy_lock.py: one lock file per repository, and a holder that cannot
wait on itself.

    PYTHONPATH=. python3 -m unittest tests.test_heavy_lock -v

Hermetic: `SharedLockCase` builds a throwaway repository with a linked worktree
in a temp directory. The lock paths must name the same file from both
checkouts; the negative control shows the old per-checkout default (`build/`
relative to the working directory) fails that same check, and the gate cases
show a gate started in the worktree refuses (or with `--wait`, waits) while the
main checkout's gate lock is held. `AcquireCase` runs real processes on a temp
lock file, each bounded by a timeout so a regression fails instead of hanging:
a nested acquire under the marker proceeds, a child under a raw-flock parent
exits 2, and an unrelated holder is waited for. `QueueCase` runs real
`heavy_lock.py run` holders and waiters and asserts on the order in which they
wrote to a shared file while holding the lock: a priority waiter goes first, the
godot class shares the machine up to its cap, an exclusive holder excludes it,
and a killed waiter does not block the queue. The memory budget's cases are
in tests/test_heavy_lock_budget.py, on the same `LockProcesses` fixture.
"""
import contextlib
import fcntl
import io
import os
import select
import signal
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from tools import heavy_lock as heavy_lock_module
from tools import verify_candidate
from tools.gate import GateRefused, run_gate
from tools.heavy_lock import (EXIT_HELD_BY_PARENT, GODOT_SLOTS_ENV, HEAVY_LOCK_ENV, HELD_CLASS_ENV,
                              HELD_ENV, HELD_SLOTS_ENV, PRIORITY_ENV, RUNNER_WAIT_ENV, UNITS_ENV,
                              gate_lock_path, heavy_lock_path, hold_heavy_lock, lock_holders,
                              main_checkout, raw_flock_parent)

REPO_ROOT = Path(__file__).resolve().parents[1]
TOOLS = REPO_ROOT / "tools"
#: Bounds every child process here: a lock that hangs fails the case instead.
TIMEOUT = 30

#: Takes `argv[1]` with hold_heavy_lock, then prints `acquired`.
ACQUIRE_CHILD = f"""
import sys
sys.path.insert(0, {str(TOOLS)!r})
from heavy_lock import hold_heavy_lock
with hold_heavy_lock(sys.argv[1]):
    print("acquired", flush=True)
"""

#: A raw flock(1) stand-in: holds `argv[1]` on its own fd, runs ACQUIRE_CHILD.
RAW_FLOCK_PARENT = """
import fcntl, subprocess, sys
held = open(sys.argv[1], "a")
fcntl.flock(held, fcntl.LOCK_EX)
sys.exit(subprocess.run([sys.executable, "-c", sys.argv[2], sys.argv[1]]).returncode)
"""

#: An unrelated holder: holds `argv[1]`, says so, and keeps it until killed.
HOLDER = """
import fcntl, sys, time
held = open(sys.argv[1], "a")
fcntl.flock(held, fcntl.LOCK_EX)
print("held", flush=True)
time.sleep(120)
"""


LOCK_KEYS = (HEAVY_LOCK_ENV, HELD_ENV, HELD_CLASS_ENV, HELD_SLOTS_ENV, PRIORITY_ENV,
             GODOT_SLOTS_ENV, UNITS_ENV, RUNNER_WAIT_ENV)


def clean_env():
    """os.environ without the lock override, the held markers, priority, a slot
    cap or a job runner's wait directory (a test's waits aren't its runner's)."""
    return {key: value for key, value in os.environ.items() if key not in LOCK_KEYS}


def old_relative_default(start):
    """verify_candidate's former default: `build/continuation-heavy.lock` under the cwd."""
    return (Path(start) / "build" / "continuation-heavy.lock").resolve()


class SharedLockCase(unittest.TestCase):
    """A main checkout and a linked worktree of one throwaway repository."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        base = Path(tmp.name).resolve()
        self.main = base / "main"
        self.worktree = base / "lanes" / "lane-a"
        self.main.mkdir()
        self.git(self.main, "init", "-q", "-b", "main")
        self.git(self.main, "config", "user.email", "lock@example.invalid")
        self.git(self.main, "config", "user.name", "lock test")
        self.git(self.main, "config", "commit.gpgsign", "false")
        (self.main / ".gitignore").write_text("build/\n")
        self.git(self.main, "add", "-A")
        self.git(self.main, "commit", "-q", "-m", "init")
        self.git(self.main, "worktree", "add", "-q", "-b", "lane-a", str(self.worktree))
        self.nested = self.worktree / "tools"
        self.nested.mkdir()
        patcher = mock.patch.dict(os.environ, clean_env(), clear=True)
        patcher.start()
        self.addCleanup(patcher.stop)

    @staticmethod
    def git(cwd, *args):
        subprocess.run(["git", *args], cwd=cwd, check=True, stdout=subprocess.DEVNULL)

    def assert_one_file(self, resolve):
        """`resolve(start)` names one file from the main checkout, the worktree
        and a directory inside the worktree."""
        paths = {start: resolve(start) for start in (self.main, self.worktree, self.nested)}
        self.assertEqual(len(set(paths.values())), 1, paths)
        return paths[self.main]

    def test_heavy_lock_is_one_file_for_every_checkout(self):
        lock = self.assert_one_file(lambda start: heavy_lock_path(start, environ={}))
        self.assertEqual(lock, self.main / "build" / "continuation-heavy.lock")

    def test_gate_lock_is_one_file_for_every_checkout(self):
        lock = self.assert_one_file(gate_lock_path)
        self.assertEqual(lock, self.main / "build" / "gate" / ".lock")

    def test_main_checkout_from_the_worktree(self):
        self.assertEqual(main_checkout(self.worktree), self.main)

    def test_the_environment_overrides_the_heavy_lock(self):
        override = self.worktree / "elsewhere.lock"
        self.assertEqual(heavy_lock_path(self.worktree, environ={HEAVY_LOCK_ENV: str(override)}),
                         override)

    def test_negative_control_the_old_relative_default_is_per_checkout(self):
        """The check above, given the default verify_candidate used before, fails."""
        with self.assertRaises(AssertionError):
            self.assert_one_file(old_relative_default)

    def test_verify_candidate_takes_the_shared_lock(self):
        """verify_candidate's build step holds the main checkout's heavy lock
        (certify and the gate take it themselves)."""
        held = []

        def runner(command, log, lock, lock_class):
            held.append(lock)
            return 0

        environ = {key: value for key, value in os.environ.items() if key != HEAVY_LOCK_ENV}
        cwd = Path.cwd()
        with mock.patch.dict(os.environ, environ, clear=True), \
                contextlib.redirect_stdout(io.StringIO()), \
                contextlib.redirect_stderr(io.StringIO()):
            os.chdir(self.worktree)
            try:
                verify_candidate.main([], runner=runner)
            finally:
                os.chdir(cwd)
        locks = [lock for lock in held if lock is not None]
        self.assertEqual(locks, [self.main / "build" / "continuation-heavy.lock"])  # build

    def test_a_gate_in_the_worktree_refuses_while_the_main_checkouts_gate_runs(self):
        lock_file = gate_lock_path(self.main)
        lock_file.parent.mkdir(parents=True, exist_ok=True)
        marker = self.worktree / "ran.txt"
        command = f"python3 -c \"open('{marker}', 'w').write('ran')\""
        receipt_dir = self.worktree / "build" / "gate" / "run"
        with lock_file.open("a") as held:
            fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(GateRefused) as caught, \
                    contextlib.redirect_stdout(io.StringIO()):
                run_gate([command], self.worktree, receipt_dir)
        self.assertIn(str(lock_file), str(caught.exception))
        self.assertFalse(marker.exists())

    def test_a_gate_with_wait_waits_for_the_main_checkouts_gate(self):
        lock_file = gate_lock_path(self.main)
        lock_file.parent.mkdir(parents=True, exist_ok=True)
        holder = subprocess.Popen([sys.executable, "-c", HOLDER, str(lock_file)],
                                  stdout=subprocess.PIPE, text=True)
        self.addCleanup(stop, holder)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        marker = self.worktree / "ran.txt"
        command = f"python3 -c \"open('{marker}', 'w').write('ran')\""
        output, outcome = io.StringIO(), {}

        def gate():
            outcome["receipt"] = run_gate([command], self.worktree,
                                          self.worktree / "build" / "gate" / "run", wait=True)

        with contextlib.redirect_stdout(output):
            thread = threading.Thread(target=gate)
            thread.start()
            thread.join(timeout=1.0)
            self.assertTrue(thread.is_alive(), "the gate must wait, not refuse")
            self.assertFalse(marker.exists())
            stop(holder)
            thread.join(timeout=TIMEOUT)
        self.assertFalse(thread.is_alive())
        self.assertIn(f"waiting for {lock_file} held by pid {holder.pid}", output.getvalue())
        self.assertEqual(outcome["receipt"]["exit_code"], 0)
        self.assertTrue(marker.exists())


def stop(process):
    """Stop `process` by its own handle; a no-op once it has exited."""
    if process.poll() is None:
        process.kill()
    process.wait(timeout=TIMEOUT)
    if process.stdout:
        process.stdout.close()


class AcquireCase(unittest.TestCase):
    """hold_heavy_lock across real processes, on a temp lock file."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.lock = Path(tmp.name).resolve() / "build" / "continuation-heavy.lock"
        self.lock.parent.mkdir()
        self.env = clean_env()

    def run_child(self, *argv, env=None):
        return subprocess.run([sys.executable, *argv], capture_output=True, text=True,
                              timeout=TIMEOUT, env=env or self.env)

    def test_run_exports_the_marker_and_a_nested_acquire_does_not_block(self):
        env = {**self.env, HEAVY_LOCK_ENV: str(self.lock)}
        done = self.run_child(str(TOOLS / "heavy_lock.py"), "run", "--class", "exclusive",
                              "--", sys.executable,
                              "-c", ACQUIRE_CHILD, str(self.lock), env=env)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stdout.strip(), "acquired")
        self.assertNotIn("waiting", done.stderr)

    def test_negative_control_without_the_marker_the_nested_acquire_is_refused(self):
        """The same nesting through a raw flock parent: no marker, so no pass."""
        done = self.run_child("-c", RAW_FLOCK_PARENT, str(self.lock), ACQUIRE_CHILD)
        self.assertEqual(done.returncode, EXIT_HELD_BY_PARENT, done.stdout + done.stderr)
        self.assertNotIn("acquired", done.stdout)
        self.assertIn("already held by your parent pid", done.stderr)
        self.assertIn("heavy_lock.py run --", done.stderr)

    def test_an_unrelated_holder_is_waited_for(self):
        holder = subprocess.Popen([sys.executable, "-c", HOLDER, str(self.lock)],
                                  stdout=subprocess.PIPE, text=True, env=self.env)
        self.addCleanup(stop, holder)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        self.assertEqual(lock_holders(self.lock), [holder.pid])
        child = subprocess.Popen([sys.executable, "-c", ACQUIRE_CHILD, str(self.lock)],
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                 text=True, env=self.env)
        self.addCleanup(stop, child)
        waiting = child.stderr.readline()
        self.assertIn(f"waiting for {self.lock} held by pid {holder.pid}", waiting)
        time.sleep(0.5)
        self.assertIsNone(child.poll(), "the acquire must wait for an unrelated holder")
        stop(holder)
        out, _ = child.communicate(timeout=TIMEOUT)
        self.assertEqual(child.returncode, 0)
        self.assertEqual(out.strip(), "acquired")

    def test_the_marker_is_set_only_while_held(self):
        environ = {}
        with contextlib.redirect_stderr(io.StringIO()):
            with hold_heavy_lock(self.lock, environ=environ):
                self.assertEqual(environ[HELD_ENV], str(self.lock))
                with hold_heavy_lock(self.lock, environ=environ):  # re-entrant: no block
                    self.assertEqual(environ[HELD_ENV], str(self.lock))
            self.assertNotIn(HELD_ENV, environ)

#: Under the lock: says `held`, then keeps it until its stdin closes.
HOLD_UNTIL_STDIN = 'import sys; print("held", flush=True); sys.stdin.readline()'

#: Under the lock: appends `argv[2]` to the order file `argv[1]`.
APPEND = 'import sys; open(sys.argv[1], "a").write(sys.argv[2] + "\\n")'

#: Takes `argv[1]` itself with hold_heavy_lock (priority from the environment),
#: then appends `argv[3]` to the order file `argv[2]`.
APPEND_UNDER_HOLD = f"""
import sys
sys.path.insert(0, {str(TOOLS)!r})
from heavy_lock import hold_heavy_lock
with hold_heavy_lock(sys.argv[1]):
    open(sys.argv[2], "a").write(sys.argv[3] + "\\n")
"""

#: Asks for `argv[2]` under whatever holds `argv[1]`; prints what it got.
NESTED_REQUEST = f"""
import sys
sys.path.insert(0, {str(TOOLS)!r})
from heavy_lock import hold_heavy_lock
with hold_heavy_lock(sys.argv[1], lock_class=sys.argv[2]) as hold:
    print(hold.lock_class, hold.slots, hold.nested, flush=True)
"""


def read_until(stream, needle, timeout=TIMEOUT):
    """Bytes from an unbuffered pipe up to and including `needle`, as text;
    fails rather than hangs when `needle` does not come within `timeout`."""
    deadline = time.monotonic() + timeout
    seen = b""
    while needle.encode() not in seen:
        remaining = deadline - time.monotonic()
        ready, _, _ = select.select([stream], [], [], max(remaining, 0))
        if not ready:
            raise AssertionError(f"{needle!r} did not arrive in {timeout} s; got {seen!r}")
        chunk = os.read(stream.fileno(), 4096)
        if not chunk:
            raise AssertionError(f"{needle!r} never arrived before EOF; got {seen!r}")
        seen += chunk
    return seen.decode()


def with_class(flags):
    """`flags` with `--class exclusive` added when they name no class."""
    return flags if "--class" in flags else ("--class", "exclusive", *flags)


class LockProcesses(unittest.TestCase):
    """Real holders and waiters on a temp lock. Each waiter appends its name to
    `self.order` while it holds the lock, so the file is the grant order."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        base = Path(tmp.name).resolve()
        self.lock = base / "build" / "continuation-heavy.lock"
        self.lock.parent.mkdir()
        self.order = base / "order.txt"
        self.env = {**clean_env(), HEAVY_LOCK_ENV: str(self.lock)}

    def spawn(self, argv, env=None):
        process = subprocess.Popen([sys.executable, *argv], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0,
                                   env={**self.env, **(env or {})})
        self.addCleanup(stop, process)
        self.addCleanup(process.stderr.close)
        self.addCleanup(process.stdin.close)
        return process

    def holder(self, lock_class="exclusive", env=None):
        """A `run --class` holder that has said `held`."""
        process = self.spawn([str(TOOLS / "heavy_lock.py"), "run", "--class", lock_class,
                              "--", sys.executable, "-c", HOLD_UNTIL_STDIN], env)
        read_until(process.stdout, "held")
        return process

    def waiter(self, name, *flags, env=None):
        """A `run` waiter that has printed its first waiting line (so it is queued)."""
        process = self.spawn([str(TOOLS / "heavy_lock.py"), "run", *with_class(flags), "--",
                              sys.executable, "-c", APPEND, str(self.order), name], env)
        process.first_line = read_until(process.stderr, "\n")
        return process

    @staticmethod
    def release(process):
        process.stdin.close()
        process.wait(timeout=TIMEOUT)

    def granted(self):
        return self.order.read_text().split() if self.order.exists() else []

    def finish(self, *waiters):
        for process in waiters:
            self.assertEqual(process.wait(timeout=TIMEOUT), 0)
        return self.granted()

    def immediate(self, name, *flags, env=None):
        """A `run` that must finish without waiting: (exit code, stderr)."""
        done = subprocess.run(
            [sys.executable, str(TOOLS / "heavy_lock.py"), "run", *with_class(flags), "--",
             sys.executable, "-c", APPEND, str(self.order), name],
            capture_output=True, text=True, timeout=TIMEOUT, env={**self.env, **(env or {})})
        return done.returncode, done.stderr


class QueueCase(LockProcesses):
    """The queue, the godot class and the slot cap; see LockProcesses."""

    def test_run_without_a_class_prints_the_classes_and_runs_nothing(self):
        done = subprocess.run(
            [sys.executable, str(TOOLS / "heavy_lock.py"), "run", "--",
             sys.executable, "-c", APPEND, str(self.order), "classless"],
            capture_output=True, text=True, timeout=TIMEOUT, env=self.env)
        self.assertEqual(done.returncode, 2, done.stderr)
        for name in heavy_lock_module.LOCK_CLASSES:
            self.assertIn(f"  {name} ", done.stderr)
        self.assertEqual(self.granted(), [])
        self.assertFalse(self.lock.exists(), "nothing was taken")

    def test_negative_control_the_same_run_with_a_class_runs(self):
        code, stderr = self.immediate("classed", "--class", "exclusive")
        self.assertEqual(code, 0, stderr)
        self.assertEqual(self.granted(), ["classed"])

    def acquisition_order(self, *second_flags, second_env=None):
        """Holder, then waiter `first`, then waiter `second`; release; the grant order."""
        holder = self.holder()
        first = self.waiter("first")
        self.assertIn(f"held by pid {holder.pid} (exclusive)", first.first_line)
        second = self.waiter("second", *second_flags, env=second_env)
        self.assertEqual(self.granted(), [], "a holder is never preempted")
        self.release(holder)
        return self.finish(first, second)

    def assert_priority_first(self, order):
        self.assertEqual(order, ["second", "first"])

    def test_a_priority_waiter_acquires_before_an_earlier_normal_waiter(self):
        self.assert_priority_first(self.acquisition_order("--priority"))

    def test_negative_control_without_the_flag_arrival_order_holds(self):
        order = self.acquisition_order()
        self.assertEqual(order, ["first", "second"])
        with self.assertRaises(AssertionError):
            self.assert_priority_first(order)

    def test_the_environment_form_marks_hold_heavy_lock_as_priority(self):
        holder = self.holder()
        first = self.waiter("first")
        second = self.spawn(["-c", APPEND_UNDER_HOLD, str(self.lock), str(self.order), "second"],
                            env={PRIORITY_ENV: "1"})
        self.assertIn("this waiter: exclusive, priority", read_until(second.stderr, "\n"))
        self.release(holder)
        self.assert_priority_first(self.finish(first, second))

    def test_a_waiter_names_the_holder_and_the_priority_waiters_ahead(self):
        holder = self.holder()
        first = self.waiter("first")
        self.assertIn("0 waiter(s) ahead, 0 of them priority", first.first_line)
        second = self.waiter("second", "--priority")
        self.assertIn("0 waiter(s) ahead", second.first_line)
        # The earlier waiter reprints when a priority waiter queues ahead of it.
        line = read_until(first.stderr, "1 waiter(s) ahead, 1 of them priority")
        self.assertIn(f"held by pid {holder.pid}", line.splitlines()[-1])
        self.release(holder)
        self.finish(first, second)

    def test_godot_holders_share_the_machine_up_to_the_cap(self):
        cap = {GODOT_SLOTS_ENV: "2"}
        one, two = self.holder("godot", cap), self.holder("godot", cap)  # both at once
        third = self.waiter("third", "--class", "godot", env=cap)
        self.assertIn(f"{one.pid} (godot)", third.first_line)
        exclusive = self.waiter("exclusive", env=cap)
        self.release(one)
        self.assertEqual(third.wait(timeout=TIMEOUT), 0)
        self.assertEqual(self.granted(), ["third"], "a godot holder excludes an exclusive run")
        self.release(two)
        self.assertEqual(self.finish(exclusive), ["third", "exclusive"])

    def test_negative_control_a_larger_cap_admits_the_third_godot_holder(self):
        cap = {GODOT_SLOTS_ENV: "3"}
        for _ in range(3):
            self.holder("godot", cap)  # fails (no `held`) if the third had to wait

    def test_an_exclusive_holder_excludes_godot_runs(self):
        holder = self.holder()
        godot = self.waiter("godot", "--class", "godot")
        self.assertIn(f"held by pid {holder.pid} (exclusive)", godot.first_line)
        self.assertEqual(self.granted(), [])
        self.release(holder)
        self.assertEqual(self.finish(godot), ["godot"])

    def kernel_waiters(self):
        """PIDs blocked in flock(2) on the lock file (`->` lines in /proc/locks)."""
        inode = os.stat(self.lock).st_ino
        pids = []
        for line in Path("/proc/locks").read_text().splitlines():
            fields = line.split()
            if len(fields) > 6 and fields[1] == "->" \
                    and int(fields[6].rsplit(":", 1)[1]) == inode:
                pids.append(int(fields[5]))
        return pids

    def test_the_head_waits_in_the_kernel_like_a_raw_flock_waiter(self):
        """So a release wakes it together with any raw flock(1) waiter, instead
        of the raw waiter winning every race against a poll."""
        holder = self.holder()
        first = self.waiter("first")
        second = self.waiter("second")
        blocked = False
        for _ in range(150):  # bounded: 30 s for the head's blocker to enter flock(2)
            blocked = first.pid in self.kernel_waiters()
            if blocked:  # a later read may skip it (#204), so this one counts
                break
            time.sleep(0.2)
        self.assertTrue(blocked, "the head never blocked in flock(2)")
        self.assertNotIn(second.pid, self.kernel_waiters(), "only the head blocks")
        self.release(holder)
        self.assertEqual(self.finish(first, second), ["first", "second"])

    def test_a_godot_arrival_queues_behind_a_waiting_exclusive_run(self):
        """A stream of godot runs cannot starve an exclusive waiter: a godot
        request that could share the current holders still queues behind it."""
        godot = self.holder("godot")
        exclusive = self.waiter("exclusive")
        later = self.waiter("later", "--class", "godot")  # fails here if it jumped the queue
        self.assertIn("1 waiter(s) ahead", later.first_line)
        self.release(godot)
        self.assertEqual(self.finish(exclusive, later), ["exclusive", "later"])

    def test_a_stray_file_in_the_queue_is_ignored_and_kept(self):
        queue = Path(f"{self.lock}.queue")
        queue.mkdir()
        for stray in ("notes.txt", "1-abc", "0-000000000001-1-unknown"):
            (queue / stray).write_text("not a waiter\n")
        holder = self.holder()
        waiter = self.waiter("waiter")
        self.release(holder)
        self.assertEqual(self.finish(waiter), ["waiter"])
        self.assertEqual(sorted(path.name for path in queue.iterdir()),
                         ["0-000000000001-1-unknown", "1-abc", "notes.txt"])

    def test_an_entry_removed_during_a_scan_is_not_an_error(self):
        """A dead waiter's entry that someone else removes between this scan's
        lock test and its unlink (the race a leave outside the mutex opened)."""
        queue = heavy_lock_module._Queue(self.lock)
        (queue.dir / "1-000000000001-999999-exclusive").write_text("")  # dead: unlocked
        real = os.unlink

        def raced(path, *args, **kwargs):
            real(path)  # the concurrent remover gets there first
            return real(path, *args, **kwargs)

        with mock.patch.object(heavy_lock_module.os, "unlink", raced):
            self.assertEqual(queue.entries(), [])

    def test_an_aborted_wait_leaves_the_queue_under_the_mutex(self):
        holder = self.holder()
        queue = heavy_lock_module._Queue(self.lock)
        under_mutex = []
        real_leave = heavy_lock_module._Queue.leave

        def leave(this, entry):
            under_mutex.append(os.getpid() in lock_holders(this.mutex))
            real_leave(this, entry)

        def boom(*args):
            raise RuntimeError("interrupted while waiting")

        with mock.patch.object(heavy_lock_module._Queue, "leave", leave), \
                mock.patch.object(heavy_lock_module, "_wait_line", boom), \
                self.assertRaises(RuntimeError):
            with hold_heavy_lock(self.lock, environ={}, report=io.StringIO()):
                self.fail("the lock is held by another process")
        self.assertEqual(under_mutex, [True])
        self.assertEqual(os.listdir(queue.dir), [])
        self.release(holder)

    def test_a_held_slot_beyond_this_cap_is_refused(self):
        """Another process runs with a larger PSIV_GODOT_SLOTS (it holds slot 3)."""
        slot = self.spawn(["-c", HOLDER_SLOT, f"{self.lock}.godot3"])
        read_until(slot.stdout, "held")
        done = subprocess.run(
            [sys.executable, str(TOOLS / "heavy_lock.py"), "run", "--class", "godot", "--",
             sys.executable, "-c", APPEND, str(self.order), "small-cap"],
            capture_output=True, text=True, timeout=TIMEOUT,
            env={**self.env, GODOT_SLOTS_ENV: "3"})
        self.assertEqual(done.returncode, EXIT_HELD_BY_PARENT, done.stderr)
        self.assertIn("every process must use the same value", done.stderr)
        self.assertEqual(self.granted(), [])

    def test_negative_control_the_same_cap_as_the_slot_holder_proceeds(self):
        slot = self.spawn(["-c", HOLDER_SLOT, f"{self.lock}.godot3"])
        read_until(slot.stdout, "held")
        done = subprocess.run(
            [sys.executable, str(TOOLS / "heavy_lock.py"), "run", "--class", "godot", "--",
             sys.executable, "-c", APPEND, str(self.order), "same-cap"],
            capture_output=True, text=True, timeout=TIMEOUT,
            env={**self.env, GODOT_SLOTS_ENV: "4"})
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(self.granted(), ["same-cap"])

    def godot_arrival(self, cap):
        return subprocess.run(
            [sys.executable, str(TOOLS / "heavy_lock.py"), "run", "--class", "godot", "--",
             sys.executable, "-c", APPEND, str(self.order), f"cap-{cap}"],
            capture_output=True, text=True, timeout=TIMEOUT,
            env={**self.env, GODOT_SLOTS_ENV: cap})

    def test_a_larger_cap_arrival_is_refused_by_the_cap_a_holder_recorded(self):
        self.holder("godot", {GODOT_SLOTS_ENV: "3"})  # records 3 in its slot file
        done = self.godot_arrival("5")
        self.assertEqual(done.returncode, EXIT_HELD_BY_PARENT, done.stderr)
        self.assertIn(f"{GODOT_SLOTS_ENV}=3, this one has 5", done.stderr)
        self.assertEqual(self.granted(), [])

    def test_negative_control_an_arrival_with_the_holders_cap_proceeds(self):
        self.holder("godot", {GODOT_SLOTS_ENV: "3"})
        done = self.godot_arrival("3")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(self.granted(), ["cap-3"])

    def test_a_killed_waiter_does_not_block_the_queue(self):
        holder = self.holder()
        dead = self.waiter("dead")
        dead.send_signal(signal.SIGKILL)
        dead.wait(timeout=TIMEOUT)
        alive = self.waiter("alive")
        self.assertIn("0 waiter(s) ahead", alive.first_line)
        self.release(holder)
        self.assertEqual(self.finish(alive), ["alive"])

    def test_an_exclusive_request_under_a_godot_holder_is_refused(self):
        done = subprocess.run(
            [sys.executable, str(TOOLS / "heavy_lock.py"), "run", "--class", "godot", "--",
             sys.executable, "-c", NESTED_REQUEST, str(self.lock), "exclusive"],
            capture_output=True, text=True, timeout=TIMEOUT, env=self.env)
        self.assertEqual(done.returncode, EXIT_HELD_BY_PARENT, done.stdout + done.stderr)
        self.assertIn("excludes nobody", done.stderr)

    def test_a_godot_request_under_an_exclusive_holder_gets_every_slot(self):
        done = subprocess.run(
            [sys.executable, str(TOOLS / "heavy_lock.py"), "run", "--class", "exclusive", "--",
             sys.executable, "-c", NESTED_REQUEST, str(self.lock), "godot"],
            capture_output=True, text=True, timeout=TIMEOUT,
            env={**self.env, GODOT_SLOTS_ENV: "4"})
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stdout.split(), ["exclusive", "4", "True"])

    def test_raw_flock_parent_names_only_an_unmarked_ancestor(self):
        probe = (f"import sys; sys.path.insert(0, {str(TOOLS)!r}); "
                 "from heavy_lock import raw_flock_parent; print(raw_flock_parent(sys.argv[1]))")
        raw = subprocess.run([sys.executable, "-c", RAW_FLOCK_PARENT_PRINT, str(self.lock),
                              probe], capture_output=True, text=True, timeout=TIMEOUT,
                             env=self.env)
        self.assertEqual(raw.stdout.split(), [raw.stdout.split()[0]] * 2, raw.stderr)
        marked = subprocess.run([sys.executable, str(TOOLS / "heavy_lock.py"), "run",
                                 "--class", "exclusive", "--",
                                 sys.executable, "-c", probe, str(self.lock)],
                                capture_output=True, text=True, timeout=TIMEOUT, env=self.env)
        self.assertEqual(marked.stdout.strip(), "None")
        self.assertIsNone(raw_flock_parent(self.lock, environ={}))  # free: nobody


#: Holds the slot file `argv[1]` (as a godot holder would), says `held`,
#: and keeps it until stdin closes.
HOLDER_SLOT = """
import fcntl, sys
held = open(sys.argv[1], "a")
fcntl.flock(held, fcntl.LOCK_EX)
print("held", flush=True)
sys.stdin.readline()
"""

#: RAW_FLOCK_PARENT, but printing its own pid before running the probe child.
RAW_FLOCK_PARENT_PRINT = """
import fcntl, os, subprocess, sys
held = open(sys.argv[1], "a")
fcntl.flock(held, fcntl.LOCK_EX)
print(os.getpid(), flush=True)
sys.exit(subprocess.run([sys.executable, "-c", sys.argv[2], sys.argv[1]]).returncode)
"""


if __name__ == "__main__":
    unittest.main()
