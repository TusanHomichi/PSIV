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
controls (a missing link target, a missing anchor, a missing command path, an
unmatched glob, a `yaml` block that does not parse, a host-absolute home
path, and issue #119's run records: a log or ledger file name or title, a dated
title, every deleted record's name and title, a dated receipt table and a
task-graph `yaml` block, each rejected by name and line) and positive cases that pin
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

from tools.repo_files import repo_files

ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "tools" / "check_docs.py"

SUMMARY_RE = re.compile(
    r"checked (?P<files>\d+) files, (?P<links>\d+) links, "
    r"(?P<anchors>\d+) anchors, (?P<paths>\d+) command paths, "
    r"(?P<yaml>\d+) yaml blocks, (?P<code>\d+) code paths; "
    r"(?P<problems>\d+) problem\(s\)"
)


def git_env(home):
    """The environment that keeps a case's Git answers its own."""
    return dict(os.environ, HOME=str(home), GIT_CONFIG_NOSYSTEM="1",
                GIT_CONFIG_GLOBAL=os.devnull)


def lines(*parts):
    """A file body, one newline-terminated line per part."""
    return "".join(part + "\n" for part in parts)


YAML_LANGS = ("yaml", "yml")


def yaml_block_count(texts):
    """How many closed `yaml`/`yml` fenced blocks `texts` hold, counted here.

    Deliberately not the checker's own parser: a fence opens with three or
    more backticks or tildes behind at most three spaces (a backtick fence's
    info string holds no backtick) and closes on a line of at least as many of
    the same character. A block left open at the end of a file is never
    checked, so it is not counted.
    """
    count = 0
    for text in texts:
        fence = None  # (character, length, is_yaml)
        for line in text.splitlines():
            body = line.lstrip(" ")
            if len(line) - len(body) > 3:
                continue  # indented code or block content, never a fence line
            if fence is None:
                char = body[:1]
                run = len(body) - len(body.lstrip(char)) if char in ("`", "~") else 0
                info = body[run:].strip()
                if run >= 3 and not (char == "`" and "`" in info):
                    words = info.split()
                    fence = (char, run, bool(words) and words[0].lower() in YAML_LANGS)
            elif body.rstrip() == fence[0] * len(body.rstrip()) and len(body.rstrip()) >= fence[1]:
                count += fence[2]
                fence = None
    return count


def tracked_markdown(root):
    """The Markdown texts the checker scans: the change's `*.md` files."""
    texts = []
    for name in repo_files(root, ("*.md",)):
        try:
            texts.append((Path(root) / name).read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError):
            continue  # the checker skips an unreadable file the same way
    return texts


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
        # The tree may hold no yaml block at all, so the count is checked
        # against an independent tally of the same files rather than a floor.
        self.assertEqual(int(summary["yaml"]), yaml_block_count(tracked_markdown(ROOT)), out)
        self.assertEqual(summary["problems"], "0")

    def test_a_yaml_block_appended_to_a_copy_is_counted(self):
        """One yaml block added to a copy of a real document counts exactly once."""
        original = (ROOT / "docs" / "ROADMAP.md").read_text(encoding="utf-8")
        before = yaml_block_count([original])
        appended = original + lines("", "```yaml", "added: [one]", "```")
        self.assertEqual(yaml_block_count([appended]), before + 1)
        code, out = self.check({"docs/ROADMAP.md": appended})
        summary = SUMMARY_RE.search(out)
        self.assertIsNotNone(summary, out)
        self.assertEqual(int(summary["yaml"]), before + 1, out)

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

    def test_a_yaml_block_that_does_not_parse(self):
        """An unterminated quote in a task graph is named at the line it opens."""
        code, out = self.check({
            "docs/graph.md": lines(
                "# Graph", "",
                "```yaml",
                "outcome: \"a route\"",
                "nodes:",
                "  - id: C",
                "    evidence: [\"one\", \"two]",
                "    state: ready",
                "```",
            ),
        })
        self.assertEqual(code, 1, out)
        self.assertIn(
            "docs/graph.md:7: yaml block does not parse: while scanning a quoted scalar", out
        )

    def test_a_host_absolute_home_path(self):
        """A `/home/<user>` or `/Users/<user>` path is named, in prose or a fence."""
        code, out = self.check({
            "docs/report.md": lines(
                "# Report", "",
                "Set TOOL=/home/alice/.local/bin/tool first.",
                "```sh", "/Users/bob/bin/godot --path godot", "```",
            ),
        })
        self.assertEqual(code, 1, out)
        self.assertIn("docs/report.md:3: host-absolute home path: /home/alice", out)
        self.assertIn("docs/report.md:5: host-absolute home path: /Users/bob", out)
        self.assertIn("; 2 problem(s)", out)

    def test_a_home_path_in_a_file_url_or_without_a_trailing_slash(self):
        """`file:///home/<user>/...` and a bare `/home/<user>` at a word end are named."""
        code, out = self.check({
            "docs/report.md": lines(
                "# Report", "",
                "The capture is file:///home/carol/frames/f.png here.",
                "Saves went to /home/dave.",
                "Or `/Users/erin`",
            ),
        })
        self.assertEqual(code, 1, out)
        self.assertIn("docs/report.md:3: host-absolute home path: /home/carol", out)
        self.assertIn("docs/report.md:4: host-absolute home path: /home/dave", out)
        self.assertIn("docs/report.md:5: host-absolute home path: /Users/erin", out)
        self.assertIn("; 3 problem(s)", out)

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

    def test_home_relative_paths_and_urls_pass(self):
        """`~/` paths, `$HOME` and a URL whose path holds `/home/` are not host paths."""
        code, out = self.check({
            "docs/report.md": lines(
                "# Report", "",
                "The default is `~/.local/bin/psiv-tool`, or $HOME/.local/bin.",
                "[site](https://example.invalid/home/alice/page)",
                "See https://example.com/home/x for the page, and ~/home/a/b.",
            ),
        })
        self.assertEqual(code, 0, out)

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


class RunRecords(RepoCase):
    """Issue #119: no new run log, dated receipt ledger or task graph in a doc.

    One negative control per class, and the research shapes that must pass.
    """

    def test_a_run_log_file_name(self):
        for name in ("docs/campaign/RUNNER_LOG_KURAN.md", "docs/run-log.md", "docs/LANE_LOGS.md"):
            with self.subTest(name=name):
                code, out = self.check({name: lines("# Runs")})
                self.assertEqual(code, 1, out)
                self.assertIn(f"{name}:1: run record file name", out)

    #: The deleted records, by name and first lines (`git show 0faa91b^:<path>`,
    #: and `3ea02d7^` for the last two). Each must be refused by its name or title.
    DELETED_RECORDS = {
        "docs/campaign/NATIVE_PLAYABILITY.md": ["# Native playability — evidence ledger"],
        "docs/campaign/PLAYABILITY_ACADEMY.md": [
            "_Academy — part of the [native playability ledger](NATIVE_PLAYABILITY.md); "
            "see the [documentation index](../README.md)._", "",
            "## Native combat continuation"],
        "docs/campaign/PLAYABILITY_ALSHLINE_ZEMA.md": [
            "_Alshline and Zema — part of the [native playability ledger]"
            "(NATIVE_PLAYABILITY.md); see the [documentation index](../README.md)._", "",
            "## Connected Alshline progression"],
        "docs/campaign/PLAYABILITY_FOUNDATIONS.md": [
            "_Foundations and audit — part of the [native playability ledger]"
            "(NATIVE_PLAYABILITY.md); see the [documentation index](../README.md)._", "",
            "## Audit context"],
        "docs/campaign/PLAYABILITY_HOLT_TONOE.md": [
            "_Holt and Tonoe — part of the [native playability ledger](NATIVE_PLAYABILITY.md); "
            "see the [documentation index](../README.md)._", "",
            "## Field TECH and SKILL recovery"],
        "docs/campaign/RUNNER_HALTS_EARLY.md": ["# Early runner halt receipts (H1–H16)"],
        "docs/campaign/RUNNER_LOG_ZELAN.md": ["# Runner log: the Zelan arc (C4, M23)"],
        "docs/campaign/AIEDO.md": ["# Aiedo arrival (`$54`) — campaign segment ledger"],
        "docs/records/CLAUDE_LANES.md": ["# Claude Code lane ledger"],
        "docs/records/WORKFLOW_SETUP.md": ["# Workflow setup ledger"],
        "docs/records/REDSHIRT_BATTLE.md": ["# Redshirt battle-decision experiment (archived)"],
        "docs/records/history/2026-08-14-origin.md": [
            "# Origin conversation (recovered 2026-08-14)"],
        "docs/DISPATCH.md": ["# Dispatch tiers (2026-08-15)"],
        "docs/IMPLEMENTATION_HISTORY.md": ["# Implementation archive"],
    }

    def test_the_deleted_records_are_refused(self):
        for name, body in self.DELETED_RECORDS.items():
            with self.subTest(name=name):
                code, out = self.check({name: lines(*body)})
                self.assertEqual(code, 1, out)
                self.assertRegex(out, rf"{re.escape(name)}:1: (run record (file name|title)"
                                      r"|dated title)")

    def test_a_record_title(self):
        for title, word in (("# Runner log: the Zelan arc", "log"),
                            ("# Lane receipts", "receipts"),
                            ("# Implementation history", "history"),
                            ("# Field journal", "journal")):
            with self.subTest(title=title):
                code, out = self.check({"docs/notes.md": lines("", title, "", "Text.")})
                self.assertEqual(code, 1, out)
                self.assertIn(f"docs/notes.md:2: run record title ({word})", out)

    def test_a_dated_title(self):
        code, out = self.check({"docs/tiers.md": lines("# Dispatch tiers (2026-08-15)")})
        self.assertEqual(code, 1, out)
        self.assertIn("docs/tiers.md:1: dated title (2026-08-15)", out)

    def test_a_message_log_document_passes(self):
        code, out = self.check({
            "docs/battle/MESSAGE_LOG.md": lines("# The battle message log", "",
                                                "## Log lines", "", "Each line is a window."),
            "docs/scenes/LOG_WINDOW.md": lines("# Log window"),
        })
        self.assertEqual(code, 0, out)

    def test_a_receipt_ledger_file_name(self):
        code, out = self.check({"docs/LANE_LEDGER.md": lines("# Lanes"),
                                "docs/receipts.md": lines("# Receipts")})
        self.assertEqual(code, 1, out)
        self.assertIn("docs/LANE_LEDGER.md:1: run record file name (LEDGER)", out)
        self.assertIn("docs/receipts.md:1: run record file name (RECEIPTS)", out)

    def test_a_dated_receipt_table(self):
        code, out = self.check({"docs/notes.md": lines(
            "# Notes", "",
            "| date | lane | outcome |", "|---|---|---|",
            "| 2026-09-23 | bake-D | accepted |",
            "| 2026-09-24 | ab-F | accepted |",
            "| 2026-09-24 | ab-G | sent back |",
        )})
        self.assertEqual(code, 1, out)
        self.assertIn("docs/notes.md:5: dated receipt ledger (3 table rows open with a date)", out)

    def test_a_task_graph_block(self):
        graphs = {
            "nodes": ["outcome: a route", "nodes:", "  - id: C1", "    state: ready"],
            "depends_on": ["- id: C2", "  depends_on: [C1]"],
            "next_action": ["plan:", "  next_action: C3"],
        }
        for key, body in graphs.items():
            with self.subTest(key=key):
                code, out = self.check({"docs/plan.md": lines("# Plan", "", "```yaml", *body, "```")})
                self.assertEqual(code, 1, out)
                self.assertIn(f"docs/plan.md:3: yaml block is a task graph ({key})", out)

    def test_a_run_evidence_path(self):
        code, out = self.check({"docs/notes.md": lines(
            "# Notes", "", "Transcripts are in `build/lane-evidence/run.log`.",
            "", "```bash", "ls build/x86-evidence/", "```")})
        self.assertEqual(code, 1, out)
        self.assertIn("docs/notes.md:3: run evidence path (build/lane-evidence)", out)
        self.assertIn("docs/notes.md:6: run evidence path (build/x86-evidence)", out)

    def test_research_shapes_pass(self):
        code, out = self.check({
            "docs/source-notes/battle.md": lines(
                "# Battle", "",
                "## 2026-10-03 - Rounds that open no party command window", "",
                "## 2026-09-24 - The vehicle's own attack", "",
                "## 2026-09-12 - Battle items", "",
                "| ability | captured |", "|---|---|",
                "| VOL | 2026-10-15, three fixtures |",
                "| 2026-09-23 | one dated row |",
                "| 2026-09-24 | and a second |",
                "", "```yaml", "formation: 0x1E1", "rounds: [1, 2]", "```",
                "", "Captures go to `build/forced/helex` and `build/sweep`.",
            ),
            "CHANGELOG.md": lines("# Changes"),
            "docs/CATALOG.md": lines("# Catalog"),
        })
        self.assertEqual(code, 0, out)


class PinnedVersions(RepoCase):
    """Issue #133: a document points at a pin file instead of copying its version."""

    PINS = {".godot-version": "4.7.1\n",
            "rust-toolchain.toml": '[toolchain]\nchannel = "1.97.1"\n'}

    def test_a_pinned_version_written_out(self):
        code, out = self.check({**self.PINS, "README.md": lines(
            "# Game", "", "| Godot | 4.7.1 | standard build |", "", "Rust 1.97.1 builds it.",
            "", "```bash", "export PSIV_GODOT=/opt/engine/v4.7.1/bin/engine", "```")})
        self.assertEqual(code, 1, out)
        self.assertIn("README.md:3: pinned version 4.7.1 written out: point at .godot-version", out)
        self.assertIn("README.md:5: pinned version 1.97.1 written out: point at "
                      "rust-toolchain.toml", out)
        self.assertIn("README.md:8: pinned version 4.7.1 written out", out)

    def test_other_versions_and_pointers_pass(self):
        code, out = self.check({**self.PINS, "README.md": lines(
            "# Game", "", "Godot is the version in `.godot-version`; 4.6.1 is refused,",
            "and so are 4.7.10, 14.7.1 and 4.7.1.1; Rust is `rust-toolchain.toml`'s.")})
        self.assertEqual(code, 0, out)

    def test_without_pin_files_nothing_is_pinned(self):
        code, out = self.check({"README.md": lines("# Game", "", "Godot 4.7.1 and Rust 1.97.1.")})
        self.assertEqual(code, 0, out)


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
            "docs/notes.md": lines(
                "# Notes", "## Notes", "",
                "```yaml", "note: \"a quoted: colon\"", "evidence: [\"a\", \"b\"]", "```",
                "", "```yml", "also: [checked]", "```",
            ),
            "tests/test_one.py": lines("# a test"),
        })
        self.assertEqual(code, 0, out)
        summary = SUMMARY_RE.search(out)
        self.assertEqual(
            [summary[field] for field in ("files", "links", "anchors", "paths", "yaml", "problems")],
            ["2", "1", "1", "1", "2", "0"],
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
