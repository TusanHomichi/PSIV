"""The documentation guard: `tools/check_docs.py`, the gate's first command.

    PYTHONPATH=. python3 -m unittest tests.test_check_docs -v

Links, heading anchors and the paths inside shell command blocks drift as the
tree moves; the guard is what turns that drift into a red gate instead of a
broken link a reader finds. The cases here cover the three rule sets and their
skip rules, all of it end to end: every case below runs the tool the gate runs,
in a throwaway git repository built for it, and reads the `path:line: message`
lines it prints.

The real tree passes case runs the same tool against this repository, so the
guard is checked against the documents it guards. The rest are negative
controls (a missing link target, a missing anchor, a missing command path and
an unmatched glob, each rejected by name and line) and positive cases that pin
what the checker deliberately lets through: a duplicate heading's `-1` anchor,
a link written inside a fenced block or inline code, an external URL, and
git-ignored local inputs, as a link target and as a command path.

Every case builds its own repository: the checker answers from
`tools/repo_files.py` and `git check-ignore`, so it needs a real repository and
a clean Git environment to be tested hermetically.
"""
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "tools" / "check_docs.py"

SUMMARY_RE = re.compile(
    r"checked (?P<files>\d+) files, (?P<links>\d+) links, "
    r"(?P<anchors>\d+) anchors, (?P<paths>\d+) command paths; "
    r"(?P<problems>\d+) problem\(s\)"
)


def git_env(home):
    """The environment that keeps a case's Git answers its own."""
    return dict(os.environ, HOME=str(home), GIT_CONFIG_NOSYSTEM="1",
                GIT_CONFIG_GLOBAL=os.devnull)


def lines(*parts):
    """A file body, one newline-terminated line per part."""
    return "".join(part + "\n" for part in parts)


def run_checker(cwd, home):
    """Run the checker from `cwd` with a throwaway Git environment."""
    proc = subprocess.run(
        [sys.executable, str(CHECKER)], cwd=str(cwd), capture_output=True,
        text=True, env=git_env(home),
    )
    return proc.returncode, proc.stdout + proc.stderr


class RepoCase(unittest.TestCase):
    """A throwaway git repository holding a small Markdown tree."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="check-docs-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.home = self.root / "home"
        self.home.mkdir()

    def git(self, *args):
        proc = subprocess.run(
            ["git", "-C", str(self.root), *args], capture_output=True,
            text=True, check=True, env=git_env(self.home),
        )
        return proc.stdout

    def check(self, files):
        """Write `files`, track them, and return (exit status, output)."""
        for name, text in files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        self.git("init", "-q", "-b", "main")
        self.git("add", "-A")  # the tracked files are the change the checker reads
        return run_checker(self.root, self.home)


class RealTree(RepoCase):
    """The repository's own documents, checked by the gate's own command."""

    def test_repository_tree_passes(self):
        code, out = run_checker(ROOT, self.home)
        self.assertEqual(code, 0, out)
        summary = SUMMARY_RE.search(out)
        self.assertIsNotNone(summary, out)
        # A vacuous pass would report nothing inspected: the scan sees the tree.
        self.assertGreater(int(summary["files"]), 50)
        self.assertGreater(int(summary["links"]), 100)
        self.assertGreater(int(summary["anchors"]), 10)
        self.assertGreater(int(summary["paths"]), 10)
        self.assertEqual(summary["problems"], "0")

    def test_missing_link_in_a_copy_is_rejected(self):
        """A broken link in a copy of a real document is named with its line."""
        original = (ROOT / "docs" / "README.md").read_text(encoding="utf-8")
        broken = original.replace("(DEVELOPMENT.md)", "(DEVELOPMENT-moved.md)")
        self.assertNotEqual(broken, original)
        code, out = self.check({
            "docs/README.md": broken,
            "docs/DEVELOPMENT.md": (ROOT / "docs" / "DEVELOPMENT.md").read_text(
                encoding="utf-8"
            ),
        })
        self.assertEqual(code, 1, out)
        line = next(
            number for number, text in enumerate(broken.splitlines(), 1)
            if "DEVELOPMENT-moved.md" in text
        )
        self.assertIn(
            f"docs/README.md:{line}: broken link target: DEVELOPMENT-moved.md", out
        )


class NegativeControls(RepoCase):
    """Each rule rejects its defect, naming the file and the line."""

    def test_missing_link_target(self):
        code, out = self.check({
            "docs/report.md": lines("", "# Report", "", "[gone](missing.md)"),
            "docs/other.md": lines("# Other"),
        })
        self.assertEqual(code, 1, out)
        self.assertIn("docs/report.md:4: broken link target: missing.md", out)
        self.assertIn("; 1 problem(s)", out)

    def test_missing_anchor_in_another_file(self):
        code, out = self.check({
            "docs/report.md": lines("", "# Report", "", "[there](other.md#no-such-heading)"),
            "docs/other.md": lines("# Other", "", "## Present"),
        })
        self.assertEqual(code, 1, out)
        self.assertIn("docs/report.md:4: broken anchor: docs/other.md#no-such-heading", out)

    def test_missing_anchor_in_the_same_file(self):
        code, out = self.check({
            "docs/report.md": lines("", "# Report", "", "[up](#no-such-heading)"),
        })
        self.assertEqual(code, 1, out)
        self.assertIn("docs/report.md:4: broken anchor: #no-such-heading", out)

    def test_missing_command_path(self):
        code, out = self.check({
            "docs/report.md": lines(
                "# Report", "", "```sh", "python3 tools/missing.py", "```",
            ),
            "tools/present.py": lines("# a tool"),
        })
        self.assertEqual(code, 1, out)
        self.assertIn("docs/report.md:4: command path not found: tools/missing.py", out)

    def test_unmatched_glob(self):
        code, out = self.check({
            "docs/report.md": lines("# Report", "", "```bash", "ls tools/absent_*.py", "```"),
            "tools/present.py": lines("# a tool"),
        })
        self.assertEqual(code, 1, out)
        self.assertIn(
            "docs/report.md:4: command glob matches no tracked file: tools/absent_*.py", out
        )

    def test_a_parent_relative_link_is_reported_as_written(self):
        """A link that climbs out of its document's directory still names a file."""
        code, out = self.check({
            "docs/report.md": lines("# Report", "", "[up](../notes/gone.md)"),
        })
        self.assertEqual(code, 1, out)
        self.assertIn("docs/report.md:3: broken link target: ../notes/gone.md", out)


class AcceptedCases(RepoCase):
    """What the checker deliberately reads past, and what it must still catch."""

    def test_duplicate_heading_gets_a_numbered_anchor(self):
        code, out = self.check({
            "docs/report.md": lines(
                "# Report", "", "[second](notes.md#notes-1)", "[first](notes.md#notes)",
            ),
            "docs/notes.md": lines("# Notes", "", "## Notes", "", "## Notes"),
        })
        self.assertEqual(code, 0, out)
        summary = SUMMARY_RE.search(out)
        self.assertEqual(summary["anchors"], "2", out)  # both were resolved
        self.assertEqual(summary["problems"], "0", out)

    def test_link_inside_a_fence_or_inline_code_is_not_a_link(self):
        code, out = self.check({
            "docs/report.md": lines(
                "# Report", "",
                "Fenced, and so is the one below:",
                "",
                "```text",
                "[gone](missing.md)",
                "```",
                "",
                "Inline: `[gone](missing.md)` stays text.",
            ),
        })
        self.assertEqual(code, 0, out)
        self.assertEqual(SUMMARY_RE.search(out)["links"], "0", out)

    def test_external_targets_are_skipped(self):
        code, out = self.check({
            "docs/report.md": lines(
                "# Report", "",
                "[site](https://example.invalid/page#anchor)",
                "[mail](mailto:nobody@example.invalid)",
                "[host relative](//example.invalid/page)",
            ),
        })
        self.assertEqual(code, 0, out)
        self.assertEqual(SUMMARY_RE.search(out)["links"], "0", out)

    def test_ignored_paths_are_local_inputs(self):
        code, out = self.check({
            ".gitignore": lines("generated/", "docs/local/"),
            "docs/report.md": lines(
                "# Report", "",
                "[local capture](local/frame_1.png)",
                "",
                "```sh",
                "oracle_dump docs/local/frame_1.png",
                "oracle_dump docs/local",  # the directory-only pattern, no slash
                "oracle_dump generated/rom.bin",
                "```",
            ),
            "oracle_dump": lines("# a local tool"),
        })
        self.assertEqual(code, 0, out)
        self.assertEqual(SUMMARY_RE.search(out)["problems"], "0", out)

    def test_a_tracked_path_beside_an_ignored_one_still_fails(self):
        """The skip is for ignored paths only, not for a missing sibling."""
        code, out = self.check({
            ".gitignore": lines("docs/local/"),
            "docs/report.md": lines(
                "# Report", "", "```sh", "ls docs/local docs/absent", "```",
            ),
        })
        self.assertEqual(code, 1, out)
        self.assertIn("docs/report.md:4: command path not found: docs/absent", out)
        self.assertNotIn("docs/local", out)

    def test_matched_glob_and_existing_paths_pass(self):
        code, out = self.check({
            "docs/report.md": lines(
                "# Report", "",
                "[index](../README.md)",
                "",
                "```console",
                "./tools/run.sh docs/*.md tools/*.py",
                '"$PWD/tools/run.sh"',
                "```",
            ),
            "README.md": lines("# Root"),
            "docs/notes.md": lines("# Notes"),
            "tools/run.sh": lines("# a tool"),
            "tools/helper.py": lines("# another"),
        })
        self.assertEqual(code, 0, out)
        # ./tools/run.sh, docs/*.md, tools/*.py and "$PWD/tools/run.sh"
        self.assertEqual(SUMMARY_RE.search(out)["paths"], "4", out)


class SummaryAndExit(RepoCase):
    """The summary line, the exit status and the root guard."""

    def test_summary_counts_what_was_inspected(self):
        code, out = self.check({
            "docs/report.md": lines(
                "# Report", "",
                "[page](https://example.invalid)", "[note](notes.md#notes)",
                "",
                "```bash", "ls tests/", "```",
            ),
            "docs/notes.md": lines("# Notes", "## Notes"),
            "tests/test_one.py": lines("# a test"),
        })
        self.assertEqual(code, 0, out)
        summary = SUMMARY_RE.search(out)
        self.assertEqual(
            [summary[field] for field in ("files", "links", "anchors", "paths", "problems")],
            ["2", "1", "1", "1", "0"],
            out,
        )

    def test_running_outside_the_repository_root_is_refused(self):
        self.check({"docs/report.md": lines("# Report")})
        nested = self.root / "docs"
        code, out = run_checker(nested, self.home)
        self.assertEqual(code, 2, out)
        self.assertIn("run from the repository root", out)

    def test_a_directory_without_git_is_refused(self):
        code, out = run_checker(self.root, self.home)
        self.assertEqual(code, 2, out)
        self.assertIn("check_docs:", out)


if __name__ == "__main__":
    unittest.main()
