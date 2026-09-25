"""ds-lane's supervisor: slots, group kills, timeouts, stop and the watchdog.

    PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v

The cases drive the real CLI, so every one of them goes through the detached
supervisor: queueing under the machine-wide lane cap, killing a worker's process
group on timeout or `stop`, the stall watchdog and its automatic resume.
"""
import datetime as dt
import json
import os
import re
import time

try:  # `unittest discover -s tests` imports this module top level
    from .ds_lane_support import DS, DS_LANE, LaneFixture, RECEIPT_RESULT
except ImportError:  # `python -m unittest tests.test_ds_lane_supervisor`
    from ds_lane_support import DS, DS_LANE, LaneFixture, RECEIPT_RESULT


class SupervisorCase(LaneFixture):
    """The supervisor: machine-wide slots, group kills, timeouts and the watchdog."""

    # -- 1. wall clock, queueing under the lane cap

    def test_timeout_kills_worker_and_still_finalizes(self):
        pid_file = self.worker_path("t1", "worker.pid")
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

    def test_timeout_reaps_orphaned_group_members(self):
        """A killed worker must not leave its spawned children running.

        The worker itself ignores nothing (it dies on the group SIGTERM); the
        child it spawns is the stubborn case the report describes - a member
        that only the final group SIGKILL clears, whatever the parent did.
        """
        pid_file = self.worker_path("t1", "worker.pid")
        child_file = self.worker_path("t1", "child.pid")
        spec = {"files": {}, "pid_file": str(pid_file),
                "spawn_sleep": {"seconds": 120, "pid_file": str(child_file)}, "sleep": 120}
        self.start("Long job.\n", "t1", spec, extra=["--timeout", "2"])
        self.assertEqual(self.run_json("t1", timeout=90)["exit_code"], 124)
        self.wait_for(lambda: pid_file.exists() and child_file.exists(), "worker and child pids")
        self.wait_for(lambda: not self.pid_exists(int(pid_file.read_text())), "worker exit",
                      timeout=15)
        self.wait_for(lambda: not self.pid_exists(int(child_file.read_text())), "orphan child exit",
                      timeout=15)

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


    # -- 2. stop (orchestrator request)

    def test_stop_running_worker_finalizes_with_143(self):
        pid_file = self.worker_path("t1", "worker.pid")
        child_file = self.worker_path("t1", "child.pid")
        spec = {"files": {"tools/half.txt": "partial\n"}, "pid_file": str(pid_file),
                "spawn_sleep": {"seconds": 120, "pid_file": str(child_file)}, "sleep": 120}
        env = {"DS_LANE_MAX_LANES": "1"}
        self.start("Run long.\n", "t1", spec, env=env)
        self.wait_for(lambda: pid_file.exists() and child_file.exists(), "worker and child pids")
        r = self.cli("stop", "t1", "--repo", self.repo, check=0, timeout=90)
        self.assertIn("stopping supervisor", r.stdout)
        self.assertIn("exit=143", r.stdout)          # the run's summary is printed
        self.assertIn("stopped by orchestrator", r.stdout)

        run = self.run_json("t1")
        self.assertEqual(run["exit_code"], 143)
        self.assertEqual(run["outcome"], "stopped")
        self.assertEqual(run["turn_error"], "stopped by orchestrator")
        # The run still committed what the worker left, and released its slot.
        self.assertTrue(run["committed"])
        self.assertIn("tools/half.txt", self.git("show", "--name-only", "--format=", "HEAD",
                                                 cwd=self.lane_wt("t1")))
        self.assertFalse(self.lane_state("t1", "run-1", "worker.slot").exists())
        self.wait_for(lambda: not self.pid_exists(int(pid_file.read_text())), "worker exit", timeout=15)
        # The worker's group is finished too, children included.
        self.wait_for(lambda: not self.pid_exists(int(child_file.read_text())), "child exit", timeout=15)
        # With the cap at one lane, only a released slot lets the next lane run.
        self.start("Quick job.\n", "t2", {"files": {}}, env=env)
        self.assertEqual(self.run_json("t2", timeout=60)["exit_code"], 0)
        # A second stop has nothing left to do.
        self.assertIn("no run in flight", self.cli("stop", "t1", "--repo", self.repo, check=0).stdout)

    def test_stop_queued_run_records_it_stopped(self):
        env = {"DS_LANE_MAX_LANES": "1"}
        self.start("Slow job.\n", "a1", {"files": {}, "sleep": 6}, env=env)
        self.wait_for(lambda: self.lane_state("a1", "run-1", "worker.slot").exists(), "a1 slot")
        b_pid = self.worker_path("b1", "worker.pid")
        self.start("Queued job.\n", "b1", {"files": {"tools/b1.txt": "x\n"},
                                           "pid_file": str(b_pid)}, env=env)
        log = self.lane_state("b1", "run-1", "supervisor.log")
        self.wait_for(lambda: log.exists() and "queued: 1 lanes running (cap 1)" in log.read_text(),
                      "b1 queue announcement", timeout=30)
        r = self.cli("stop", "b1", "--repo", self.repo, check=0, timeout=90)
        self.assertIn("stopped by orchestrator", r.stdout)
        run = self.run_json("b1")
        self.assertEqual(run["exit_code"], 143)
        self.assertEqual(run["outcome"], "stopped")
        self.assertEqual(run["duration_s"], 0.0)
        self.assertFalse(run["committed"])
        # Nothing ran: no worker process, no worker output, no file, no slot.
        self.assertFalse(b_pid.exists())
        self.assertFalse(self.lane_state("b1", "run-1", "stdout.log").exists())
        self.assertFalse(self.lane_state("b1", "run-1", "worker.slot").exists())
        self.assertFalse(self.lane_wt("b1", "tools", "b1.txt").exists())
        # The lane that held the slot is unaffected.
        self.assertEqual(self.run_json("a1", timeout=90)["exit_code"], 0)

    def test_stop_idle_lane_exits_zero(self):
        self.start("Touch only tools/keep.txt.\n", "t1", {"files": {"tools/keep.txt": "x\n"}})
        self.run_json("t1")
        for _ in range(2):
            r = self.cli("stop", "t1", "--repo", self.repo, check=0)
            self.assertIn("no run in flight", r.stdout)


    # -- 3. stall watchdog

    def test_stalled_run_is_finalized_125_and_auto_resumed(self):
        """A worker alive with a dead stream is stopped, then resumed by the harness.

        DS_LANE_STALL_POLL only shortens the sampling interval: the detector,
        the exit code, the resumed run and the `wait` chain are the real ones.
        """
        env = {"DS_LANE_MAX_LANES": "1", "DS_LANE_STALL_POLL": "0.2"}
        spec = {"files": {},
                "events": [{"kind": "turn_phase", "sessionId": "sess-stall"},
                           {"kind": "turn_done", "status": "completed"}],
                "sleep": 300,  # silent and idle: the host-suspend shape
                "resume_spec": {"sleep": 0, "exit_code": 7,
                                "files": {"tools/resumed.txt": "continued\n"},
                                "result": RECEIPT_RESULT}}
        self.start("Run long.\n", "t1", spec, env=env,
                   extra=["--stall-timeout", "1", "--timeout", "600"])
        # `wait` attaches before the stall and must report the final run; it can
        # only have been blocked while run-1 stalled if it stayed for a window.
        attached = time.monotonic()
        r = self.cli("wait", "t1", "--repo", self.repo, check=7, timeout=120)
        self.assertGreater(time.monotonic() - attached, 1.0)
        self.assertIn("exit=125", r.stdout)
        self.assertIn("auto-resumed after the stall as run-2", r.stdout)
        self.assertIn("exit=7", r.stdout)

        run1 = self.run_json("t1")
        self.assertEqual(run1["exit_code"], 125)
        self.assertEqual(run1["outcome"], "stalled")
        self.assertTrue(run1["stalled"])
        self.assertFalse(run1["timed_out"])
        self.assertEqual(run1["turn_error"], "stalled: no progress for 1 s")
        self.assertEqual(run1["stall_timeout_s"], 1)
        self.assertEqual(run1["stall_retries_left"], 1)
        self.assertFalse(run1["resumed_after_stall"])
        self.assertEqual(run1["stall_resumed_run"], 2)
        self.assertFalse(self.lane_state("t1", "run-1", "worker.slot").exists())

        run2 = self.run_json("t1", run=2)
        self.assertEqual(run2["exit_code"], 7)
        self.assertEqual(run2["outcome"], "completed")
        self.assertEqual(run2["resumed_session"], "sess-stall")
        self.assertTrue(run2["resumed_after_stall"])
        self.assertEqual(run2["stall_retries_left"], 0)   # the retry is spent
        self.assertEqual(run2["stall_timeout_s"], 1)
        self.assertEqual(run2["timeout_s"], 600)          # the budget is carried over
        self.assertIsNone(run2["stall_resumed_run"])
        self.assertTrue(run2["committed"])
        self.assertIn("tools/resumed.txt", self.git("show", "--name-only", "--format=", "HEAD",
                                                    cwd=self.lane_wt("t1")))
        cmd = json.loads(self.lane_state("t1", "run-2", "command.json").read_text())
        self.assertEqual(cmd[cmd.index("--resume") + 1], "sess-stall")
        prompt = self.lane_state("t1", "run-2", "prompt.md").read_text()
        self.assertIn("stalled", prompt)
        self.assertIn("session context is intact", prompt)
        self.assertIn("## Receipt", prompt)
        self.assertEqual([r["run"] for r in self.lane_json("t1")["runs"]], [1, 2])
        # A waiter pinned to the stalled run itself follows the same chain.
        self.assertEqual(DS.wait_run(self.lane_state("t1", "run-1")), 7)

    def test_cpu_burning_worker_is_not_stalled(self):
        """A silent `cargo build` burns CPU: the watchdog must leave it alone."""
        env = {"DS_LANE_STALL_POLL": "0.2"}
        spec = {"files": {}, "burn": 3, "result": RECEIPT_RESULT}
        self.start("Burn CPU.\n", "t1", spec, env=env,
                   extra=["--stall-timeout", "1", "--stall-retries", "0"])
        run = self.run_json("t1", timeout=90)
        self.assertEqual(run["exit_code"], 0)
        self.assertEqual(run["outcome"], "completed")
        self.assertFalse(run["stalled"])
        self.assertIsNone(run["stall_resumed_run"])
        self.assertFalse(self.lane_state("t1", "run-2").exists())

    def test_low_cpu_worker_is_stalled(self):
        """A process kept alive by a dead stream gains a tick every so often.

        0.004 s of CPU a second is 0.4% of a core: alive, and nowhere near
        work, so the window rate decides it is stalled. The measured rate
        stays under the threshold, which is what the supervisor log records.
        """
        env = {"DS_LANE_STALL_POLL": "0.2"}
        spec = {"files": {}, "burn_low": {"period": 1.0, "busy": 0.004}}
        self.start("Run long.\n", "t1", spec, env=env,
                   extra=["--stall-timeout", "3", "--stall-retries", "0", "--timeout", "20"])
        run = self.run_json("t1", timeout=90)
        self.assertEqual(run["exit_code"], 125)
        self.assertEqual(run["outcome"], "stalled")
        self.assertEqual(run["turn_error"], "stalled: no progress for 3 s")
        log = self.lane_state("t1", "run-1", "supervisor.log").read_text()
        self.assertIn("stalled: no progress for 3 s", log)
        rate = float(re.search(r"group CPU ([\d.]+)% of a core", log).group(1))
        self.assertLess(rate, 1.0, log)
        self.assertIn("under 1%", log)

    def test_stall_retries_zero_finalizes_without_resume(self):
        env = {"DS_LANE_STALL_POLL": "0.2"}
        spec = {"files": {}, "events": [{"kind": "turn_phase", "sessionId": "sess-x"}],
                "sleep": 300}
        self.start("Run long.\n", "t1", spec, env=env,
                   extra=["--stall-timeout", "1", "--stall-retries", "0"])
        run = self.run_json("t1", timeout=90)
        self.assertEqual(run["exit_code"], 125)
        self.assertEqual(run["outcome"], "stalled")
        self.assertEqual(run["turn_error"], "stalled: no progress for 1 s")
        self.assertEqual(run["stall_retries_left"], 0)
        self.assertIsNone(run["stall_resumed_run"])
        self.assertIn("stalled: no progress for 1 s", self.summary("t1"))
        # Nothing follows: no successor run appears, and `wait` returns the stall.
        time.sleep(2)
        self.assertFalse(self.lane_state("t1", "run-2").exists())
        self.assertEqual([r["run"] for r in self.lane_json("t1")["runs"]], [1])
        self.assertEqual(self.cli("wait", "t1", "--repo", self.repo, check=125).returncode, 125)

    def test_stall_settings_default_and_zero_disables(self):
        spec = {"files": {}}
        self.start("Quick job.\n", "t1", spec)
        run = self.run_json("t1")
        self.assertEqual((run["stall_timeout_s"], run["stall_retries_left"]), (900, 1))
        self.assertFalse(run["resumed_after_stall"])
        spec_json = json.loads(self.lane_state("t1", "run-1", "spec.json").read_text())
        self.assertEqual(spec_json["stall_timeout"], 900)
        self.assertEqual(spec_json["stall_retries_left"], 1)
        self.assertEqual(spec_json["timeout"], 5400)
        self.start("Quick job two.\n", "t2", spec, extra=["--stall-timeout", "0"])
        run2 = self.run_json("t2")
        self.assertEqual(run2["exit_code"], 0)
        self.assertEqual(run2["stall_timeout_s"], 0)  # watchdog off, run unaffected
        self.assertFalse(run2["stalled"])


    # -- 4. the entry point, through a symlink, into the supervisor

    def test_cli_runs_through_a_symlink(self):
        """~/.local/bin/ds-lane is a symlink: the shim resolves its own directory.

        The detached supervisor re-enters the same file, so this covers the
        whole path - launch, slot, worker, commit and receipts.
        """
        bindir = self.root / "bin"
        bindir.mkdir()
        link = bindir / "ds-lane"
        link.symlink_to(DS_LANE)
        brief = self.write("t1.md", "Write tools/new.txt.\n")
        r = self.cli("start", brief, "--repo", self.repo, "--id", "t1", tool=link,
                     spec={"files": {"tools/new.txt": "x\n"}}, check=0, timeout=120)
        self.assertIn("worktree", r.stdout)
        run = self.run_json("t1")
        self.assertEqual(run["exit_code"], 0)
        self.assertTrue(run["committed"])
        self.assertTrue(self.lane_wt("t1", "tools", "new.txt").exists())

