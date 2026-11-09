"""Public gate/heavy-lock waits progress even when holder discovery is blocked.

The base flake's gate-test glob runs this contract too: a synchronous report
must fail the grant/deadline assertions, without relying on a new helper API.
"""
import contextlib
import fcntl
import io
import os
import tempfile
import sys
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from tools import gate as gate_module
from tools import heavy_lock


def pending_lines(output):
    return sum(line.startswith("still waiting") and "diagnostics pending" in line
               for line in output.getvalue().splitlines())


@contextlib.contextmanager
def track_threads():
    """Join the real threads a public call creates before restoring its mocks."""
    real_thread, threads = threading.Thread, []

    def create(*args, **kwargs):
        thread = real_thread(*args, **kwargs)
        threads.append(thread)
        return thread

    with mock.patch.object(threading, "Thread", create):
        try:
            yield
        finally:
            for thread in threads:
                if thread.ident is not None:
                    thread.join(3)
                    if thread.is_alive():
                        raise AssertionError("owned diagnostic/kernel thread outlived teardown")


def policy_for(entry):
    return (sys.modules[gate_module.wait_reporting.__module__]
            if entry == "gate_lock" else heavy_lock)


@contextlib.contextmanager
def wait_at(entry, lock, output):
    policy = policy_for(entry)
    if entry == "gate_lock":
        with gate_module.gate_lock(lock, wait=True):
            yield
    elif entry == "wait_reporting":
        handle = policy.wait_reporting(lock, fcntl.LOCK_EX, output, "test")
        try:
            yield
        finally:
            os.close(handle)
    else:
        with policy.hold_heavy_lock(lock, environ={}, report=output):
            yield


class GateLockReportingCase(unittest.TestCase):
    def test_a_real_grant_and_periodic_messages_continue_during_blocked_discovery(self):
        for entry in ["heavy", "wait_reporting", "gate_lock"]:
            policy = policy_for(entry)
            with self.subTest(entry=entry), tempfile.TemporaryDirectory() as tmp:
                lock = Path(tmp) / "lock"
                held = os.open(lock, os.O_RDWR | os.O_CREAT, 0o600)
                fcntl.flock(held, fcntl.LOCK_EX)
                entered, release_scan, scan_done, acquired = (threading.Event() for _ in range(4))
                output, errors = io.StringIO(), []
                real_modes = policy.lock_modes

                def modes(path, proc_locks=None, read=heavy_lock.flock_holders.ALWAYS):
                    if read == heavy_lock.flock_holders.WHEN_UNSEEN:
                        entered.set()
                        release_scan.wait(10)
                        scan_done.set()
                        return []
                    return real_modes(path, proc_locks, read)

                def waiter():
                    try:
                        with wait_at(entry, lock, output):
                            acquired.set()
                    except BaseException as exc:
                        errors.append(exc)

                with contextlib.redirect_stdout(output), \
                        mock.patch.object(policy, "lock_modes", modes), \
                        mock.patch.object(policy, "raw_flock_parent", return_value=None), \
                        mock.patch.object(policy.flock_holders, "held_here", return_value=False), \
                        mock.patch.object(policy, "WAIT_REPORT_S", 0.02), \
                        mock.patch.object(policy, "POLL_S", 0.01), track_threads():
                    thread = threading.Thread(target=waiter)
                    thread.start()
                    try:
                        self.assertTrue(entered.wait(3))
                        deadline = time.monotonic() + 0.2
                        while pending_lines(output) < 2 \
                                and time.monotonic() < deadline:
                            time.sleep(0.005)
                        self.assertFalse(acquired.is_set(), "diagnostics cannot bypass a holder")
                        os.close(held)
                        held = None
                        self.assertTrue(acquired.wait(1), "discovery cannot delay a real grant")
                        self.assertFalse(release_scan.is_set(), "grant preceded scan completion")
                        self.assertGreaterEqual(pending_lines(output), 2)
                    finally:
                        release_scan.set()
                        if held is not None:
                            os.close(held)
                        thread.join(3)
                        self.assertTrue(scan_done.wait(3))
                    self.assertFalse(thread.is_alive())
                    self.assertEqual(errors, [])

    def test_public_status_returns_with_a_pending_notice_before_discovery_finishes(self):
        with tempfile.TemporaryDirectory() as tmp:
            lock = Path(tmp) / "lock"
            lock.touch()
            entered, release_scan, returned = (threading.Event() for _ in range(3))
            output, errors = io.StringIO(), []

            def modes(*args, **kwargs):
                entered.set()
                release_scan.wait(10)
                return []

            def status():
                try:
                    heavy_lock.status(lock, output)
                except BaseException as exc:
                    errors.append(exc)
                finally:
                    returned.set()

            with mock.patch.object(heavy_lock, "lock_modes", modes), \
                    mock.patch.object(heavy_lock, "lock_units", return_value=None), track_threads():
                thread = threading.Thread(target=status)
                thread.start()
                try:
                    self.assertTrue(entered.wait(3))
                    self.assertTrue(returned.wait(1.5), "status cannot await a diagnostic census")
                    self.assertIn("diagnostics pending", output.getvalue())
                    self.assertFalse(release_scan.is_set())
                finally:
                    release_scan.set()
                    thread.join(3)
                self.assertFalse(thread.is_alive())
                self.assertEqual(errors, [])

    def test_initial_heavy_notice_does_not_wait_for_fdinfo_discovery(self):
        policy = heavy_lock
        with tempfile.TemporaryDirectory() as tmp:
            lock = Path(tmp) / "lock"
            held = os.open(lock, os.O_RDWR | os.O_CREAT, 0o600)
            fcntl.flock(held, fcntl.LOCK_EX)
            entered, release_scan, returned = (threading.Event() for _ in range(3))
            output, errors = io.StringIO(), []

            def modes(path, proc_locks=None, read=policy.flock_holders.ALWAYS):
                if read == policy.flock_holders.NEVER:
                    entered.set()
                    release_scan.wait(10)
                return []

            def waiter():
                try:
                    with wait_at("heavy", lock, output):
                        returned.set()
                except BaseException as exc:
                    errors.append(exc)

            with mock.patch.object(policy, "lock_modes", modes), \
                    mock.patch.object(policy, "raw_flock_parent", return_value=None), \
                    mock.patch.object(policy.flock_holders, "held_here", return_value=False), track_threads():
                thread = threading.Thread(target=waiter)
                thread.start()
                try:
                    deadline = time.monotonic() + 0.2
                    while not entered.is_set() and not output.getvalue() \
                            and time.monotonic() < deadline:
                        time.sleep(0.005)
                    os.close(held)
                    held = None
                    self.assertTrue(returned.wait(1), "fdinfo diagnostics cannot delay a real grant")
                    self.assertFalse(entered.is_set(), "the granting loop does not discover holders")
                    self.assertIn("holder diagnostics pending", output.getvalue())
                finally:
                    release_scan.set()
                    if held is not None:
                        os.close(held)
                    thread.join(3)
                self.assertFalse(thread.is_alive())
                self.assertEqual(errors, [])

    def test_nonwaiting_gate_still_refuses_and_names_the_holder(self):
        with tempfile.TemporaryDirectory() as tmp:
            lock = Path(tmp) / "lock"
            with lock.open("w") as held:
                fcntl.flock(held, fcntl.LOCK_EX)
                with mock.patch.object(gate_module, "lock_holders", return_value=[4242]) as lookup:
                    with self.assertRaisesRegex(gate_module.GateRefused, "pid 4242"):
                        with gate_module.gate_lock(lock, wait=False):
                            self.fail("a held gate cannot grant")
                lookup.assert_called_once_with(lock, read=gate_module.WHEN_UNSEEN)
