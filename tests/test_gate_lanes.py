"""Cases for the full gate's lanes in tools/gate.py: two lanes side by side.

    PYTHONPATH=. python3 -m unittest tests.test_gate_lanes -v

Hermetic, like tests/test_gate.py, whose throwaway-repo fixture this borrows:
fake commands only, never the real gate. The lanes run at once is shown by
two commands that each wait for the other's file, which only lanes side by
side can finish; the negative control runs them in one lane and they can't.
"""
import contextlib
import io
import json
import os
import signal
import subprocess
import sys
import time
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from unittest import mock

from tests import test_gate as fixture
from tools.gate import (CERTIFY_COMMAND, FAST_COMMANDS, GATE_COMMANDS, GATE_LANES, lanes_for,
                        main, run_gate)

#: Seconds a rendezvous command waits for its partner before it gives up.
RENDEZVOUS_S = 20


def rendezvous(root, mine, theirs, exit_code=0):
    """A command that writes `mine`, waits for `theirs`, then exits `exit_code`
    (3 when `theirs` never came)."""
    code = ("import os, sys, time\n"
            f"open({str(root / mine)!r}, 'w').write('here')\n"
            f"deadline = time.monotonic() + {RENDEZVOUS_S}\n"
            f"while not os.path.exists({str(root / theirs)!r}):\n"
            "    if time.monotonic() > deadline:\n"
            "        sys.exit(3)\n"
            "    time.sleep(0.02)\n"
            f"print('{mine} met {theirs}')\n"
            f"sys.exit({exit_code})\n")
    script = root / f"{mine}.py"
    script.write_text(code)
    return f"python3 {script}"


class TheGatesLanesCase(unittest.TestCase):
    """GATE_LANES is the documented gate, split so no lane waits on another's cargo."""

    def test_the_lanes_hold_every_command_once_in_order(self):
        lanes = lanes_for(GATE_COMMANDS, GATE_LANES)
        self.assertEqual(len(lanes), 2)
        self.assertEqual(sorted(i for lane in lanes for i in lane), list(range(len(GATE_COMMANDS))))

    def test_every_debug_target_build_is_in_one_lane(self):
        """cargo fmt, test, clippy and certify's psiv-godot build share the debug
        target, so a second lane of them would only wait on cargo's lock."""
        debug = [command for command in GATE_COMMANDS
                 if command.startswith("cargo ") or command == CERTIFY_COMMAND]
        self.assertEqual(len(debug), 4)
        self.assertEqual([lane for lane in GATE_LANES if set(debug) & set(lane)],
                         [tuple(debug)])

    def test_a_lane_that_reorders_drops_or_adds_a_command_is_refused(self):
        a, b, c = "a", "b", "c"
        for lanes in (((b, a), (c,)), ((a,), (c,)), ((a, b), (c, "d")), ((a, b), (b, c))):
            with self.subTest(lanes=lanes), self.assertRaises(ValueError):
                lanes_for([a, b, c], lanes)
        self.assertEqual(lanes_for([a, b, c], ((a, c), (b,))), ((0, 2), (1,)))
        self.assertEqual(lanes_for([a, a]), ((0, 1),))  # one lane may repeat a command

    def test_main_runs_the_full_gate_in_its_lanes_and_the_fast_tier_in_one(self):
        for argv, commands, lanes in (([], GATE_COMMANDS, GATE_LANES), (["--fast"], FAST_COMMANDS, None)):
            with self.subTest(argv=argv), \
                    mock.patch("tools.gate.toolchain_missing", return_value=[]), \
                    mock.patch("tools.gate.receipt_dir_for", return_value=Path("/nonexistent")), \
                    mock.patch("tools.gate.run_gate", return_value={"exit_code": 0}) as gate:
                self.assertEqual(main(argv), 0)
            self.assertEqual(gate.call_args.args[0], commands)
            self.assertEqual(gate.call_args.kwargs["lanes"], lanes)


class LaneRunCase(unittest.TestCase):
    """Lanes run side by side under one gate, one receipt."""

    setUp = fixture.GateRunCase.setUp
    git = fixture.GateRunCase.git
    init_repo = fixture.GateRunCase.init_repo

    def run_lanes(self, commands, lanes):
        """`run_gate` with stdout captured: (receipt, its printed lines)."""
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            receipt = run_gate(commands, self.root, self.receipt_dir, lanes=lanes)
        return receipt, buffer.getvalue().splitlines()

    def test_the_lanes_run_side_by_side(self):
        left = rendezvous(self.root, "left", "right")
        right = rendezvous(self.root, "right", "left")
        receipt, _ = self.run_lanes([left, right], ((left,), (right,)))
        self.assertEqual([r["exit_code"] for r in receipt["commands"]], [0, 0])
        self.assertEqual([r["lane"] for r in receipt["commands"]], [0, 1])
        self.assertEqual(receipt["lanes"], [[0], [1]])
        self.assertEqual(receipt["exit_code"], 0)

    def test_negative_control_one_lane_cannot_meet_itself(self):
        """A gate that runs its commands one at a time: the same two in one lane. The
        first never sees its partner and gives up, so the case above fails on
        a gate that runs its commands in order."""
        with mock.patch(f"{__name__}.RENDEZVOUS_S", 1):
            left = rendezvous(self.root, "left", "right")
        right = rendezvous(self.root, "right", "left")
        receipt, _ = self.run_lanes([left, right], None)
        self.assertEqual(receipt["commands"][0]["exit_code"], 3)
        self.assertEqual(receipt["exit_code"], 1)

    def test_the_receipt_is_in_gate_order_and_its_duration_is_the_runs_own(self):
        """Records keep the documented order whichever lane finishes first, and
        `duration_s` is start to end, under the commands' sum when lanes overlap."""
        slow = 'python3 -c "import time; time.sleep(1.5)"'
        slow2 = 'python3 -c "import time; time.sleep(1.5); print(2)"'
        quick = fixture.fake_step("quick")
        commands = [slow, quick, slow2]
        receipt, printed = self.run_lanes(commands, ((slow, quick), (slow2,)))
        self.assertEqual([r["command"] for r in receipt["commands"]], commands)
        self.assertEqual([r["lane"] for r in receipt["commands"]], [0, 0, 1])
        added = sum(r["duration_s"] for r in receipt["commands"])
        self.assertLess(receipt["duration_s"], added)
        self.assertIn(f"in {receipt['duration_s']:.1f}s", receipt["summary"])
        self.assertEqual(printed[-1], receipt["summary"])
        self.assertEqual(json.loads((self.receipt_dir / "receipt.json").read_text()), receipt)

    def test_a_failing_command_stops_neither_lane(self):
        fail = fixture.fake_step("fail", marker=str(self.root / "fail.txt"), exit_code=4)
        after = fixture.fake_step("after", marker=str(self.root / "after.txt"))
        other = fixture.fake_step("other", marker=str(self.root / "other.txt"))
        receipt, _ = self.run_lanes([fail, other, after], ((fail, after), (other,)))
        self.assertEqual([r["exit_code"] for r in receipt["commands"]], [4, 0, 0])
        for marker in ("fail.txt", "after.txt", "other.txt"):
            self.assertTrue((self.root / marker).is_file(), marker)
        self.assertIn("gate FAILED", receipt["summary"])

    #: The ways a command's descendant can get away from it: in its process
    #: group, under `timeout` (a group of its own), in a session of its own,
    #: orphaned by a parent that exits, and a zombie whose parent never reaps it.
    ESCAPES = ("grouped", "timed", "session", "orphan", "zombie")

    def escaping_command(self, ways=ESCAPES):
        """A command that starts one descendant each way ESCAPES names, each writing
        its pid to `<way>.pid` (and the zombie's to `zombie-child.pid`), then
        sleeping 30 s before it writes `woke`."""
        spawned = self.root / "spawned"
        spawned.write_text(
            "import os, signal, sys, time\n"
            "way = sys.argv[1]\n"
            "if way == 'session':\n    os.setsid()\n"
            "if way == 'stubborn':\n    signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
            "if way == 'orphan' and os.fork():\n    sys.exit(0)\n"
            "if way == 'zombie':\n"
            "    child = os.fork()\n"
            "    if child == 0:\n        os._exit(0)\n"
            f"    open({str(self.root)!r} + '/zombie-child.pid', 'w').write(str(child))\n"
            f"open({str(self.root)!r} + f'/{{way}}.pid', 'w').write(str(os.getpid()))\n"
            "time.sleep(30)\n"
            f"open({str(self.root / 'woke')!r}, 'a').write(way)\n")
        run = f"python3 {spawned}"
        return " ".join(f"timeout 60 {run} {way} &" if way == "timed" else f"{run} {way} &"
                        for way in ways) + " wait"

    def pid_files(self, ways):
        return (*ways, "zombie-child") if "zombie" in ways else tuple(ways)

    def started(self, ways=ESCAPES):
        """Whether every descendant `escaping_command(ways)` starts has written its pid."""
        return all((self.root / f"{way}.pid").is_file() for way in self.pid_files(ways))

    def assert_none_outlived(self, ways=ESCAPES):
        for way in self.pid_files(ways):
            pid = int((self.root / f"{way}.pid").read_text())
            with self.subTest(way=way), \
                    self.assertRaises(ProcessLookupError, msg=f"the {way} descendant outlived the gate"):
                os.kill(pid, 0)
        self.assertFalse((self.root / "woke").exists())

    def test_a_lane_that_breaks_stops_everything_the_other_lane_started(self):
        """The gate itself raising in one lane (here its load sampling) stops every
        process the other lane's command started, wherever its group or session,
        orphaned or a zombie, before it raises: nothing outlives the gate's locks."""
        escaping = self.escaping_command()
        breaks = 'python3 -c "import time; time.sleep(30)"'

        def load():
            # Fails only once every descendant is running.
            if self.started():
                raise RuntimeError("the gate broke")
            return 1.0

        began = time.monotonic()
        with mock.patch("tools.gate.CPU_SAMPLE_S", 0.05), \
                mock.patch("tools.gate.load_1m", side_effect=load), \
                self.assertRaisesRegex(RuntimeError, "the gate broke"):
            self.run_lanes([escaping, breaks], ((escaping,), (breaks,)))
        self.assertLess(time.monotonic() - began, 15)
        self.assert_none_outlived()

    def start_gate(self, command, ways, grace_s=None):
        """`python3 tools/gate.py --fast` running `command` alone, as its own process
        (`main`, with its signal handlers), once `ways`' descendants have started."""
        script = ("import sys\n"
                  "from unittest import mock\n"
                  "from tools import gate\n"
                  f"gate.FAST_COMMANDS[:] = [{command!r}]\n"
                  + (f"gate.STOP_GRACE_S = {grace_s!r}\n" if grace_s is not None else "")
                  + "with mock.patch('tools.gate.toolchain_missing', return_value=[]):\n"
                  "    sys.exit(gate.main(['--fast']))\n")
        env = dict(os.environ, PYTHONPATH=str(fixture.REPO_ROOT), PSIV_GATE_RECEIPT="build/gate/stopped")
        gate = subprocess.Popen([sys.executable, "-c", script], cwd=self.root, env=env,
                                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        self.addCleanup(gate.kill)
        deadline = time.monotonic() + 15
        while not self.started(ways):
            self.assertLess(time.monotonic(), deadline, "the command's descendants never started")
            time.sleep(0.05)
        return gate

    def test_a_signal_stops_the_gate_and_everything_below_it(self):
        """SIGTERM to the gate alone (as from a terminal or a supervisor that signals
        only its pid): it stops every descendant, then exits 128 plus the signal."""
        gate = self.start_gate(self.escaping_command(), self.ESCAPES)
        gate.send_signal(signal.SIGTERM)
        _out, err = gate.communicate(timeout=15)
        self.assertEqual(gate.returncode, 128 + signal.SIGTERM, err)
        self.assertIn("stopped by signal", err)
        self.assert_none_outlived()

    def test_more_signals_during_the_stop_wait_for_it(self):
        """A descendant that ignores SIGTERM holds the stop for its grace; a
        second SIGTERM, a SIGINT and a SIGHUP then don't cut the stop short: it
        still kills that descendant before the gate exits."""
        ways = ("stubborn",)
        gate = self.start_gate(self.escaping_command(ways), ways, grace_s=2.0)
        gate.send_signal(signal.SIGTERM)
        time.sleep(0.5)
        for again in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
            gate.send_signal(again)
            time.sleep(0.1)
        _out, err = gate.communicate(timeout=15)
        self.assertEqual(gate.returncode, 128 + signal.SIGTERM, err)
        self.assert_none_outlived(ways)

    def test_a_lane_that_fails_to_start_stops_the_lanes_that_did(self):
        """Starting the second lane fails once the first is running: the first
        starts no further command, and everything it started is stopped."""
        escaping = self.escaping_command()
        after = fixture.fake_step("after", marker=str(self.root / "after.txt"))
        other = fixture.fake_step("other")
        started = self.started

        class SecondSubmitFails(ThreadPoolExecutor):
            submitted = 0

            def submit(self, *args, **kwargs):
                SecondSubmitFails.submitted += 1
                if SecondSubmitFails.submitted == 2:
                    deadline = time.monotonic() + 15
                    while not started() and time.monotonic() < deadline:
                        time.sleep(0.05)
                    raise RuntimeError("the second lane didn't start")
                return super().submit(*args, **kwargs)

        with mock.patch("tools.gate.ThreadPoolExecutor", SecondSubmitFails), \
                mock.patch("tools.gate.CPU_SAMPLE_S", 0.05), \
                self.assertRaisesRegex(RuntimeError, "the second lane didn't start"):
            self.run_lanes([escaping, other, after], ((escaping, after), (other,)))
        self.assertTrue(started())
        self.assert_none_outlived()
        self.assertFalse((self.root / "after.txt").exists())


if __name__ == "__main__":
    unittest.main()
