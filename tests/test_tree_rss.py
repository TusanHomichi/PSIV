"""Cases for tools/tree_rss.py, the process-tree memory sampler (#165).

    PYTHONPATH=. python3 -m unittest tests.test_tree_rss -v

The tree case runs a real child that holds a known allocation, so it reads
/proc and is Linux only; a run holds 128 MB and takes about a second idle. The
child holds its allocation until the sampler has read the tree HELD_SAMPLES times
while it held it (a handshake through two files, not a sleep), so the case does not
depend on how many samples a loaded host fits into a fixed time (#232).
"""

import functools
import io
import json
import os
import sys
import tempfile
import textwrap
import time
import unittest
import unittest.mock
from pathlib import Path

import tools.tree_rss
from tools.tree_rss import (cpu_mhz, load_1m, main, measure, summary_line, tree_rss_kb)

#: What the synthetic child holds, in MB: far above an idle interpreter (~10 MB).
HELD_MB = 128

#: Tree samples the sampler takes while the child holds its allocation.
HELD_SAMPLES = 5

#: Seconds until an unreleased child gives up and exits nonzero: a broken sampler
#: fails the case instead of hanging the gate. It bounds a failure, not a pass.
GIVE_UP_S = 300

#: The child allocates HELD_MB, says so by creating argv[1], and holds it until argv[2] exists.
CHILD = textwrap.dedent(f"""
    import pathlib, sys, time
    held = b"x" * ({HELD_MB} * 1024 * 1024)
    pathlib.Path(sys.argv[1]).touch()
    deadline = time.monotonic() + {GIVE_UP_S}
    while not pathlib.Path(sys.argv[2]).exists():
        if time.monotonic() > deadline:
            sys.exit("never released")
        time.sleep(0.01)
""")

#: A parent that starts one child and exits with its code.
PARENT = textwrap.dedent(f"""
    import subprocess, sys
    sys.exit(subprocess.run([sys.executable, "-c", {CHILD!r}, *sys.argv[1:]]).returncode)
""")


@unittest.skipUnless(Path("/proc/self/status").is_file(), "reads /proc (Linux)")
class TreeCase(unittest.TestCase):
    """The peak counts a child's memory, which the parent's own RSS does not hold."""

    def run_parent(self, descendants, stall_s=0):
        """Measure PARENT; return the result and the readings taken while the child held.

        The sampler's own tree reads drive the handshake: each read after the
        child created `held` is recorded, and the HELD_SAMPLES-th creates
        `release`, so those reads all fall inside the hold however slow the host. `stall_s`
        delays each read, as a loaded host does.
        """
        real_tree_rss_kb = tools.tree_rss.tree_rss_kb
        held_kb = []
        with tempfile.TemporaryDirectory() as tmp:
            held, release = Path(tmp) / "held", Path(tmp) / "release"

            @functools.wraps(real_tree_rss_kb)
            def sampling(*args, **kwargs):
                time.sleep(stall_s)
                # Checked before the read, so a /proc walk begun before the hold never counts.
                during_hold = held.exists() and not release.exists()
                kb = real_tree_rss_kb(*args, **kwargs)
                if during_hold:
                    held_kb.append(kb)
                    if len(held_kb) >= HELD_SAMPLES:
                        release.touch()
                return kb

            with (tempfile.TemporaryFile("w+") as log,
                  unittest.mock.patch.object(tools.tree_rss, "tree_rss_kb", sampling)):
                result = measure([sys.executable, "-c", PARENT, str(held), str(release)],
                                 stdout=log, descendants=descendants)
        return result, held_kb

    def assert_every_held_read_saw_the_allocation(self, result, held_kb):
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(len(held_kb), HELD_SAMPLES)
        for kb in held_kb:  # every read during the hold sees the allocation
            self.assertGreaterEqual(kb // 1024, HELD_MB)
        self.assertGreaterEqual(result["peak_tree_rss_mb"], HELD_MB)
        self.assertLess(result["peak_tree_rss_mb"], HELD_MB + 100)  # two interpreters' worth
        self.assertGreaterEqual(result["tree_samples"], HELD_SAMPLES)

    def test_the_tree_peak_holds_the_childs_allocation(self):
        self.assert_every_held_read_saw_the_allocation(*self.run_parent(descendants=True))

    def test_a_sampler_slower_than_the_interval_still_sees_the_allocation(self):
        # 5x the interval per read: the old 1.5 s hold got 3 samples, failing `> 5` (#232).
        stall_s = 5 * tools.tree_rss.DEFAULT_INTERVAL_S
        self.assert_every_held_read_saw_the_allocation(
            *self.run_parent(descendants=True, stall_s=stall_s))

    def test_negative_control_a_sampler_that_misses_children_reads_low(self):
        result, held_kb = self.run_parent(descendants=False)
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(len(held_kb), HELD_SAMPLES)  # it read while the child held
        self.assertLess(result["peak_tree_rss_mb"], HELD_MB // 2)

    def test_a_process_with_no_children_is_its_own_tree(self):
        here = tree_rss_kb(os.getpid(), descendants=False)
        self.assertGreater(here, 0)
        self.assertGreaterEqual(tree_rss_kb(os.getpid()), here)


class CommandLineCase(unittest.TestCase):
    """The CLI line, the JSON record and the command's exit code."""

    def test_the_exit_code_and_the_json_record(self):
        with tempfile.TemporaryDirectory() as tmp:
            record, log = Path(tmp) / "result.json", Path(tmp) / "out.log"
            stdout = io.StringIO()
            with unittest.mock.patch("sys.stdout", stdout):
                code = main(["--log", str(log), "--json", str(record), "--",
                             sys.executable, "-c", "print('hello'); raise SystemExit(3)"])
            self.assertEqual(code, 3)
            self.assertIn("hello", log.read_text())
            result = json.loads(record.read_text())
            self.assertEqual(result["exit_code"], 3)
            self.assertEqual(result["interval_s"], 0.1)
            self.assertEqual(stdout.getvalue().strip(), summary_line(result))
            self.assertTrue(stdout.getvalue().startswith("exit=3 wall="))

    def test_a_killed_command_exits_like_the_shell_says(self):
        with tempfile.TemporaryDirectory() as tmp:
            with unittest.mock.patch("sys.stdout", io.StringIO()):
                code = main(["--log", str(Path(tmp) / "out.log"), "--", sys.executable, "-c",
                             "import os, signal; os.kill(os.getpid(), signal.SIGKILL)"])
        self.assertEqual(code, 128 + 9)


class ClockCase(unittest.TestCase):
    """`cpu_mhz` reads /proc/cpuinfo's per-core clocks, `load_1m` /proc/loadavg.

    A missing reading is None, not 0.
    """

    def read(self, text):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "cpuinfo"
            if text is not None:
                path.write_text(text)
            return cpu_mhz(path)

    def test_the_mean_over_every_core(self):
        # Hand-written, in /proc/cpuinfo's layout: two cores at 2000 and 3000 MHz.
        text = ("processor\t: 0\ncpu MHz\t\t: 2000.000\n\n"
                "processor\t: 1\ncpu MHz\t\t: 3000.000\n")
        self.assertEqual(self.read(text), 2500.0)

    def test_negative_control_no_clock_lines_or_no_file_is_none(self):
        self.assertIsNone(self.read("processor\t: 0\nmodel name\t: x\n"))
        self.assertIsNone(self.read(None))

    def test_the_load_is_loadavgs_first_field(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "loadavg"
            path.write_text("10.11 7.50 4.97 3/1200 1126765\n")  # hand-written, /proc layout
            self.assertEqual(load_1m(path), 10.11)
            self.assertIsNone(load_1m(Path(tmp) / "absent"))


if __name__ == "__main__":
    unittest.main()
