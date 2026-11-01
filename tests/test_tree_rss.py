"""Cases for tools/tree_rss.py, the process-tree memory sampler (#165).

    PYTHONPATH=. python3 -m unittest tests.test_tree_rss -v

The tree case runs a real child that holds a known allocation, so it reads
/proc and is Linux only; a run holds 128 MB and takes about a second idle. The
child holds until the sampler has read it, so load slows a run without changing it.
"""

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

from tools import tree_rss
from tools.tree_rss import (cpu_mhz, load_1m, main, measure, summary_line, tree_rss_kb)

#: What the synthetic child holds, in MB: far above an idle interpreter (~10 MB).
HELD_MB = 128

#: Samples that must start while the child holds its allocation before it may let go.
HELD_SAMPLES = 3

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
        """`measure` the parent, its child holding for HELD_SAMPLES samples.

        A handshake, not a sleep: the real sampler runs, and a sample counts only
        if the child announced its allocation before the sample began. `stall_s`
        delays each sample, as a loaded host does.
        """
        real_sampler = tree_rss.tree_rss_kb
        with tempfile.TemporaryDirectory() as tmp, tempfile.TemporaryFile("w+") as log:
            allocated, release = Path(tmp) / "allocated", Path(tmp) / "release"
            held_samples = 0

            def sampler(*args, **kwargs):
                nonlocal held_samples
                time.sleep(stall_s)
                while_held = allocated.exists()
                reading = real_sampler(*args, **kwargs)
                if while_held:
                    held_samples += 1
                    if held_samples >= HELD_SAMPLES:
                        release.touch()
                return reading

            with unittest.mock.patch.object(tree_rss, "tree_rss_kb", sampler):
                result = measure([sys.executable, "-c", PARENT, str(allocated), str(release)],
                                 stdout=log, descendants=descendants)
        self.assertEqual(result["exit_code"], 0)
        self.assertGreaterEqual(held_samples, HELD_SAMPLES)
        return result

    def assert_the_peak_holds_the_allocation(self, result):
        self.assertGreaterEqual(result["peak_tree_rss_mb"], HELD_MB)
        self.assertLess(result["peak_tree_rss_mb"], HELD_MB + 100)  # two interpreters' worth

    def test_the_tree_peak_holds_the_childs_allocation(self):
        self.assert_the_peak_holds_the_allocation(self.run_parent(descendants=True))

    def test_a_sampler_slower_than_the_interval_still_sees_the_allocation(self):
        # 5x the interval per sample: the old 1.5 s hold got 3 samples, failing `> 5` (#232).
        result = self.run_parent(descendants=True, stall_s=5 * tree_rss.DEFAULT_INTERVAL_S)
        self.assert_the_peak_holds_the_allocation(result)

    def test_negative_control_a_sampler_that_misses_children_reads_low(self):
        result = self.run_parent(descendants=False)
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
