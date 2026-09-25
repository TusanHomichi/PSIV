"""The size guard: `tools/size_guard.py`, the 1,000-line rule as a gate check.

    PYTHONPATH=. python3 -m unittest tests.test_size_guard -v

docs/DEVELOPMENT.md says source files stay under 1,000 lines. Until this guard
existed only the lane harness flagged the limit, and only on its own commits,
so 11 tracked files sat over it on `main` (issue #11). The guard scans every
file `git ls-files` lists that is text, and `tools/size_baseline.txt` records
the files that were already over the limit when it landed: the ratchet that
makes the rule a gate check without reorganizing 11 files inside the change
that adds the check, with #12 tracking the cleanup.

Every case runs the tool the gate runs. `RealTree` runs it against this
repository, and every other class runs it in a throwaway git repository built
for the case. Each way the ratchet can go stale is a negative control naming
its path: a new over-limit file, a baselined file that grew, a baselined file
now at or under the limit, a baselined path that is gone, and a baselined path
the rule skips. The positive cases pin what the rule deliberately passes over:
an exempt `*.json`, a binary file (and a NUL past the 8 KiB sniff window, which
is text), `**/replay_fixtures/**` at any depth, a file at exactly the limit, an
untracked file, and a baselined file that shrank while staying over the limit.
`WriteBaseline` covers the last entry point: `--write-baseline` seeds a baseline
no tree had yet and may lower a count, and it refuses to record growth, so the
ratchet cannot be reset by running it. `Documentation` pins
docs/DEVELOPMENT.md's list of globs to the guard's `EXEMPT`, so the doc and the
rule cannot drift apart.
"""
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tools.size_guard import (BINARY_SNIFF_BYTES, EXEMPT, MAX_LINES, over_limit,
                              read_baseline, scan)

ROOT = Path(__file__).resolve().parents[1]
GUARD = ROOT / "tools" / "size_guard.py"
DEVELOPMENT = ROOT / "docs" / "DEVELOPMENT.md"


def git_env(home):
    """The environment that keeps a case's Git answers its own."""
    return dict(os.environ, HOME=str(home), GIT_CONFIG_NOSYSTEM="1",
                GIT_CONFIG_GLOBAL=os.devnull)


def text_lines(count, tag="line"):
    """Exactly `count` lines of text, one per line."""
    return "".join(f"{tag} {index}\n" for index in range(1, count + 1))


def run_guard(cwd, home, *args):
    """Run the guard from `cwd` with a throwaway Git environment."""
    proc = subprocess.run(
        [sys.executable, str(GUARD), *args], cwd=str(cwd), capture_output=True,
        text=True, env=git_env(home),
    )
    return proc.returncode, proc.stdout + proc.stderr


def tracked_in(root):
    """The tracked paths of `root`, read the way the guard reads them."""
    proc = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z"], capture_output=True, text=True,
        check=True,
    )
    return [record for record in proc.stdout.split("\0") if record]


def size_section(text):
    """docs/DEVELOPMENT.md's `### File size` section, up to the next heading."""
    lines = text.splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.strip() == "### File size"),
        None,
    )
    if start is None:
        raise AssertionError("docs/DEVELOPMENT.md has no `### File size` heading")
    section = []
    for line in lines[start + 1:]:
        if line.startswith("#"):
            break
        section.append(line)
    return "\n".join(section)


class HomeCase(unittest.TestCase):
    """A throwaway HOME, so a case's Git answers are its own."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="size-guard-")
        self.addCleanup(tmp.cleanup)
        self.home = Path(tmp.name) / "home"
        self.home.mkdir()


class RepoCase(HomeCase):
    """A throwaway git repository holding only the files a case names."""

    def setUp(self):
        super().setUp()
        self.root = self.home.parent / "repo"
        self.root.mkdir()

    def git(self, *args):
        return subprocess.run(
            ["git", "-C", str(self.root), *args], capture_output=True, text=True,
            check=True, env=git_env(self.home),
        ).stdout

    def write(self, name, content):
        """One file of the fixture: `str` or `bytes`, parents created."""
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(content, bytes):
            path.write_bytes(content)
        else:
            path.write_text(content, encoding="utf-8")

    def repository(self, files, baseline=None):
        """Write `files` (and an optional baseline), then track everything."""
        for name, content in files.items():
            self.write(name, content)
        (self.root / "tools").mkdir(exist_ok=True)  # where the baseline lives
        if baseline is not None:
            self.write("tools/size_baseline.txt", baseline)
        self.git("init", "-q", "-b", "main")
        self.git("add", "-A")  # staged is enough: the guard reads `git ls-files`

    def guard(self, *args):
        return run_guard(self.root, self.home, *args)


class RealTree(HomeCase):
    """This repository, checked by the gate's own command."""

    def test_repository_tree_passes_against_the_committed_baseline(self):
        code, out = run_guard(ROOT, self.home)
        self.assertEqual(code, 0, out)
        self.assertIn("; 0 problem(s)", out)
        scanned = re.search(
            r"scanned (\d+) files .*?(\d+) over the limit, (\d+) baselined", out
        )
        self.assertIsNotNone(scanned, out)
        self.assertGreater(int(scanned.group(1)), 500)  # a vacuous pass scans nothing
        self.assertGreater(int(scanned.group(2)), 0)  # issue #11's files are there
        self.assertEqual(scanned.group(2), scanned.group(3))  # and all of them baselined

    def test_the_baseline_is_exactly_the_trees_over_limit_files(self):
        """The ratchet's file and a fresh scan agree: it is neither stale nor empty."""
        entries, exists, problems = read_baseline(ROOT)
        self.assertTrue(exists)
        self.assertEqual(problems, [])
        sizes, skipped = scan(ROOT, tracked_in(ROOT))
        self.assertEqual(sorted(entries.items()), over_limit(sizes))
        self.assertGreater(len(entries), 0)
        self.assertGreater(skipped["exempt"], 0)  # generated data is really passed over


class BaselineRatchet(RepoCase):
    """Each way the baseline can go stale is a failure naming its path."""

    def test_new_over_limit_file_fails(self):
        self.repository({"src/big.py": text_lines(1001)})
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn("src/big.py: 1001 lines (limit 1000)", out)
        self.assertIn("; 1 problem(s)", out)

    def test_baselined_file_that_grew_fails(self):
        self.repository({"src/big.py": text_lines(1002)}, baseline="1001 src/big.py\n")
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn(
            "src/big.py: 1002 lines, over its baseline of 1001 (limit 1000)", out
        )

    def test_baselined_file_now_under_the_limit_fails(self):
        self.repository({"src/big.py": text_lines(999)}, baseline="1001 src/big.py\n")
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn(
            "src/big.py: 999 lines, at or under the limit; remove it from the baseline",
            out,
        )

    def test_baselined_path_that_is_gone_fails(self):
        self.repository({"src/small.py": text_lines(10)}, baseline="1005 src/gone.py\n")
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn(
            "src/gone.py: in the baseline but not tracked; remove it from the baseline",
            out,
        )

    def test_baselined_path_the_rule_skips_fails(self):
        """A data file cannot sit in the baseline: the rule never counts it."""
        self.repository(
            {"data/rows.json": text_lines(2000, tag="row")},
            baseline="2000 data/rows.json\n",
        )
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn(
            "data/rows.json: recorded as 2000 lines but the rule counts no lines "
            "for it (exempt, binary or missing); remove it from the baseline",
            out,
        )

    def test_shrunk_baselined_file_passes_and_suggests_a_lower_count(self):
        self.repository({"src/big.py": text_lines(1050)}, baseline="1100 src/big.py\n")
        code, out = self.guard()
        self.assertEqual(code, 0, out)
        self.assertIn("note: src/big.py: 1050 lines, down from 1100; lower its count", out)
        self.assertIn("; 0 problem(s)", out)

    def test_a_file_at_exactly_the_limit_passes(self):
        """The rule reads "under 1,000": the limit itself is not over it."""
        self.repository({"src/exact.py": text_lines(MAX_LINES)})
        code, out = self.guard()
        self.assertEqual(code, 0, out)
        self.assertIn("0 over the limit", out)

    def test_malformed_baseline_line_fails(self):
        self.repository({"src/small.py": text_lines(10)}, baseline="not a baseline line\n")
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn(
            "tools/size_baseline.txt:1: malformed baseline line: not a baseline line",
            out,
        )


class PassedOver(RepoCase):
    """What the rule deliberately does not count."""

    def test_exempt_json_over_the_limit_passes(self):
        self.repository({"data/rows.json": text_lines(2000, tag="row")})
        code, out = self.guard()
        self.assertEqual(code, 0, out)
        self.assertIn("1 exempt", out)

    def test_a_binary_file_is_skipped_and_a_late_nul_is_not(self):
        """The sniff is 8 KiB: a NUL inside it is a blob, a NUL past it is text."""
        self.repository({
            "data/blob.dat": b"\0".join(b"chunk" for _ in range(2000)),
            # The x run is one line (the NUL rides on it), then 1500 lines of text.
            "data/late-nul.txt": b"x" * BINARY_SNIFF_BYTES + b"\0\n"
            + text_lines(1500).encode(),
        })
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn("data/late-nul.txt: 1501 lines (limit 1000)", out)
        self.assertNotIn("data/blob.dat", out)
        self.assertIn("1 binary or missing", out)

    def test_replay_fixtures_globs_exempt_paths_at_any_depth(self):
        """`**/replay_fixtures/**` reaches nested dirs and matches none of them."""
        fixtures = (
            "tests/replay_fixtures/tape.txt",
            "tests/replay_fixtures/run-1/deep/tape.txt",
            "replay_fixtures/tape.txt",  # `**/` also matches no directory at all
        )
        patterns = dict(zip(fixtures, [text_lines(1500)] * len(fixtures)))
        patterns["tests/tape.txt"] = text_lines(1500)  # beside them, and not exempt
        self.repository(patterns)
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn("tests/tape.txt: 1500 lines (limit 1000)", out)
        for path in fixtures:
            self.assertNotIn(f"{path}: ", out)
        self.assertIn("3 exempt", out)

    def test_an_untracked_file_is_not_scanned(self):
        """The rule is about what the repository holds, not what a tree holds."""
        self.repository({"src/keep.py": text_lines(5)})
        self.write("src/untracked.py", text_lines(2000))
        code, out = self.guard()
        self.assertEqual(code, 0, out)
        self.assertNotIn("src/untracked.py", out)


class WriteBaseline(RepoCase):
    """`--write-baseline` seeds and lowers; it never records growth."""

    def test_seeding_writes_the_tree_and_the_check_then_passes(self):
        self.repository({"src/big.py": text_lines(1500)})
        code, out = self.guard("--write-baseline")
        self.assertEqual(code, 0, out)
        self.assertIn("wrote tools/size_baseline.txt: 1 entries", out)
        self.assertEqual(
            (self.root / "tools" / "size_baseline.txt").read_text(encoding="utf-8"),
            "1500 src/big.py\n",
        )
        code, out = self.guard()
        self.assertEqual(code, 0, out)

    def test_growth_is_refused_and_the_baseline_is_left_alone(self):
        self.repository({"src/big.py": text_lines(1600)}, baseline="1500 src/big.py\n")
        code, out = self.guard("--write-baseline")
        self.assertEqual(code, 1, out)
        self.assertIn("refusing to write tools/size_baseline.txt", out)
        self.assertIn("src/big.py: 1600 lines, up from 1500", out)
        self.assertEqual(
            (self.root / "tools" / "size_baseline.txt").read_text(encoding="utf-8"),
            "1500 src/big.py\n",
        )
        code, out = self.guard()  # the ratchet still reports the growth
        self.assertEqual(code, 1, out)
        self.assertIn("src/big.py: 1600 lines, over its baseline of 1500", out)

    def test_a_lower_count_and_a_dropped_path_are_written(self):
        self.repository(
            {"src/big.py": text_lines(1050)},
            baseline="1100 src/big.py\n1005 src/gone.py\n",
        )
        code, out = self.guard("--write-baseline")
        self.assertEqual(code, 0, out)
        self.assertEqual(
            (self.root / "tools" / "size_baseline.txt").read_text(encoding="utf-8"),
            "1050 src/big.py\n",
        )


class Invocation(RepoCase):
    """The guard refuses to guess a tree or a baseline path."""

    def test_not_a_repository_refuses(self):
        plain = self.home.parent / "plain"
        plain.mkdir()
        code, out = run_guard(plain, self.home)
        self.assertEqual(code, 2, out)
        self.assertIn("size_guard: not a git repository", out)

    def test_a_subdirectory_is_refused(self):
        self.repository({"src/small.py": text_lines(5)})
        code, out = run_guard(self.root / "src", self.home)
        self.assertEqual(code, 2, out)
        self.assertIn("size_guard: run from the repository root", out)


class Documentation(HomeCase):
    """The rule's owner document names the guard and its list."""

    def test_documented_exempt_globs_are_the_guards(self):
        section = size_section(DEVELOPMENT.read_text(encoding="utf-8"))
        self.assertEqual(set(re.findall(r"`([^`]*\*[^`]*)`", section)), set(EXEMPT))
        self.assertIn("tools/size_guard.py", section)
        self.assertIn("tools/size_baseline.txt", section)
        self.assertIn("tests/test_size_guard.py", section)  # the suite the gate runs
        self.assertIn("issues/12", section)  # the cleanup that empties the ratchet
        self.assertNotIn("only tool that flags", section)


if __name__ == "__main__":
    unittest.main()
