"""ds-lane's lane commands, end to end, against a throwaway repo and fake worker.

    PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v

`start` and `resume`, the phrasing and write-set preflights, the prompt's trip
to the worker on stdin, worker host state, the write set each run is checked
against, `verify`, `tail` and `rm`.
"""
import json
import os
import time
from pathlib import Path

try:  # `unittest discover -s tests` imports this module top level
    from .ds_lane_support import DS, LaneFixture, RECEIPT_RESULT
except ImportError:  # `python -m unittest tests.test_ds_lane_lanes`
    from ds_lane_support import DS, LaneFixture, RECEIPT_RESULT


class LaneCase(LaneFixture):
    """The lane commands end to end: `start` and `resume`, and the receipts they leave."""

    # -- 1. start, worktree and commit

    def test_worker_never_sees_the_callers_secrets(self):
        opts_out = self.write("env-opts.json", "stale\n")
        spec = {"files": {}, "opts_out": str(opts_out), "result": RECEIPT_RESULT}
        self.start("Record the environment.\n", "tenv", spec,
                   env={"GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_leak", "SSH_AUTH_SOCK": "/tmp/agent"})
        self.assertEqual(self.run_json("tenv")["exit_code"], 0)
        names = json.loads(opts_out.read_text())["env"]
        self.assertNotIn("GITHUB_PERSONAL_ACCESS_TOKEN", names)
        self.assertNotIn("SSH_AUTH_SOCK", names)
        self.assertIn("PATH", names)
        self.assertIn("CARGO_BUILD_JOBS", names)

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


    # -- 4. a follow-up's write set replaces the lane's

    def test_resume_follow_up_write_set_replaces_the_lanes(self):
        """A follow-up's own write-set block governs the run and those after it.

        Lane ab-H run-3 named two new paths in its follow-up and was still
        checked against the brief's set; the block becomes the lane's set from
        that run on (orchestrator decision 2026-09-24).
        """
        brief = ("# Brief\n\nTouch only tools/keep.txt.\n\n"
                 "```write-set\ntools/keep.txt\n```\n")
        self.start(brief, "t1", {"files": {"tools/keep.txt": "one\n"}})
        run1 = self.run_json("t1")
        self.assertEqual(run1["write_set"], ["tools/keep.txt"])
        self.assertEqual(run1["write_set_violations"], [])

        followup = ("# More work\n\nTouch only tools/keep.txt and tools/second.txt.\n\n"
                    "```write-set\ntools/keep.txt\ntools/second.txt\n```\n")
        self.resume("t1", followup, spec={"files": {"tools/second.txt": "two\n"}}, check=0)
        run2 = self.run_json("t1", run=2)
        self.assertEqual(run2["write_set"], ["tools/keep.txt", "tools/second.txt"])
        self.assertEqual(run2["write_set_violations"], [])
        self.assertEqual(self.lane_json("t1")["write_set"], ["tools/keep.txt", "tools/second.txt"])
        self.assertNotIn("outside write set", self.summary("t1", run=2))
        changed = self.git("diff", "--name-only", f"{self.base_sha}..{run2['head_sha']}",
                           cwd=self.lane_wt("t1"))
        self.assertEqual(sorted(changed.splitlines()), ["tools/keep.txt", "tools/second.txt"])
        # The negative control: the brief's set alone would have flagged the new path.
        self.assertEqual(DS.write_set_violations(["tools/keep.txt", "tools/second.txt"],
                                                 ["tools/keep.txt"]), ["tools/second.txt"])

    def test_resume_without_a_write_set_inherits_the_lanes(self):
        """A follow-up with no block leaves the lane's write set in place."""
        brief = ("# Brief\n\nTouch only tools/keep.txt.\n\n"
                 "```write-set\ntools/keep.txt\n```\n")
        self.start(brief, "t1", {"files": {"tools/keep.txt": "one\n"}})
        self.run_json("t1")
        self.resume("t1", "# Repair\n\nTouch only tools/keep.txt.\n",
                    spec={"files": {"docs/elsewhere.md": "outside the set\n"}}, check=0)
        run2 = self.run_json("t1", run=2)
        self.assertEqual(run2["write_set"], ["tools/keep.txt"])
        self.assertEqual(run2["write_set_violations"], ["docs/elsewhere.md"])
        self.assertIn("WARNING: outside write set: docs/elsewhere.md", self.summary("t1", run=2))
        self.assertEqual(self.lane_json("t1")["write_set"], ["tools/keep.txt"])


    # -- 5. worker host state (.reasonix/) is filed, not committed

    def test_worker_host_state_stays_out_of_the_commit_and_is_filed(self):
        """`.reasonix/` is the worker's own agent runtime, not lane output.

        The finalize `git add -A` swept it into lane ab-J's commit and the
        write-set check flagged it (2026-09-24); the commit step now excludes
        it at the source and the receipt keeps a copy under host-state/.
        """
        host_state = DS.HOST_STATE_PATHS[0]
        brief = ("# Brief\n\nTouch only tools/new.txt.\n\n"
                 "```write-set\ntools/new.txt\n```\n")
        spec = {"files": {"tools/new.txt": "owned\n",
                          f"{host_state}/tasks/x/events.jsonl": '{"kind": "task"}\n'}}
        self.start(brief, "t1", spec)
        run = self.run_json("t1")
        self.assertEqual(run["exit_code"], 0)
        self.assertTrue(run["committed"])
        self.assertEqual(run["write_set_violations"], [])

        wt = self.lane_wt("t1")
        self.assertEqual(self.git("show", "--name-only", "--format=", "HEAD", cwd=wt).splitlines(),
                         ["tools/new.txt"])
        self.assertEqual(self.git("diff", "--name-only", f"{self.base_sha}..{run['head_sha']}",
                                  cwd=wt).splitlines(), ["tools/new.txt"])
        self.assertIn(f"?? {host_state}/", self.git("status", "--porcelain", cwd=wt))
        # Untouched on disk in the worktree, and reviewable in the receipt.
        self.assertTrue((wt / host_state / "tasks/x/events.jsonl").exists())
        filed = self.lane_state("t1", "run-1", "host-state", host_state, "tasks", "x", "events.jsonl")
        self.assertTrue(filed.exists(), "host state is filed with the run's receipts")
        self.assertEqual(filed.read_text(), '{"kind": "task"}\n')

        # A worker that wrote no host state has no host-state/ receipt either.
        self.start("Write tools/new.txt.\n", "t2", {"files": {"tools/new.txt": "x\n"}})
        self.run_json("t2")
        self.assertFalse(self.lane_state("t2", "run-1", "host-state").exists())


    # -- 6. blocks are counted from tool results, not prose

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


    # -- 7. session id / resume

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


    # -- 8. verify, tail and rm

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

    # -- 9. the prompt reaches the worker on stdin, not on the command line

    def test_prompt_travels_on_stdin_and_never_as_an_argument(self):
        """No brief text is ever an argv word of the worker's command line.

        A prompt in argv is text the worker's own process matching can hit -
        lane sw-S1-motavia ran a `pkill -f` whose pattern sat in its brief and
        SIGTERM'd the harness that started it (2026-09-24) - so the supervisor
        feeds the run's `prompt.md` to the worker's stdin, and the recorded
        command names that file instead of quoting it. The fake worker refuses
        an argv prompt, so a regression here shows up as a failed run.
        """
        token = "MOTAVIA-ARGV-TOKEN"
        first = self.write("t1-opts.json", "stale\n")
        brief = f"# Brief\n\nTouch only tools/keep.txt and name {token} once.\n"
        self.start(brief, "t1", {"files": {"tools/keep.txt": "one\n"},
                                 "opts_out": str(first), "result": RECEIPT_RESULT})
        self.assertEqual(self.run_json("t1")["exit_code"], 0)
        run1 = self.lane_state("t1", "run-1")
        seen = json.loads(first.read_text())
        self.assertEqual(seen["prompt"], (run1 / "prompt.md").read_text(),
                         "the worker read the run's own prompt.md")
        self.assertIn(token, seen["prompt"])
        self.assertIn("# ds-lane worker rules", seen["prompt"])

        cmd = json.loads((run1 / "command.json").read_text())
        self.assertEqual(cmd[-1], "<stdin: prompt.md>")
        self.assertEqual(cmd[:-1], seen["argv"], "the recorded command is the worker's argv")
        self.assertNotIn(token, json.dumps(cmd))
        self.assertNotIn(token, " ".join(seen["argv"]))
        self.assertNotIn(token, (run1 / "spec.json").read_text())

        # A resumed run gets the same treatment with its follow-up prompt.
        token2 = "MOTAVIA-RESUME-TOKEN"
        second = self.write("t1-resume-opts.json", "stale\n")
        follow = f"# Repair\n\nTouch only tools/keep.txt and name {token2} once.\n"
        self.resume("t1", follow, spec={"files": {}, "opts_out": str(second)}, check=0)
        self.assertEqual(self.run_json("t1", run=2)["exit_code"], 0)
        run2 = self.lane_state("t1", "run-2")
        seen2 = json.loads(second.read_text())
        self.assertEqual(seen2["prompt"], (run2 / "prompt.md").read_text())
        self.assertIn("# Follow-up from the orchestrator", seen2["prompt"])
        self.assertIn(token2, seen2["prompt"])

        cmd2 = json.loads((run2 / "command.json").read_text())
        self.assertEqual(cmd2[-1], "<stdin: prompt.md>")
        self.assertEqual(cmd2[:-1], seen2["argv"])
        for text in (json.dumps(cmd2), " ".join(seen2["argv"])):
            self.assertNotIn(token2, text)
            self.assertNotIn(token, text)  # the brief's own text neither
        self.assertNotIn(token2, (run2 / "spec.json").read_text())
