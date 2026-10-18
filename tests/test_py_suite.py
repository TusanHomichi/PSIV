"""Cases for tools/py_suite.py, the gate's parallel Python suite (#166).

    PYTHONPATH=. python3 -m unittest tests.test_py_suite -v

Each run case writes a throwaway suite of small test modules into a temp
directory and runs it through `run_suite` on real spawned workers, so the
pool, the report and the exit code are the ones the gate gets. The parity case
runs the same throwaway suite through `python3 -m unittest discover` too and
holds the two to the same counts.
"""

import io
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

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

    def run(self, workers=2, excluded=()):
        """`run_suite` over the suite: (exit code, report text, its parsed counts)."""
        stream = io.StringIO()
        code = run_suite(self.start, excluded=excluded, workers=workers, stream=stream)
        text = stream.getvalue()
        return code, text, parse_unittest(text)

    def serial(self):
        """`python3 -m unittest discover` over the same suite: its parsed counts."""
        completed = subprocess.run([sys.executable, "-m", "unittest", "discover", "-s",
                                    str(self.start), "-t", str(self.start)],
                                   capture_output=True, text=True, cwd=self.start)
        return completed.returncode, parse_unittest(completed.stderr)


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
