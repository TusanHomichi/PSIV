"""ds-lane's 1,000-line rule: the size check every run records and prints.

    PYTHONPATH=. python3 -m unittest discover -s tests -p 'test_ds_lane*.py' -v

The owner's rule is that a file over roughly 1,000 lines is reorganized when
touched. Lane ab-M grew `rust/psiv-core/src/battle/enemy_damage_tests.rs` to
1,504 lines and review missed it, because nothing flagged it (2026-09-24), so
the harness reports it the way it reports a write-set violation. These cases
cover the count and its binary sniff, the deletion and environment branches, the
preamble line, and a lane that grows a file end to end: one past the limit
(flagged with its counts at both ends), one already over it that the run left
alone, a binary, and the worker's own host state.
"""
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

try:  # `unittest discover -s tests` imports this module top level
    from .ds_lane_support import DS, LaneFixture
except ImportError:  # `python -m unittest tests.test_ds_lane_size`
    from ds_lane_support import DS, LaneFixture


def lines_text(count, tag="line"):
    """Exactly `count` lines of text, one per line."""
    return "".join(f"{tag} {i}\n" for i in range(1, count + 1))


class SizeUnitCase(unittest.TestCase):
    """The count, the sniff and the knob: no repo, no worker."""

    def test_count_lines_counts_what_an_editor_shows(self):
        self.assertEqual(DS.count_lines(b""), 0)
        self.assertEqual(DS.count_lines(b"one\n"), 1)
        self.assertEqual(DS.count_lines(b"one\ntwo"), 2)  # a last line without a newline
        self.assertEqual(DS.count_lines(b"one\r\ntwo\r\n"), 2)

    def test_binary_sniff_covers_the_first_8_kib(self):
        self.assertEqual(DS.BINARY_SNIFF_BYTES, 8192)
        # A NUL anywhere in the first 8 KiB marks the content binary ...
        self.assertIsNone(DS.count_lines(b"head\0tail\n"))
        self.assertIsNone(DS.count_lines(b"a" * (DS.BINARY_SNIFF_BYTES - 1) + b"\0text\n"))
        # ... and a NUL past the window does not: the sniff is bounded on purpose.
        self.assertEqual(DS.count_lines(b"a" * DS.BINARY_SNIFF_BYTES + b"\0\n"), 1)

    def test_limit_is_read_per_call_and_falls_back(self):
        for moved in ("250", "1", "1000"):
            with mock.patch.dict(os.environ, {"DS_LANE_MAX_FILE_LINES": moved}):
                self.assertEqual(DS.max_file_lines(), int(moved))
        # A value that is not a number, or not positive, flags every changed file.
        for junk in ("junk", "", "0", "-5"):
            with mock.patch.dict(os.environ, {"DS_LANE_MAX_FILE_LINES": junk}):
                self.assertEqual(DS.max_file_lines(), DS.MAX_FILE_LINES)
        stripped = {k: v for k, v in os.environ.items() if k != "DS_LANE_MAX_FILE_LINES"}
        with mock.patch.dict(os.environ, stripped, clear=True):
            self.assertEqual(DS.MAX_FILE_LINES, 1000)
            self.assertEqual(DS.max_file_lines(), 1000)

    def test_deleted_path_has_no_size_at_the_new_head(self):
        """A path the run removed is skipped: "still exists" is about the new head.

        The fake worker only writes files, so this case drives real git to
        reach the deletion branch - and shows the same call reporting the file
        while a revision where it exists is the head.
        """
        with tempfile.TemporaryDirectory(prefix="ds-lane-size-") as tmp:
            wt = Path(tmp)

            def git(*args):
                return subprocess.run(["git", "-C", str(wt), *args], check=True, text=True,
                                      stdout=subprocess.PIPE, stderr=subprocess.DEVNULL).stdout.strip()

            git("init", "-q", "-b", "main")
            git("config", "user.email", "size@example.invalid")
            git("config", "user.name", "size case")
            (wt / "tools").mkdir()
            (wt / "tools" / "gone.txt").write_text(lines_text(1500))
            git("add", "-A")
            git("commit", "-q", "-m", "base: 1500 lines")
            base = git("rev-parse", "HEAD")
            (wt / "tools" / "gone.txt").unlink()
            git("add", "-A")
            git("commit", "-q", "-m", "delete it")
            head = git("rev-parse", "HEAD")
            self.assertEqual(DS.oversize_files(wt, base, head, ["tools/gone.txt"], 1000), [])
            self.assertEqual(DS.oversize_files(wt, base, base, ["tools/gone.txt"], 1000),
                             [{"path": "tools/gone.txt", "lines": 1500, "base_lines": 1500}])

    def test_preamble_states_the_rule_positively(self):
        """Every brief carries the rule, so it must never trip the phrasing preflight."""
        self.assertIn("Keep every file you touch under 1,000 lines", DS.PREAMBLE)
        self.assertIn("reorganize it into cohesive modules", DS.PREAMBLE)
        self.assertEqual(DS.phrasing_violations(DS.PREAMBLE), [])
        DS.preflight_phrasing(DS.PREAMBLE, "brief")
        DS.preflight_phrasing(DS.PREAMBLE + "# Brief\n\nTouch only tools/keep.txt.\n", "brief")


class SizeCase(LaneFixture):
    """The check end to end: real CLI, detached supervisor, fake worker."""

    def base_file(self, rel, text):
        """Commit `rel` into the source repo, so it sits at the lane's base."""
        path = self.repo / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        self.git("add", "-A")
        self.git("commit", "-q", "-m", f"base: {rel}")
        self.base_sha = self.git("rev-parse", "HEAD")

    def oversize_warnings(self, lane_id, run=1):
        """A run's size-rule warning lines, in order (the summary's diffstat names files too)."""
        return [l for l in self.summary(lane_id, run).splitlines()
                if l.startswith("WARNING: over ")]

    def test_grown_file_is_flagged_with_counts(self):
        """The ab-M shape: a run that leaves a changed file over the limit."""
        self.base_file("tools/grown.txt", lines_text(900))
        spec = {"files": {"tools/grown.txt": lines_text(1200),
                          "tools/fresh.txt": lines_text(1500),
                          "tools/small.txt": "one\ntwo\n"}}
        self.start("Touch the owned files.\n", "t1", spec)
        run = self.run_json("t1")
        self.assertEqual(run["exit_code"], 0)
        self.assertEqual(run["max_file_lines"], DS.MAX_FILE_LINES)
        # 1200 lines was 900 at the base; 1500 lines is new; three lines is fine.
        self.assertEqual(run["oversize_files"],
                         [{"path": "tools/fresh.txt", "lines": 1500, "base_lines": None},
                          {"path": "tools/grown.txt", "lines": 1200, "base_lines": 900}])
        self.assertEqual(self.oversize_warnings("t1"),
                         ["WARNING: over 1000 lines: tools/fresh.txt (1500, was new)",
                          "WARNING: over 1000 lines: tools/grown.txt (1200, was 900)"])

    def test_file_over_the_limit_at_the_base_is_flagged_only_when_changed(self):
        """A run reports what it touched: an untouched big file is not its doing."""
        self.base_file("tools/already_big.txt", lines_text(1500))
        self.start("Touch only tools/small.txt.\n", "t1", {"files": {"tools/small.txt": "one\n"}})
        run = self.run_json("t1")
        self.assertEqual(run["oversize_files"], [])
        self.assertEqual(self.oversize_warnings("t1"), [])
        # The same file, changed by the next run, is reported with both counts.
        self.resume("t1", "# More work\n\nTouch only tools/already_big.txt.\n",
                    spec={"files": {"tools/already_big.txt": lines_text(1501)}}, check=0)
        run2 = self.run_json("t1", run=2)
        self.assertEqual(run2["oversize_files"], [{"path": "tools/already_big.txt",
                                                  "lines": 1501, "base_lines": 1500}])
        self.assertEqual(self.oversize_warnings("t1", run=2),
                         ["WARNING: over 1000 lines: tools/already_big.txt (1501, was 1500)"])

    def test_binary_file_is_skipped_while_its_text_twin_is_flagged(self):
        """A blob has no line count: the NUL in the first 8 KiB skips it."""
        body = lines_text(1400)
        spec = {"files": {"tools/blob.bin": "\u0000" + body, "tools/same.txt": body}}
        self.start("Touch the owned files.\n", "t1", spec)
        run = self.run_json("t1")
        self.assertEqual(run["oversize_files"],
                         [{"path": "tools/same.txt", "lines": 1400, "base_lines": None}])
        self.assertEqual(self.oversize_warnings("t1"),
                         ["WARNING: over 1000 lines: tools/same.txt (1400, was new)"])
        # The control: the flag is the NUL byte, not the content's length.
        self.assertEqual(DS.count_lines(body.encode()), 1400)
        self.assertIsNone(DS.count_lines(b"\0" + body.encode()))

    def test_worker_host_state_is_never_counted(self):
        """Host state never reaches a commit, so it is not lane output to measure."""
        host_state = DS.HOST_STATE_PATHS[0]
        spec = {"files": {"tools/small.txt": "ok\n",
                          f"{host_state}/tasks/x/events.jsonl": lines_text(1400)}}
        self.start("Touch only tools/small.txt.\n", "t1", spec)
        run = self.run_json("t1")
        self.assertEqual(run["oversize_files"], [])
        self.assertEqual(self.oversize_warnings("t1"), [])
        changed = self.git("diff", "--name-only", f"{self.base_sha}..{run['head_sha']}",
                           cwd=self.lane_wt("t1"))
        self.assertEqual(changed.splitlines(), ["tools/small.txt"])

    def test_limit_moves_with_the_environment(self):
        """DS_LANE_MAX_FILE_LINES reaches the detached supervisor's check."""
        spec = {"files": {"tools/medium.txt": lines_text(150)}}
        self.start("Touch only tools/medium.txt.\n", "t1", spec,
                   env={"DS_LANE_MAX_FILE_LINES": "100"})
        run = self.run_json("t1")
        self.assertEqual(run["max_file_lines"], 100)
        self.assertEqual(run["oversize_files"],
                         [{"path": "tools/medium.txt", "lines": 150, "base_lines": None}])
        self.assertEqual(self.oversize_warnings("t1"),
                         ["WARNING: over 100 lines: tools/medium.txt (150, was new)"])
        # The same write under the default limit is not reported.
        self.start("Touch only tools/medium.txt.\n", "t2", spec)
        run2 = self.run_json("t2")
        self.assertEqual(run2["max_file_lines"], DS.MAX_FILE_LINES)
        self.assertEqual(run2["oversize_files"], [])
        self.assertEqual(self.oversize_warnings("t2"), [])
