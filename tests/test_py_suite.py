"""Cases for tools/py_suite.py, the gate's parallel Python suite (#166).

    PYTHONPATH=. python3 -m unittest tests.test_py_suite -v

Each run case writes a throwaway suite of small test modules into a temp
directory and runs it through `run_suite` on real spawned workers, so the
pool, the report and the exit code are the ones the gate gets. The parity case
runs the same throwaway suite through `python3 -m unittest discover` too and
holds the two to the same counts.
"""

import io
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.gate import parse_unittest
from tools.py_suite import (PYTHON_MEASURED_WORKERS, SLOW_PYTHON_MODULES, discover_modules,
                            run_suite, schedule, status_line, worker_count)

REPO_ROOT = Path(__file__).resolve().parents[1]

#: Module bodies for the throwaway suites, by what they do.
PASSING = """
    import unittest

    class Case(unittest.TestCase):
        def test_one(self):
            self.assertTrue(True)

        def test_two(self):
            self.assertEqual(1 + 1, 2)
"""
SKIPPING = """
    import unittest

    class Case(unittest.TestCase):
        @unittest.skip("not here")
        def test_skipped(self):
            pass

        def test_runs(self):
            pass
"""
FAILING = """
    import unittest

    class Case(unittest.TestCase):
        def test_fails(self):
            self.fail("this module fails")

        def test_errors(self):
            raise RuntimeError("this module errors")
"""
ASSERTION_ONLY = """
    import unittest

    class Case(unittest.TestCase):
        def test_fails(self):
            self.assertEqual(1, 2)
"""
BROKEN_IMPORT = """
    import a_module_that_does_not_exist  # noqa: F401
"""
#: Kills its own worker process mid-test, as an interpreter crash or the OOM killer would.
KILLED = """
    import os, signal, unittest

    class Case(unittest.TestCase):
        def test_dies(self):
            os.kill(os.getpid(), signal.SIGKILL)
"""


#: Fails its first try and passes every one after, as a flaky test does.
FLAKY_ONCE = """
    import unittest
    from pathlib import Path

    class Case(unittest.TestCase):
        def test_wobbles(self):
            tried = Path(__file__).with_name("tried")
            if not tried.exists():
                tried.write_text("once")
                self.fail("the first try fails")

        def test_steady(self):
            pass
"""
#: Each test fails its first try and skips the next, in its body, its setUp or a subtest: a
#: skip is no pass.
SKIPS_AGAIN = """
    import unittest
    from pathlib import Path

    def first(name):
        tried = Path(__file__).with_name("tried-" + name)
        if tried.exists():
            return False
        tried.write_text("once")
        return True

    class Body(unittest.TestCase):
        def test_skips(self):
            if not first("body"):
                self.skipTest("skipped on its second try")
            self.fail("the first try fails")

    class Setup(unittest.TestCase):
        def setUp(self):
            if Path(__file__).with_name("tried-setup").exists():
                self.skipTest("skipped on its second try")

        def test_skips(self):
            first("setup")
            self.fail("the first try fails")

    class Sub(unittest.TestCase):
        def test_skips(self):
            again = not first("sub")
            with self.subTest(n=1):
                if again:
                    self.skipTest("skipped on its second try")
                self.fail("the first try fails")
"""
#: A flaky test that happens to be named test_case, as a subtest's attribute is.
NAMED_TEST_CASE = """
    import unittest
    from pathlib import Path

    class Case(unittest.TestCase):
        def test_case(self):
            tried = Path(__file__).with_name("tried")
            if not tried.exists():
                tried.write_text("once")
                self.fail("an ordinary test")
"""
#: Fails in one subtest every time.
SUBTEST_FAILS = """
    import unittest

    class Case(unittest.TestCase):
        def test_cases(self):
            for n in range(3):
                with self.subTest(n=n):
                    self.assertNotEqual(n, 1)
"""


class ThrowawaySuite:
    """A temp directory holding `test_<name>.py` for each `{name: body}`."""

    def __init__(self, modules):
        self.tmp = tempfile.TemporaryDirectory()
        self.start = Path(self.tmp.name)
        for name, body in modules.items():
            (self.start / f"test_{name}.py").write_text(textwrap.dedent(body))

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.tmp.cleanup()

    def run(self, workers=2, excluded=(), flaky=None):
        """`run_suite` over the suite: (exit code, report text, its parsed counts).

        It never sees the flaky step the gate running this case may be under: only the
        variables `flaky` gives it.
        """
        stream = io.StringIO()
        variables = {k: v for k, v in os.environ.items() if not k.startswith("FLAKY_")}
        with patch.dict(os.environ, {**variables, **(flaky or {})}, clear=True):
            code = run_suite(self.start, excluded=excluded, workers=workers, stream=stream)
        text = stream.getvalue()
        return code, text, parse_unittest(text)

    def serial(self):
        """`python3 -m unittest discover` over the same suite: its parsed counts."""
        completed = subprocess.run([sys.executable, "-m", "unittest", "discover", "-s",
                                    str(self.start), "-t", str(self.start)],
                                   capture_output=True, text=True, cwd=self.start)
        return completed.returncode, parse_unittest(completed.stderr)


    def flaky(self, expired=()):
        """`run` under burrow's flaky step, with `expired` the tests whose quarantine has run out:
        (exit code, report text, counts, its report's entries)."""
        (self.start / "expired").write_text("".join(f"{name}\n" for name in expired))
        report = self.start / "report.jsonl"
        report.write_text("")
        code, text, counts = self.run(flaky={"FLAKY_EXPIRED": str(self.start / "expired"),
                                             "FLAKY_REPORT": str(report)})
        return code, text, counts, [json.loads(line) for line in report.read_text().splitlines()]


class PlanCase(unittest.TestCase):
    """Which modules run, in what order, on how many workers."""

    def test_every_slow_module_is_a_test_module(self):
        missing = sorted(name for name in SLOW_PYTHON_MODULES
                         if not (REPO_ROOT / "tests" / f"{name}.py").is_file())
        self.assertEqual(missing, [], "SLOW_PYTHON_MODULES names a module tests/ lacks")

    def test_workers_are_the_cores_capped_at_the_measurement(self):
        # 4: the fastest worker count measured on the 4-core host (DEVELOPMENT.md table).
        self.assertEqual(PYTHON_MEASURED_WORKERS, 4)
        self.assertEqual(worker_count(16), 4)
        self.assertEqual(worker_count(8), 4)
        self.assertEqual(worker_count(2), 2)
        self.assertEqual(worker_count(1), 1)

    def test_measured_modules_go_first_longest_first(self):
        order = schedule(["test_b", "test_slow", "test_a", "test_slower"],
                         {"test_slow": 7.0, "test_slower": 30.0})
        self.assertEqual(order, ["test_slower", "test_slow", "test_a", "test_b"])

    def test_discovery_is_unittests_default_pattern(self):
        with ThrowawaySuite({"alpha": PASSING, "beta": PASSING}) as suite:
            # Neither matches `test*.py` as an importable name: a helper module
            # and a file whose stem is not an identifier.
            for stem in ("helper", "test-not-a-name"):
                (suite.start / stem).with_suffix(".py").write_text("")
            (suite.start / "data.json").write_text("{}")
            self.assertEqual(discover_modules(suite.start), ["test_alpha", "test_beta"])

    def test_a_test_package_is_refused_not_skipped(self):
        with ThrowawaySuite({"alpha": PASSING}) as suite:
            (suite.start / "sub").mkdir()
            (suite.start / "sub" / "__init__.py").write_text("")
            with self.assertRaises(ValueError):
                discover_modules(suite.start)

    def test_the_status_line_is_unittests(self):
        zero = dict.fromkeys(("failures", "errors", "skipped", "expected_failures",
                              "unexpected_successes"), 0)
        self.assertEqual(status_line(zero), "OK")
        self.assertEqual(status_line({**zero, "skipped": 2}), "OK (skipped=2)")
        self.assertEqual(status_line({**zero, "failures": 1, "skipped": 2}),
                         "FAILED (failures=1, skipped=2)")


class RunCase(unittest.TestCase):
    """Real runs on spawned workers: counts, failures and crashes all reach the exit code."""

    def test_a_passing_suite_reports_like_unittest(self):
        modules = {"alpha": PASSING, "beta": PASSING, "gamma": SKIPPING}
        with ThrowawaySuite(modules) as suite:
            code, text, counts = suite.run()
            serial_code, serial = suite.serial()
        self.assertEqual(code, 0, text)
        self.assertEqual(counts, {"tests_run": 6, "failures": 0, "errors": 0, "skipped": 1,
                                  "status": "OK"})
        self.assertEqual((serial_code, serial), (code, counts))  # the same as one process

    def test_negative_control_a_failure_in_one_worker_fails_the_step(self):
        modules = {"alpha": PASSING, "beta": PASSING, "gamma": FAILING}
        with ThrowawaySuite(modules) as suite:
            code, text, counts = suite.run()
            serial_code, serial = suite.serial()
        self.assertEqual(code, 1, text)
        self.assertEqual(counts["status"], "FAILED")
        self.assertEqual((counts["failures"], counts["errors"]), (1, 1))
        self.assertIn("FAIL: test_fails (test_gamma.Case.test_fails)", text)
        self.assertIn("this module fails", text)
        self.assertIn("RuntimeError: this module errors", text)
        self.assertEqual((serial_code, serial), (code, counts))

    def test_negative_control_one_failed_assertion_alone_fails_the_step(self):
        with ThrowawaySuite({"alpha": PASSING, "only": ASSERTION_ONLY}) as suite:
            code, text, counts = suite.run()
        self.assertEqual(code, 1, text)
        self.assertEqual((counts["failures"], counts["errors"]), (1, 0))

    def test_negative_control_a_module_that_cannot_import_is_one_error(self):
        with ThrowawaySuite({"alpha": PASSING, "broken": BROKEN_IMPORT}) as suite:
            code, text, counts = suite.run()
            serial_code, serial = suite.serial()
        self.assertEqual(code, 1, text)
        self.assertEqual((counts["tests_run"], counts["errors"]), (3, 1))
        self.assertIn("a_module_that_does_not_exist", text)
        self.assertEqual((serial_code, serial), (code, counts))

    def test_negative_control_a_killed_worker_fails_the_step_and_names_its_modules(self):
        modules = {"alpha": PASSING, "beta": PASSING, "killed": KILLED}
        with ThrowawaySuite(modules) as suite:
            code, text, counts = suite.run(workers=1)
        self.assertEqual(code, 1, text)
        self.assertEqual(counts["status"], "FAILED")
        self.assertGreaterEqual(counts["errors"], 1)
        self.assertIn("a worker process died", text)
        self.assertIn("test_killed", text.split("a worker process died")[1])

    def test_a_killed_worker_names_every_unfinished_module(self):
        """The killer runs before one queued module, with more modules than
        workers: the module queued behind it never runs, and each unfinished
        module is named and counted."""
        modules = {"killed": KILLED, "alpha": PASSING, "beta": PASSING,
                   "zeta": PASSING}
        with ThrowawaySuite(modules) as suite:
            code, text, counts = suite.run(workers=1)
        self.assertEqual(code, 1, text)
        lost = text.split("a worker process died")[1]
        for name in ("test_killed", "test_zeta"):
            self.assertIn(name, lost)
        self.assertEqual(counts["errors"], 2)

    def test_an_excluded_module_does_not_run(self):
        with ThrowawaySuite({"alpha": PASSING, "gamma": FAILING}) as suite:
            code, text, counts = suite.run(excluded={"test_gamma"})
        self.assertEqual(code, 0, text)
        self.assertEqual(counts["tests_run"], 2)

    def test_modules_spread_over_the_workers(self):
        """Each module waits until two worker pids have checked in, so one worker cannot take all."""
        with tempfile.TemporaryDirectory() as seen:
            body = textwrap.dedent(f"""
                import os, time, unittest

                class Case(unittest.TestCase):
                    def test_meets_another_worker(self):
                        open(os.path.join({seen!r}, str(os.getpid())), "w").close()
                        deadline = time.monotonic() + 60
                        while len(os.listdir({seen!r})) < 2:
                            self.assertLess(time.monotonic(), deadline, "alone after 60 s")
                            time.sleep(0.01)
            """)
            with ThrowawaySuite({f"m{index}": body for index in range(4)}) as suite:
                code, text, _ = suite.run(workers=3)
            pids = set(os.listdir(seen))
        self.assertEqual(code, 0, text)
        self.assertGreaterEqual(len(pids), 2, text)
        self.assertLessEqual(len(pids), 3, text)
        self.assertNotIn(str(os.getpid()), pids)  # never in the parent


if __name__ == "__main__":
    unittest.main()


class FlakyCase(unittest.TestCase):
    """Under burrow's flaky step a failed test runs once more; without it, nothing changes."""

    def test_a_test_that_fails_then_passes_is_reported_flaky_and_fails_nothing(self):
        with ThrowawaySuite({"alpha": PASSING, "wobbly": FLAKY_ONCE}) as suite:
            code, text, counts, entries = suite.flaky()
        self.assertEqual(code, 0, text)
        self.assertEqual(counts, {"tests_run": 4, "failures": 0, "errors": 0, "skipped": 0,
                                  "status": "OK"})
        # Its name alone: a test's output never reaches the report.
        self.assertEqual(entries, [{"test": "test_wobbly.Case.test_wobbles"}])
        self.assertIn("test_wobbly.Case.test_wobbles failed, then passed when it ran again", text)

    def test_negative_control_without_the_step_the_same_failure_fails(self):
        with ThrowawaySuite({"alpha": PASSING, "wobbly": FLAKY_ONCE}) as suite:
            code, text, counts = suite.run()
        self.assertEqual(code, 1, text)
        self.assertEqual(counts["failures"], 1)

    def test_without_both_variables_nothing_changes(self):
        with ThrowawaySuite({"alpha": PASSING, "wobbly": FLAKY_ONCE}) as suite:
            (suite.start / "expired").write_text("")
            report = suite.start / "report.jsonl"
            report.write_text("")
            for flaky in ({"FLAKY_REPORT": str(report)},
                          {"FLAKY_REPORT": str(report), "FLAKY_EXPIRED": ""},
                          {"FLAKY_EXPIRED": str(suite.start / "expired")},
                          {"FLAKY_EXPIRED": str(suite.start / "expired"), "FLAKY_REPORT": ""}):
                with self.subTest(flaky=flaky):
                    for tried in suite.start.rglob("tried"):
                        tried.unlink()  # its first try again
                    code, text, counts = suite.run(flaky=flaky)
                    self.assertEqual((code, counts["failures"]), (1, 1), text)
                    self.assertEqual(report.read_text(), "")
                    self.assertNotIn("ran again", text)

    def test_negative_control_a_test_that_fails_again_fails_the_step(self):
        with ThrowawaySuite({"only": ASSERTION_ONLY, "sub": SUBTEST_FAILS}) as suite:
            code, text, counts, entries = suite.flaky()
        self.assertEqual(code, 1, text)
        self.assertEqual((counts["failures"], entries), (2, []))

    def test_negative_control_a_test_whose_quarantine_ran_out_gets_no_second_try(self):
        with ThrowawaySuite({"alpha": PASSING, "wobbly": FLAKY_ONCE}) as suite:
            code, text, counts, entries = suite.flaky(expired=["test_wobbly.Case.test_wobbles"])
        self.assertEqual(code, 1, text)
        self.assertEqual((counts["failures"], entries), (1, []))
        self.assertIn("test_wobbly.Case.test_wobbles's quarantine has run out, so it had no second try",
                      text)
        self.assertNotIn("ran again", text)

    def test_a_retry_that_skips_did_not_pass(self):
        with ThrowawaySuite({"skips": SKIPS_AGAIN}) as suite:
            code, text, counts, entries = suite.flaky()
        self.assertEqual(code, 1, text)
        self.assertEqual((counts["failures"], entries), (3, []))

    def test_a_test_named_test_case_keeps_its_own_id(self):
        with ThrowawaySuite({"named": NAMED_TEST_CASE}) as suite:
            code, text, counts = suite.run()
            self.assertEqual((code, counts["failures"]), (1, 1), text)
            (suite.start / "tried").unlink()  # its first try again
            code, text, counts, entries = suite.flaky()
        self.assertEqual(code, 0, text)
        self.assertEqual(entries, [{"test": "test_named.Case.test_case"}])

    def test_a_module_that_cannot_import_is_not_tried_again_and_still_fails(self):
        with ThrowawaySuite({"alpha": PASSING, "broken": BROKEN_IMPORT}) as suite:
            code, text, counts, entries = suite.flaky()
        self.assertEqual((code, counts["errors"], entries), (1, 1, []), text)
