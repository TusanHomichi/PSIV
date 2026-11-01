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
So are the user's systemd manager's two commands (`FAKE_SYSTEMD_RUN`,
`FAKE_SYSTEMCTL`), which keep a scope by an environment variable its
processes inherit, where systemd keeps one by its cgroup.

`PassCase` runs the gate at the commit, in the kept worktree, and posts its
receipt's rows; `BehindMainCase` runs it on a head behind main merged with
main, and fails one that doesn't merge cleanly before anything runs. `VerdictCase` fails each way a gate falls short of a full,
clean, complete pass, and `AttributionCase` counts only the receipt this run
asked for, never another run's. `MachineCase` passes a signal, a gate that
can't start and an exit of 126 or more through, for a runner to try again.
`StepCase` fails a run whose worktree or inputs couldn't be prepared, or
whose commit isn't there, without running the gate. `LockCase` waits for
the worktree's lock, and `StopCase` keeps the worktree the run's till
everything it started has stopped, however the run ends, and a machine with
no user manager is a machine failure; `ScopeCase` binds a step's scope to the
user unit a run is in. `PostCase` keeps logs, paths and anything
else the receipt holds off the summary.
"""

import fcntl
import json
import os
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

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
#: main checkout's build/ran.json, then writes the receipt SPEC describes where
#: tools/gate.py would: the directory PSIV_GATE_RECEIPT names, or (`old`, a gate
#: from before it) one under build/gate/. With `hold`, it first starts a child
#: (its pid in `child`), and with `detach` another that keeps none of its
#: descriptors, in a session of its own (its pid there), and waits for the file
#: `hold` names; `me` gets its own pid, and `mark` and `ended` name files it
#: writes as it starts and ends.
FAKE_GATE = """import json, os, signal, subprocess, sys, time
from pathlib import Path
spec = json.loads(Path(sys.argv[1]).read_text())
head = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True,
                      check=True).stdout.strip()
Path("build/ran.json").write_text(json.dumps({"cwd": os.getcwd(), "head": head}))
if spec.get("me"):
    Path(spec["me"]).write_text(str(os.getpid()))
if spec.get("mark"):
    Path(spec["mark"]).write_text(str(time.monotonic()))
if spec.get("hold"):
    child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(300)"])
    if spec.get("detach"):
        # One that keeps none of the gate's descriptors, in a session of its own.
        detached = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(300)"],
                                    start_new_session=True, close_fds=True)
        Path(spec["detach"]).write_text(str(detached.pid))
    Path(spec["child"]).write_text(str(child.pid))
    deadline = time.monotonic() + 60
    while not Path(spec["hold"]).exists() and time.monotonic() < deadline:
        time.sleep(0.05)
    child.kill()
if spec.get("signal"):
    os.kill(os.getpid(), spec["signal"])
for other in spec.get("others", []):
    # Another run's receipt for this commit, newer than this one's, in the shared directory.
    out = Path("build/gate") / other["name"]
    out.mkdir(parents=True)
    (out / "receipt.json").write_text(json.dumps({**other["receipt"], "candidate_sha": head}))
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
    asked = os.environ.get("PSIV_GATE_RECEIPT")
    if spec.get("old") or not asked:
        asked = Path("build/gate") / (time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()) + "-" + head[:7])
    out = Path(asked)
    out.mkdir(parents=True)
    (out / "receipt.json").write_text(json.dumps(receipt))
if spec.get("ended"):
    Path(spec["ended"]).write_text(str(time.monotonic()))
sys.exit(spec.get("exit", 0))
"""


#: systemd-run --scope, as the step runs it: fails before the command with
#: FAKE_SCOPE_FAILS, expands `${VAR}` and `$VAR` in it unless told not to, as
#: systemd does, refuses a scope still running, else marks the command as its
#: (FAKE_SCOPE) and becomes it.
FAKE_SYSTEMD_RUN = """import os, re, sys
args = sys.argv[1:]
split = args.index("--")
unit = next(a.split("=", 1)[1] for a in args[:split] if a.startswith("--unit="))
with open(os.environ["FAKE_SYSTEMD_LOG"], "a") as log:
    log.write(" ".join(["run", *args[:split]]) + "\\n")
if os.environ.get("FAKE_SCOPE_FAILS"):
    sys.exit("Failed to start transient scope unit: Access denied")
command = args[split + 1:]
if "--expand-environment=no" not in args[:split]:
    expand = lambda m: os.environ.get(m.group(1) or m.group(2), "")
    command = [re.sub(r"\\$\\{(\\w+)\\}|\\$(\\w+)", expand, a) for a in command]
for pid in os.listdir("/proc"):
    try:
        if pid.isdigit() and f"FAKE_SCOPE={unit}".encode() in open(f"/proc/{pid}/environ", "rb").read().split(b"\\0"):
            sys.exit(f"Failed to start transient scope unit: Unit {unit} already exists.")
    except OSError:
        pass
os.environ["FAKE_SCOPE"] = unit
os.execvp(command[0], command)
"""

#: systemctl --user stop and show, for a scope FAKE_SYSTEMD_RUN keeps: stop
#: ends once every process of it has (SIGTERM, then SIGKILL), and show says
#: whether it runs and what its cgroup is, as systemd's properties do.
FAKE_SYSTEMCTL = """import os, signal, sys, time
if os.environ.get("FAKE_NO_MANAGER"):
    sys.exit("Failed to connect to bus: No medium found")
verb, unit = sys.argv[2], sys.argv[-1]
def members():
    found = []
    for pid in os.listdir("/proc"):
        try:
            if not pid.isdigit() or open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()[0] == "Z":
                continue
            if f"FAKE_SCOPE={unit}".encode() in open(f"/proc/{pid}/environ", "rb").read().split(b"\\0"):
                found.append(int(pid))
        except OSError:
            pass
    return found
if verb == "show":
    left = members()
    print(f"ActiveState={'active' if left else 'inactive'}")
    print(f"ControlGroup={'/fake/' + unit if left else ''}")
    sys.exit(0)
with open(os.environ["FAKE_SYSTEMD_LOG"], "a") as log:
    log.write(f"stop {unit} {len(members())}\\n")
for signum in (signal.SIGTERM, signal.SIGKILL):
    for pid in members():
        try:
            os.kill(pid, signum)
        except OSError:
            pass
    deadline = time.monotonic() + 10
    while members() and time.monotonic() < deadline:
        time.sleep(0.05)
sys.exit(0 if not members() else 1)
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
        self.bin = self.tmp / "bin"
        self.bin.mkdir()
        for name, source in (("systemd-run", FAKE_SYSTEMD_RUN), ("systemctl", FAKE_SYSTEMCTL)):
            (self.bin / name).write_text(f"#!{sys.executable}\n{source}")
            (self.bin / name).chmod(0o755)
        self.systemd_log = self.tmp / "systemd.log"

    def gate(self, **spec):
        """The stand-in gate's command for a spec."""
        path = self.tmp / f"spec-{len(list(self.tmp.glob('spec-*')))}.json"
        path.write_text(json.dumps(spec))
        return [sys.executable, str(self.fake), str(path)]

    def start(self, gate, commit=None, directory=None, env=None, summary=None):
        """main's copy on stdin in the main checkout, as the steward runs it."""
        source = self.tmp / "pr_gate-main.py"
        source.write_text(git(self.repo, "show", "origin/main:tools/pr_gate.py") + "\n")
        args = [sys.executable, "-", str(directory or self.gate_dir), commit or self.head,
                "--summary", str(summary or self.summary_file), "--", *gate]
        with open(source) as stdin:
            return subprocess.Popen(args, cwd=self.repo, stdin=stdin, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, text=True,
                                    env={**os.environ, "PATH": f"{self.bin}:{os.environ['PATH']}",
                                         "FAKE_SYSTEMD_LOG": str(self.systemd_log), **(env or {})})

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


class BehindMainCase(ScratchRepo):
    """A head from before main moved on is gated merged with main (#278)."""

    def advance_main(self, name, text):
        """Move origin/main on by a commit that writes `name`, leaving the head where it is."""
        main = git(self.repo, "rev-parse", "origin/main")
        env = {**os.environ, "GIT_INDEX_FILE": str(self.tmp / "main-index")}
        blob = subprocess.run(["git", "hash-object", "-w", "--stdin"], cwd=self.repo, input=text,
                              capture_output=True, text=True, check=True).stdout.strip()
        for args in (["read-tree", main], ["update-index", "--add", "--cacheinfo", f"100644,{blob},{name}"]):
            subprocess.run(["git", *args], cwd=self.repo, env=env, check=True, capture_output=True)
        tree = subprocess.run(["git", "write-tree"], cwd=self.repo, env=env, capture_output=True,
                              text=True, check=True).stdout.strip()
        moved = git(self.repo, "commit-tree", tree, "-p", main, "-m", "main moves on")
        git(self.repo, "update-ref", "refs/remotes/origin/main", moved)
        return moved

    def test_a_head_behind_main_is_gated_merged_with_it(self):
        main = self.advance_main("main.txt", "main's own change\n")
        code, said, _ = self.run_pr_gate(self.gate(missing=[]))
        self.assertEqual(code, 0, said)
        ran = self.ran()
        self.assertEqual(ran["cwd"], str(self.gate_dir))
        self.assertEqual(git(self.gate_dir, "rev-list", "--parents", "-n", "1", ran["head"]).split()[1:],
                         [self.head, main])
        self.assertTrue((self.gate_dir / "main.txt").is_file())
        self.assertTrue(said.startswith(f"**Full gate on {self.head[:12]} merged with main at "
                                        f"{main[:12]}: passed, in "), said)

    def test_the_next_head_that_has_main_moves_the_worktree_off_the_merge(self):
        main = self.advance_main("main.txt", "main's own change\n")
        self.assertEqual(self.run_pr_gate(self.gate(missing=[]))[0], 0)
        git(self.repo, "merge", "-q", "--no-edit", main)
        merged = git(self.repo, "rev-parse", "HEAD")
        code, said, _ = self.run_pr_gate(self.gate(missing=[]), commit=merged)
        self.assertEqual(code, 0, said)
        self.assertEqual(self.ran()["head"], merged)
        self.assertTrue(said.startswith(f"**Full gate on {merged[:12]}: passed, in "), said)

    def test_a_head_that_conflicts_with_main_fails_before_the_gate(self):
        main = self.advance_main("change.txt", "main's own change.txt\n")
        code, said, _ = self.run_pr_gate(self.gate(missing=[]))
        self.assertEqual(code, 1, said)
        self.assertEqual(said, f"**Full gate on {self.head[:12]} merged with main at {main[:12]}: "
                               f"failed before it ran: it doesn't merge cleanly with main at "
                               f"{main[:12]}; merge main into the branch.**\n")
        self.assertIsNone(self.ran())
        mid_merge = subprocess.run(["git", "rev-parse", "-q", "--verify", "MERGE_HEAD"],
                                   cwd=self.gate_dir, capture_output=True)
        self.assertNotEqual(mid_merge.returncode, 0, "the worktree was left mid-merge")


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
        self.assertFails(self.gate(write=False), "wrote no receipt")

    def test_a_receipt_for_another_commit_fails(self):
        self.assertFails(self.gate(sha="0" * 40, missing=[]), "wrote no receipt")

    def test_a_gate_exiting_nonzero_after_every_command_passed_fails(self):
        self.assertFails(self.gate(missing=[], exit=1), "every command passed, but the gate exited 1")


PASSING = {"dirty": False, "tier": "full", "duration_s": 1, "exit_code": 0, "missing_inputs": [],
           "commands": []}
FAILING = {"dirty": False, "tier": "fast", "duration_s": 1, "exit_code": 1, "missing_inputs": [],
           "commands": [{"command": "x", "exit_code": 1, "duration_s": 1, "counts": {}}]}


class AttributionCase(ScratchRepo):
    """Only the receipt this run asked its gate for counts, never another in the
    shared build/gate/, however new and for whatever commit."""

    def plant(self, name, receipt):
        out = self.repo / "build" / "gate" / name
        out.mkdir(parents=True)
        (out / "receipt.json").write_text(json.dumps({**receipt, "candidate_sha": self.head}))

    def test_a_passing_receipt_an_earlier_run_left_never_counts(self):
        self.plant(f"29991231T235959Z-{self.head[:7]}", PASSING)
        code, said, _ = self.run_pr_gate(self.gate(write=False))
        self.assertEqual(code, 1, said)
        self.assertIn("wrote no receipt", said)

    def test_a_failing_receipt_another_run_writes_meanwhile_never_counts(self):
        """A fast run of the same commit, finished while this gate waited on it."""
        others = [{"name": f"29991231T235959Z-{self.head[:7]}-fast", "receipt": FAILING}]
        code, said, _ = self.run_pr_gate(self.gate(missing=[], others=others))
        self.assertEqual(code, 0, said)

    def test_a_passing_receipt_another_run_writes_meanwhile_never_counts(self):
        others = [{"name": f"29991231T235959Z-{self.head[:7]}", "receipt": PASSING}]
        code, said, _ = self.run_pr_gate(self.gate(missing=[], write=False, others=others))
        self.assertEqual(code, 1, said)

    def test_a_gate_from_before_it_could_be_asked_fails(self):
        """One that writes its receipt where it always did, ignoring PSIV_GATE_RECEIPT."""
        code, said, _ = self.run_pr_gate(self.gate(missing=[], old=True))
        self.assertEqual(code, 1, said)
        self.assertIn("needs main merged in", said)


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


def alive(pid):
    """Whether pid is a process that hasn't ended (a zombie has)."""
    try:
        with open(f"/proc/{pid}/stat") as stat:
            return stat.read().rsplit(")", 1)[1].split()[0] != "Z"
    except FileNotFoundError:
        return False


def wait_for(path, timeout=60):
    deadline = time.monotonic() + timeout
    while not path.exists():
        if time.monotonic() > deadline:
            raise AssertionError(f"{path} never came")
        time.sleep(0.05)


class StopCase(ScratchRepo):
    """The worktree stays the run's till everything it started has stopped."""

    def held(self, **spec):
        return self.gate(missing=[], hold=str(self.tmp / "release"), child=str(self.tmp / "child"),
                         me=str(self.tmp / "gate-pid"),
                         mark=str(self.tmp / "started"), ended=str(self.tmp / "ended"), **spec)

    def lock_is_free(self):
        with open(self.tmp / "gate.lock", "a") as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                return False
            fcntl.flock(lock, fcntl.LOCK_UN)
            return True

    def test_a_stopped_run_stops_its_gate_and_all_it_started_before_the_lock_goes(self):
        for signum in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
            for name in ("started", "gate-pid", "child", "detached", "ended"):
                (self.tmp / name).unlink(missing_ok=True)
            process = self.start(self.held(detach=str(self.tmp / "detached")))
            wait_for(self.tmp / "child")
            gate = int((self.tmp / "gate-pid").read_text())
            started = [gate, int((self.tmp / "child").read_text()), int((self.tmp / "detached").read_text())]
            process.send_signal(signum)
            output, _ = process.communicate(timeout=60)
            self.assertEqual(process.returncode, 128 + signum, output)
            self.assertFalse(any(alive(pid) for pid in started), signum)
            self.assertFalse((self.tmp / "ended").exists())
            self.assertTrue(self.lock_is_free())
            self.assertIn(f"stopped by signal {int(signum)}", self.summary_file.read_text())

    def test_a_run_killed_outright_has_all_it_left_stopped_before_the_next_touches_the_worktree(self):
        first = self.start(self.held(detach=str(self.tmp / "detached")))
        wait_for(self.tmp / "child")
        wait_for(self.tmp / "detached")
        gate = int((self.tmp / "gate-pid").read_text())
        left = [gate, int((self.tmp / "child").read_text()), int((self.tmp / "detached").read_text())]
        first.kill()
        first.wait(timeout=60)
        # Its gate lives on, holding the other end of this.
        first.stdout.close()
        self.assertTrue(all(alive(pid) for pid in left))
        before = len(self.systemd_log.read_text().splitlines())
        code, said, output = self.run_pr_gate(self.gate(missing=[], mark=str(self.tmp / "second")),
                                              summary=self.tmp / "second.md")
        self.assertEqual(code, 0, output)
        self.assertFalse(any(alive(pid) for pid in left))
        self.assertFalse((self.tmp / "ended").exists())
        # The first thing the next run did, once it held the lock: stop what was left.
        first_call = self.systemd_log.read_text().splitlines()[before]
        self.assertTrue(first_call.startswith("stop ") and not first_call.endswith(" 0"), first_call)

    def test_a_stop_before_the_lock_is_a_stop_too(self):
        """A stop while it reads the commit, before any step: a git that waits."""
        bin_dir = self.tmp / "git-bin"
        bin_dir.mkdir()
        asked = self.tmp / "asked"
        real = shutil.which("git")
        (bin_dir / "git").write_text(
            f'#!/bin/sh\ncase "$1" in rev-parse) : >{shlex.quote(str(asked))}; exec sleep 300;; esac\n'
            f'exec {shlex.quote(real)} "$@"\n')
        (bin_dir / "git").chmod(0o755)
        process = self.start(self.gate(missing=[]), env={"PATH": f"{bin_dir}:{self.bin}:{os.environ['PATH']}"})
        wait_for(asked)
        process.send_signal(signal.SIGTERM)
        output, _ = process.communicate(timeout=60)
        self.assertEqual(process.returncode, 128 + signal.SIGTERM, output)
        self.assertIn("stopped by signal 15", self.summary_file.read_text())
        self.assertIsNone(self.ran())

    def test_no_user_manager_is_the_machine_failing(self):
        code, said, output = self.run_pr_gate(self.gate(missing=[]), env={"FAKE_NO_MANAGER": "1"})
        self.assertEqual(code, 126, output)
        self.assertIn("no user systemd manager", said)
        self.assertIsNone(self.ran())

    def test_no_systemctl_is_the_machine_failing(self):
        """A PATH with git and systemd-run but no systemctl."""
        bare = self.tmp / "bare"
        bare.mkdir()
        (bare / "git").symlink_to(shutil.which("git"))
        (bare / "systemd-run").symlink_to(self.bin / "systemd-run")
        code, said, output = self.run_pr_gate(self.gate(missing=[]), env={"PATH": str(bare)})
        self.assertEqual(code, 126, output)
        self.assertIn("no user systemd manager", said)
        self.assertIsNone(self.ran())

    def test_a_scope_that_never_ran_its_command_is_the_machine_failing(self):
        code, said, output = self.run_pr_gate(self.gate(missing=[]), env={"FAKE_SCOPE_FAILS": "1"})
        self.assertEqual(code, 126, output)
        self.assertIn("never ran its command", said)
        self.assertIsNone(self.ran())

    def test_arguments_reach_the_step_as_they_are(self):
        got = self.tmp / "argv.json"
        script = self.tmp / "argv.py"
        script.write_text(f"import json, sys\nopen({str(got)!r}, 'w').write(json.dumps(sys.argv[1:]))\n")
        literal = ["${HOME}", "$HOME", "${PR_GATE_UNSET}", "a $b ${c}"]
        self.run_pr_gate([sys.executable, str(script), *literal])
        self.assertEqual(json.loads(got.read_text()), literal)

    def test_every_spelling_of_the_worktree_shares_its_lock_and_scope(self):
        (self.tmp / "sub").mkdir()
        (self.tmp / "alias").symlink_to(self.tmp)
        with open(self.tmp / "gate.lock", "a") as held:
            fcntl.flock(held, fcntl.LOCK_EX)
            process = self.start(self.gate(missing=[]), directory=self.tmp / "sub" / ".." / "gate")
            time.sleep(1)
            self.assertIsNone(process.poll())
            self.assertIsNone(self.ran())
            fcntl.flock(held, fcntl.LOCK_UN)
            output, _ = process.communicate(timeout=120)
            self.assertEqual(process.returncode, 0, output)
        code, _, output = self.run_pr_gate(self.gate(missing=[]), directory=self.tmp / "alias" / "gate")
        self.assertEqual(code, 0, output)
        code, _, output = self.run_pr_gate(self.gate(missing=[]))
        self.assertEqual(code, 0, output)
        units = {word for line in self.systemd_log.read_text().splitlines()
                 for word in line.split() if word.startswith("--unit=") or line.startswith("stop ") and
                 word.endswith(".scope")}
        self.assertEqual(len({u.removeprefix("--unit=") for u in units}), 1, units)
        self.assertFalse((self.tmp / "sub" / "gate").exists() or (self.tmp / "sub" / "gate.lock").exists())

    def test_whatever_a_step_left_running_is_stopped_before_the_next_step(self):
        """A gate that exits leaving a process behind: one of its session, and
        one of a session of its own that kept none of its descriptors."""
        orphans = [self.tmp / "orphan", self.tmp / "detached"]
        script = self.tmp / "leave.py"
        script.write_text(
            "import subprocess, sys\n"
            "child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(300)'])\n"
            f"open({str(orphans[0])!r}, 'w').write(str(child.pid))\n"
            "child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(300)'],\n"
            "                         start_new_session=True, close_fds=True)\n"
            f"open({str(orphans[1])!r}, 'w').write(str(child.pid))\n")
        code, said, output = self.run_pr_gate([sys.executable, str(script)])
        self.assertEqual(code, 1, output)
        self.assertFalse(any(alive(int(orphan.read_text())) for orphan in orphans))


class ScopeCase(unittest.TestCase):
    def test_a_steps_scope_stops_with_the_user_unit_it_runs_in(self):
        from tools import pr_gate
        with tempfile.TemporaryDirectory() as tmp:
            cgroups = Path(tmp) / "cgroup"
            cgroups.write_text("0::/user.slice/user-1001.slice/user@1001.service/app.slice/run-7.service\n")
            self.assertEqual(pr_gate.caller_unit(cgroups), "run-7.service")
            # A login session's scope is the system manager's: nothing to bind to.
            cgroups.write_text("0::/user.slice/user-1001.slice/session-3.scope\n")
            self.assertIsNone(pr_gate.caller_unit(cgroups))
            self.assertIsNone(pr_gate.caller_unit(Path(tmp) / "none"))

    def stop(self, show, events=None):
        """stop_scope against a systemctl that shows `show`, and a cgroup
        tree where the scope's cgroup.events reads `events` (None: gone)."""
        from tools import pr_gate
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            (tmp / "systemctl").write_text(f"#!/bin/sh\n[ \"$2\" = show ] && printf '%s' {shlex.quote(show)}\nexit 0\n")
            (tmp / "systemctl").chmod(0o755)
            if events is not None:
                (tmp / "cg" / "app.slice" / "s.scope").mkdir(parents=True)
                (tmp / "cg" / "app.slice" / "s.scope" / "cgroup.events").write_text(events)
            with mock.patch.dict(os.environ, {"PATH": f"{tmp}:{os.environ['PATH']}"}):
                pr_gate.stop_scope("s.scope", cgroups=tmp / "cg")

    def test_a_stop_counts_only_once_its_cgroup_is_gone_or_empty(self):
        from tools import pr_gate
        group = "ControlGroup=/app.slice/s.scope\n"
        self.stop("ActiveState=inactive\nControlGroup=\n")
        self.stop("ActiveState=inactive\n" + group)
        self.stop("ActiveState=failed\n" + group, "populated 0\nfrozen 0\n")
        # Processes that outlived SIGKILL: the scope failed, its cgroup still full.
        with self.assertRaisesRegex(pr_gate.Unsupervised, "outlived its stop"):
            self.stop("ActiveState=failed\n" + group, "populated 1\nfrozen 0\n")
        with self.assertRaisesRegex(pr_gate.Unsupervised, "outlived its stop"):
            self.stop("ActiveState=deactivating\n" + group)


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
