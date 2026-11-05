"""Cases for the full gate's lanes in tools/gate.py: two lanes side by side.

    PYTHONPATH=. python3 -m unittest tests.test_gate_lanes -v

Hermetic, like tests/test_gate.py, whose throwaway-repo fixture this borrows:
fake commands only, never the real gate. The lanes run at once is shown by
two commands that each wait for the other's file, which only lanes side by
side can finish; the negative control runs them in one lane and they can't.
"""
import contextlib
import io
import itertools
import json
import os
import time
import unittest
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

    def test_a_lane_that_breaks_stops_the_other_lanes_command(self):
        """The gate itself raising in one lane (here its load sampling) stops the
        other lane's running command, and what that command started, before it
        raises: nothing outlives the gate's locks."""
        child = self.root / "spawned"
        child.write_text("import time\ntime.sleep(30)\n"
                         f"open({str(self.root / 'woke')!r}, 'w')\n")
        sleeper = f"python3 {child} & echo $! > {self.root / 'child.pid'}; wait"
        breaks = 'python3 -c "import time; time.sleep(30)"'
        calls = itertools.count()

        def load():
            # Fails only once both commands are running (a few samples in).
            if next(calls) > 6:
                raise RuntimeError("the gate broke")
            return 1.0

        started = time.monotonic()
        with mock.patch("tools.gate.CPU_SAMPLE_S", 0.05), \
                mock.patch("tools.gate.load_1m", side_effect=load), \
                self.assertRaisesRegex(RuntimeError, "the gate broke"):
            self.run_lanes([sleeper, breaks], ((sleeper,), (breaks,)))
        self.assertLess(time.monotonic() - started, 15)
        pid = int((self.root / "child.pid").read_text())
        with self.assertRaises(ProcessLookupError, msg="the command's child outlived the gate"):
            os.kill(pid, 0)
        self.assertFalse((self.root / "woke").exists())


if __name__ == "__main__":
    unittest.main()
