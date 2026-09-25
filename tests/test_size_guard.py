"""The size guard: `tools/size_guard.py`, the 1,000-line rule as a gate check.

    PYTHONPATH=. python3 -m unittest tests.test_size_guard -v

docs/DEVELOPMENT.md says source files stay under 1,000 lines. Until this guard
existed only the lane harness flagged the limit, and only on its own commits,
so tracked files sat over it on `main` (issue #11). The guard scans every file
of the change that is text - the paths `tools/repo_files.py` lists, what a
commit made with `git add -A` would contain, so a new unstaged file is counted
too - and fails a non-exempt file over the limit.

The rule has one form and no exceptions list: the files a ratchet once
grandfathered have all been reorganized, so nothing records a file the rule
should pass, and no option of the guard could seed one.

Every case runs the tool the gate runs. `RealTree` runs it against this
repository, and every other class runs it in a throwaway git repository built
for the case. The failure cases are negative controls naming their path: a
1,001-line file fails while 1,000 lines passes - the limit itself is not over
it - and a second offender is named too. The positive cases pin what the rule
deliberately passes over: an exempt `*.json`, `**/replay_fixtures/**` at any
depth, a binary file (and a NUL past the 8 KiB sniff window, which is text),
and an ignored file. `Invocation` covers the refusals - no repository, a
subdirectory, an argument the guard does not take - and `Documentation` pins
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

from tools.size_guard import BINARY_SNIFF_BYTES, EXEMPT, MAX_LINES, over_limit, scan

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

    def repository(self, files):
        """Write `files`, then track everything: the change the guard reads."""
        for name, content in files.items():
            self.write(name, content)
        self.git("init", "-q", "-b", "main")
        self.git("add", "-A")

    def guard(self, *args):
        return run_guard(self.root, self.home, *args)


class RealTree(HomeCase):
    """This repository, checked by the gate's own command."""

    def test_repository_tree_passes_with_nothing_over_the_limit(self):
        code, out = run_guard(ROOT, self.home)
        self.assertEqual(code, 0, out)
        scanned = re.search(
            r"scanned (\d+) files \((\d+) exempt, (\d+) binary or missing\), "
            r"(\d+) over the limit; 0 problem\(s\)",
            out,
        )
        self.assertIsNotNone(scanned, out)
        self.assertGreater(int(scanned.group(1)), 500)  # a vacuous pass scans nothing
        self.assertEqual(int(scanned.group(4)), 0)  # no exempt entry hides an offender
        self.assertGreater(int(scanned.group(2)), 0)  # generated data is really passed over

    def test_the_scan_finds_no_file_over_the_limit(self):
        """The module's own answer for the tree, beside the command's."""
        sizes, skipped = scan(ROOT)
        self.assertEqual(over_limit(sizes), [])
        self.assertGreater(len(sizes), 500)
        self.assertGreater(skipped["exempt"], 0)


class OverLimit(RepoCase):
    """A non-exempt file over the limit fails, and the report names it."""

    def test_a_tracked_file_one_line_over_the_limit_fails(self):
        self.repository({"src/big.py": text_lines(1001)})
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn("src/big.py: 1001 lines (limit 1000)", out)
        self.assertIn("; 1 problem(s)", out)

    def test_a_new_unstaged_file_over_the_limit_fails(self):
        """What a `git add -A` commit would hold, not just what is tracked."""
        self.repository({"src/kept.py": text_lines(5)})
        self.write("src/new.py", text_lines(1200))
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn("src/new.py: 1200 lines (limit 1000)", out)

    def test_every_offender_is_named(self):
        self.repository(
            {"src/big.py": text_lines(1001), "src/bigger.py": text_lines(2000)}
        )
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn("src/big.py: 1001 lines (limit 1000)", out)
        self.assertIn("src/bigger.py: 2000 lines (limit 1000)", out)
        self.assertIn("2 over the limit; 2 problem(s)", out)

    def test_a_last_line_without_a_newline_still_counts(self):
        """The count is the editor's: a final line with no newline is a line."""
        self.repository({"src/big.py": text_lines(1001).rstrip("\n")})
        code, out = self.guard()
        self.assertEqual(code, 1, out)
        self.assertIn("src/big.py: 1001 lines (limit 1000)", out)

    def test_a_file_at_exactly_the_limit_passes(self):
        """The rule reads "under 1,000": the limit itself is not over it."""
        self.repository({"src/exact.py": text_lines(MAX_LINES)})
        code, out = self.guard()
        self.assertEqual(code, 0, out)
        self.assertIn("0 over the limit", out)


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

    def test_an_ignored_file_is_not_scanned(self):
        """An ignored file is a local input: not what a commit would hold."""
        self.repository({".gitignore": "local/\n", "src/keep.py": text_lines(5)})
        self.write("local/rom.bin", text_lines(2000))
        code, out = self.guard()
        self.assertEqual(code, 0, out)
        self.assertNotIn("local/rom.bin", out)


class Invocation(RepoCase):
    """The guard refuses to guess a tree, and takes no argument at all."""

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

    def test_an_argument_the_guard_does_not_take_is_refused(self):
        """There is nothing to configure: no option can exempt a file."""
        self.repository({"src/small.py": text_lines(5)})
        code, out = self.guard("--force")
        self.assertEqual(code, 2, out)
        self.assertIn("unrecognized arguments: --force", out)


class Documentation(HomeCase):
    """The rule's owner document names the guard, its list and its one form."""

    def test_documented_exempt_globs_are_the_guards(self):
        section = size_section(DEVELOPMENT.read_text(encoding="utf-8"))
        self.assertEqual(set(re.findall(r"`([^`]*\*[^`]*)`", section)), set(EXEMPT))
        self.assertIn("tools/size_guard.py", section)
        self.assertIn("tests/test_size_guard.py", section)  # the suite the gate runs
        self.assertIn("no exceptions list", section)  # the rule has one form
        self.assertNotIn("issues/", section)  # the rule, not a tracked cleanup
        self.assertNotIn("only tool that flags", section)


if __name__ == "__main__":
    unittest.main()
