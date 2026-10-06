"""tools/verify_candidate.py: the candidate check's order, verdict and receipt.

    PYTHONPATH=. python3 -m unittest tests.test_verify_candidate -v

Each case runs `main` in a throwaway git repository with a fake step runner
that writes what the real steps would (the route's report, log and tape, the
certify and gate `receipt=` lines), so the order, the heavy lock, the
expectations and the receipt are checked without building or running anything.
"""
import contextlib
import io
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from tools import verify_candidate as vc


def fake_runner(codes=None, digest="abc", chapters=("one", "two")):
    """A step runner that records calls and writes each step's outputs."""
    calls = []
    codes = codes or {}

    def run(command, log, lock):
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
        log.write_text(text)
        return codes.get(name, 0)

    return run, calls


class Candidate(unittest.TestCase):
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

    def run_main(self, argv, runner):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            code = vc.main(argv, runner=runner)
        return code, out.getvalue()

    def receipt(self):
        (path,) = Path("build/candidate").glob("*/receipt.json")
        return json.loads(path.read_text())

    def test_all_steps_pass_in_order_and_only_certify_holds_the_lock(self):
        runner, calls = fake_runner()
        code, out = self.run_main(["--expect-digest", "abc", "--expect-chapters", "2"], runner)
        self.assertEqual(code, vc.EXIT_OK, out)
        self.assertEqual([name for name, _ in calls], ["build", "route", "certify", "gate"])
        self.assertEqual([lock is not None for _, lock in calls], [False, False, True, False])
        receipt = self.receipt()
        self.assertEqual(receipt["route"]["digest"], "abc")
        self.assertEqual(receipt["route"]["chapters"], 2)
        self.assertEqual(receipt["route"]["frames"], 20)
        self.assertEqual(receipt["failures"], [])
        self.assertEqual(receipt["steps"][3]["receipt"], "build/gate/x")

    def test_a_digest_mismatch_fails(self):
        runner, _ = fake_runner(digest="def")
        code, out = self.run_main(["--expect-digest", "abc"], runner)
        self.assertEqual(code, vc.EXIT_FAILED, out)
        self.assertIn("route digest def != expected abc", self.receipt()["failures"])

    def test_a_failing_step_fails_but_the_later_steps_still_run(self):
        runner, calls = fake_runner(codes={"certify": 1})
        code, out = self.run_main([], runner)
        self.assertEqual(code, vc.EXIT_FAILED, out)
        self.assertEqual([name for name, _ in calls], ["build", "route", "certify", "gate"])
        self.assertIn("step certify exited 1", self.receipt()["failures"])

    def test_a_dirty_tree_is_refused_before_anything_runs(self):
        Path("stray.txt").write_text("x")
        runner, calls = fake_runner()
        code, out = self.run_main([], runner)
        self.assertEqual(code, vc.EXIT_REFUSED, out)
        self.assertEqual(calls, [])
        self.assertIn("dirty", out)


if __name__ == "__main__":
    unittest.main()
