"""Cases for tools/gate.py, the repository's one entry point for the gate.

    PYTHONPATH=. python3 -m unittest tests.test_gate -v

Hermetic and quick: every case builds a throwaway git repo in a temp directory
and hands `run_gate` its own fake commands, so nothing here runs the real
four-command gate, cargo, or the game. The two guards get both halves: a held
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
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tools.gate import (GATE_COMMANDS, GateRefused, command_slug, lock_path, main,
                        parse_counts, run_gate)

REPO_ROOT = Path(__file__).resolve().parents[1]
DEVELOPMENT = REPO_ROOT / "docs" / "DEVELOPMENT.md"

UNITTEST_COMMAND, FMT_COMMAND, CARGO_TEST_COMMAND, CLIPPY_COMMAND = GATE_COMMANDS

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
MMAP_CHILD = """
import mmap, sys, time
handle = open(sys.argv[1], "r+b")
mapping = mmap.mmap(handle.fileno(), 0)
print("mapped", flush=True)
time.sleep(60)
"""


def run_checks_block(text):
    """The command lines of the first fenced `bash` block under `## Run checks`.

    Every non-empty line of that block is a command, so a comment or a second
    command there shows up as a mismatch against GATE_COMMANDS.
    """
    lines = text.splitlines()
    heading = next((i for i, line in enumerate(lines) if line.strip() == "## Run checks"), None)
    if heading is None:
        raise AssertionError("docs/DEVELOPMENT.md has no `## Run checks` heading")
    inside, collected, closed = False, [], False
    for line in lines[heading + 1:]:
        if inside:
            if line.strip() == "```":
                closed = True
                break
            if line.strip():
                collected.append(line.rstrip())
        elif line.strip() == "```bash":
            inside = True
        elif line.startswith("## "):
            break
    if not closed:
        raise AssertionError("the `## Run checks` fenced bash block is missing or unclosed")
    return collected


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
        block = run_checks_block(DEVELOPMENT.read_text())
        for command in GATE_COMMANDS:
            self.assertIn(command, block)
        self.assertEqual(block, GATE_COMMANDS)


class EntryPointCase(unittest.TestCase):
    """The `--list` path: print the commands, run none of them."""

    def test_list_prints_the_commands_and_exits_zero(self):
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            code = main(["--list"])
        self.assertEqual(code, 0)
        self.assertEqual(buffer.getvalue().splitlines(), GATE_COMMANDS)


class UnitCase(unittest.TestCase):
    """The module's own units: log names per command."""

    def test_command_slugs_name_the_real_commands(self):
        self.assertEqual([command_slug(command) for command in GATE_COMMANDS],
                         ["python3-unittest", "cargo-fmt", "cargo-test", "cargo-clippy"])


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

    def run_quiet(self, commands, receipt_dir=None):
        """`run_gate` with stdout captured: (receipt, its non-empty printed lines)."""
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            receipt = run_gate(commands, self.root, receipt_dir or self.receipt_dir)
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
        lock_file = lock_path(self.receipt_dir)
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


class NotARepositoryCase(unittest.TestCase):
    """A root git cannot describe still gets a receipt, saying why it could not."""

    def test_receipt_records_that_git_could_not_describe_the_tree(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            receipt_dir = root / "build" / "gate" / "run"
            buffer = io.StringIO()
            with contextlib.redirect_stdout(buffer):
                receipt = run_gate([fake_step("only")], root, receipt_dir)
            self.assertEqual(receipt["exit_code"], 0)
            self.assertIsNone(receipt["candidate_sha"])
            self.assertIsNone(receipt["dirty"])
            self.assertEqual(receipt["dirty_paths"], [])
            self.assertIn("git_error", receipt)
            self.assertEqual(json.loads((receipt_dir / "receipt.json").read_text()), receipt)


if __name__ == "__main__":
    unittest.main()
