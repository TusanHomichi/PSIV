"""Cases for tools/gate.py, the repository's one entry point for the gate.

    PYTHONPATH=. python3 -m unittest tests.test_gate -v

Hermetic and quick: every case builds a throwaway git repo in a temp directory
and hands `run_gate` its own fake commands, so nothing here runs the real
gate commands, cargo, or the game. The gate-lock guard gets both halves: a
held lock must refuse the run (exit 2, nothing run) and let the same run
through once the holder is gone. A mapped extension no longer refuses it.

The parser samples in `CountParserCase` are captured output - the raw files are
in build/lane-evidence/capture/ (`python3 -m unittest` on a three-test sample,
`cargo test` on a throwaway crate). Traceback bodies are elided to the lines a
reader needs; the rest is verbatim.
"""
import contextlib
import fcntl
import io
import itertools
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

from tools.gate import (FAST_COMMANDS, GATE_COMMANDS, GATE_INPUTS, GATE_MEASURED_PARALLELISM,
                        GateRefused, available_cores, cargo_parallelism, command_slug, cpu_mhz,
                        gate_environment, gate_lock, main, parse_counts, parse_unittest,
                        run_gate)
from tools.heavy_lock import (HEAVY_LOCK_ENV, HELD_CLASS_ENV, HELD_ENV, HELD_SLOTS_ENV,
                              PRIORITY_ENV, gate_lock_path, heavy_lock_path, lock_holders)

REPO_ROOT = Path(__file__).resolve().parents[1]
DEVELOPMENT = REPO_ROOT / "docs" / "DEVELOPMENT.md"

(DOCS_COMMAND, UNITTEST_COMMAND, FMT_COMMAND, CARGO_TEST_COMMAND, CLIPPY_COMMAND,
 PARITY_COMMAND) = GATE_COMMANDS
FAST_UNITTEST_COMMAND = FAST_COMMANDS[3]

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

#: A gate from before the lock queue: the gate lock, then (on a line from
#: stdin) the heavy lock, both by raw flock; prints `gate`, then `ran`.
OLD_GATE = """
import fcntl, sys
gate = open(sys.argv[1], "a")
fcntl.flock(gate, fcntl.LOCK_EX)
print("gate", flush=True)
sys.stdin.readline()
heavy = open(sys.argv[2], "a")
fcntl.flock(heavy, fcntl.LOCK_EX)
print("ran", flush=True)
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
        slugs = [command_slug(command) for command in GATE_COMMANDS]
        # The Python suite's slug is its program and script path, joined; it is
        # spelled in two halves because tools/check_paths.py reads a whole
        # `<name>.py` token here as a file name that must exist.
        python_suite = slugs.pop(1)
        self.assertTrue(python_suite.startswith("python3-tools-py_suite"), python_suite)
        self.assertTrue(python_suite.endswith(".py"), python_suite)
        parity = slugs.pop()
        self.assertEqual(parity, "python3-tools-pack_parity" + ".py")
        self.assertEqual(slugs, ["python3-tools-check_docs.py", "cargo-fmt", "cargo-test",
                                 "cargo-clippy"])


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

    def test_both_python_suite_commands_are_counted_as_unittest(self):
        """tools/py_suite.py prints unittest's closing lines, so both tiers parse them."""
        expected = parse_unittest(SAMPLE_UNITTEST_FAILED)
        self.assertEqual(expected["failures"], 1)
        for command in (UNITTEST_COMMAND, FAST_UNITTEST_COMMAND,
                        "PYTHONPATH=. python3 -m unittest discover -s tests"):
            self.assertEqual(parse_counts(command, SAMPLE_UNITTEST_FAILED), expected, command)

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
            if cpu_mhz() is not None:  # Linux: a sample before the command can finish
                self.assertGreaterEqual(record["cpu_mhz"]["samples"], 1)
                self.assertLessEqual(record["cpu_mhz"]["min"], record["cpu_mhz"]["max"])
        self.assertEqual(receipt["cpu_count"], available_cores())
        expected = cargo_parallelism(available_cores())
        self.assertEqual((receipt["cargo_build_jobs"], receipt["rust_test_threads"]),
                         (expected, expected))
        self.assertTrue((self.root / "second.txt").is_file())

    def run_sampled(self, loads, samples):
        """One command that exits only after the gate has taken `samples` load samples.

        The command waits on a file the patched `load_1m` writes at its
        `samples`th call, so the sample count is set by the test, not by how
        fast the host runs (a fixed sleep took fewer samples under load). Each
        call returns the next value of `loads`. The command gives up after 60 s.
        """
        release = self.root / "sampled"
        calls = itertools.count(1)

        def load():
            if next(calls) == samples:
                release.write_text("")
            return next(loads)

        waiter = ("import os, sys, time; deadline = time.monotonic() + 60\n"
                  f"while not os.path.exists({str(release)!r}):\n"
                  "    time.sleep(0.01)\n"
                  "    if time.monotonic() > deadline: sys.exit('never released')\n")
        script = self.root / "waiter"
        script.write_text(waiter)
        with mock.patch("tools.gate.CPU_SAMPLE_S", 0.01), \
                mock.patch("tools.gate.cpu_mhz", return_value=2300.0), \
                mock.patch("tools.gate.load_1m", side_effect=load):
            return self.run_quiet([f"python3 {script}"])[0]

    def test_the_clock_is_sampled_while_a_command_runs(self):
        """A longer command gets one sample per CPU_SAMPLE_S, not one in all."""
        loads = itertools.chain([1.0, 9.5], itertools.repeat(2.0))
        record = self.run_sampled(loads, samples=20)["commands"][0]
        self.assertEqual(record["exit_code"], 0)
        self.assertEqual(record["cpu_mhz"]["mean"], 2300)
        self.assertGreaterEqual(record["cpu_mhz"]["samples"], 20)
        self.assertEqual(record["load_1m_max"], 9.5)  # the highest, not the first or last

    def test_commands_see_the_capped_cargo_environment(self):
        """What a command's cargo reads, end to end, on a 16- and a 4-core host."""
        echo = f'echo "$CARGO_BUILD_JOBS $RUST_TEST_THREADS" > {self.root / "seen"}'
        for cores, expected in ((16, "8 8"), (4, "4 4")):
            with mock.patch("tools.gate.available_cores", return_value=cores), \
                    mock.patch.dict(os.environ, {"CARGO_BUILD_JOBS": "1"}):
                receipt, _ = self.run_quiet([echo])
            self.assertEqual((self.root / "seen").read_text().strip(), expected)
            self.assertEqual(receipt["cargo_build_jobs"], int(expected.split()[0]))

    def test_negative_control_a_finite_load_supply_runs_out(self):
        """The pre-fix form: a fixed list of loads, shorter than the samples taken.

        A list sized for one host's speed ran out when the command ran longer;
        here the command outlives three values, so `next` raises in the gate's
        sampling loop, which kills the command and re-raises.
        """
        with self.assertRaises(StopIteration):
            self.run_sampled(iter([1.0, 9.5, 2.0]), samples=10)

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

    def test_every_command_runs_in_the_exclusive_class_and_the_receipt_says_so(self):
        probe = ("python3 -c \"import os; open('class.txt', 'a')"
                 f".write(os.environ.get('{HELD_CLASS_ENV}', 'none') + '\\\\n')\"")
        receipt, _ = self.run_quiet([probe, probe])
        self.assertEqual(receipt["exit_code"], 0)
        self.assertEqual((self.root / "class.txt").read_text().split(), ["exclusive"] * 2)
        self.assertEqual(receipt["heavy_lock_class"], "exclusive")
        self.assertEqual([record["lock_class"] for record in receipt["commands"]],
                         ["exclusive"] * 2)
        self.assertEqual(self.stored()["heavy_lock_class"], "exclusive")

    def test_negative_control_a_gate_under_a_build_holder_is_refused(self):
        """The class is real: under a holder that excludes nobody the gate's
        exclusive request exits 2 and runs nothing."""
        heavy = heavy_lock_path(self.root)
        with mock.patch.dict(os.environ, {HELD_ENV: str(heavy.resolve()),
                                          HELD_CLASS_ENV: "build"}), \
                contextlib.redirect_stdout(io.StringIO()) as out, \
                self.assertRaises(SystemExit) as refused:
            run_gate([fake_step("only", marker="ran.txt")], self.root, self.receipt_dir)
        self.assertEqual(refused.exception.code, 2)
        self.assertIn("an exclusive run was started under a build-class holder",
                      out.getvalue())
        self.assertFalse((self.root / "ran.txt").exists())

    def gate_beside(self, lock_class):
        """Start a `run --class lock_class` holder of the fixture's heavy lock,
        then a gate; returns (whether the gate ran while the holder held, receipt)."""
        heavy = heavy_lock_path(self.root)
        heavy.parent.mkdir(parents=True, exist_ok=True)
        holder = subprocess.Popen(
            [sys.executable, str(REPO_ROOT / "tools" / "heavy_lock.py"), "run", "--class",
             lock_class, "--", sys.executable, "-c",
             'import sys; print("held", flush=True); sys.stdin.readline()'],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
            env={**os.environ, HEAVY_LOCK_ENV: str(heavy)})
        self.addCleanup(holder.wait, 30)
        self.addCleanup(holder.stdout.close)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        outcome = {}
        gate = threading.Thread(target=lambda: outcome.update(
            receipt=run_gate([fake_step("only", marker="ran.txt")], self.root,
                             self.receipt_dir, heavy_lock=heavy)))
        with contextlib.redirect_stdout(io.StringIO()):
            gate.start()
            time.sleep(1.5)
            ran_beside = (self.root / "ran.txt").exists()
            holder.stdin.close()
            gate.join(30)
        return ran_beside, outcome["receipt"]

    def test_the_gate_waits_for_a_python_suite_holder(self):
        ran_beside, receipt = self.gate_beside("python")
        self.assertFalse(ran_beside, "the gate is exclusive: it waits for every shared holder")
        self.assertEqual(receipt["exit_code"], 0)

    def test_negative_control_a_build_beside_the_python_holder_does_not_wait(self):
        """The same shape with a shared request instead of the gate proceeds, so
        the wait above is the gate's class."""
        heavy = heavy_lock_path(self.root)
        heavy.parent.mkdir(parents=True, exist_ok=True)
        env = {**os.environ, HEAVY_LOCK_ENV: str(heavy)}
        tool = str(REPO_ROOT / "tools" / "heavy_lock.py")
        holder = subprocess.Popen(
            [sys.executable, tool, "run", "--class", "python", "--", sys.executable, "-c",
             'import sys; print("held", flush=True); sys.stdin.readline()'],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env)
        self.addCleanup(holder.wait, 30)
        self.addCleanup(holder.stdout.close)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        done = subprocess.run([sys.executable, tool, "run", "--class", "build", "--", "true"],
                              capture_output=True, text=True, timeout=30, env=env)
        holder.stdin.close()
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertNotIn("waiting", done.stderr)

    def test_receipt_records_a_dirty_tree_and_its_paths(self):
        (self.root / "scratch.txt").write_text("uncommitted\n")
        receipt, _ = self.run_quiet([fake_step("only")])
        self.assertTrue(receipt["dirty"])
        self.assertEqual(receipt["dirty_paths"], ["?? scratch.txt"])

    def test_receipt_names_each_missing_input_and_the_summary_says_partial(self):
        """The fixture repo has none of the inputs; giving it the ROM and a link to the
        disassembly leaves the pack missing, and a link to nothing counts as missing."""
        (self.root / GATE_INPUTS[0]).write_bytes(b"")
        (self.root / "elsewhere").mkdir()
        (self.root / "reference").mkdir()
        (self.root / GATE_INPUTS[1]).symlink_to(self.root / "elsewhere")
        (self.root / GATE_INPUTS[2]).symlink_to(self.root / "gone")
        with open(self.root / ".git" / "info" / "exclude", "a") as exclude:
            exclude.write("".join(f"/{name}\n" for name in (GATE_INPUTS[0], "elsewhere",
                                                             "reference", GATE_INPUTS[2])))
        receipt, _ = self.run_quiet([fake_step("only")])
        self.assertEqual(receipt["missing_inputs"], [GATE_INPUTS[2]])
        self.assertFalse(receipt["dirty"])
        self.assertIn("gate OK: 1/1 commands passed", receipt["summary"])
        self.assertIn(f"; partial: {GATE_INPUTS[2]} missing;", receipt["summary"])
        (self.root / GATE_INPUTS[2]).unlink()
        (self.root / GATE_INPUTS[2]).mkdir()
        complete, _ = self.run_quiet([fake_step("only")], self.root / "build" / "gate" / "complete")
        self.assertEqual(complete["missing_inputs"], [])
        self.assertNotIn("partial", complete["summary"])

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

    def gate_lock_holder(self):
        lock_file = gate_lock_path(self.root)
        lock_file.parent.mkdir(parents=True, exist_ok=True)
        holder = subprocess.Popen([sys.executable, "-c", HEAVY_HOLDER, str(lock_file)],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.addCleanup(self.stop, holder)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        return lock_file, holder

    def test_a_nested_gate_refuses_a_busy_gate_lock_even_with_wait(self):
        """Under a heavy holder the heavy lock is already taken; waiting for the
        gate lock would invert the lock order, so the gate refuses at once."""
        heavy = heavy_lock_path(self.root)
        heavy.parent.mkdir(parents=True, exist_ok=True)
        lock_file, holder = self.gate_lock_holder()
        outcome = {}

        def gate():
            try:
                run_gate([fake_step("nested", marker="nested.txt")], self.root,
                         self.receipt_dir, wait=True)
            except GateRefused as refusal:
                outcome["refused"] = refusal

        with mock.patch.dict(os.environ, {HELD_ENV: str(heavy.resolve()),
                                          HELD_CLASS_ENV: "exclusive"}), \
                contextlib.redirect_stdout(io.StringIO()):
            thread = threading.Thread(target=gate, daemon=True)
            thread.start()
            thread.join(timeout=10)
        self.assertFalse(thread.is_alive(), "the nested gate waited for the gate lock")
        self.assertEqual(outcome["refused"].exit_code, 2)
        self.assertIn(f"pid {holder.pid}", str(outcome["refused"]))
        self.assertFalse((self.root / "nested.txt").exists())

    def test_negative_control_a_blocking_gate_lock_acquire_there_hangs(self):
        lock_file, holder = self.gate_lock_holder()

        def acquire():
            with gate_lock(lock_file, wait=True):
                pass

        with contextlib.redirect_stdout(io.StringIO()):
            thread = threading.Thread(target=acquire, daemon=True)
            thread.start()
            thread.join(timeout=1.0)
            self.assertTrue(thread.is_alive(), "a blocking acquire waits on the holder")
            holder.stdin.close()
            thread.join(timeout=30)
        self.assertFalse(thread.is_alive())

    def test_the_gate_lock_is_taken_before_the_heavy_lock(self):
        """One order everywhere (heavy_lock's docstring): every deployed gate
        takes the gate lock and then the heavy lock."""
        taken = []

        @contextlib.contextmanager
        def recorder(name):
            taken.append(name)
            yield

        with mock.patch("tools.gate.gate_lock", lambda *a, **k: recorder("gate")), \
                mock.patch("tools.gate.hold_heavy_lock", lambda *a, **k: recorder("heavy")):
            self.run_quiet([fake_step("ordered")])
        self.assertEqual(taken, ["gate", "heavy"])

    def test_an_old_protocol_gate_and_a_new_gate_do_not_deadlock(self):
        """An old gate holds the gate lock while the heavy lock is busy; a new
        `--wait` gate arrives; the heavy holder leaves; only then does the old
        gate ask for the heavy lock. Both must finish. A gate that took the
        heavy lock first would hold it here while waiting for the gate lock,
        and the old gate would wait on it forever."""
        heavy = heavy_lock_path(self.root)
        heavy.parent.mkdir(parents=True, exist_ok=True)
        lock_file = gate_lock_path(self.root)
        lock_file.parent.mkdir(parents=True, exist_ok=True)
        holder = subprocess.Popen([sys.executable, "-c", HEAVY_HOLDER, str(heavy)],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.addCleanup(self.stop, holder)
        self.assertEqual(holder.stdout.readline().strip(), "held")
        old = subprocess.Popen([sys.executable, "-c", OLD_GATE, str(lock_file), str(heavy)],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.addCleanup(self.stop, old)
        self.assertEqual(old.stdout.readline().strip(), "gate")
        output, outcome = io.StringIO(), {}

        def gate():
            outcome["receipt"] = run_gate([fake_step("new")], self.root, self.receipt_dir,
                                          wait=True)

        with contextlib.redirect_stdout(output):
            thread = threading.Thread(target=gate, daemon=True)
            thread.start()
            for _ in range(300):  # bounded: 30 s for the new gate to start waiting
                if "waiting for" in output.getvalue():
                    break
                time.sleep(0.1)
            self.assertIn("waiting for", output.getvalue())
            holder.stdin.close()
            holder.wait(timeout=30)
            for _ in range(20):  # give a heavy-first gate 2 s to take the heavy lock
                if os.getpid() in lock_holders(heavy):
                    break
                time.sleep(0.1)
            try:
                ran, _ = old.communicate("go\n", timeout=30)
            except subprocess.TimeoutExpired:
                self.fail("deadlock: the old gate never got the heavy lock")
            thread.join(timeout=30)
        self.assertEqual(ran.strip(), "ran")
        self.assertFalse(thread.is_alive())
        self.assertEqual(outcome["receipt"]["exit_code"], 0)

    def test_a_loaded_extension_does_not_refuse_the_gate(self):
        """Run roots made the old refusal protect nothing (tools/godot.py):
        a process mapping the checkout's library, the editor's case, leaves the
        gate free to run and rebuild it."""
        extension = self.root / "rust" / "target" / "debug" / "libpsiv_godot.so"
        extension.parent.mkdir(parents=True)
        extension.write_bytes(b"\x7fELF gate fixture\n")
        child = subprocess.Popen([sys.executable, "-c", MMAP_CHILD, str(extension)],
                                 stdout=subprocess.PIPE, text=True)
        self.addCleanup(self.stop, child)
        self.assertEqual(child.stdout.readline().strip(), "mapped")
        receipt, _ = self.run_quiet([fake_step("ran", marker="ran.txt")])
        self.assertEqual(receipt["exit_code"], 0)
        self.assertTrue((self.root / "ran.txt").exists())

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


class ParallelismCase(unittest.TestCase):
    """Cargo gets cargo's defaults only up to the measured 8 jobs and threads (#157)."""

    def test_more_cores_than_measured_are_capped(self):
        self.assertEqual(cargo_parallelism(16), 8)

    def test_fewer_cores_than_measured_use_them_all(self):
        self.assertEqual(cargo_parallelism(4), 4)

    def test_the_cap_is_the_measured_eight(self):
        # 8: the jobs of the cold build measured at 4,979 MB (docs/DEVELOPMENT.md table).
        self.assertEqual(GATE_MEASURED_PARALLELISM, 8)
        self.assertEqual(cargo_parallelism(8), 8)
        self.assertEqual(cargo_parallelism(1), 1)

    def test_the_environment_overrides_a_callers_setting(self):
        env = gate_environment({"CARGO_BUILD_JOBS": "1", "HOME": "/h"}, 16)
        self.assertEqual((env["CARGO_BUILD_JOBS"], env["RUST_TEST_THREADS"]), ("8", "8"))
        self.assertEqual(env["HOME"], "/h")


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
