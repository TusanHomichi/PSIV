"""Cases for tools/gate.py, the repository's one entry point for the gate.

    PYTHONPATH=. python3 -m unittest tests.test_gate -v

Hermetic and quick: every case builds a throwaway git repo in a temp directory
and hands `run_gate` its own fake commands, so nothing here runs the real
gate commands, cargo, or the game. The two guards get both halves: a held
lock and a mapped extension must refuse the run (exit 2, nothing run), and each
must let the same run through once the holder is gone.

The parser samples in `CountParserCase` are captured output - the raw files are
in build/lane-evidence/capture/ (`python3 -m unittest` on a three-test sample,
`cargo test` on a throwaway crate). Traceback bodies are elided to the lines a
reader needs; the rest is verbatim.
"""
import contextlib
import fcntl
import io
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from tools.gate import (FAST_COMMANDS, GATE_COMMANDS, SLOW_PYTHON_MODULES, GateRefused,
                        command_slug, main, parse_counts, quick_unittest, run_gate)
from tools.heavy_lock import (HEAVY_LOCK_ENV, HELD_CLASS_ENV, HELD_ENV, HELD_SLOTS_ENV,
                              PRIORITY_ENV, gate_lock_path, heavy_lock_path)

REPO_ROOT = Path(__file__).resolve().parents[1]
DEVELOPMENT = REPO_ROOT / "docs" / "DEVELOPMENT.md"

DOCS_COMMAND, UNITTEST_COMMAND, FMT_COMMAND, CARGO_TEST_COMMAND, CLIPPY_COMMAND = GATE_COMMANDS
QUICK_UNITTEST_COMMAND = FAST_COMMANDS[3]

# Captured: `python3 -m unittest gate_sample_skip` (build/lane-evidence/capture/
# unittest-skip.capture.txt), minus the shell prompt line.
SAMPLE_UNITTEST_PASS = """.s.
----------------------------------------------------------------------
Ran 3 tests in 0.000s

OK (skipped=1)
"""

# Captured: `python3 -m unittest gate_sample_fail` (unittest-fail.capture.txt),
# with the two traceback bodies elided.
SAMPLE_UNITTEST_FAILED = """EF.
======================================================================
ERROR: test_error (gate_sample_fail.Sample.test_error)
----------------------------------------------------------------------
Traceback (most recent call last):
  File ".../capture/gate_sample_fail.py", line 13, in test_error
    raise RuntimeError("sample error")
RuntimeError: sample error

======================================================================
FAIL: test_failure (gate_sample_fail.Sample.test_failure)
----------------------------------------------------------------------
Traceback (most recent call last):
  File ".../capture/gate_sample_fail.py", line 10, in test_failure
    self.assertEqual(1, 2, "sample failure")
AssertionError: 1 != 2 : sample failure

----------------------------------------------------------------------
Ran 3 tests in 0.001s

FAILED (failures=1, errors=1)
"""

# Captured: `cargo test -- --skip add_one_fails` and `cargo test add_one_fails`
# on a throwaway crate (cargo-pass.capture.txt, cargo-fail.capture.txt): two
# library/doc-test result lines from the first, a failing one from the second.
SAMPLE_CARGO_TEST = """
running 2 tests
i.
test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 1 filtered out; finished in 0.00s


running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s

tests::add_one_fails --- FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s
"""

#: Maps the temp file the guard watches and keeps the mapping alive until killed.
#: Holds `argv[1]` exclusively, says `held`, and keeps it until stdin closes.
HEAVY_HOLDER = """
import fcntl, sys
held = open(sys.argv[1], "a")
fcntl.flock(held, fcntl.LOCK_EX)
print("held", flush=True)
sys.stdin.readline()
"""

MMAP_CHILD = """
import mmap, sys, time
handle = open(sys.argv[1], "r+b")
mapping = mmap.mmap(handle.fileno(), 0)
print("mapped", flush=True)
time.sleep(60)
"""


def run_checks_blocks(text):
    """The command lines of each fenced `bash` block under `## Run checks`.

    The first block is the full gate, the second the fast tier. Every
    non-empty line of a block is a command, so a comment or an extra command
    there shows up as a mismatch against GATE_COMMANDS or FAST_COMMANDS.
    """
    lines = text.splitlines()
    heading = next((i for i, line in enumerate(lines) if line.strip() == "## Run checks"), None)
    if heading is None:
        raise AssertionError("docs/DEVELOPMENT.md has no `## Run checks` heading")
    blocks, inside, collected = [], False, []
    for line in lines[heading + 1:]:
        if inside:
            if line.strip() == "```":
                blocks.append(collected)
                inside, collected = False, []
            elif line.strip():
                collected.append(line.rstrip())
        elif line.strip() == "```bash":
            inside = True
        elif line.startswith("## "):
            break
    if inside:
        raise AssertionError("a `## Run checks` fenced bash block is unclosed")
    return blocks


def fake_step(label, marker=None, exit_code=0):
    """A fake gate command: prints `label ok`, writes `marker`, exits `exit_code`."""
    code = f"print('{label} ok')"
    if marker:
        code += f"; open('{marker}', 'w').write('ran')"
    if exit_code:
        code += f"; raise SystemExit({exit_code})"
    return f'python3 -c "{code}"'


class DocAgreementCase(unittest.TestCase):
    """GATE_COMMANDS and the documented Run checks block are the same list."""

    def test_gate_commands_are_the_run_checks_block(self):
        block = run_checks_blocks(DEVELOPMENT.read_text())[0]
        for command in GATE_COMMANDS:
            self.assertIn(command, block)
        self.assertEqual(block, GATE_COMMANDS)

    def test_fast_commands_are_the_second_run_checks_block(self):
        blocks = run_checks_blocks(DEVELOPMENT.read_text())
        self.assertGreaterEqual(len(blocks), 2, "no fast-tier block under `## Run checks`")
        self.assertEqual(blocks[1], FAST_COMMANDS)

    def test_the_fast_tier_runs_the_full_gates_static_checks(self):
        """fmt, clippy and check_docs are the same commands in both tiers."""
        for command in (DOCS_COMMAND, FMT_COMMAND, CLIPPY_COMMAND):
            self.assertIn(command, FAST_COMMANDS)


class EntryPointCase(unittest.TestCase):
    """The `--list` path: print the commands, run none of them."""

    def test_list_prints_the_commands_and_exits_zero(self):
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            code = main(["--list"])
        self.assertEqual(code, 0)
        self.assertEqual(buffer.getvalue().splitlines(), GATE_COMMANDS)

    def test_fast_list_prints_the_fast_commands(self):
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            code = main(["--fast", "--list"])
        self.assertEqual(code, 0)
        self.assertEqual(buffer.getvalue().splitlines(), FAST_COMMANDS)


class UnitCase(unittest.TestCase):
    """The module's own units: log names per command."""

    def test_command_slugs_name_the_real_commands(self):
        self.assertEqual([command_slug(command) for command in GATE_COMMANDS],
                         ["python3-tools-check_docs.py", "python3-unittest", "cargo-fmt", "cargo-test", "cargo-clippy"])


class CountParserCase(unittest.TestCase):
    """Parsed counts, from captured sample output (see the module docstring)."""

    def test_unittest_pass_with_a_skip(self):
        self.assertEqual(parse_counts(UNITTEST_COMMAND, SAMPLE_UNITTEST_PASS),
                         {"tests_run": 3, "failures": 0, "errors": 0, "skipped": 1,
                          "status": "OK"})

    def test_unittest_failures_and_errors(self):
        self.assertEqual(parse_counts(UNITTEST_COMMAND, SAMPLE_UNITTEST_FAILED),
                         {"tests_run": 3, "failures": 1, "errors": 1, "skipped": 0,
                          "status": "FAILED"})

    def test_cargo_test_sums_every_test_result_line(self):
        self.assertEqual(parse_counts(CARGO_TEST_COMMAND, SAMPLE_CARGO_TEST),
                         {"passed": 2, "failed": 1, "ignored": 1, "suites": 3})

    def test_counts_are_null_when_the_log_reports_nothing(self):
        crashed = "error: could not compile `psiv-core`\n"
        self.assertEqual(parse_counts(UNITTEST_COMMAND, crashed),
                         {"tests_run": None, "failures": None, "errors": None,
                          "skipped": None, "status": None})
        self.assertEqual(parse_counts(CARGO_TEST_COMMAND, crashed),
                         {"passed": None, "failed": None, "ignored": None, "suites": 0})

    def test_the_quick_unittest_command_is_counted_as_unittest(self):
        self.assertEqual(parse_counts(QUICK_UNITTEST_COMMAND, SAMPLE_UNITTEST_FAILED),
                         parse_counts(UNITTEST_COMMAND, SAMPLE_UNITTEST_FAILED))

    def test_only_unittest_and_cargo_test_are_counted(self):
        self.assertEqual(parse_counts(FMT_COMMAND, SAMPLE_UNITTEST_PASS), {})
        self.assertEqual(parse_counts(CLIPPY_COMMAND, SAMPLE_CARGO_TEST), {})


class GateRunCase(unittest.TestCase):
    """A throwaway git repo, a receipt directory and fake commands."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.receipt_dir = self.root / "build" / "gate" / "20260101T000000Z-fixture"
        self.head_sha = self.init_repo()
        # The run's heavy lock is the fixture repo's own, never the real one.
        environ = {key: value for key, value in os.environ.items()
                   if key not in (HEAVY_LOCK_ENV, HELD_ENV, HELD_CLASS_ENV, HELD_SLOTS_ENV,
                                  PRIORITY_ENV)}
        patcher = mock.patch.dict(os.environ, environ, clear=True)
        patcher.start()
        self.addCleanup(patcher.stop)

    def git(self, *args):
        completed = subprocess.run(["git", *args], cwd=self.root, check=True,
                                   stdout=subprocess.PIPE, text=True)
        return completed.stdout.strip()

    def init_repo(self):
        """A committed repo whose `build/` is ignored, so only real edits are dirty."""
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.email", "gate@example.invalid")
        self.git("config", "user.name", "gate test")
        self.git("config", "commit.gpgsign", "false")
        (self.root / ".gitignore").write_text("build/\n")
        (self.root / "README.md").write_text("# throwaway repo\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "init")
        return self.git("rev-parse", "HEAD")

    def run_quiet(self, commands, receipt_dir=None, tier="full"):
        """`run_gate` with stdout captured: (receipt, its non-empty printed lines)."""
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            receipt = run_gate(commands, self.root, receipt_dir or self.receipt_dir, tier)
        return receipt, [line for line in buffer.getvalue().splitlines() if line.strip()]

    def stored(self, receipt_dir=None):
        """The receipt as written to disk."""
        return json.loads(((receipt_dir or self.receipt_dir) / "receipt.json").read_text())

    def test_all_pass_writes_a_complete_receipt(self):
        commands = [fake_step("first"), fake_step("second", marker="second.txt")]
        receipt, printed = self.run_quiet(commands)
        self.assertEqual(receipt["exit_code"], 0)
        self.assertEqual(receipt["candidate_sha"], self.head_sha)
        self.assertFalse(receipt["dirty"])
        self.assertEqual(receipt["dirty_paths"], [])
        self.assertEqual([record["command"] for record in receipt["commands"]], commands)
        self.assertIn("gate OK", receipt["summary"])
        self.assertEqual(printed[-1], receipt["summary"])  # last stdout line is the summary
        self.assertEqual(self.stored(), receipt)  # and it is what the receipt holds
        for record in receipt["commands"]:
            self.assertEqual(record["exit_code"], 0)
            self.assertGreaterEqual(record["duration_s"], 0.0)
            self.assertRegex(record["start_utc"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
            self.assertLessEqual(record["start_utc"], record["end_utc"])
            log = self.root / record["log"]
            self.assertTrue(log.is_file(), record["log"])
            self.assertIn("ok", log.read_text())
        self.assertTrue((self.root / "second.txt").is_file())

    def test_a_failure_is_recorded_and_later_commands_still_run(self):
        commands = [fake_step("first", marker="first.txt"),
                    fake_step("second", marker="second.txt", exit_code=3),
                    fake_step("third", marker="third.txt")]
        receipt, _ = self.run_quiet(commands)
        self.assertEqual(receipt["exit_code"], 1)
        self.assertEqual([record["exit_code"] for record in receipt["commands"]], [0, 3, 0])
        self.assertTrue((self.root / "second.txt").is_file())
        self.assertTrue((self.root / "third.txt").is_file())  # ran despite the failure
        self.assertIn("gate FAILED", receipt["summary"])
        self.assertIn("second ok", (self.root / receipt["commands"][1]["log"]).read_text())
        stored = self.stored()
        self.assertEqual(stored["exit_code"], 1)
        self.assertEqual(stored["summary"], receipt["summary"])
        self.assertEqual(stored["commands"][1]["exit_code"], 3)

    def test_the_fast_tier_summary_names_the_tier_not_the_gate(self):
        receipt, _ = self.run_quiet([fake_step("only")], tier="fast")
        self.assertEqual(receipt["tier"], "fast")
        self.assertTrue(receipt["summary"].startswith("fast tier OK: 1/1"), receipt["summary"])
        failed, _ = self.run_quiet([fake_step("bad", exit_code=1)],
                                   self.receipt_dir.with_name("fast-failed"), tier="fast")
        self.assertTrue(failed["summary"].startswith("fast tier FAILED: 1/1"),
                        failed["summary"])
        self.assertNotIn("gate OK", failed["summary"] + receipt["summary"])

    def test_the_run_holds_the_heavy_lock_and_exports_the_marker(self):
        probe = ("python3 -c \"import os; open('marker.txt', 'w')"
                 f".write(os.environ.get('{HELD_ENV}', ''))\"")
        receipt, _ = self.run_quiet([probe])
        self.assertEqual(receipt["exit_code"], 0)
        self.assertEqual((self.root / "marker.txt").read_text(),
                         str(self.root / "build" / "continuation-heavy.lock"))

    def test_receipt_records_a_dirty_tree_and_its_paths(self):
        (self.root / "scratch.txt").write_text("uncommitted\n")
        receipt, _ = self.run_quiet([fake_step("only")])
        self.assertTrue(receipt["dirty"])
        self.assertEqual(receipt["dirty_paths"], ["?? scratch.txt"])

    def test_counts_come_from_the_command_log(self):
        (self.root / "sample_tests.py").write_text(
            "import unittest\n\n\n"
            "class Sample(unittest.TestCase):\n"
            "    def test_one(self):\n"
            "        self.assertEqual(1, 1)\n\n"
            "    @unittest.skip('sample skip')\n"
            "    def test_two(self):\n"
            "        self.fail('not run')\n")
        receipt, _ = self.run_quiet(["python3 -m unittest sample_tests"])
        self.assertEqual(receipt["exit_code"], 0)
        self.assertEqual(receipt["commands"][0]["counts"],
                         {"tests_run": 2, "failures": 0, "errors": 0, "skipped": 1,
                          "status": "OK"})
        self.assertIn("Ran 2 tests, failures=0, errors=0, skipped=1", receipt["summary"])

    def test_a_held_lock_refuses_the_second_gate(self):
        lock_file = gate_lock_path(self.root)
        lock_file.parent.mkdir(parents=True, exist_ok=True)
        with lock_file.open("a") as held:
            fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(GateRefused) as caught:
                self.run_quiet([fake_step("refused", marker="refused.txt")])
        self.assertEqual(caught.exception.exit_code, 2)
        self.assertIn(str(lock_file), str(caught.exception))
        self.assertFalse((self.root / "refused.txt").exists())  # nothing ran
        self.assertFalse((self.receipt_dir / "receipt.json").exists())
        receipt, _ = self.run_quiet([fake_step("after")])  # released: it starts again
        self.assertEqual(receipt["exit_code"], 0)
        receipt, _ = self.run_quiet([fake_step("again")])  # and the run released its own
        self.assertEqual(receipt["exit_code"], 0)

    def test_a_gate_queued_for_the_heavy_lock_holds_no_gate_lock(self):
        """The heavy lock comes first, so a waiting gate never holds the gate
        lock: its queue alone orders gates (a priority gate is not stuck behind
        a normal one that took the gate lock first), and no gate waits on the
        heavy lock while holding what a heavy holder's nested gate needs."""
        heavy = heavy_lock_path(self.root)
        heavy.parent.mkdir(parents=True, exist_ok=True)
        holder = subprocess.Popen([sys.executable, "-c", HEAVY_HOLDER, str(heavy)],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.addCleanup(self.stop, holder)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        output, outcome = io.StringIO(), {}

        def gate():
            outcome["receipt"] = run_gate([fake_step("queued")], self.root, self.receipt_dir,
                                          wait=True)

        with contextlib.redirect_stdout(output):
            thread = threading.Thread(target=gate)
            thread.start()
            for _ in range(300):  # bounded: 30 s for the gate to queue
                if "waiting for" in output.getvalue():
                    break
                time.sleep(0.1)
            self.assertIn(f"waiting for {heavy}", output.getvalue())
            lock_file = gate_lock_path(self.root)
            lock_file.parent.mkdir(parents=True, exist_ok=True)
            with lock_file.open("a") as probe:  # free while the gate waits
                fcntl.flock(probe, fcntl.LOCK_EX | fcntl.LOCK_NB)
                fcntl.flock(probe, fcntl.LOCK_UN)
            holder.stdin.close()
            thread.join(timeout=60)
        self.assertFalse(thread.is_alive())
        self.assertEqual(outcome["receipt"]["exit_code"], 0)

    def test_a_loaded_extension_refuses_until_the_holder_exits(self):
        extension = self.root / "rust" / "target" / "debug" / "libpsiv_godot.so"
        extension.parent.mkdir(parents=True)
        extension.write_bytes(b"\x7fELF gate fixture\n")
        child = subprocess.Popen([sys.executable, "-c", MMAP_CHILD, str(extension)],
                                 stdout=subprocess.PIPE, text=True)
        self.addCleanup(self.stop, child)
        self.assertEqual(child.stdout.readline().strip(), "mapped")
        with self.assertRaises(GateRefused) as caught:
            self.run_quiet([fake_step("refused", marker="refused.txt")])
        self.assertEqual(caught.exception.exit_code, 2)
        self.assertIn(str(child.pid), str(caught.exception))
        self.assertFalse((self.root / "refused.txt").exists())  # nothing ran
        self.stop(child)
        receipt, _ = self.run_quiet([fake_step("after")])  # the mapping is gone
        self.assertEqual(receipt["exit_code"], 0)

    @staticmethod
    def stop(child):
        """Stop `child` by its own handle, waiting for the mapping to disappear."""
        if child.poll() is None:
            child.terminate()
        try:
            child.wait(timeout=15)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=15)
        if child.stdout:
            child.stdout.close()

    def test_negative_control_the_failure_case_can_fail(self):
        """The failure case's fixture with the failing command made to pass.

        `test_a_failure_is_recorded_and_later_commands_still_run` gets the same
        three commands with the middle one exiting 3; here it exits 0, so that
        case's `exit_code == 1`, `[0, 3, 0]` and `"gate FAILED"` would all be
        false - the check is not vacuous. Nothing needs restoring: both cases
        build the fixture from `fake_step`, and this one holds the passing
        variant it just made.
        """
        commands = [fake_step("first"), fake_step("second"), fake_step("third")]
        receipt, _ = self.run_quiet(commands)
        self.assertEqual([record["exit_code"] for record in receipt["commands"]], [0, 0, 0])
        self.assertNotEqual(receipt["exit_code"], 1)
        self.assertNotIn("gate FAILED", receipt["summary"])


class QuickUnittestCase(unittest.TestCase):
    """The fast tier's Python suite: discovery minus SLOW_PYTHON_MODULES."""

    def test_every_slow_module_is_a_test_module(self):
        missing = sorted(name for name in SLOW_PYTHON_MODULES
                         if not (REPO_ROOT / "tests" / f"{name}.py").is_file())
        self.assertEqual(missing, [], "SLOW_PYTHON_MODULES names a module tests/ lacks")

    def run_quick(self, excluded):
        """quick_unittest over a temp suite: one passing and one failing module."""
        with tempfile.TemporaryDirectory() as tmp:
            start = Path(tmp)
            for name, body in (("test_quick_pass", "self.assertTrue(True)"),
                               ("test_quick_fail", "self.fail('slow module ran')")):
                (start / f"{name}.py").write_text(
                    "import unittest\n\n\nclass Case(unittest.TestCase):\n"
                    f"    def test_it(self):\n        {body}\n")
            stream = io.StringIO()
            code = quick_unittest(start, excluded=excluded, stream=stream)
            for name in ("test_quick_pass", "test_quick_fail"):
                sys.modules.pop(name, None)
        return code, stream.getvalue()

    def test_an_excluded_module_does_not_run(self):
        code, output = self.run_quick({"test_quick_fail"})
        self.assertEqual(code, 0, output)
        self.assertIn("Ran 1 test", output)

    def test_negative_control_without_the_exclusion_the_failure_runs(self):
        code, output = self.run_quick(set())
        self.assertEqual(code, 1, output)
        self.assertIn("Ran 2 tests", output)
        self.assertIn("slow module ran", output)


class NotARepositoryCase(unittest.TestCase):
    """A root git cannot describe still gets a receipt, saying why it could not."""

    def test_receipt_records_that_git_could_not_describe_the_tree(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            receipt_dir = root / "build" / "gate" / "run"
            buffer = io.StringIO()
            with contextlib.redirect_stdout(buffer):
                receipt = run_gate([fake_step("only")], root, receipt_dir,
                                   heavy_lock=root / "build" / "heavy.lock")
            self.assertEqual(receipt["exit_code"], 0)
            self.assertIsNone(receipt["candidate_sha"])
            self.assertIsNone(receipt["dirty"])
            self.assertEqual(receipt["dirty_paths"], [])
            self.assertIn("git_error", receipt)
            self.assertEqual(json.loads((receipt_dir / "receipt.json").read_text()), receipt)


if __name__ == "__main__":
    unittest.main()
