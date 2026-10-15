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
exits 2, and an unrelated holder is waited for.
"""
import contextlib
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

from tools import verify_candidate
from tools.gate import GateRefused, run_gate
from tools.heavy_lock import (EXIT_HELD_BY_PARENT, HEAVY_LOCK_ENV, HELD_ENV, gate_lock_path,
                              heavy_lock_path, hold_heavy_lock, lock_holders, main_checkout)

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


def clean_env():
    """os.environ without the lock override or the held marker."""
    return {key: value for key, value in os.environ.items()
            if key not in (HEAVY_LOCK_ENV, HELD_ENV)}


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
        """verify_candidate's certify step holds the main checkout's heavy lock."""
        held = []

        def runner(command, log, lock):
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
        self.assertEqual(locks, [self.main / "build" / "continuation-heavy.lock"])

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
        done = self.run_child(str(TOOLS / "heavy_lock.py"), "run", "--", sys.executable,
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


if __name__ == "__main__":
    unittest.main()
