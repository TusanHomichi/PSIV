#!/usr/bin/env python3
"""tools/py_suite.py - the gate's Python suite, one test module per task across processes (#166).

    PYTHONPATH=. python3 tools/py_suite.py [--fast] [--jobs N]

Runs the modules `python3 -m unittest discover -s tests` would run, each as one
task, on a pool of worker processes started fresh (`spawn`), so a module's
tests run in its worker exactly as `unittest` runs them, fixtures and all
(`setUpModule`, `setUpClass`). A worker keeps running modules until the queue
is empty, so one-time process fixtures (the pack's map-independent sections,
the shared PackFixtureCase pack, the map table) are paid once per worker, not
once per module. `--fast` leaves out SLOW_PYTHON_MODULES: the per-commit
tier. The full gate runs every module.

The pool has min(available cores, PYTHON_MEASURED_WORKERS) workers, or
`--jobs N`; the cap is the worker count measured to fit the gate's lock class.
Modules go out longest first: SLOW_PYTHON_MODULES by their measured seconds,
then the rest in name order, so a slow module starts early instead of ending
the run alone.

The report is unittest's: each failure's and error's block, then `Ran N tests
in Xs` and `OK` or `FAILED (failures=.., errors=.., skipped=..)`, summed over
every module, which is what the gate's receipt parses. A module that cannot be
imported is one error, as `discover` reports it. A worker that dies (killed,
or a crash in the interpreter) fails the step: every module it had not
finished is reported by name and counted as one error, never dropped. Exit 0
when every test passed, 1 otherwise.

Stdlib only, Python 3.
"""

from __future__ import annotations

import argparse
import importlib
import io
import os
import sys
import time
import traceback
import unittest
from concurrent.futures import ProcessPoolExecutor, as_completed
from concurrent.futures.process import BrokenProcessPool
from multiprocessing import get_context
from pathlib import Path

TOOLS_DIR = str(Path(__file__).resolve().parent)

#: What discovery imports: `unittest discover`'s default pattern.
PATTERN = "test*.py"

#: Python test modules the fast tier leaves to the full gate: every module whose
#: intrinsic time is 6 s or more. Intrinsic means measured in a process whose
#: one-time fixtures are already built -- the pack's map-independent sections
#: (#130), the shared PackFixtureCase pack and the map table, ~13 s that the
#: first module to need them pays in any process -- taking the lower of a forward
#: and a reverse discovery-order run (2026-10-16, CPU capped at 2.3 GHz).
#: Values are those seconds; they also order the pool's queue. Every name must be
#: a module under tests/ (tests/test_py_suite.py checks), so a rename cannot leave
#: a stale entry here.
SLOW_PYTHON_MODULES = {
    "test_pack_maps": 37.1,
    "test_reload_frames": 32.0,
    "test_battle_art_pack": 14.7,
    "test_pack_manifest": 14.2,
    "test_map_patches": 12.8,
    "test_oracle_host_binary": 12.3,
    "test_extract_stamp": 7.7,
    "test_sprites": 7.2,
}

#: The most worker processes the suite runs: the measured fastest count (#166,
#: 2026-10-18, 8-thread 4-core host; docs/DEVELOPMENT.md, "Heavy-run lock"). The
#: full suite took 270 s in one process, then 151, 117, 91, 92, 93 and 100 s on
#: 2, 3, 4, 5, 6 and 8 workers: past the 4 physical cores, each worker's
#: one-time fixtures and the shared cores cost more than the extra worker gains,
#: and 4 peaks lowest of the plateau (1,256 MB tree RSS against 1,639 MB at 8).
#: A different host keeps 4 until it is measured again.
PYTHON_MEASURED_WORKERS = 4


def available_cores():
    """The cores this process may run on, at least 1."""
    try:
        return max(1, len(os.sched_getaffinity(0)))
    except AttributeError:  # not Linux
        return max(1, os.cpu_count() or 1)


def worker_count(cores, measured=PYTHON_MEASURED_WORKERS):
    """Workers for `cores` cores: one per core, capped at the measurement."""
    return max(1, min(cores, measured))


def discover_modules(start):
    """The module names `unittest discover -s start` imports, in name order.

    Discovery's default walk: every `test*.py` file directly under `start`
    whose stem is an identifier, plus any package below it. Packages are
    refused: the suite has none, and splitting by module would need their
    `load_tests` handled.
    """
    start = Path(start)
    names = []
    for path in sorted(start.iterdir()):
        if path.is_dir() and (path / "__init__.py").is_file():
            raise ValueError(f"{path} is a test package; py_suite splits modules only")
        if path.is_file() and path.match(PATTERN) and path.stem.isidentifier():
            names.append(path.stem)
    return names


def schedule(names, seconds=SLOW_PYTHON_MODULES):
    """`names` longest first: those with measured `seconds` by them, then the rest by name."""
    return sorted(names, key=lambda name: (-seconds.get(name, 0.0), name))


def _import_failure(name):
    """A one-test suite whose test errors with `name`'s import traceback, as discover reports it."""
    message = f"Failed to import test module: {name}\n{traceback.format_exc()}"

    def test(self):
        raise ImportError(message)

    case = type("_FailedTest", (unittest.TestCase,), {name: test})
    return unittest.TestSuite([case(name)])


def _worker_setup(start, cwd):
    """A worker's import path, as `python3 -m unittest discover -s <start>` gives the tests.

    Discovery puts the start directory first; `-m` puts the cwd before the rest;
    this script's own tools/ directory is not on the tests' path.
    """
    os.chdir(cwd)
    rest = [entry for entry in sys.path if entry not in (TOOLS_DIR, start, cwd, "")]
    sys.path[:] = [start, cwd] + rest


def run_module(name):
    """Run one test module in this worker; return its counts and report text."""
    stream = _Lines()
    result = unittest.TextTestResult(stream, descriptions=True, verbosity=0)
    loader = unittest.TestLoader()
    try:
        suite = loader.loadTestsFromModule(importlib.import_module(name), pattern=PATTERN)
    except unittest.SkipTest as skip:
        suite = unittest.TestSuite([_skipped_module(name, str(skip))])
    except Exception:  # noqa: BLE001 - any import failure is that module's one error
        suite = _import_failure(name)
    started = time.monotonic()
    result.startTestRun()
    try:
        suite(result)
    finally:
        result.stopTestRun()
    seconds = time.monotonic() - started
    result.printErrors()
    return {
        "module": name,
        "pid": os.getpid(),
        "seconds": round(seconds, 3),
        "run": result.testsRun,
        "failures": len(result.failures),
        "errors": len(result.errors),
        "skipped": len(result.skipped),
        "expected_failures": len(result.expectedFailures),
        "unexpected_successes": len(result.unexpectedSuccesses),
        "report": stream.getvalue(),
    }


class _Lines(io.StringIO):
    """A text buffer with the `writeln` unittest's text result writes through."""

    def writeln(self, text=""):
        self.write(text + "\n")


def _skipped_module(name, reason):
    """A one-test suite skipped with `reason`, as discover reports a module raising SkipTest."""

    @unittest.skip(reason)
    def test(self):
        pass

    return type("ModuleSkipped", (unittest.TestCase,), {name: test})(name)


def status_line(totals):
    """unittest's closing line for `totals`: `OK` or `FAILED`, with its parenthetical."""
    infos = []
    failed = totals["failures"] or totals["errors"] or totals["unexpected_successes"]
    if totals["failures"]:
        infos.append(f"failures={totals['failures']}")
    if totals["errors"]:
        infos.append(f"errors={totals['errors']}")
    if totals["skipped"]:
        infos.append(f"skipped={totals['skipped']}")
    if totals["expected_failures"]:
        infos.append(f"expected failures={totals['expected_failures']}")
    if totals["unexpected_successes"]:
        infos.append(f"unexpected successes={totals['unexpected_successes']}")
    word = "FAILED" if failed else "OK"
    return f"{word} ({', '.join(infos)})" if infos else word


def run_suite(start="tests", excluded=(), workers=None, stream=None, executor_factory=None):
    """Run every discovered module of `start` not in `excluded`; return 0 when all passed.

    `stream` receives the report (default stderr, where unittest writes).
    `executor_factory(workers, start, cwd)` builds the pool; tests pass one to
    inject a failure.
    """
    stream = stream or sys.stderr
    start = str(Path(start).resolve())
    cwd = os.getcwd()
    names = schedule([name for name in discover_modules(start) if name not in excluded])
    workers = workers or worker_count(available_cores())
    factory = executor_factory or _spawn_pool
    totals = dict.fromkeys(("run", "failures", "errors", "skipped", "expected_failures",
                            "unexpected_successes"), 0)
    reports, lost = [], []
    started = time.monotonic()
    print(f"py_suite: {len(names)} modules on {workers} worker processes", file=stream, flush=True)
    with factory(workers, start, cwd) as pool:
        futures = {pool.submit(run_module, name): name for name in names}
        for future in as_completed(futures):
            name = futures[future]
            try:
                record = future.result()
            except BrokenProcessPool:
                lost.append(name)
                continue
            for key in totals:
                totals[key] += record[key]
            if record["report"].strip():
                reports.append(record["report"])
            print(f"py_suite: {name} {record['run']} tests {record['seconds']:.1f}s "
                  f"pid {record['pid']}", file=stream, flush=True)
    for report in reports:
        stream.write(report)
    if lost:
        totals["errors"] += len(lost)
        stream.write("=" * 70 + "\n")
        stream.write(f"ERROR: a worker process died; {len(lost)} module(s) did not finish: "
                     f"{', '.join(sorted(lost))}\n")
    stream.write("-" * 70 + "\n")
    stream.write(f"Ran {totals['run']} tests in {time.monotonic() - started:.3f}s\n\n")
    stream.write(status_line(totals) + "\n")
    stream.flush()
    failed = totals["failures"] or totals["errors"] or totals["unexpected_successes"]
    return 1 if failed else 0


def _spawn_pool(workers, start, cwd):
    """The worker pool: fresh interpreters, each set up as discovery would."""
    return ProcessPoolExecutor(max_workers=workers, mp_context=get_context("spawn"),
                               initializer=_worker_setup, initargs=(start, cwd))


def main(argv=None):
    """`python3 tools/py_suite.py [--fast] [--jobs N] [--start DIR]`, run from the root."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--fast", action="store_true",
                        help="leave out SLOW_PYTHON_MODULES (the fast tier)")
    parser.add_argument("--jobs", type=int, help="worker processes (default: "
                        f"cores, at most {PYTHON_MEASURED_WORKERS})")
    parser.add_argument("--start", default="tests", help="the test directory (default tests)")
    args = parser.parse_args(argv)
    excluded = SLOW_PYTHON_MODULES if args.fast else ()
    return run_suite(args.start, excluded, workers=args.jobs)


if __name__ == "__main__":
    sys.exit(main())
