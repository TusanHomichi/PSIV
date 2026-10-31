"""tools/heavy_lock.py and tools/gate.py: a lock's waiter tells a job runner it
waits in line, through the runner's wait directory (`PROJ_WAIT_DIR`, which
burrow's proj sets for each turn of a job and stops counting the turn's time
against its limit while a file there is held).

    PYTHONPATH=. python3 -m unittest tests.test_heavy_lock_runner_wait -v

`RunnerWaitCase` checks `runner_wait` alone: for the block, one file, held
flocked, whose line says what the wait is for, gone afterwards; nothing
without the variable, and a directory that is gone does not stop the wait.
`LockWaitCase` waits behind a real holder: a heavy-lock waiter and a gate
waiting for the gate lock each keep one held file naming their lock while
they wait, and none once granted; the negative control, a free lock, never
makes one. A waiter killed mid-wait leaves a file nobody holds, which is how
the runner knows to count again. The gate's own commands get no wait
directory: their waits are the gate's work.
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

from tests.test_heavy_lock import HOLDER, TIMEOUT, TOOLS, clean_env, stop
from tools.gate import gate_environment, gate_lock
from tools.heavy_lock import RUNNER_WAIT_ENV, gate_lock_path, hold_heavy_lock, runner_wait

#: Waits for the heavy lock `argv[1]` in a process of its own.
WAITER = f"""
import sys
sys.path.insert(0, {str(TOOLS)!r})
from heavy_lock import hold_heavy_lock
with hold_heavy_lock(sys.argv[1]):
    pass
"""


def held(path):
    """Whether anyone holds `path` flocked, as the runner asks it."""
    with open(path) as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return True
        return False


class WaitDir(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.base = Path(tmp.name).resolve()
        self.waits = self.base / "waits"
        self.waits.mkdir()

    def files(self):
        return sorted(self.waits.iterdir())

    def one_file(self):
        """The one wait file, once its waiter holds it and has written its
        whole line (up to TIMEOUT): a waiter makes the file before it locks
        and writes it."""
        deadline = time.monotonic() + TIMEOUT
        while True:
            files = self.files()
            if len(files) == 1 and held(files[0]) and files[0].read_text().endswith("\n"):
                return files[0]
            self.assertLess(time.monotonic(), deadline, f"no held wait file with its line: {files}")
            time.sleep(0.05)


class RunnerWaitCase(WaitDir):
    def test_a_wait_is_a_held_file_saying_what_for_until_it_ends(self):
        with runner_wait("PSIV's gate lock", {RUNNER_WAIT_ENV: str(self.waits)}):
            [path] = self.files()
            self.assertEqual(path.read_text(), "PSIV's gate lock\n")
            self.assertTrue(held(path))
        self.assertEqual(self.files(), [])

    def test_without_the_variable_there_is_no_file(self):
        with runner_wait("PSIV's gate lock", {}):
            self.assertEqual(self.files(), [])

    def test_a_directory_that_is_gone_does_not_stop_the_wait(self):
        ran = []
        with runner_wait("PSIV's gate lock", {RUNNER_WAIT_ENV: str(self.base / "gone")}):
            ran.append(True)
        self.assertEqual(ran, [True])

    def test_a_file_it_cannot_lock_or_remove_does_not_stop_the_wait(self):
        ran = []
        with mock.patch("tools.heavy_lock.fcntl.flock", side_effect=OSError("no locks here")), \
                mock.patch.object(Path, "unlink", side_effect=PermissionError("read-only")), \
                runner_wait("PSIV's gate lock", {RUNNER_WAIT_ENV: str(self.waits)}):
            ran.append(True)
        self.assertEqual(ran, [True])

    def test_a_file_it_cannot_remove_is_left_unheld_and_hides_no_error(self):
        with mock.patch.object(Path, "unlink", side_effect=PermissionError("read-only")):
            with runner_wait("PSIV's gate lock", {RUNNER_WAIT_ENV: str(self.waits)}):
                pass
            with self.assertRaisesRegex(ValueError, "the wait's own"), \
                    runner_wait("PSIV's gate lock", {RUNNER_WAIT_ENV: str(self.waits)}):
                raise ValueError("the wait's own")
        self.assertEqual(len(self.files()), 2)
        self.assertFalse(any(held(path) for path in self.files()))


class LockWaitCase(WaitDir):
    def hold(self, lock):
        """An unrelated holder of `lock`, started and holding."""
        holder = subprocess.Popen([sys.executable, "-c", HOLDER, str(lock)],
                                  stdout=subprocess.PIPE, text=True, env=clean_env())
        self.addCleanup(stop, holder)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        return holder

    def wait_behind(self, holder, wait):
        """Run `wait()` in a thread behind `holder`: the wait file it keeps
        while it waits, and whether any is left once it is through."""
        thread = threading.Thread(target=wait, daemon=True)  # a hang must not hold the run
        thread.start()
        path = self.one_file()
        line, was_held = path.read_text(), held(path)
        self.assertTrue(thread.is_alive(), "the wait must block while the holder holds")
        stop(holder)
        thread.join(timeout=TIMEOUT)
        self.assertFalse(thread.is_alive())
        return line, was_held, self.files()

    def heavy_lock(self):
        lock = self.base / "build" / "continuation-heavy.lock"
        lock.parent.mkdir()
        return lock

    def test_a_heavy_lock_waiter_says_so_while_it_waits(self):
        lock = self.heavy_lock()
        holder = self.hold(lock)

        def wait():
            with hold_heavy_lock(lock, environ={**clean_env(), RUNNER_WAIT_ENV: str(self.waits)},
                                 report=io.StringIO()):
                pass

        self.assertEqual(self.wait_behind(holder, wait),
                         ("PSIV's heavy-run lock (exclusive class)\n", True, []))

    def test_a_gate_waiting_for_the_gate_lock_says_so_while_it_waits(self):
        lock = gate_lock_path(self.base)
        lock.parent.mkdir(parents=True)
        holder = self.hold(lock)

        def wait():
            with contextlib.redirect_stdout(io.StringIO()), gate_lock(lock, wait=True):
                pass

        with mock.patch.dict(os.environ, {RUNNER_WAIT_ENV: str(self.waits)}):
            self.assertEqual(self.wait_behind(holder, wait),
                             ("PSIV's gate lock, while another gate runs\n", True, []))

    def test_a_wait_file_it_cannot_remove_strands_no_lock(self):
        lock = self.heavy_lock()
        holder = self.hold(lock)
        ran = []

        def wait():
            with hold_heavy_lock(lock, environ={**clean_env(), RUNNER_WAIT_ENV: str(self.waits)},
                                 report=io.StringIO()):
                ran.append(True)

        with mock.patch.object(Path, "unlink", side_effect=PermissionError("read-only")):
            _, _, left = self.wait_behind(holder, wait)
        self.assertEqual(ran, [True])
        self.assertEqual(len(left), 1)
        self.assertFalse(held(left[0]))
        # And the lock it was granted went with the block: free again.
        with hold_heavy_lock(lock, environ=clean_env(), report=io.StringIO()):
            pass

    def test_negative_control_a_free_lock_is_no_wait(self):
        lock = self.heavy_lock()
        with hold_heavy_lock(lock, environ={**clean_env(), RUNNER_WAIT_ENV: str(self.waits)},
                             report=io.StringIO()):
            self.assertEqual(self.files(), [])
        self.assertEqual(self.files(), [])

    def test_a_waiter_killed_mid_wait_leaves_a_file_nobody_holds(self):
        lock = self.heavy_lock()
        self.hold(lock)
        waiter = subprocess.Popen([sys.executable, "-c", WAITER, str(lock)],
                                  stderr=subprocess.DEVNULL,
                                  env={**clean_env(), RUNNER_WAIT_ENV: str(self.waits)})
        self.addCleanup(stop, waiter)
        path = self.one_file()
        self.assertTrue(held(path))
        waiter.kill()
        waiter.wait(timeout=TIMEOUT)
        self.assertFalse(held(path))


class GateCommandsCase(unittest.TestCase):
    def test_the_gates_commands_get_no_wait_directory(self):
        env = gate_environment({RUNNER_WAIT_ENV: "/somewhere", "HOME": "/h"}, 16)
        self.assertNotIn(RUNNER_WAIT_ENV, env)
        self.assertEqual(env["HOME"], "/h")


if __name__ == "__main__":
    unittest.main()
