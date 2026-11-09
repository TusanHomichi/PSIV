"""Diagnostic discovery is advisory: a slow scan cannot delay a kernel grant."""
import io
import threading
import time
import unittest

from tools.lock_reporting import Snapshot, bounded_output, still_line, wait_line
from tests.test_gate_lock_reporting import track_threads


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
            with track_threads():
                try:
                    bounded_output(write, output, timeout_s=0.02)
                finally:
                    release.set()
            self.assertLess(time.monotonic() - began, 1)
            self.assertIn("diagnostics pending", output.getvalue())
            self.assertNotIn("holders: none", output.getvalue())
        finally:
            release.set()

    def test_complete_status_and_errors_are_preserved(self):
        output = io.StringIO()
        with track_threads():
            bounded_output(lambda out: out.write("known\n"), output)
        self.assertEqual(output.getvalue(), "known\n")
        def invalid(out):
            raise ValueError("bad input")

        with track_threads(), self.assertRaisesRegex(ValueError, "bad input"):
            bounded_output(invalid, output)

    def test_formatting_consumes_only_completed_holder_values(self):
        observation = ((999999, "build", 61, "holder worker"),)
        self.assertIn("pid 999999 (build, up 1 min): holder worker",
                      still_line("lock", 2, observation))
        self.assertIn("held by pid 999999 (build); 1 waiter(s) ahead",
                      wait_line("lock", "exclusive", 1, 0, 0, observation))
