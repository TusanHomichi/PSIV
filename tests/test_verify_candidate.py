"""tools/verify_candidate.py: the candidate check's order, verdict and receipt.

    PYTHONPATH=. python3 -m unittest tests.test_verify_candidate -v

Each case runs `main` in a throwaway git repository with a fake step runner
that writes what the real steps would (the route's report, log and tape, the
certify and gate `receipt=` lines, the resume check's `summary=`), so the order,
the heavy lock, the expectations and the receipt are checked without building
or running anything. `RawFlockParent` runs `main` in a child process under a raw
`flock(1)` stand-in, because the refusal reads the real process tree.
"""
import contextlib
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools import verify_candidate as vc
from tools.heavy_lock import HEAVY_LOCK_ENV, HELD_CLASS_ENV, HELD_ENV

ROOT = Path(__file__).resolve().parents[1]

#: `main` in the cwd with a runner that only appends each step's name to `argv[1]`.
CANDIDATE_CHILD = f"""
import sys
sys.path.insert(0, {str(ROOT)!r})
from tools import verify_candidate as vc

def runner(command, log, lock, lock_class):
    open(sys.argv[1], "a").write(log.stem + "\\n")
    return 0

sys.exit(vc.main([], runner=runner))
"""

#: A raw flock(1) stand-in: holds `argv[1]` on its own fd and runs `argv[2:]`.
RAW_FLOCK = """
import fcntl, subprocess, sys
held = open(sys.argv[1], "a")
fcntl.flock(held, fcntl.LOCK_EX)
sys.exit(subprocess.run(sys.argv[2:]).returncode)
"""


def fake_runner(codes=None, digest="abc", chapters=("one", "two")):
    """A step runner that records calls and writes each step's outputs."""
    calls = []
    codes = codes or {}

    def run(command, log, lock, lock_class):
        name = log.stem
        calls.append((name, lock))
        text = ""
        if name == "route":
            route = log.parent / "route"
            route.mkdir(parents=True, exist_ok=True)
            (route / "report.json").write_text(json.dumps(
                {"digest": digest, "result": "completed"}))
            (route / "run.tape").write_text("tape")
            text = "".join(f"chapter {c}: 10 frames, 0 battles\n" for c in chapters)
        elif name in ("certify", "gate"):
            text = f"{name} done; receipt=build/{name}/x\n"
        elif name == "resume":
            text = "resume check: 9 of 9 resumes completed; summary=build/resume/x\n"
        log.write_text(text)
        return codes.get(name, 0)

    return run, calls


class RepoCase(unittest.TestCase):
    """The cwd is a throwaway git repository with one clean commit."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="candidate-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                   HOME=str(self.root))
        for args in (["init", "-q", "-b", "main"], ["add", "-A"],
                     ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q",
                      "--allow-empty", "-m", "c"]):
            subprocess.run(["git", "-C", str(self.root), *args], check=True, env=env,
                           capture_output=True)
        (self.root / ".gitignore").write_text("build/\n")
        subprocess.run(["git", "-C", str(self.root), "add", ".gitignore"], check=True, env=env)
        subprocess.run(["git", "-C", str(self.root), "-c", "user.name=t", "-c",
                        "user.email=t@t", "commit", "-q", "-m", "ignore"], check=True, env=env)
        old = Path.cwd()
        os.chdir(self.root)
        self.addCleanup(os.chdir, old)



class Candidate(RepoCase):
    def run_main(self, argv, runner):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            code = vc.main(argv, runner=runner)
        return code, out.getvalue()

    def receipt(self):
        (path,) = Path("build/candidate").glob("*/receipt.json")
        return json.loads(path.read_text())

    def test_all_steps_pass_in_order_and_only_the_build_is_held_here(self):
        runner, calls = fake_runner()
        code, out = self.run_main(["--expect-digest", "abc", "--expect-chapters", "2"], runner)
        self.assertEqual(code, vc.EXIT_OK, out)
        self.assertEqual([name for name, _ in calls],
                         ["build", "route", "resume", "issue-refs",
                          "certify", "gate"])
        # The headless route and resumes take no lock; certify and the gate
        # take their own classes, so only the build is held from here.
        self.assertEqual([lock is not None for _, lock in calls],
                         [True, False, False, False, False, False])
        receipt = self.receipt()
        self.assertEqual([step["lock_class"] for step in receipt["steps"]],
                         ["build", None, None, None, "own", "own"])
        self.assertEqual(receipt["route"]["digest"], "abc")
        self.assertEqual(receipt["route"]["chapters"], 2)
        self.assertEqual(receipt["route"]["frames"], 20)
        self.assertEqual(receipt["failures"], [])
        self.assertEqual(receipt["steps"][2]["receipt"], "build/resume/x")
        self.assertEqual(receipt["steps"][5]["receipt"], "build/gate/x")

    def test_a_digest_mismatch_fails(self):
        runner, _ = fake_runner(digest="def")
        code, out = self.run_main(["--expect-digest", "abc"], runner)
        self.assertEqual(code, vc.EXIT_FAILED, out)
        self.assertIn("route digest def != expected abc", self.receipt()["failures"])

    def test_a_failing_step_fails_but_the_later_steps_still_run(self):
        runner, calls = fake_runner(codes={"certify": 1})
        code, out = self.run_main([], runner)
        self.assertEqual(code, vc.EXIT_FAILED, out)
        self.assertEqual([name for name, _ in calls],
                         ["build", "route", "resume", "issue-refs",
                          "certify", "gate"])
        self.assertIn("step certify exited 1", self.receipt()["failures"])

    def test_the_resume_check_runs_on_the_route_step_s_run_and_its_failure_fails(self):
        runner, calls = fake_runner(codes={"resume": 1})
        code, out = self.run_main([], runner)
        self.assertEqual(code, vc.EXIT_FAILED, out)
        self.assertEqual([name for name, _ in calls],
                         ["build", "route", "resume", "issue-refs",
                          "certify", "gate"])
        receipt = self.receipt()
        self.assertIn("step resume exited 1", receipt["failures"])
        route = Path(receipt["steps"][1]["log"]).parent / "route"
        self.assertIn(f"--run {route} ", receipt["steps"][2]["command"])

    def test_a_dirty_tree_is_refused_before_anything_runs(self):
        Path("stray.txt").write_text("x")
        runner, calls = fake_runner()
        code, out = self.run_main([], runner)
        self.assertEqual(code, vc.EXIT_REFUSED, out)
        self.assertEqual(calls, [])
        self.assertIn("dirty", out)


class RawFlockParent(RepoCase):
    """A candidate under a raw flock parent is refused before the build runs;
    the long build and route would otherwise end in a certify step waiting
    forever on its own ancestor."""

    def setUp(self):
        super().setUp()
        tmp = tempfile.TemporaryDirectory(prefix="candidate-lock-")
        self.addCleanup(tmp.cleanup)
        self.lock = Path(tmp.name) / "continuation-heavy.lock"
        self.steps = Path(tmp.name) / "steps.txt"
        self.env = {key: value for key, value in os.environ.items() if key != HELD_ENV}
        self.env[HEAVY_LOCK_ENV] = str(self.lock)

    def run_child(self, *wrapper):
        return subprocess.run([*wrapper, sys.executable, "-c", CANDIDATE_CHILD, str(self.steps)],
                              capture_output=True, text=True, timeout=60, env=self.env)

    def test_a_raw_flock_parent_is_refused_before_the_build(self):
        done = self.run_child(sys.executable, "-c", RAW_FLOCK, str(self.lock))
        self.assertEqual(done.returncode, vc.EXIT_REFUSED, done.stdout + done.stderr)
        self.assertIn("already held by your parent pid", done.stderr)
        self.assertFalse(self.steps.exists(), "no step may run")
        self.assertFalse(Path("build/candidate").exists())

    def test_negative_control_without_the_wrapper_the_steps_run(self):
        done = self.run_child()
        self.assertNotIn("already held", done.stderr)
        # Every step verify_candidate defines ran, in order; read from steps()
        # so a new step cannot leave this control asserting a stale list.
        self.assertEqual(self.steps.read_text().split(),
                         [name for name, _, _ in vc.steps(Path("build/candidate/x"))])


class BuildStepLock(RepoCase):
    """The build step's command really runs holding the heavy lock in the
    build class: each step's command is swapped for a probe that records
    the class marker its process sees, run through the real `run_step`."""

    def setUp(self):
        super().setUp()
        tmp = tempfile.TemporaryDirectory(prefix="candidate-lock-")
        self.addCleanup(tmp.cleanup)
        self.lock = Path(tmp.name) / "continuation-heavy.lock"
        environ = {key: value for key, value in os.environ.items()
                   if key not in (HELD_ENV, HELD_CLASS_ENV)}
        environ[HEAVY_LOCK_ENV] = str(self.lock)
        patcher = mock.patch.dict(os.environ, environ, clear=True)
        patcher.start()
        self.addCleanup(patcher.stop)

    def seen_classes(self):
        seen = {}

        def runner(command, log, lock, lock_class):
            probe = f'echo "class=${{{HELD_CLASS_ENV}:-none}}"'
            code = vc.run_step(probe, log, lock, lock_class)
            seen[log.stem] = log.read_text().strip()
            return code

        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            vc.main([], runner=runner)
        return seen

    def test_the_build_runs_in_the_build_class(self):
        self.assertEqual(self.seen_classes()["build"], "class=build")

    def test_certify_and_the_gate_run_under_no_holder_and_take_their_own(self):
        seen = self.seen_classes()
        self.assertEqual((seen["certify"], seen["gate"]), ("class=none", "class=none"))

    def test_negative_control_the_headless_route_holds_nothing(self):
        self.assertEqual(self.seen_classes()["route"], "class=none")


if __name__ == "__main__":
    unittest.main()
