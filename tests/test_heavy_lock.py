"""tools/heavy_lock.py: one lock file per repository, not one per worktree.

    PYTHONPATH=. python3 -m unittest tests.test_heavy_lock -v

Hermetic: each case builds a throwaway repository with a linked worktree in a
temp directory. The lock paths must name the same file from both checkouts;
the negative control shows the old per-checkout default (`build/` relative to
the working directory) fails that same check, and the gate case shows a gate
started in the worktree refuses while the main checkout's gate lock is held.
"""
import contextlib
import fcntl
import io
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools import verify_candidate
from tools.gate import GateRefused, run_gate
from tools.heavy_lock import (HEAVY_LOCK_ENV, gate_lock_path, heavy_lock_path,
                              main_checkout)


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

    def test_a_gate_in_the_worktree_waits_for_the_main_checkouts_gate(self):
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


if __name__ == "__main__":
    unittest.main()
