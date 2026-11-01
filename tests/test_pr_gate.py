"""tools/pr_gate.py: the full gate on a pull request's commit, in the gate's
kept worktree, judged from its receipt, with a summary fit to post.

    PYTHONPATH=. python3 -m unittest tests.test_pr_gate -v

Every case runs main's copy on stdin, as the steward does, in a scratch
repository whose `origin/main` carries this checkout's `tools/pr_gate.py` and
`tools/gate_worktree.py` and a stand-in `tools/local_inputs.py` that links the
main checkout's `build/` into the worktree, as the real one does. The commit
under test carries copies of both tools that fail, so a run that passes ran
main's. The gate is a stand-in too (`FAKE_GATE`): it writes the receipt a
spec asks for under `build/gate/`, records where it ran, and exits as told.

`PassCase` runs the gate at the commit, in the kept worktree, and posts its
receipt's rows. `VerdictCase` fails each way a gate falls short of a full,
clean, complete pass, and `StaleCase` never counts a receipt an earlier run
left. `MachineCase` passes a signal, a gate that can't start and an exit of
126 or more through, for a runner to try again. `StepCase` fails a run whose
worktree or inputs couldn't be prepared, or whose commit isn't there, without
running the gate. `LockCase` waits for the worktree's lock. `PostCase` keeps
logs, paths and anything else the receipt holds off the summary.
"""

import fcntl
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

IGNORED = """/rust/target
/build
"""

#: The stand-in for main's `tools/local_inputs.py link`: links the main
#: checkout's build/ in, or fails when the environment asks it to.
LOCAL_INPUTS = """import os, subprocess, sys
from pathlib import Path
if os.environ.get("PR_GATE_TEST_INPUTS") == "fail":
    sys.exit(1)
common = subprocess.run(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
                        capture_output=True, text=True, check=True).stdout.strip()
Path("build").symlink_to(Path(common).parent / "build")
"""

#: What the commit under test carries in place of both tools.
BROKEN_TOOL = "import sys\nsys.exit(9)\n"

#: The stand-in gate: `python3 FAKE_GATE SPEC`. It records where it ran in the
#: main checkout's build/ran.json, then writes the receipt SPEC describes.
FAKE_GATE = """import json, os, signal, subprocess, sys, time
from pathlib import Path
spec = json.loads(Path(sys.argv[1]).read_text())
head = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True,
                      check=True).stdout.strip()
Path("build/ran.json").write_text(json.dumps({"cwd": os.getcwd(), "head": head}))
if spec.get("signal"):
    os.kill(os.getpid(), spec["signal"])
if spec.get("write", True):
    commands = spec.get("commands", [{"command": "python3 tools/check_docs.py", "exit_code": 0,
                                      "duration_s": 1.5, "counts": {}}])
    receipt = {"candidate_sha": spec.get("sha", head), "dirty": spec.get("dirty", False),
               "tier": spec.get("tier", "full"), "duration_s": 754.2,
               "exit_code": 1 if any(c["exit_code"] for c in commands) else 0,
               "commands": commands}
    if "missing" in spec:
        receipt["missing_inputs"] = spec["missing"]
    receipt.update(spec.get("extra", {}))
    stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    out = Path("build/gate") / f"{stamp}-{head[:7]}-{os.getpid()}"
    out.mkdir(parents=True)
    (out / "receipt.json").write_text(json.dumps(receipt))
sys.exit(spec.get("exit", 0))
"""


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True,
                          text=True).stdout.strip()


class ScratchRepo(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="pr-gate-")
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name).resolve()
        self.repo = self.tmp / "repo"
        (self.repo / "tools").mkdir(parents=True)
        git(self.tmp, "init", "-q", "-b", "main", str(self.repo))
        git(self.repo, "config", "user.email", "test@example.invalid")
        git(self.repo, "config", "user.name", "test")
        git(self.repo, "config", "commit.gpgsign", "false")
        (self.repo / ".gitignore").write_text(IGNORED)
        for tool in ("pr_gate.py", "gate_worktree.py"):
            shutil.copy(ROOT / "tools" / tool, self.repo / "tools" / tool)
        (self.repo / "tools" / "local_inputs.py").write_text(LOCAL_INPUTS)
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "-m", "main")
        git(self.repo, "update-ref", "refs/remotes/origin/main", "HEAD")
        for tool in ("gate_worktree.py", "local_inputs.py"):
            (self.repo / "tools" / tool).write_text(BROKEN_TOOL)
        (self.repo / "change.txt").write_text("the pull request\n")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "-m", "head")
        self.head = git(self.repo, "rev-parse", "HEAD")
        (self.repo / "build").mkdir()
        self.gate_dir = self.tmp / "gate"
        self.fake = self.tmp / "fake_gate.py"
        self.fake.write_text(FAKE_GATE)
        self.summary_file = self.tmp / "summary.md"

    def gate(self, **spec):
        """The stand-in gate's command for a spec."""
        path = self.tmp / f"spec-{len(list(self.tmp.glob('spec-*')))}.json"
        path.write_text(json.dumps(spec))
        return [sys.executable, str(self.fake), str(path)]

    def start(self, gate, commit=None, directory=None, env=None):
        """main's copy on stdin in the main checkout, as the steward runs it."""
        source = git(self.repo, "show", "origin/main:tools/pr_gate.py")
        args = [sys.executable, "-", str(directory or self.gate_dir), commit or self.head,
                "--summary", str(self.summary_file), "--", *gate]
        process = subprocess.Popen(args, cwd=self.repo, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                                   env={**os.environ, **(env or {})})
        process.stdin.write(source + "\n")
        process.stdin.close()
        return process

    def run_pr_gate(self, gate, **kwargs):
        """(exit, summary, output) of one run."""
        process = self.start(gate, **kwargs)
        output, _ = process.communicate(timeout=120)
        code = process.returncode
        said = self.summary_file.read_text() if self.summary_file.exists() else None
        return code, said, output

    def ran(self):
        """Where the stand-in gate ran, or None if it never did."""
        path = self.repo / "build" / "ran.json"
        return json.loads(path.read_text()) if path.exists() else None


class PassCase(ScratchRepo):
    def test_a_full_clean_complete_pass_passes_at_the_commit_in_the_kept_worktree(self):
        commands = [
            {"command": "PYTHONPATH=. python3 tools/py_suite.py", "exit_code": 0, "duration_s": 300.4,
             "counts": {"tests_run": 812, "failures": 0, "errors": 0, "skipped": 3, "status": "OK"}},
            {"command": "cargo test --manifest-path rust/Cargo.toml --workspace", "exit_code": 0,
             "duration_s": 61, "counts": {"passed": 1204, "failed": 0, "ignored": 2, "suites": 40}},
        ]
        code, said, _ = self.run_pr_gate(self.gate(commands=commands, missing=[]))
        self.assertEqual(code, 0, said)
        self.assertEqual(self.ran(), {"cwd": str(self.gate_dir), "head": self.head})
        self.assertTrue((self.tmp / "gate.lock").is_file())
        self.assertEqual(said.splitlines()[0], f"**Full gate on {self.head[:12]}: passed, in 12m 34s.**")
        self.assertIn("| `PYTHONPATH=. python3 tools/py_suite.py` | 0 | 5m 00s "
                      "| 812 run, 0 failed, 0 errors, 3 skipped |", said)
        self.assertIn("| `cargo test --manifest-path rust/Cargo.toml --workspace` | 0 | 1m 01s "
                      "| 1204 passed, 0 failed, 2 ignored |", said)

    def test_the_next_commit_moves_the_same_worktree(self):
        self.assertEqual(self.run_pr_gate(self.gate(missing=[]))[0], 0)
        (self.repo / "change.txt").write_text("the next head\n")
        git(self.repo, "commit", "-q", "-am", "next")
        nxt = git(self.repo, "rev-parse", "HEAD")
        code, said, _ = self.run_pr_gate(self.gate(missing=[]), commit=nxt)
        self.assertEqual(code, 0, said)
        self.assertEqual(self.ran(), {"cwd": str(self.gate_dir), "head": nxt})

    def test_a_short_commit_is_judged_by_its_full_sha(self):
        code, said, _ = self.run_pr_gate(self.gate(missing=[]), commit=self.head[:10])
        self.assertEqual(code, 0, said)


class VerdictCase(ScratchRepo):
    def assertFails(self, gate, words):
        code, said, _ = self.run_pr_gate(gate)
        self.assertEqual(code, 1, said)
        self.assertIn(words, said.splitlines()[0])
        self.assertNotIn(": passed", said.splitlines()[0])

    def test_a_failed_command_fails(self):
        commands = [{"command": "a", "exit_code": 0, "duration_s": 1, "counts": {}},
                    {"command": "b", "exit_code": 101, "duration_s": 1, "counts": {}}]
        self.assertFails(self.gate(commands=commands, missing=[], exit=1), "failed, 1 of 2 commands")

    def test_the_fast_tier_fails(self):
        self.assertFails(self.gate(tier="fast", missing=[]), "the fast tier, not the full gate")

    def test_a_dirty_tree_fails(self):
        self.assertFails(self.gate(dirty=True, missing=[]), "wasn't clean")

    def test_a_tree_git_could_not_describe_fails(self):
        self.assertFails(self.gate(dirty=None, missing=[]), "wasn't clean")

    def test_a_partial_gate_fails_and_names_what_it_lacked(self):
        self.assertFails(self.gate(missing=["reference/ps4disasm", "runtime-pack"]),
                         "partial, without `reference/ps4disasm`, `runtime-pack`")

    def test_a_gate_that_doesnt_say_what_it_had_fails(self):
        self.assertFails(self.gate(), "doesn't say which inputs it had")

    def test_no_receipt_fails(self):
        self.assertFails(self.gate(write=False), "left no receipt")

    def test_a_receipt_for_another_commit_fails(self):
        self.assertFails(self.gate(sha="0" * 40, missing=[]), "left no receipt")

    def test_a_gate_exiting_nonzero_after_every_command_passed_fails(self):
        self.assertFails(self.gate(missing=[], exit=1), "every command passed, but the gate exited 1")


class StaleCase(ScratchRepo):
    def test_a_receipt_an_earlier_run_left_never_counts(self):
        """A passing receipt for this very commit, newer than any this run could
        write, is already in the shared build/gate/ when the gate starts."""
        old = self.repo / "build" / "gate" / f"29991231T235959Z-{self.head[:7]}"
        old.mkdir(parents=True)
        (old / "receipt.json").write_text(json.dumps({
            "candidate_sha": self.head, "dirty": False, "tier": "full", "duration_s": 1,
            "exit_code": 0, "missing_inputs": [], "commands": []}))
        code, said, _ = self.run_pr_gate(self.gate(write=False))
        self.assertEqual(code, 1, said)
        self.assertIn("left no receipt", said)


class MachineCase(ScratchRepo):
    def test_a_signal_is_128_plus_it(self):
        code, said, _ = self.run_pr_gate(self.gate(signal=signal.SIGTERM, missing=[]))
        self.assertEqual(code, 128 + signal.SIGTERM, said)
        self.assertIn(f"stopped by signal {int(signal.SIGTERM)}", said)

    def test_126_and_up_pass_through(self):
        code, said, _ = self.run_pr_gate(self.gate(missing=[], exit=126))
        self.assertEqual(code, 126, said)

    def test_a_gate_that_cant_start_is_127(self):
        code, said, output = self.run_pr_gate([str(self.tmp / "no-such-gate")])
        self.assertEqual(code, 127, said)
        self.assertIn("couldn't start", output)


class StepCase(ScratchRepo):
    def test_inputs_that_cant_be_linked_fail_before_the_gate(self):
        code, said, _ = self.run_pr_gate(self.gate(missing=[]), env={"PR_GATE_TEST_INPUTS": "fail"})
        self.assertEqual(code, 1, said)
        self.assertEqual(said, f"**Full gate on {self.head[:12]}: failed before it ran: "
                               "the local inputs couldn't be prepared.**\n")
        self.assertIsNone(self.ran())

    def test_a_worktree_that_cant_be_prepared_fails_before_the_gate(self):
        """The main checkout is never the gate's worktree: main's tool refuses it."""
        code, said, _ = self.run_pr_gate(self.gate(missing=[]), directory=self.repo)
        self.assertEqual(code, 1, said)
        self.assertIn("failed before it ran: the gate's worktree couldn't be prepared", said)
        self.assertIsNone(self.ran())
        self.assertEqual(git(self.repo, "rev-parse", "HEAD"), self.head)

    def test_a_commit_this_checkout_lacks_fails_before_anything(self):
        code, said, _ = self.run_pr_gate(self.gate(missing=[]), commit="1" * 40)
        self.assertEqual(code, 1, said)
        self.assertIn("the commit isn't in this checkout", said)
        self.assertFalse(self.gate_dir.exists())

    def test_a_call_it_cant_read_is_2(self):
        for args in ([], ["DIR", "COMMIT"], ["DIR", "COMMIT", "--"], ["DIR", "--"]):
            source = git(self.repo, "show", "origin/main:tools/pr_gate.py")
            done = subprocess.run([sys.executable, "-", *args], cwd=self.repo, input=source,
                                  capture_output=True, text=True)
            self.assertEqual(done.returncode, 2, (args, done.stderr))


class LockCase(ScratchRepo):
    def test_it_waits_for_the_worktrees_lock(self):
        with open(self.tmp / "gate.lock", "a") as held:
            fcntl.flock(held, fcntl.LOCK_EX)
            process = self.start(self.gate(missing=[]))
            time.sleep(1)
            self.assertIsNone(process.poll())
            self.assertIsNone(self.ran())
            self.assertFalse(self.gate_dir.exists())
            fcntl.flock(held, fcntl.LOCK_UN)
            output, _ = process.communicate(timeout=120)
            self.assertEqual(process.returncode, 0, output)
        self.assertEqual(self.ran()["head"], self.head)


class PostCase(ScratchRepo):
    def test_the_summary_holds_nothing_but_commands_exits_times_and_counts(self):
        log = str(self.tmp / "build" / "gate" / "SECRET-LOG.log")
        commands = [{"command": "echo `a|b`", "exit_code": 1, "duration_s": 2, "counts": {},
                     "log": log, "stdout": "SECRET-OUTPUT"}]
        extra = {"summary": f"gate FAILED; receipt={log}", "dirty_paths": ["?? SECRET-PATH"],
                 "receipt_dir": log, "git_error": "SECRET-GIT"}
        code, said, _ = self.run_pr_gate(self.gate(commands=commands, missing=[], extra=extra, exit=1))
        self.assertEqual(code, 1, said)
        self.assertIn("| `echo 'a\\|b'` | 1 | 2s |  |", said)
        for leak in ("SECRET", str(self.tmp), "receipt="):
            self.assertNotIn(leak, said)


if __name__ == "__main__":
    unittest.main()
