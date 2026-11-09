"""Public gate/heavy-lock waits progress even when holder discovery is blocked.

The base flake's gate-test glob runs this contract too: a synchronous report
must fail the grant/deadline assertions, without relying on a new helper API.
"""
import fcntl
import io
import os
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from tools import heavy_lock


class GateLockReportingCase(unittest.TestCase):
    def test_a_real_grant_and_periodic_messages_continue_during_blocked_discovery(self):
        for gate in [False, True]:
            with self.subTest(gate=gate), tempfile.TemporaryDirectory() as tmp:
                lock = Path(tmp) / "lock"
                held = os.open(lock, os.O_RDWR | os.O_CREAT, 0o600)
                fcntl.flock(held, fcntl.LOCK_EX)
                entered, release_scan, scan_done, acquired = (threading.Event() for _ in range(4))
                output, errors = io.StringIO(), []
                real_modes = heavy_lock.lock_modes

                def modes(path, proc_locks=None, read=heavy_lock.flock_holders.ALWAYS):
                    if read == heavy_lock.flock_holders.WHEN_UNSEEN:
                        entered.set()
                        release_scan.wait(10)
                        scan_done.set()
                        return []
                    return real_modes(path, proc_locks, read)

                def waiter():
                    try:
                        if gate:
                            handle = heavy_lock.wait_reporting(lock, fcntl.LOCK_EX, output, "test")
                            os.close(handle)
                            acquired.set()
                        else:
                            with heavy_lock.hold_heavy_lock(lock, environ={}, report=output):
                                acquired.set()
                    except BaseException as exc:
                        errors.append(exc)

                with mock.patch.object(heavy_lock, "lock_modes", modes), \
                        mock.patch.object(heavy_lock, "raw_flock_parent", return_value=None), \
                        mock.patch.object(heavy_lock.flock_holders, "held_here", return_value=False), \
                        mock.patch.object(heavy_lock, "WAIT_REPORT_S", 0.02), \
                        mock.patch.object(heavy_lock, "POLL_S", 0.01):
                    thread = threading.Thread(target=waiter)
                    thread.start()
                    try:
                        self.assertTrue(entered.wait(3))
                        deadline = time.monotonic() + 0.2
                        while output.getvalue().count("diagnostics pending") < 2 \
                                and time.monotonic() < deadline:
                            time.sleep(0.005)
                        self.assertFalse(acquired.is_set(), "diagnostics cannot bypass a holder")
                        os.close(held)
                        held = None
                        self.assertTrue(acquired.wait(1), "discovery cannot delay a real grant")
                        self.assertFalse(release_scan.is_set(), "grant preceded scan completion")
                        self.assertGreaterEqual(output.getvalue().count("diagnostics pending"), 2)
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
                    mock.patch.object(heavy_lock, "lock_units", return_value=None):
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

    def test_negative_control_synchronous_discovery_blocks_the_report(self):
        entered, release, returned = (threading.Event() for _ in range(3))

        def slow(*args, **kwargs):
            entered.set()
            release.wait(10)
            return []

        with mock.patch.object(heavy_lock, "lock_modes", slow):
            thread = threading.Thread(target=lambda: (heavy_lock._still_line("lock", 1),
                                                     returned.set()))
            thread.start()
            try:
                self.assertTrue(entered.wait(3))
                self.assertFalse(returned.wait(0.02))
            finally:
                release.set()
                thread.join(3)
        self.assertTrue(returned.is_set())
