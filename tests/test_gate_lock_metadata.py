"""Gate/heavy wait reports cannot block on holder age or command-line reads."""
import contextlib
import fcntl
import io
import os
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from tests.test_gate_lock_reporting import pending_lines, policy_for, track_threads, wait_at


class ReportCancelled(RuntimeError):
    pass


class MetadataWaitCase(unittest.TestCase):
    def exercise(self, entry, field, cancel):
        policy = policy_for(entry)
        entered, release, metadata_done, kernel_done, returned, acquired, cancelled = (
            threading.Event() for _ in range(7))
        errors = []

        class Report(io.StringIO):
            def write(self, text):
                if cancel and entered.is_set() and text.startswith("still waiting") \
                        and pending_lines(self) >= 2:
                    raise ReportCancelled("cancel while metadata is blocked")
                return super().write(text)

        output = Report()
        with tempfile.TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
            lock = Path(tmp) / "lock"
            held = os.open(lock, os.O_RDWR | os.O_CREAT, 0o600)
            fcntl.flock(held, fcntl.LOCK_EX)

            def blocked(pid):
                entered.set()
                release.wait(10)
                metadata_done.set()
                return 3 if field == "process_age" else "holder worker"

            def modes(path, proc_locks=None, read=policy.flock_holders.ALWAYS):
                return [(999999, "exclusive")] if read == policy.flock_holders.WHEN_UNSEEN else []

            real_kernel_wait = policy._Blocker._wait

            def kernel_wait(this, mode):
                try:
                    real_kernel_wait(this, mode)
                finally:
                    kernel_done.set()

            stack.enter_context(contextlib.redirect_stdout(output))
            for target, name, value in [
                (policy, "lock_modes", modes),
                (policy, "raw_flock_parent", lambda *args, **kwargs: None),
                (policy.flock_holders, "held_here", lambda *args: False),
                (policy.flock_holders, "process_age", blocked if field == "process_age" else lambda pid: 3),
                (policy.flock_holders, "cmdline", blocked if field == "cmdline" else lambda pid: "holder worker"),
                (policy, "WAIT_REPORT_S", 0.02),
                (policy, "POLL_S", 0.01),
                (policy._Blocker, "_wait", kernel_wait),
            ]:
                stack.enter_context(mock.patch.object(target, name, value))
            stack.enter_context(track_threads())

            def waiter():
                try:
                    with wait_at(entry, lock, output):
                        acquired.set()
                except ReportCancelled:
                    cancelled.set()
                except BaseException as exc:
                    errors.append(exc)
                finally:
                    returned.set()

            thread = threading.Thread(target=waiter)
            thread.start()
            try:
                self.assertTrue(entered.wait(3), "the tested metadata lookup must run")
                deadline = time.monotonic() + 0.2
                while pending_lines(output) < 2 and time.monotonic() < deadline:
                    time.sleep(0.005)
                self.assertFalse(acquired.is_set(), "diagnostics cannot bypass the real holder")
                if not cancel:
                    os.close(held)
                    held = None
                self.assertTrue(returned.wait(1), "metadata cannot delay a grant or cancellation")
                self.assertFalse(release.is_set(), "the caller returned before metadata completed")
                self.assertGreaterEqual(pending_lines(output), 2)
                self.assertEqual(acquired.is_set(), not cancel)
                self.assertEqual(cancelled.is_set(), cancel)
            finally:
                release.set()
                if held is not None:
                    os.close(held)
                thread.join(3)
                self.assertTrue(metadata_done.wait(3))
                self.assertTrue(kernel_done.wait(3), "the abandoned/granted kernel waiter closed")
            self.assertFalse(thread.is_alive())
            self.assertEqual(errors, [])

    def test_grants_and_messages_survive_blocked_metadata(self):
        for entry in ["heavy", "wait_reporting", "gate_lock"]:
            for field in ["process_age", "cmdline"]:
                with self.subTest(entry=entry, field=field):
                    self.exercise(entry, field, cancel=False)

    def test_cancellation_survives_blocked_metadata(self):
        for entry in ["heavy", "wait_reporting", "gate_lock"]:
            for field in ["process_age", "cmdline"]:
                with self.subTest(entry=entry, field=field):
                    self.exercise(entry, field, cancel=True)
