"""Tests for tools/ds-lane: the DeepSeek worker harness.

Hermetic: every test builds a throwaway git repo and a throwaway DS_LANE_HOME
in temp directories and points DS_LANE_REASONIX at a fake worker script, so no
test calls the real Reasonix, a model or the network.

    PYTHONPATH=. python3 -m unittest tests.test_ds_lane -v

Unit-level cases load tools/ds-lane (no .py suffix) with SourceFileLoader;
end-to-end cases drive the CLI through subprocess.
"""
import datetime as dt
import importlib.machinery
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
DS_LANE = ROOT / "tools" / "ds-lane"

_loader = importlib.machinery.SourceFileLoader("ds_lane_under_test", str(DS_LANE))
_spec = importlib.util.spec_from_loader(_loader.name, _loader)
DS = importlib.util.module_from_spec(_spec)
_loader.exec_module(DS)

# ---------------------------------------------------------------- fake worker

FAKE_REASONIX = '''#!/usr/bin/env python3
"""Fake `reasonix` for the ds-lane test suite: no model, no network, no sandbox.

Reads a JSON behaviour spec from FAKE_REASONIX_SPEC (inline JSON or a path):
files to write, trajectory events to append, seconds to sleep, exit code.
"""
import json
import os
import sys
import time
from pathlib import Path

VALUE_FLAGS = {"--dir", "--trajectory", "--metrics", "--model", "--effort",
               "--permission-mode", "--output-format", "--max-steps", "--add-dir",
               "--resume"}


def load_spec():
    raw = os.environ.get("FAKE_REASONIX_SPEC", "{}")
    path = Path(raw)
    return json.loads(path.read_text()) if path.exists() else json.loads(raw)


def main(argv):
    if not argv or argv[0] == "--version":
        print("reasonix 0.0.0-fake")
        return 0
    if argv[0] != "run":
        print("fake-reasonix: unsupported command " + argv[0], file=sys.stderr)
        return 2
    opts, prompt, i = {}, "", 1
    while i < len(argv):
        arg = argv[i]
        if arg in VALUE_FLAGS:
            opts[arg] = argv[i + 1]
            i += 2
        elif arg.startswith("--"):
            i += 1
        else:
            prompt = arg
            i += 1
    spec = load_spec()
    cwd = Path.cwd()
    for rel, content in sorted(spec.get("files", {}).items()):
        dest = cwd / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(content)
    if spec.get("pid_file"):
        Path(spec["pid_file"]).write_text(str(os.getpid()))
    if spec.get("opts_out"):
        Path(spec["opts_out"]).write_text(json.dumps({"opts": opts, "prompt": prompt}, indent=2))
    trajectory = opts.get("--trajectory")
    if trajectory:
        with open(trajectory, "a") as handle:
            for event in spec.get("events", []):
                handle.write(json.dumps({"event": event}) + "\\n")
    if spec.get("sleep"):
        time.sleep(spec["sleep"])
    print(json.dumps({
        "type": "result", "subtype": "success",
        "is_error": bool(spec.get("is_error", False)),
        "num_turns": spec.get("num_turns", 1),
        "result": spec.get("result", "fake worker finished\\n"),
        "total_cost_usd": spec.get("cost_usd", 0.001),
        "usage": {"promptTokens": 10, "completionTokens": 5,
                  "cacheHitTokens": 0, "cacheMissTokens": 10, "totalTokens": 15},
    }))
    return int(spec.get("exit_code", 0))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
'''

RECEIPT_RESULT = ("Wrote the owned file.\n\n"
                  "## Receipt\n- Status: done\n- Changed files: `tools/new.txt`\n"
                  "- Commands run: `true` -> exit 0\n- Acceptance: met\n- Open issues: none\n")


class UnitCase(unittest.TestCase):
    """Unit-level cases: import the harness and call into it."""

    def test_env_is_read_per_call(self):
        with mock.patch.dict(os.environ, {"DS_LANE_HOME": "/tmp/ds-home"}):
            self.assertEqual(DS.wt_root(), Path("/tmp/ds-home/wt"))
            self.assertEqual(DS.state_root(), Path("/tmp/ds-home/state"))
        with mock.patch.dict(os.environ, {"DS_LANE_REASONIX": "/bin/fake-reasonix"}):
            self.assertEqual(DS.reasonix_bin(), "/bin/fake-reasonix")
        with mock.patch.dict(os.environ, {"DS_LANE_MAX_LANES": "1"}):
            self.assertEqual(DS.max_lanes(), 1)
        with mock.patch.dict(os.environ, {"DS_LANE_MAX_LANES": "junk"}):
            self.assertEqual(DS.max_lanes(), DS.MAX_LANES)
        stripped = {k: v for k, v in os.environ.items()
                    if k not in ("DS_LANE_HOME", "DS_LANE_REASONIX", "DS_LANE_MAX_LANES")}
        with mock.patch.dict(os.environ, stripped, clear=True):
            self.assertEqual(DS.wt_root(), DS.WT_ROOT)
            self.assertEqual(DS.state_root(), DS.STATE_ROOT)
            self.assertEqual(DS.reasonix_bin(), "reasonix")
            self.assertEqual(DS.max_lanes(), DS.MAX_LANES)

    def test_phrasing_violations(self):
        flagged = ["do not edit anything", "Don't modify the ledger", "never write files",
                   "you must not change tests", "make no changes", "the lane is read-only",
                   "treat this as review only", "an investigation only task",
                   "without editing the fixture", "keep the file unchanged"]
        clean = ["touch only tools/ds-lane", "run the focused tests", "keep the ledger as-is",
                 "the preamble stays positive"]
        for text in flagged:
            self.assertTrue(DS.phrasing_violations(text), f"expected a violation: {text!r}")
        for text in clean:
            self.assertEqual(DS.phrasing_violations(text), [], f"unexpected violation: {text!r}")

    def test_phrasing_line_numbers(self):
        brief = "# Brief\n\nTouch only a.py.\nDo not edit b.py.\n"
        self.assertEqual([n for n, _ in DS.phrasing_violations(brief)], [4])

    def test_phrasing_ignores_fences_and_inline_code(self):
        brief = "Quote the rule:\n\n```text\ndo not edit anything; no changes; read-only\n```\n\n" \
                "And inline: `never modify files` and `keep it unchanged`.\n"
        self.assertEqual(DS.phrasing_violations(brief), [])
        # Only fences and backticks are exempt (Reasonix strips the same two), so
        # indented code is scanned too: a false refusal is recoverable, a missed
        # ban silently wastes a run.
        indented = "# Brief\n\n    do not write tests\n"
        self.assertEqual([n for n, _ in DS.phrasing_violations(indented)], [3])
        # A fence that closes lets later prose be scanned again.
        reopened = "```\ndo not edit\n```\nread-only lane\n"
        self.assertEqual([n for n, _ in DS.phrasing_violations(reopened)], [4])

    def test_own_text_passes_preflight(self):
        # ds-lane's preamble is prepended to every brief; it must never trip the
        # preflight. READ_ONLY_CLAUSE states the ban on purpose (that is what
        # --read-only is for), and a read-only lane skips the preflight.
        self.assertEqual(DS.phrasing_violations(DS.PREAMBLE), [])
        self.assertTrue(DS.phrasing_violations(DS.READ_ONLY_CLAUSE))
        self.assertEqual(DS.phrasing_violations(DS.PREAMBLE + "# Brief\n\nTouch only a.py.\n"), [])
        self.assertIsNone(DS.preflight_phrasing(DS.PREAMBLE, "brief"))

    def test_preflight_exits_nonzero_with_line_and_hint(self):
        brief = "# Brief\n\nTouch only a.py. Do not touch b.py.\n"
        with self.assertRaises(SystemExit) as ctx:
            DS.preflight_phrasing(brief, "brief")
        self.assertNotEqual(ctx.exception.code, 0)
        message = str(ctx.exception.code)
        self.assertIn("phrasing preflight", message)
        self.assertIn("line 3", message)
        self.assertIn("Do not touch b.py", message)
        self.assertIn("touch only X", message)
        DS.preflight_phrasing(brief, "brief", allow=True)

    def test_write_set_parsing(self):
        brief = "# Brief\n\ntext\n\n```write-set\ntools/ds-lane\ntests/**\n```\n\nmore\n"
        self.assertEqual(DS.parse_write_set(brief), ["tools/ds-lane", "tests/**"])
        self.assertIsNone(DS.parse_write_set("# Brief\n\nno block here\n"))
        self.assertIsNone(DS.parse_write_set("# Brief\n\n```\ntools/ds-lane\n```\n"))
        self.assertIsNone(DS.parse_write_set("# Brief\n\n```write-set\n\n```\n"))

    def test_write_set_globs(self):
        patterns = ["tools/ds-lane", "tests/test_*.py", "docs/**/*.md", "CLAUDE.md"]
        inside = ["tools/ds-lane", "tests/test_ds_lane.py", "docs/README.md",
                  "docs/a/b/deep.md", "CLAUDE.md"]
        outside = ["tools/other", "tests/fixtures/x.py", "docs/README.txt", "README.md",
                   "CLAUDE.md.bak", "docsx/README.md"]
        for path in inside:
            self.assertTrue(DS.write_set_match(path, patterns), path)
        for path in outside:
            self.assertFalse(DS.write_set_match(path, patterns), path)
        self.assertEqual(DS.write_set_violations(["tools/ds-lane", "README.md"], patterns),
                         ["README.md"])

    def test_receipt_block(self):
        self.assertIsNone(DS.receipt_block("no receipt here\n"))
        self.assertEqual(DS.receipt_block("done\n\n## Receipt\n- Status: done\n"),
                         "## Receipt\n- Status: done")
        # Only the final block is reported when the text mentions the heading twice.
        self.assertEqual(DS.receipt_block("## Receipt placeholder\n\n## Receipt\n- Status: partial\n"),
                         "## Receipt\n- Status: partial")


class LaneCase(unittest.TestCase):
    """End-to-end cases: drive the CLI against a throwaway repo and fake worker."""

    def setUp(self):
        self.tmpdir = tempfile.TemporaryDirectory(prefix="ds-lane-test-")
        self.addCleanup(self.tmpdir.cleanup)
        self.root = Path(self.tmpdir.name)
        self.home = self.root / "home"
        self.home.mkdir()
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.work = self.root / "work"  # scratch: briefs, specs, pid files
        self.work.mkdir()
        self.fake = self.root / "fake-reasonix"
        self.fake.write_text(FAKE_REASONIX)
        self.fake.chmod(0o755)
        self.init_repo()
        self.env = {"DS_LANE_HOME": str(self.home), "DS_LANE_REASONIX": str(self.fake)}
        self.base_sha = self.git("rev-parse", "HEAD")
        self.addCleanup(self.reap_workers)

    # -- scaffolding

    def init_repo(self):
        subprocess.run(["git", "init", "-q", "-b", "main", str(self.repo)], check=True)
        self.git("config", "user.email", "lane@example.invalid")
        self.git("config", "user.name", "ds-lane test")
        (self.repo / "README.md").write_text("# throwaway repo\n")
        (self.repo / ".gitignore").write_text("generated/\nbuild/\nruntime-pack/\n")
        (self.repo / "generated").mkdir()
        (self.repo / "generated" / "data.txt").write_text("generated input\n")
        pack = self.repo / "build" / "snap" / "runtime-pack"
        pack.mkdir(parents=True)
        (pack / "pack.bin").write_text("pack\n")
        (self.repo / "tools").mkdir()
        (self.repo / "tools" / "keep.txt").write_text("keep\n")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "init")

    def git(self, *args, cwd=None):
        r = subprocess.run(["git", "-C", str(cwd or self.repo), *args], text=True,
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        return r.stdout.strip()

    def reap_workers(self):
        """Kill any supervisor or fake worker a failed test left behind."""
        for pidfile in self.work.glob("*.pid"):
            self.kill_pid(pidfile)
        for pidfile in self.home.glob("state/*/*/run-*/supervisor.pid"):
            self.kill_pid(pidfile)

    def kill_pid(self, pidfile):
        try:
            os.kill(int(pidfile.read_text().strip()), 9)
        except (OSError, ValueError):
            pass

    def cli(self, *args, spec=None, env=None, timeout=120, check=None, cwd=None):
        environ = dict(os.environ)
        environ.update(self.env)
        if spec is not None:
            environ["FAKE_REASONIX_SPEC"] = json.dumps(spec) if isinstance(spec, dict) else str(spec)
        environ.update(env or {})
        r = subprocess.run([sys.executable, str(DS_LANE)] + [str(a) for a in args], text=True,
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=environ,
                           cwd=str(cwd or self.repo), timeout=timeout)
        if check is not None:
            self.assertEqual(r.returncode, check, r.stdout)
        return r

    def write(self, name, text):
        path = self.work / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    def lane_state(self, lane_id, *more):
        path = self.home / "state" / self.repo.name / lane_id
        return path.joinpath(*more) if more else path

    def lane_wt(self, lane_id, *more):
        path = self.home / "wt" / self.repo.name / lane_id
        return path.joinpath(*more) if more else path

    def wait_for(self, predicate, what, timeout=60, interval=0.1):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            value = predicate()
            if value:
                return value
            time.sleep(interval)
        self.fail(f"timed out after {timeout}s waiting for {what}")

    def read_json(self, path, timeout=60):
        def attempt():
            try:
                return json.loads(Path(path).read_text())
            except (OSError, json.JSONDecodeError):
                return None
        return self.wait_for(attempt, str(path), timeout=timeout)

    def run_json(self, lane_id, run=1, timeout=60):
        return self.read_json(self.lane_state(lane_id, f"run-{run}", "run.json"), timeout=timeout)

    def lane_json(self, lane_id):
        return self.read_json(self.lane_state(lane_id, "lane.json"))

    def summary(self, lane_id, run=1):
        path = self.lane_state(lane_id, f"run-{run}", "summary.txt")
        self.wait_for(path.exists, str(path))
        return path.read_text()

    def start(self, brief_text, lane_id="t1", spec=None, extra=(), env=None, wait=False):
        brief = brief_text if isinstance(brief_text, Path) else self.write(f"{lane_id}.md", brief_text)
        r = self.cli("start", brief, "--repo", self.repo, "--id", lane_id,
                     *(["--no-wait"] if not wait else []), *extra, spec=spec, env=env, check=0)
        if wait:
            return r, self.run_json(lane_id)
        return r

    def resume(self, lane_id, followup_text, spec=None, extra=(), check=None):
        followup = followup_text if isinstance(followup_text, Path) else \
            self.write(f"{lane_id}-followup.md", followup_text)
        return self.cli("resume", lane_id, followup, "--repo", self.repo, "--no-wait",
                        *extra, spec=spec, check=check)

    # -- 1. configurable paths, binary and lane cap

    def test_start_creates_worktree_branch_and_commits(self):
        prompt_out = self.write("opts.json", "stale\n")
        spec = {"files": {"tools/new.txt": "written by the worker\n"},
                "opts_out": str(prompt_out), "result": RECEIPT_RESULT}
        self.start("Write tools/new.txt.\n", "t1", spec)
        run = self.run_json("t1")
        self.assertEqual(run["exit_code"], 0)
        self.assertTrue(run["committed"])
        self.assertIsNone(run["write_set_violations"])

        wt = self.lane_wt("t1")
        self.assertTrue(wt.is_dir(), "worktree exists")
        self.assertEqual(self.git("rev-parse", "ds/t1", cwd=self.repo), run["head_sha"])
        self.assertEqual(self.git("rev-parse", "HEAD", cwd=wt), run["head_sha"])
        self.assertEqual(self.git("log", "-1", "--format=%s", cwd=wt),
                         "ds-lane t1 run-1: Write tools/new.txt.")
        self.assertIn("tools/new.txt", self.git("show", "--name-only", "--format=", "HEAD", cwd=wt))

        lane = self.lane_json("t1")
        self.assertEqual(Path(lane["worktree"]), wt)
        self.assertEqual(Path(lane["state_dir"]), self.lane_state("t1"))
        self.assertEqual(lane["reasonix_version"], "reasonix 0.0.0-fake")
        self.assertIsNone(lane["write_set"])

        # The preamble reaches the worker; the receipt reaches the summary.
        opts = json.loads(prompt_out.read_text())
        self.assertIn("# ds-lane worker rules", opts["prompt"])
        self.assertIn("Write tools/new.txt.", opts["prompt"])
        self.assertEqual(opts["opts"]["--dir"], str(wt))
        self.assertEqual(opts["opts"]["--trajectory"],
                         str(self.lane_state("t1", "run-1", "trajectory.jsonl")))
        self.assertEqual(opts["opts"]["--metrics"], str(self.lane_state("t1", "run-1", "metrics.json")))
        summary = self.summary("t1")
        self.assertIn("lane t1 run-1: exit=0", summary)
        self.assertIn("committed=True", summary)
        self.assertTrue(summary.rstrip().endswith("- Open issues: none"),
                        "summary ends with the worker receipt")

        # list/show keep working off DS_LANE_HOME.
        listing = self.cli("list", "--repo", self.repo, check=0)
        self.assertIn("t1", listing.stdout)
        self.assertIn("exit=0", listing.stdout)
        shown = self.cli("show", "t1", "--repo", self.repo, check=0)
        self.assertEqual(json.loads(shown.stdout)["base_sha"], self.base_sha)
        self.assertEqual(self.cli("wait", "t1", "--repo", self.repo, check=0).returncode, 0)

        # A run whose result carries no receipt leaves the summary free of one.
        self.start("Touch only tools/keep.txt.\n", "t2", {"files": {"tools/keep.txt": "x\n"}})
        self.run_json("t2")
        self.assertNotIn("## Receipt", self.summary("t2"))

    def test_link_symlinks_and_keeps_them_out_of_the_commit(self):
        spec = {"files": {"tools/new.txt": "x\n"}}
        self.start("Write tools/new.txt.\n", "t1", spec,
                   extra=["--link", "generated", "--link", "runtime-pack=build/snap/runtime-pack"])
        run = self.run_json("t1")
        wt = self.lane_wt("t1")
        generated, runtime_pack = wt / "generated", wt / "runtime-pack"
        self.assertTrue(generated.is_symlink())
        self.assertEqual(os.readlink(generated), str(self.repo / "generated"))
        self.assertTrue(runtime_pack.is_symlink())
        self.assertEqual(os.readlink(runtime_pack), str(self.repo / "build" / "snap" / "runtime-pack"))
        self.assertEqual((generated / "data.txt").read_text(), "generated input\n")
        self.assertEqual(sorted(self.lane_json("t1")["links"]), ["generated", "runtime-pack"])

        committed = self.git("show", "--name-only", "--format=", "HEAD", cwd=wt).splitlines()
        self.assertIn("tools/new.txt", committed)
        self.assertEqual([p for p in committed if p in ("generated", "runtime-pack")], [])
        changed = self.git("diff", "--name-only", f"{self.base_sha}..{run['head_sha']}", cwd=wt)
        self.assertNotIn("generated", changed.splitlines())
        self.assertNotIn("runtime-pack", changed.splitlines())
        self.assertIsNone(run["write_set_violations"])

    # -- 2. brief phrasing preflight

    def test_preflight_refuses_bad_brief(self):
        brief = "# Brief\n\nTouch only tools/keep.txt. Do not edit anything else.\n"
        r = self.cli("start", self.write("bad.md", brief), "--repo", self.repo, "--id", "t1")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("phrasing preflight", r.stdout)
        self.assertIn("line 3", r.stdout)
        self.assertIn("Do not edit anything else.", r.stdout)
        self.assertIn("touch only X", r.stdout)
        self.assertFalse(self.lane_state("t1").exists(), "no receipts written")
        self.assertFalse(self.lane_wt("t1").exists(), "no worktree created")

    def test_preflight_accepts_fenced_and_inline_wording(self):
        fenced = ("# Brief\n\nTouch only tools/keep.txt.\n\n```text\ndo not edit other files\n"
                  "no changes elsewhere\n```\n")
        spec = {"files": {"tools/keep.txt": "ok\n"}}
        self.start(fenced, "t1", spec)
        self.assertEqual(self.run_json("t1")["exit_code"], 0)
        inline = ("# Brief\n\nThe rule `do not edit` and `no changes` appear here as quotes: "
                  "touch only tools/keep.txt.\n")
        self.start(inline, "t2", spec)
        self.assertEqual(self.run_json("t2")["exit_code"], 0)

    def test_preflight_skipped_for_read_only_and_allow_phrasing(self):
        bad = "# Brief\n\nDo not edit anything without writing a report.\n"
        spec = {"files": {}, "opts_out": str(self.work / "read-only-opts.json")}
        self.start(bad, "t1", spec, extra=["--read-only"])
        self.assertEqual(self.run_json("t1")["exit_code"], 0)
        prompt = json.loads((self.work / "read-only-opts.json").read_text())["prompt"]
        self.assertIn(DS.READ_ONLY_CLAUSE.strip(), prompt)
        self.assertTrue(self.lane_json("t1")["read_only"])

        self.start(bad, "t2", {"files": {}}, extra=["--allow-phrasing"])
        self.assertEqual(self.run_json("t2")["exit_code"], 0)
        self.assertFalse(self.lane_json("t2")["read_only"])

    def test_resume_preflights_the_follow_up(self):
        spec = {"files": {}, "events": [{"kind": "turn_started", "sessionId": "sess-1"}]}
        self.start("Touch only tools/keep.txt.\n", "t1", spec)
        self.run_json("t1")
        r = self.resume("t1", "# Repair\n\nDo not touch the ledger.\n")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("line 3", r.stdout)
        self.assertFalse(self.lane_state("t1", "run-2").exists())
        self.resume("t1", "# Repair\n\nTouch only tools/keep.txt.\n", spec={"files": {}},
                    check=0)
        self.assertEqual(self.run_json("t1", run=2)["exit_code"], 0)

    # -- 3. write-set enforcement

    def test_write_set_violation_is_recorded_and_printed(self):
        brief = ("# Brief\n\nTouch only tools/keep.txt.\n\n"
                 "```write-set\ntools/keep.txt\ntests/**\n```\n")
        spec = {"files": {"tools/keep.txt": "ok\n", "tests/deep/one.py": "ok\n",
                          "docs/sneaky.md": "outside the write set\n"}}
        self.start(brief, "t1", spec)
        run = self.run_json("t1")
        self.assertEqual(run["write_set_violations"], ["docs/sneaky.md"])
        self.assertEqual(self.lane_json("t1")["write_set"], ["tools/keep.txt", "tests/**"])
        summary = self.summary("t1")
        self.assertIn("WARNING: outside write set: docs/sneaky.md", summary)
        self.assertIn("tests/deep/one.py", summary)  # glob inside the write set is not flagged

    def test_write_set_compliant_run_records_none(self):
        brief = ("# Brief\n\nTouch only tools/keep.txt.\n\n"
                 "```write-set\ntools/keep.txt\ntests/**\n```\n")
        spec = {"files": {"tools/keep.txt": "ok\n", "tests/deep/one.py": "ok\n"}}
        self.start(brief, "t1", spec)
        run = self.run_json("t1")
        self.assertEqual(run["write_set_violations"], [])
        self.assertNotIn("outside write set", self.summary("t1"))

    # -- blocks are counted from tool results, not prose

    def test_block_counts_come_from_tool_result_errors_only(self):
        prose = (f"a message quoting '{DS.CONSTRAINT_BLOCK} this' and 'outside the writable "
                 "roots' as prose")
        self.start("Narrate the constraints.\n", "t1",
                   {"files": {}, "events": [{"kind": "message", "text": prose},
                                            {"kind": "turn_done", "status": "completed"}]})
        run = self.run_json("t1")
        self.assertEqual((run["constraint_blocks"], run["sandbox_blocks"]), (0, 0))

        events = [{"kind": "message", "text": prose},
                  {"kind": "tool_started", "tool": {"id": "c1", "name": "bash"}},
                  {"kind": "tool_result", "tool": {"id": "c1", "name": "bash", "runState": "failed",
                                                   "err": DS.CONSTRAINT_BLOCK + " this operation"}},
                  {"kind": "tool_result", "tool": {"id": "c2", "name": "write_file",
                                                   "runState": "failed",
                                                   "err": "/x is outside the writable roots"}},
                  {"kind": "turn_done", "status": "completed"}]
        self.start("Hit both blocks.\n", "t2", {"files": {}, "events": events})
        run = self.run_json("t2")
        self.assertEqual((run["constraint_blocks"], run["sandbox_blocks"]), (1, 1))
        self.assertIn("WARNING: 1 tool call(s) hit a Reasonix prompt-constraint block",
                      self.summary("t2"))

    # -- session id / resume

    def test_session_id_comes_from_the_trajectory_and_resume_passes_it(self):
        opts_out = self.write("t1-opts.json", "stale\n")
        self.start("Touch only tools/keep.txt.\n", "t1",
                   {"files": {}, "opts_out": str(opts_out),
                    "events": [{"kind": "turn_phase", "sessionId": "sess-abc123"},
                               {"kind": "turn_done", "status": "completed"}]})
        run1 = self.run_json("t1")
        self.assertEqual(run1["session_id"], "sess-abc123")

        follow = self.write("follow.md", "# Repair\n\nTouch only tools/keep.txt again.\n")
        resume_opts = self.write("t1-resume-opts.json", "stale\n")
        r = self.cli("resume", "t1", follow, "--repo", self.repo, "--no-wait", check=0,
                     spec={"files": {"tools/keep.txt": "second\n"},
                           "opts_out": str(resume_opts)})
        self.assertEqual(r.returncode, 0)
        run2 = self.run_json("t1", run=2)
        self.assertEqual(run2["resumed_session"], "sess-abc123")
        cmd = json.loads(self.lane_state("t1", "run-2", "command.json").read_text())
        self.assertEqual(cmd[cmd.index("--resume") + 1], "sess-abc123")
        seen = json.loads(resume_opts.read_text())["opts"]
        self.assertEqual(seen["--resume"], "sess-abc123")
        self.assertIn("# Follow-up from the orchestrator", json.loads(
            resume_opts.read_text())["prompt"])

    # -- 4. wall-clock timeout

    def test_timeout_kills_worker_and_still_finalizes(self):
        pid_file = self.work / "t1-worker.pid"
        spec = {"files": {"tools/half.txt": "partial\n"}, "pid_file": str(pid_file), "sleep": 120}
        env = {"DS_LANE_MAX_LANES": "1"}
        started = time.monotonic()
        self.start("Run long.\n", "t1", spec, extra=["--timeout", "2"], env=env)
        run = self.run_json("t1", timeout=90)
        self.assertEqual(run["exit_code"], 124)
        self.assertTrue(run["timed_out"])
        self.assertEqual(run["timeout_s"], 2)
        self.assertEqual(run["turn_error"], "timeout after 2 s")
        self.assertLess(time.monotonic() - started, 60)
        self.assertLess(run["duration_s"], 30)
        # The run still committed what the worker left, and released its slot.
        self.assertTrue(run["committed"])
        self.assertIn("tools/half.txt", self.git("show", "--name-only", "--format=", "HEAD",
                                                 cwd=self.lane_wt("t1")))
        self.assertFalse(self.lane_state("t1", "run-1", "worker.slot").exists())
        self.wait_for(lambda: pid_file.exists(), "worker pid file")
        self.assertRaises(ProcessLookupError, os.kill, int(pid_file.read_text()), 0)
        self.assertEqual(json.loads(self.lane_state("t1", "run-1", "spec.json").read_text())["timeout"], 2)
        # With the cap at one lane, only a released slot lets the next lane run.
        self.start("Quick job.\n", "t2", {"files": {}}, env=env)
        self.assertEqual(self.run_json("t2", timeout=60)["exit_code"], 0)

    def test_second_lane_queues_until_the_first_finishes(self):
        env = {"DS_LANE_MAX_LANES": "1"}
        self.start("Slow job.\n", "a1", {"files": {}, "sleep": 4}, env=env)
        self.wait_for(lambda: self.lane_state("a1", "run-1", "worker.slot").exists(), "a1 slot")
        self.start("Queued job.\n", "b1", {"files": {}}, env=env)
        log = self.lane_state("b1", "run-1", "supervisor.log")
        self.wait_for(lambda: log.exists() and "queued: 1 lanes running (cap 1)" in log.read_text(),
                      "b1 queue announcement", timeout=30)
        a, b = self.run_json("a1", timeout=90), self.run_json("b1", timeout=90)
        self.assertEqual((a["exit_code"], b["exit_code"]), (0, 0))
        a_start = dt.datetime.fromisoformat(a["started"]).timestamp()
        b_start = dt.datetime.fromisoformat(b["started"]).timestamp()
        self.assertGreaterEqual(b_start, a_start + a["duration_s"] - 2,
                                "the queued lane started only after the slot was freed")
        self.assertFalse(self.lane_state("a1", "run-1", "worker.slot").exists())

    # -- 5. verify

    def test_verify_saves_a_log_and_returns_the_exit_code(self):
        self.start("Touch only tools/keep.txt.\n", "t1", {"files": {"tools/keep.txt": "v\n"}})
        run = self.run_json("t1")
        # verify pins CARGO_BUILD_JOBS=2 even when the caller asks for more.
        r = self.cli("verify", "--repo", self.repo, "t1", "--", "bash", "-c",
                     "echo hello-from-verify; echo jobs=$CARGO_BUILD_JOBS; exit 3",
                     env={"CARGO_BUILD_JOBS": "8"})
        self.assertEqual(r.returncode, 3)
        self.assertIn("hello-from-verify", r.stdout)
        log = self.lane_state("t1", "verify", "001.log")
        text = log.read_text()
        self.assertIn("command: bash -c echo hello-from-verify;", text)
        self.assertIn("utc_start: ", text)
        self.assertIn(f"head: {run['head_sha']}", text)
        self.assertIn("exit_code: 3", text)
        self.assertIn("duration_s: ", text)
        self.assertIn("hello-from-verify", text)
        self.assertIn("jobs=2", text)
        self.assertEqual(self.cli("verify", "--repo", self.repo, "t1", "--",
                                  "bash", "-c", "exit 0").returncode, 0)
        self.assertTrue(self.lane_state("t1", "verify", "002.log").exists())
        # CMD runs in the lane worktree.
        self.cli("verify", "--repo", self.repo, "t1", "--", "bash", "-c",
                 "test -f tools/keep.txt && echo in-worktree")
        self.assertIn("in-worktree", self.lane_state("t1", "verify", "003.log").read_text())

    # -- 6. tail

    def test_tail_prints_state_and_recent_tool_calls(self):
        events = [{"kind": "message", "text": "looking at the sources now"},
                  {"kind": "tool_started", "tool": {"id": "c1", "name": "bash"}},
                  {"kind": "tool_result", "tool": {"id": "c1", "name": "bash",
                                                   "runState": "completed",
                                                   "args": '{"command": "rg TODO docs"}'}},
                  {"kind": "tool_result", "tool": {"id": "c2", "name": "read_file",
                                                   "runState": "failed", "err": "missing"}},
                  {"kind": "turn_done", "status": "completed"}]
        spec = {"files": {"tools/keep.txt": "x\n"}, "events": events, "sleep": 4}
        self.start("Narrate.\n", "t1", spec)
        traj = self.lane_state("t1", "run-1", "trajectory.jsonl")
        self.wait_for(lambda: traj.exists() and "rg TODO docs" in traj.read_text(), "trajectory")
        r = self.cli("tail", "t1", "--repo", self.repo, check=0)
        self.assertIn("run-1: running", r.stdout)
        self.assertIn("1. bash [completed] {\"command\": \"rg TODO docs\"}", r.stdout)
        self.assertIn("2. read_file [failed]", r.stdout)
        self.assertIn("last message: looking at the sources now", r.stdout)

        self.run_json("t1", timeout=60)
        r = self.cli("tail", "-n", "1", "t1", "--repo", self.repo, check=0)
        self.assertIn("run-1: finished", r.stdout)
        self.assertIn("elapsed=", r.stdout)
        self.assertIn("tool calls (last 1 of 2)", r.stdout)
        self.assertIn("read_file [failed]", r.stdout)
        self.assertNotIn("rg TODO docs", r.stdout)

    # -- 7. rm / purge

    def test_rm_removes_worktree_and_branch_and_keeps_receipts(self):
        spec = {"files": {"tools/new.txt": "x\n"}}
        self.start("Write tools/new.txt.\n", "t1", spec, extra=["--link", "generated"])
        self.run_json("t1")
        self.assertTrue(self.lane_wt("t1", "generated").is_symlink())
        r = self.cli("rm", "t1", "--repo", self.repo, check=0)
        self.assertIn("receipts kept", r.stdout)
        self.assertFalse(self.lane_wt("t1").exists())
        self.assertEqual(self.git("branch", "--list", "ds/t1", cwd=self.repo), "")
        self.assertNotIn(str(self.lane_wt("t1")), self.git("worktree", "list", cwd=self.repo))
        self.assertTrue(self.lane_state("t1", "run-1", "run.json").exists())

        self.start("Write tools/new.txt.\n", "t2", spec)
        self.run_json("t2")
        r = self.cli("rm", "t2", "--repo", self.repo, "--purge", check=0)
        self.assertIn("receipts purged", r.stdout)
        self.assertFalse(self.lane_state("t2").exists())
        self.assertFalse(self.lane_wt("t2").exists())


if __name__ == "__main__":
    unittest.main()
