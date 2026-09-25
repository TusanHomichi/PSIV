"""Unit-level ds-lane cases: the package's own functions, called directly.

    PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v

No subprocess and no repo: environment reads, brief phrasing, write sets, the
receipt block and the stall watcher.
"""
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

try:  # `unittest discover -s tests` imports this module top level
    from .ds_lane_support import DS
except ImportError:  # `python -m unittest tests.test_ds_lane_unit`
    from ds_lane_support import DS


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

    def test_worker_env_is_an_allowlist(self):
        caller = {"PATH": "/usr/bin", "HOME": "/h", "LC_ALL": "C.UTF-8", "CARGO_BUILD_JOBS": "8",
                  "NVM_DIR": "/nvm",
                  "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_x", "GH_TOKEN": "x", "SSH_AUTH_SOCK": "/s",
                  "REMI_ADMIN_TOKEN": "x", "DISPLAY": ":0", "DS_LANE_HOME": "/d",
                  "FAKE_REASONIX_SPEC": "{}"}
        env = DS.worker_env(caller)
        self.assertEqual(env, {"PATH": "/usr/bin", "HOME": "/h", "LC_ALL": "C.UTF-8",
                               "CARGO_BUILD_JOBS": "2", "NVM_DIR": "/nvm"})
        passed = DS.worker_env({**caller, "DS_LANE_WORKER_ENV_PASS": " FAKE_REASONIX_SPEC ,"})
        self.assertEqual(passed["FAKE_REASONIX_SPEC"], "{}")
        self.assertNotIn("GH_TOKEN", passed)

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

    def test_preamble_stop_rule_passes_the_phrasing_preflight(self):
        """The rule that keeps a worker from killing its own harness is positive.

        Reasonix parses the prompt for constraints, so the same rule written as
        a negation would ban every write for the session; it has to survive the
        preflight the way it is written (lane sw-S1-motavia ran a `pkill -f`
        whose pattern sat in its own brief and SIGTERM'd the harness that
        started it, 2026-09-24).
        """
        start = DS.PREAMBLE.index("Stop processes only by the PID")
        rule = DS.PREAMBLE[start:DS.PREAMBLE.index(".", start) + 1]
        # Whitespace is free (the bullet wraps); the wording is not.
        self.assertEqual(" ".join(rule.split()),
                         "Stop processes only by the PID you recorded or the job id "
                         "your tools returned.")
        self.assertEqual(DS.phrasing_violations(rule), [])
        self.assertIsNone(DS.preflight_phrasing(rule, "preamble rule"))  # no SystemExit
        self.assertEqual(DS.phrasing_violations(DS.PREAMBLE), [])

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

    def test_harness_prompts_pass_the_phrasing_preflight(self):
        """Text ds-lane writes itself must never trip Reasonix's write ban.

        The stall follow-up goes into the resumed session's prompt without a
        preflight (the harness wrote it), so it is checked here instead.
        """
        self.assertEqual(DS.phrasing_violations(DS.STALL_FOLLOWUP.format(timeout=900)), [])
        self.assertEqual(DS.phrasing_violations(DS.STALL_FOLLOWUP.format(timeout=1)), [])

    def test_wait_run_follows_the_stall_chain(self):
        """A waiter pinned to a stalled run reports the lane's final outcome.

        Each stalled run records the number of the run the watchdog started in
        its place, so the chain can be several stalls long.
        """
        with tempfile.TemporaryDirectory(prefix="ds-lane-chain-") as tmp:
            state = Path(tmp)
            for n, (nxt, code) in {1: (2, 125), 2: (3, 125), 3: (None, 0)}.items():
                run = state / f"run-{n}"
                run.mkdir()
                (run / "run.json").write_text(json.dumps({"exit_code": code,
                                                          "stall_resumed_run": nxt}))
            self.assertEqual(DS.wait_run(state / "run-1"), 0)

    def test_stall_cpu_threshold_is_read_per_call(self):
        with mock.patch.dict(os.environ, {"DS_LANE_STALL_CPU_PCT": "2.5"}):
            self.assertEqual(DS.stall_cpu_pct(), 2.5)
        with mock.patch.dict(os.environ, {"DS_LANE_STALL_CPU_PCT": "junk"}):
            self.assertEqual(DS.stall_cpu_pct(), DS.DEFAULT_STALL_CPU_PCT)
        stripped = {k: v for k, v in os.environ.items() if k != "DS_LANE_STALL_CPU_PCT"}
        with mock.patch.dict(os.environ, stripped, clear=True):
            self.assertEqual(DS.DEFAULT_STALL_CPU_PCT, 1.0)
            self.assertEqual(DS.stall_cpu_pct(), 1.0)

    def test_stall_window_needs_growth_or_a_cpu_rate(self):
        """A window of work is trajectory growth or CPU at the threshold rate.

        The rate is what separates a worker from a corpse: 1 tick over 10 s is
        0.1% of a core, which is what a process left holding a dead stream
        gains, and it is not work.
        """
        with tempfile.TemporaryDirectory(prefix="ds-lane-watcher-") as tmp:
            traj = Path(tmp) / "trajectory.jsonl"
            traj.touch()
            now, ticks = [0.0], {7: 0}
            watcher = DS.StallWatcher(pgid=7, trajectory=traj, timeout=10, poll=1, cpu_pct=1.0,
                                      clock=lambda: now[0], groups=lambda pgid: dict(ticks))
            now[0] = 5.0
            self.assertIsNone(watcher.verdict(), "a window still open has no verdict")
            ticks[7] = 1000  # 100 ticks over 10 s: a core's worth, a long silent build
            now[0] = 10.0
            self.assertIsNone(watcher.verdict())
            ticks[7] = 1001  # 1 tick over 10 s: 0.1% of a core, an idle process
            now[0] = 20.0
            self.assertEqual(watcher.verdict(), (0.1, 0))
            ticks[7] = 1011  # exactly the threshold counts as work, not as a stall
            now[0] = 30.0
            self.assertIsNone(watcher.verdict())
            traj.write_text("event\n")  # growth is work even with no CPU at all
            now[0] = 40.0
            self.assertIsNone(watcher.verdict())
            ticks.clear()  # the group is gone: its recorded ticks do not vanish
            now[0] = 50.0
            self.assertEqual(watcher.verdict(), (0.0, 0))

