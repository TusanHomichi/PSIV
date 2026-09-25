"""ds-lane after a supervisor dies: run numbering and the record it still leaves.

    PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v

A supervisor can die before it records anything - an exception inside finalize,
or a SIGKILL from the host - and its run directory stays behind. Numbering the
next run from `len(lane["runs"])` then tried to create that same directory
again (FileExistsError), and the session its trajectory holds was lost with it
(2026-09-24). These cases cover the numbering rule (one past the highest
`run-N` directory), the record a crashed run gets in lane.json, the session a
`resume` continues after both kinds of death, and the negative control the
numbering fix exists for: the length-based number names the crashed run again.
"""
import contextlib
import io
import json
import os
import signal
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

try:  # `unittest discover -s tests` imports this module top level
    from .ds_lane_support import DS, LaneFixture
except ImportError:  # `python -m unittest tests.test_ds_lane_crash`
    from ds_lane_support import DS, LaneFixture


class NumberingUnitCase(unittest.TestCase):
    """The numbering rule and the session lookup, called directly."""

    def test_next_run_number_is_one_past_the_highest_directory(self):
        with tempfile.TemporaryDirectory(prefix="ds-lane-number-") as tmp:
            state = Path(tmp)
            self.assertEqual(DS.next_run_number(state), 1)
            for name in ("run-1", "run-3", "run-x", "run-2.tmp", "notes"):
                (state / name).mkdir()
            self.assertEqual([p.name for p in DS.run_dirs(state)], ["run-1", "run-3"])
            self.assertEqual(DS.next_run_number(state), 4)
            # run-3 is a crashed run's leftover with no entry in lane.json, so
            # the number comes from the directories and not from the record:
            # counting the record would name run-2, one the lane already has.
            lane = {"state_dir": str(state), "runs": [{"run": 1}]}
            self.assertEqual(DS.next_run_number(lane["state_dir"]), 4)
            self.assertEqual(len(lane["runs"]) + 1, 2)

    def test_latest_session_prefers_the_record_then_a_leftover_trajectory(self):
        with tempfile.TemporaryDirectory(prefix="ds-lane-session-") as tmp:
            state = Path(tmp)
            (state / "run-1").mkdir()
            (state / "run-1" / "trajectory.jsonl").write_text(json.dumps(
                {"event": {"kind": "turn_phase", "sessionId": "sess-trail"}}) + "\n")
            lane = {"state_dir": str(state), "runs": []}
            # A supervisor killed outright recorded nothing: its trajectory is
            # the only word on the session.
            self.assertEqual(DS.latest_session(lane), "sess-trail")
            # A recorded session wins over the trajectory of an older run.
            lane["runs"] = [{"run": 1, "session_id": "sess-recorded"}]
            self.assertEqual(DS.latest_session(lane), "sess-recorded")
            # Nothing to continue at all.
            self.assertIsNone(DS.latest_session({"state_dir": str(state / "none"), "runs": []}))


class CrashCase(LaneFixture):
    """The lane commands after a supervisor died, end to end."""

    def test_resume_after_a_killed_supervisor_numbers_run_2_and_keeps_the_session(self):
        """A SIGKILL leaves run-1 behind with nothing in lane.json about it.

        A host crash or an OOM kill is exactly this shape, and it is the case
        the length-based number got wrong: `len(lane["runs"]) + 1` names run-1
        again, so `resume` died on FileExistsError. The next number is one past
        the highest run directory, and the session comes from the trajectory
        the dead run left, so the resumed worker continues the same
        conversation.
        """
        worker_pid = self.work / "t1-worker.pid"
        spec = {"files": {"tools/keep.txt": "one\n"},
                "events": [{"kind": "turn_phase", "sessionId": "sess-killed"},
                           {"kind": "turn_done", "status": "completed"}],
                "pid_file": str(worker_pid), "sleep": 300}
        self.start("Touch only tools/keep.txt.\n", "t1", spec)
        traj = self.lane_state("t1", "run-1", "trajectory.jsonl")
        self.wait_for(lambda: traj.exists() and "sess-killed" in traj.read_text(), "trajectory")
        self.wait_for(lambda: worker_pid.exists(), "worker pid file")
        supervisor = int(self.lane_state("t1", "run-1", "supervisor.pid").read_text())
        os.kill(supervisor, signal.SIGKILL)  # a crash, not a `stop`
        self.wait_for(lambda: not self.pid_exists(supervisor), "supervisor exit")
        os.kill(int(worker_pid.read_text()), signal.SIGKILL)  # the orphan the kill left
        self.assertFalse(self.lane_state("t1", "run-1", "run.json").exists())
        self.assertFalse(self.lane_state("t1", "run-1", "failed").exists())
        self.assertEqual(self.lane_json("t1")["runs"], [], "nothing recorded the run")

        # The negative control: the number the record would have given is taken.
        stale = self.lane_state("t1", f"run-{len(self.lane_json('t1')['runs']) + 1}")
        self.assertTrue(stale.exists(), "the length-based number is the crashed run")
        with self.assertRaises(FileExistsError):
            stale.mkdir(parents=True)

        self.resume("t1", "# Continue\n\nTouch only tools/keep.txt.\n",
                    spec={"files": {"tools/keep.txt": "two\n"}}, check=0)
        run2 = self.run_json("t1", run=2)
        self.assertEqual(run2["exit_code"], 0)
        self.assertEqual(run2["resumed_session"], "sess-killed")
        cmd = json.loads(self.lane_state("t1", "run-2", "command.json").read_text())
        self.assertEqual(cmd[cmd.index("--resume") + 1], "sess-killed")
        self.assertEqual([r["run"] for r in self.lane_json("t1")["runs"]], [2])
        # run-1 is left as the crash left it; the resumed run is a new directory.
        self.assertFalse(self.lane_state("t1", "run-1", "run.json").exists())
        self.assertEqual(self.cli("wait", "t1", "--repo", self.repo, check=0).returncode, 0)


class CrashRecordCase(LaneFixture):
    """The record a run gets when its supervisor dies inside finalize.

    `exec_run` is the detached supervisor's entry point, so these cases drive
    the real thing in process: a real worker, a real trajectory, a real lane
    record. Only the named finalize step fails, which is the shape of a
    supervisor that dies before run.json lands.
    """

    def lane(self, lane_id="t1"):
        """The lane dict a real `start` writes, with no run launched yet."""
        wt, state = self.lane_wt(lane_id), self.lane_state(lane_id)
        wt.mkdir(parents=True)
        state.mkdir(parents=True)
        lane = {"id": lane_id, "title": "Crashed run", "repo": str(self.repo),
                "base_ref": "HEAD", "base_sha": self.base_sha, "branch": f"ds/{lane_id}",
                "worktree": str(wt), "state_dir": str(state), "links": [], "link_sources": {},
                "add_dirs": [], "created": DS.now(), "model": DS.MODEL, "effort": DS.EFFORT,
                "reasonix_version": "reasonix 0.0.0-fake", "read_only": False,
                "write_set": None, "runs": []}
        DS.save_receipt(state, lane)
        return lane

    def drive(self, lane, worker_spec, *, run=1, failing="commit_run"):
        """Run one run through exec_run with the named finalize step raising."""
        state, wt = Path(lane["state_dir"]), Path(lane["worktree"])
        run_dir = state / f"run-{run}"
        run_dir.mkdir(parents=True)
        (run_dir / "prompt.md").write_text("Touch only tools/keep.txt.\n")
        (run_dir / "trajectory.jsonl").touch()  # the supervisor touches it for the worker
        spec = {"run": run, "cmd": [sys.executable, str(self.fake), "run", "--dir", str(wt),
                                    "--trajectory", str(run_dir / "trajectory.jsonl")],
                "resume_session": "sess-before", "timeout": 300, "max_steps": 0,
                "stall_timeout": 300, "stall_retries_left": 0, "resumed_after_stall": False}
        (run_dir / "spec.json").write_text(json.dumps(spec, indent=2) + "\n")
        previous = signal.getsignal(signal.SIGTERM)  # exec_run installs its own handler
        self.addCleanup(signal.signal, signal.SIGTERM, previous)
        env = {**self.env, "FAKE_REASONIX_SPEC": json.dumps(worker_spec)}
        # `finalize_run` resolves both named steps in the receipts namespace,
        # whichever module happens to define them (receipt_block moved to
        # trajectory; it is imported back for the record's summary).
        boom = mock.patch.object(DS.receipts, failing,
                                 side_effect=RuntimeError("finalize exploded"))
        with mock.patch.dict(os.environ, env), boom, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(RuntimeError) as caught:
                DS.exec_run(str(state), run)
        self.assertEqual(str(caught.exception), "finalize exploded")
        return run_dir

    def worker_spec(self, cost=0.25):
        return {"files": {"tools/keep.txt": "one\n"},
                "events": [{"kind": "turn_phase", "sessionId": "sess-crash"},
                           {"kind": "turn_done", "status": "completed"}],
                "cost_usd": cost}

    def test_a_crashed_run_is_recorded_with_error_and_what_is_known(self):
        """The run lands in lane.json: its error, session, exit code and cost.

        Before this, a supervisor that raised left nothing but a run directory:
        `wait` reported no record at all and `resume` lost the session. The run
        still has no run.json - it did not finish - so `failed` marks it and
        `wait` reports it as failed with the error.
        """
        lane = self.lane()
        run_dir = self.drive(lane, self.worker_spec())

        self.assertFalse((run_dir / "run.json").exists(), "a crashed run is not a finished one")
        self.assertIn("finalize exploded", (run_dir / "failed").read_text())
        recorded = DS.load_receipt(lane["state_dir"])
        self.assertEqual([r["run"] for r in recorded["runs"]], [1])
        run = recorded["runs"][0]
        self.assertEqual(run["outcome"], "error")
        self.assertIn("finalize exploded", run["error"])
        self.assertEqual(run["session_id"], "sess-crash")   # read from the trajectory
        self.assertEqual(run["exit_code"], 0)               # the worker's own result
        self.assertEqual(run["cost_usd"], 0.25)
        self.assertEqual(recorded["total_cost_usd"], 0.25)
        self.assertEqual(run["resumed_session"], "sess-before")
        self.assertEqual((run["constraint_blocks"], run["sandbox_blocks"]), (0, 0))
        # `list` and `resume` read the record, so the lane stays usable.
        self.assertIn("t1", self.cli("list", "--repo", self.repo, check=0).stdout)

        summary = (run_dir / "summary.txt").read_text()
        self.assertIn("FAILED supervisor error", summary)
        self.assertIn("finalize exploded", summary)
        self.assertIn("exit=0 cost=$0.25 session=sess-crash", summary)
        with contextlib.redirect_stdout(io.StringIO()) as printed:
            self.assertEqual(DS.wait_run(run_dir), 1)
        self.assertIn("finalize exploded", printed.getvalue())
        self.assertEqual(self.cli("wait", "t1", "--repo", self.repo, check=1).returncode, 1)

    def test_a_crash_after_the_record_updates_it_instead_of_adding_a_second(self):
        """finalize appends its record before run.json: the crash fills the gaps.

        `receipt_block` runs after that append, so a failure there is the case
        the update path exists for: one run in lane.json, not two, and still
        the error that says why it has no run.json.
        """
        lane = self.lane()
        run_dir = self.drive(lane, self.worker_spec(cost=0.5), failing="receipt_block")
        self.assertFalse((run_dir / "run.json").exists())
        recorded = DS.load_receipt(lane["state_dir"])
        self.assertEqual([r["run"] for r in recorded["runs"]], [1])
        self.assertEqual(recorded["runs"][0]["outcome"], "error")
        self.assertIn("finalize exploded", recorded["runs"][0]["error"])
        self.assertEqual(recorded["runs"][0]["exit_code"], 0)  # what finalize had measured
        self.assertEqual(recorded["runs"][0]["cost_usd"], 0.5)
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(DS.wait_run(run_dir), 1)
