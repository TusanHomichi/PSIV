"""The files in the change: `tools/repo_files.py`, and the guards that read it.

    PYTHONPATH=. python3 -m unittest tests.test_repo_files -v

`repo_files(root, patterns)` owns the question "which files belong to the
repository". A guard that answered it with a bare `git ls-files` read the index
only, so a new file nobody had staged was invisible to it, and a lane worker -
who cannot stage - could not check the file just written: lanes s1, s2 and re-D
reported false failures that way on 2026-09-25 (issue #30).

The cases are:

- `TheChangeSet` pins the helper's contract: a tracked file, an unstaged new
  file and an ignored file, a tracked file deleted from disk, a pattern, and a
  directory that is no repository at all;
- `GuardsSeeUnstagedFiles` shows, per guard, the thing the change is for: each
  guard acts on an unstaged new file in a throwaway repository, pointed at it
  through the seam it already has - the working directory for `check_docs`,
  `check(root)` for `size_guard`, `check_map(path, tracked)` for the feature
  map, whose unstaged new files are the row's owning path and its focused test;
- `NegativeControl` puts each guard back on the old index-only listing and
  shows the same fixture finding nothing, the failure a reverted guard
  produces;
- `OneOwner` is the guard against regression: the string `ls-files` lives in
  the owner and in this module, nowhere else under `tools/` or `tests/`.
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tests.test_feature_map import check_map
from tools import check_docs, size_guard
from tools.repo_files import RepoFilesError, repo_files

ROOT = Path(__file__).resolve().parents[1]
CHECK_DOCS = ROOT / "tools" / "check_docs.py"
SIZE_GUARD = ROOT / "tools" / "size_guard.py"

#: A map with the five columns the feature map defines, one row. Its owning
#: path and its focused-test module are the fixture's unstaged new files.
MAP_TABLE = (
    "| Feature | Owning paths | Player reach | Focused test | Ledger |\n"
    "| --- | --- | --- | --- | --- |\n"
    "| A new feature | `rust/psiv-core/src/new.rs` | Start the build "
    "| `python3 -m unittest tests.test_new` | [docs](README.md) |\n"
)
#: The owning path, the focused-test module and the broken link the cases use.
NEW_OWNER = "rust/psiv-core/src/new.rs"
NEW_TESTS = "tests/test_new.py"


def git_env(home):
    """The environment that keeps a case's Git answers its own."""
    return dict(os.environ, HOME=str(home), GIT_CONFIG_NOSYSTEM="1",
                GIT_CONFIG_GLOBAL=os.devnull)


def lines(*parts):
    """A file body, one newline-terminated line per part."""
    return "".join(part + "\n" for part in parts)


def text_lines(count, tag="line"):
    """Exactly `count` lines of text, one per line."""
    return "".join(f"{tag} {index}\n" for index in range(1, count + 1))


class HomeCase(unittest.TestCase):
    """A throwaway HOME, and no user or system Git config, for the whole case."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="repo-files-")
        self.addCleanup(tmp.cleanup)
        self.home = Path(tmp.name) / "home"
        self.home.mkdir()
        self.cwd = Path.cwd()
        patched = patch.dict(os.environ, git_env(self.home))
        patched.start()
        self.addCleanup(patched.stop)


class RepoCase(HomeCase):
    """A throwaway Git repository, holding what a case writes into it."""

    def setUp(self):
        super().setUp()
        self.root = self.home.parent / "repo"
        self.root.mkdir()

    def git(self, *args):
        """`git args` in the fixture repository."""
        return subprocess.run(
            ["git", "-C", str(self.root), *args], capture_output=True, text=True,
            check=True,
        ).stdout

    def write(self, name, content):
        """Write one file of the fixture, its parents created."""
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def start(self, files):
        """Write `files`, init the repository and track them; nothing after is staged."""
        for name, content in files.items():
            self.write(name, content)
        self.git("init", "-q", "-b", "main")
        self.git("add", "-A")

    def files(self, *patterns):
        """`repo_files` of the fixture, the call the guards make."""
        return repo_files(self.root, patterns)

    def enter(self):
        """Work from the fixture root: the seam `check_docs` reads the tree there."""
        os.chdir(self.root)
        self.addCleanup(os.chdir, self.cwd)

    def run_tool(self, tool, *args):
        """Run one guard from the fixture root, the way the gate runs it."""
        proc = subprocess.run(
            [sys.executable, str(tool), *args], cwd=str(self.root),
            capture_output=True, text=True,
        )
        return proc.returncode, proc.stdout + proc.stderr


class TheChangeSet(RepoCase):
    """What a `git add -A` commit would contain, and nothing else."""

    def test_a_tracked_an_unstaged_and_an_ignored_file(self):
        self.start({".gitignore": lines("local/"), "docs/tracked.md": lines("# Tracked")})
        self.write("docs/unstaged.md", lines("# Unstaged"))
        self.write("local/capture.md", lines("# A local input"))
        self.assertEqual(
            self.files(),
            [".gitignore", "docs/tracked.md", "docs/unstaged.md"],  # never the ignored one
        )

    def test_a_tracked_file_deleted_from_disk_is_dropped(self):
        self.start({"docs/kept.md": lines("# Kept"), "docs/gone.md": lines("# Gone")})
        (self.root / "docs" / "gone.md").unlink()  # still in the index, not on disk
        self.assertEqual(self.files(), ["docs/kept.md"])

    def test_patterns_filter_the_change(self):
        self.start({"docs/one.md": lines("# One"), "tools/one.py": lines("# One")})
        self.write("docs/two.md", lines("# Two"))
        self.assertEqual(self.files("*.md"), ["docs/one.md", "docs/two.md"])
        self.assertEqual(self.files("docs/one.md"), ["docs/one.md"])
        self.assertEqual(self.files("*.md", "docs/*"), ["docs/one.md", "docs/two.md"])

    def test_a_directory_that_is_no_repository_is_refused(self):
        """The helper has no empty answer to give: a tree it cannot list raises."""
        plain = self.home.parent / "plain"
        plain.mkdir()
        with self.assertRaises(RepoFilesError) as caught:
            repo_files(plain)
        self.assertIn(str(plain), str(caught.exception))


class GuardsSeeUnstagedFiles(RepoCase):
    """Each guard acts on a new file nobody has staged (issue #30)."""

    def test_check_docs_flags_a_broken_link_in_a_new_unstaged_file(self):
        self.start({"docs/kept.md": lines("# Kept")})
        self.write("docs/new.md", lines("# New", "", "[gone](missing.md)"))
        code, out = self.run_tool(CHECK_DOCS)
        self.assertEqual(code, 1, out)
        self.assertIn("docs/new.md:3: broken link target: missing.md", out)
        self.assertIn("checked 2 files", out)  # the new file was scanned

    def test_size_guard_flags_a_new_unstaged_file_over_the_limit(self):
        self.start({"src/kept.py": text_lines(5)})
        self.write("src/new.py", text_lines(1001))
        code, out = self.run_tool(SIZE_GUARD)
        self.assertEqual(code, 1, out)
        self.assertIn("src/new.py: 1001 lines (limit 1000)", out)

    def test_the_feature_map_guard_reads_the_new_unstaged_files(self):
        """A row's owning path and focused-test module need no staging to count.

        The map copy keeps the repository's `docs/` for its relative links
        (`check_map`'s `ledger_root`), so the fixture writes the same
        `docs/README.md` name and the ledger check has a file of the change.
        """
        self.start({"docs/README.md": lines("# Docs")})
        self.write("docs/FEATURE_MAP.md", MAP_TABLE)
        self.write(NEW_OWNER, lines("// a new module"))
        self.write(NEW_TESTS, lines("# a new test"))
        problems = check_map(self.root / "docs" / "FEATURE_MAP.md", self.files())
        self.assertEqual(problems, [], "\n".join(problems))


class NegativeControl(RepoCase):
    """Without the owner, the same fixture finds nothing: the reverted guard fails.

    Each case rebuilds a fixture of `GuardsSeeUnstagedFiles`, puts `repo_files`
    back to the index-only listing the guard read before this change (the one
    `git ls-files` call, here as `index_only`), and asserts what the guard then
    misses. That is the false failure the lanes reported, and it is why the
    cases above mean something.
    """

    def index_only(self, root=Path("."), patterns=()):
        """The index, with nothing unstaged in it: the listing the guards had."""
        command = ["git", "ls-files", "-z"]
        if patterns:
            command += ["--", *patterns]
        proc = subprocess.run(
            command, cwd=Path(root), capture_output=True, text=True, check=True,
        )
        return sorted(record for record in proc.stdout.split("\0") if record)

    def test_check_docs_misses_the_new_file(self):
        self.start({"docs/kept.md": lines("# Kept")})
        self.write("docs/new.md", lines("# New", "", "[gone](missing.md)"))
        self.enter()
        with patch("tools.check_docs.repo_files", self.index_only):
            problems, counts = check_docs.check_tree()
        self.assertEqual(problems, [])
        self.assertEqual(counts[0], 1, "only the tracked Markdown is read")

    def test_size_guard_misses_the_new_file(self):
        self.start({"src/kept.py": text_lines(5)})
        self.write("src/new.py", text_lines(1001))
        with patch("tools.size_guard.repo_files", self.index_only):
            problems, _, counts = size_guard.check(self.root)
        self.assertEqual(problems, [])
        self.assertEqual(counts["files"], 1, "only the tracked file is counted")

    def test_the_feature_map_guard_flags_the_new_files_as_missing(self):
        self.start({"docs/README.md": lines("# Docs")})
        self.write("docs/FEATURE_MAP.md", MAP_TABLE)
        self.write(NEW_OWNER, lines("// a new module"))
        self.write(NEW_TESTS, lines("# a new test"))
        problems = check_map(
            self.root / "docs" / "FEATURE_MAP.md", self.index_only(self.root)
        )
        missing_owner = f"owning path is not in the repository: {NEW_OWNER}"
        self.assertTrue(any(missing_owner in item for item in problems), problems)
        missing_module = f"no such Python test module: {NEW_TESTS}"
        self.assertTrue(any(missing_module in item for item in problems), problems)


class OneOwner(unittest.TestCase):
    """The file list has one reader of its own, and this module is not a second."""

    ALLOWED = frozenset({"tools/repo_files.py", "tests/test_repo_files.py"})

    def test_no_other_module_names_ls_files(self):
        offenders = []
        for pattern in ("tools/*.py", "tests/*.py"):
            for path in sorted(ROOT.glob(pattern)):
                relative = path.relative_to(ROOT).as_posix()
                if relative in self.ALLOWED:
                    continue
                if "ls-files" in path.read_text(encoding="utf-8"):
                    offenders.append(relative)
        self.assertEqual(
            offenders, [],
            "these list the files themselves instead of using tools/repo_files.py: "
            + ", ".join(offenders),
        )

    def test_the_three_guards_read_the_owner(self):
        guards = ("tools/check_docs.py", "tools/size_guard.py",
                  "tests/test_feature_map.py")
        for name in guards:
            text = (ROOT / name).read_text(encoding="utf-8")
            self.assertIn("tools.repo_files", text, name)


if __name__ == "__main__":
    unittest.main()
