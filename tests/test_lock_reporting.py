"""Diagnostic discovery is advisory: a slow scan cannot delay a kernel grant."""
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
from tools.lock_reporting import Snapshot, bounded_output


class ReportingCase(unittest.TestCase):
    def test_a_blocked_snapshot_returns_pending_then_the_completed_observation(self):
        entered, release = threading.Event(), threading.Event()

        def scan():
            entered.set()
            release.wait(5)
            return [123]

        snapshot = Snapshot(scan)
        self.assertTrue(entered.wait(1))
        try:
            self.assertIsNone(snapshot.value())
            self.assertTrue(snapshot._thread.is_alive())
        finally:
            release.set()
            snapshot._thread.join(2)
        self.assertEqual(snapshot.value(), [123])

    def test_close_prevents_late_publication_and_new_scans(self):
        entered, release = threading.Event(), threading.Event()
        calls = []

        def scan():
            calls.append(1)
            entered.set()
            release.wait(5)
            return [123]

        snapshot = Snapshot(scan, refresh_s=0)
        self.assertTrue(entered.wait(1))
        snapshot.close()
        release.set()
        snapshot.close(wait=True)
        self.assertIsNone(snapshot.value(), "an observation cannot publish after its wait ended")
        self.assertEqual(calls, [1], "closed discovery cannot start another scan")

    def test_unavailable_diagnostics_are_unknown_not_an_empty_census(self):
        def unavailable():
            raise PermissionError("unavailable")

        snapshot = Snapshot(unavailable)
        snapshot._thread.join(2)
        self.assertIsNone(snapshot.value())

    def test_status_has_a_deadline_and_does_not_publish_a_partial_census(self):
        release = threading.Event()
        output = io.StringIO()

        def write(out):
            out.write("holders: none\n")
            release.wait(5)

        try:
            began = time.monotonic()
            bounded_output(write, output, timeout_s=0.02)
            self.assertLess(time.monotonic() - began, 1)
            self.assertIn("diagnostics pending", output.getvalue())
            self.assertNotIn("holders: none", output.getvalue())
        finally:
            release.set()

    def test_complete_status_and_errors_are_preserved(self):
        output = io.StringIO()
        bounded_output(lambda out: out.write("known\n"), output)
        self.assertEqual(output.getvalue(), "known\n")
        def invalid(out):
            raise ValueError("bad input")

        with self.assertRaisesRegex(ValueError, "bad input"):
            bounded_output(invalid, output)

    def test_real_grants_and_periodic_messages_continue_during_a_blocked_scan(self):
        for gate in [False, True]:
            with self.subTest(gate=gate), tempfile.TemporaryDirectory() as tmp:
                lock = Path(tmp) / "lock"
                held = os.open(lock, os.O_RDWR | os.O_CREAT, 0o600)
                fcntl.flock(held, fcntl.LOCK_EX)
                scan_started, release_scan, acquired = (threading.Event() for _ in range(3))
                output, errors, snapshots = io.StringIO(), [], []
                real_modes = heavy_lock.lock_modes

                def modes(path, proc_locks=None, read=heavy_lock.flock_holders.ALWAYS):
                    if read == heavy_lock.flock_holders.WHEN_UNSEEN:
                        scan_started.set()
                        release_scan.wait(5)
                        return []
                    return real_modes(path, proc_locks, read)

                def snapshot(scan):
                    result = Snapshot(scan)
                    snapshots.append(result)
                    return result

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

                with mock.patch.object(heavy_lock, "Snapshot", snapshot), \
                        mock.patch.object(heavy_lock, "lock_modes", modes), \
                        mock.patch.object(heavy_lock, "raw_flock_parent", return_value=None), \
                        mock.patch.object(heavy_lock.flock_holders, "held_here", return_value=False), \
                        mock.patch.object(heavy_lock, "WAIT_REPORT_S", 0.02), \
                        mock.patch.object(heavy_lock, "POLL_S", 0.01):
                    thread = threading.Thread(target=waiter)
                    thread.start()
                    try:
                        self.assertTrue(scan_started.wait(1))
                        deadline = time.monotonic() + 1
                        while output.getvalue().count("diagnostics pending") < 2 \
                                and time.monotonic() < deadline:
                            time.sleep(0.005)
                        self.assertGreaterEqual(output.getvalue().count("diagnostics pending"), 2)
                        self.assertFalse(acquired.is_set(), "diagnostics cannot bypass the holder")
                        os.close(held)
                        held = None
                        self.assertTrue(acquired.wait(1), "the scan cannot delay a real grant")
                        self.assertFalse(release_scan.is_set(), "grant preceded scan completion")
                    finally:
                        release_scan.set()
                        if held is not None:
                            os.close(held)
                        thread.join(2)
                        for observation in snapshots:
                            observation.close(wait=True)
                    self.assertFalse(thread.is_alive())
                    self.assertEqual(errors, [])

    def test_negative_control_synchronous_discovery_does_block_the_report(self):
        entered, release, returned = (threading.Event() for _ in range(3))

        def slow(*args, **kwargs):
            entered.set()
            release.wait(5)
            return []

        with mock.patch.object(heavy_lock, "lock_modes", slow):
            thread = threading.Thread(target=lambda: (heavy_lock._still_line("lock", 1),
                                                     returned.set()))
            thread.start()
            try:
                self.assertTrue(entered.wait(1))
                self.assertFalse(returned.wait(0.02))
            finally:
                release.set()
                thread.join(2)
        self.assertTrue(returned.is_set())
