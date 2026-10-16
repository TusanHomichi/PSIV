#!/usr/bin/env python3
"""Check that tracked code names only repository paths that exist (issue #114).

Run from the repository root:

    python3 tools/check_paths.py

`tools/check_docs.py` runs the same check, so it is in both gate tiers without
a gate command of its own. It reads every Rust, GDScript, Python, shell, C
and Markdown file of the change and the campaign's JSON routes
(`SOURCE_PATTERNS`), comments, string literals, prose and inline code alike
(in Markdown, fenced blocks and link targets are left to `check_docs`), and
fails on:

- a repository path (`docs/...`, `godot/...`, `oracle/...`, `psiv_tools/...`,
  `rust/...`, `tests/...`, `tools/...`, `.github/...`) that names neither a
  file nor a directory of the change. A path resolves beside the file that
  names it first (a Markdown link's way), then from the repository root, then,
  inside a Rust crate, from that crate (`tests/runner.rs`). A path written as a
  template (`oracle/states/{name}.json`, `tools/native/*.gd`) is checked up to
  its last whole directory, and a range of siblings
  (`docs/scenes/03_A.md..11_B.md`) is checked at both ends;
- a bare file name (`native_alshline.gd`, `RUNNER_LOG.md`,
  `battle/BATTLE_RULES.md`) ending in `.md`, `.gd`, `.rs` or `.py` that resolves
  nowhere as above and that no file of the change ends with;
- a section of a Markdown file that the file does not have. `path.md#anchor`
  must be a heading's slug, slugged as `check_docs` slugs it. A quoted name
  after the path, `` `path.md` "Heading" ``, `` `path.md`'s "Heading" ``,
  `path.md, "Heading"` or JSON's `path.md \"Heading\"`, must give a
  heading's leading words or open a bold paragraph label (`**Heading`).

A file that no longer exists is cited by its commit instead: `RUNNER_LOG
(commit 0faa91b^)` names the text `git show 0faa91b^:<path>` prints.

A path git ignores is a local input (`AGENTS.md`, "Protect local inputs and
evidence") and passes, as in `check_docs`. Some sources name paths outside
the change on purpose: a test that builds a throwaway repository, or a file of
the disassembly clone. Those are listed in `EXEMPT`, file by file and with the
reason, and an entry that no longer matches its file is itself a problem, so
the list cannot rot.

Problems print one per line as `path:line: message`, then a summary count.
Exit status is 1 when a problem was found, 0 when the tree is clean, and 2 when
the checker cannot run at all.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from collections.abc import Iterable
from pathlib import Path

try:  # imported as `tools.check_paths`: the suite, and the gate's `PYTHONPATH=.`
    from tools.check_docs import LINK_RE, heading_slugs, iter_fenced_lines, slugify
    from tools.repo_files import RepoFilesError, repo_files
except ModuleNotFoundError:  # `python3 tools/check_paths.py` puts `tools/` itself on sys.path
    from check_docs import LINK_RE, heading_slugs, iter_fenced_lines, slugify
    from repo_files import RepoFilesError, repo_files

#: The sources scanned: code, and the routes whose note strings cite docs.
SOURCE_PATTERNS = ("*.rs", "*.gd", "*.py", "*.sh", "*.c", "*.h", "*.md",
                   "rust/psiv-campaign/routes/*.json")

#: The top-level directories a repository path starts with.
PATH_PREFIXES = (".github", "docs", "godot", "oracle", "psiv_tools", "rust", "tests", "tools")

#: The extensions a bare file name is recognized by.
BARE_EXTENSIONS = ("md", "gd", "rs", "py")

#: Sources that name paths outside the change on purpose. A value of None exempts the
#: whole file; a set exempts those exact tokens only. Each token must still
#: occur in its file, and each file must exist.
EXEMPT: dict[str, frozenset[str] | None] = {
    # This checker's own docstring and this table name examples, and so do
    # check_docs's rules for run-record file names.
    "tools/check_paths.py": None,
    "tools/check_docs.py": frozenset({"RUNNER_LOG_KURAN.md", "LANE_LEDGER.md", "CHANGELOG.md"}),
    # These build throwaway repositories and name the files they write there.
    "tests/test_check_docs.py": None,
    "tests/test_check_paths.py": None,
    "tests/test_repo_files.py": None,
    "tests/test_feature_map.py": None,
    "tests/test_size_guard.py": None,
    "tests/test_extract_stamp.py": frozenset({"a.py", "b.py", "unused.py", "sub/__init__.py",
                                              "sub/c.py"}),
    "tests/test_route_abilities.py": frozenset({"scene.md", "parent.rs", "child.rs"}),
    "tests/test_oracle_host_binary.py": frozenset({"oracle/new_launcher.py", "new_launcher.py"}),
    # Scripts and test modules the test writes and names in captured output.
    "tests/test_certify.py": frozenset({"fake_xvfb.py"}),
    "tests/test_gate.py": frozenset({".../capture/gate_sample_fail.py",
                                     "python3-tools-check_docs.py", "sample_tests.py"}),
    # Scratch files a test or tool writes into its own output directory.
    "tests/test_player_abilities.py": frozenset({"ledger.md"}),
    "tools/verify_native_route.py": frozenset({"summary.md"}),
    "docs/campaign/CAMPAIGN_RUNNER.md": frozenset({"summary.md"}),
    # The optional, untracked local override the agent guidance reads.
    "AGENTS.md": frozenset({"AGENTS.override.md"}),
    "docs/AGENT_WORKFLOW.md": frozenset({"AGENTS.override.md"}),
    "rust/psiv-core/src/battle/replay/stream.rs": frozenset({"state.md"}),
    # Rust syntax quoted as an example: `#[path = "x.rs"] mod x;`.
    "oracle/sweep/route_abilities.py": frozenset({"x.rs"}),
    # The disassembly's own compressor, reference/ps4disasm/script/, a local
    # input. Spelling it out would change psiv_tools/text.py, whose source is
    # part of the extract stamp, and so stale every checkout's generated/.
    "psiv_tools/text.py": frozenset({"compress_script.py"}),
    # Docs renamed for their topic (BATTLE_RULES.md, SOUND_DRIVER.md). These
    # comments keep the old names until a comment edit stops changing the
    # extract stamp (issue #143); remove the entries when it does.
    "psiv_tools/battle_pack.py": frozenset({"docs/battle/BATTLE_SCOUT.md"}),
    "psiv_tools/battle_records.py": frozenset({"docs/battle/BATTLE_SCOUT.md"}),
    "psiv_tools/sound_defs.py": frozenset({"docs/sound/SOUND_SCOUT.md"}),
}

_START = r"(?<![\w./$~-])"
_PREFIXED_RE = re.compile(
    _START
    + r"(?P<path>(?:" + "|".join(re.escape(p) for p in PATH_PREFIXES) + r")/[\w./-]*[\w])"
    + r"(?P<after>[{*<%\[$]?)"
    + r"(?:#(?P<anchor>[\w-]+))?"
)
_BARE_RE = re.compile(
    _START
    + r"(?P<path>[\w.-]+(?:/[\w.-]+)*\.(?:" + "|".join(BARE_EXTENSIONS) + r"))(?![\w])"
    + r"(?:#(?P<anchor>[\w-]+))?"
)
#: A range of sibling files, `docs/scenes/03_A.md..11_B.md`; `../` is a parent.
_RANGE_RE = re.compile(r"\.\.(?!/)")
_QUOTED_HEADING_RE = re.compile(
    _START + r"`?(?P<path>[\w./-]+\.md)`?(?:'s)?,? (?:§ ?)?\\?\"(?P<heading>[^\"\\]{2,80})\\?\""
)


def _ignored(paths: Iterable[str], root: Path) -> set[str]:
    """The members of `paths` that git ignores, themselves or through a parent.

    A parent is asked first: in a checkout whose ignored input directories are
    symbolic links, git refuses to answer for a path beyond one. A
    directory-only pattern (`build/`) matches a path that does not exist only
    when it is spelled with a trailing slash, so both spellings are asked, one
    at a time, since git fails the whole question when one is beyond a link.
    """
    answers: dict[str, bool] = {}

    def ignored(path: str) -> bool:
        if path not in answers:
            answers[path] = any(
                subprocess.run(
                    ["git", "check-ignore", "-q", spelling], capture_output=True,
                    cwd=root, check=False,
                ).returncode == 0
                for spelling in (path, path + "/")
            )
        return answers[path]

    found = set()
    for path in set(paths):
        parts = path.rstrip("/").split("/")
        if any(ignored("/".join(parts[:end])) for end in range(1, len(parts) + 1)):
            found.add(path)
    return found


def _crate_root(source: str, crates: set[str]) -> str | None:
    """The directory of the Rust crate `source` lives in, if any."""
    parts = source.split("/")
    for end in range(len(parts) - 1, 0, -1):
        candidate = "/".join(parts[:end])
        if candidate in crates:
            return candidate
    return None


class Tree:
    """The change's files and directories, and the anchors of its Markdown."""

    def __init__(self, root: Path):
        self.root = root
        self.files = set(repo_files(root))
        self.dirs = {path.rsplit("/", 1)[0] for path in self.files if "/" in path}
        for directory in list(self.dirs):
            while "/" in directory:
                directory = directory.rsplit("/", 1)[0]
                self.dirs.add(directory)
        self.crates = {path.rsplit("/", 1)[0] for path in self.files
                       if path.endswith("/Cargo.toml")}
        self._slugs: dict[str, set[str]] = {}
        self._texts: dict[str, str] = {}

    def exists(self, path: str) -> bool:
        return path in self.files or path in self.dirs

    def suffix_match(self, name: str) -> str | None:
        """The file the bare `name` resolves to, the shortest match first."""
        if name in self.files:
            return name
        matches = sorted((path for path in self.files if path.endswith("/" + name)), key=len)
        return matches[0] if matches else None

    def text(self, path: str) -> str:
        if path not in self._texts:
            self._texts[path] = (self.root / path).read_text(encoding="utf-8", errors="replace")
        return self._texts[path]

    def names_section(self, path: str, anchor: str, quoted: bool) -> bool:
        """True when Markdown `path` has the section `anchor` names.

        A `#anchor` must be a heading's slug. A quoted name may give a
        heading's leading words (`"Finding 3"` for "Finding 3 - one inn ...")
        or open a bold paragraph label (`**Fade correction:**`).
        """
        if path not in self._slugs:
            self._slugs[path] = heading_slugs(self.text(path).splitlines())
        slugs = self._slugs[path]
        if not quoted:
            return anchor in slugs
        slug = slugify(anchor)
        return (any(found == slug or found.startswith(slug + "-") for found in slugs)
                or f"**{anchor}" in self.text(path))


def _resolve(tree: Tree, source: str, path: str) -> str | None:
    """The file of the change `path` names from `source`, or None."""
    path = path.rstrip("/")
    beside = os.path.normpath(os.path.join(os.path.dirname(source), path))
    if tree.exists(beside):
        return beside
    if tree.exists(path):
        return path
    crate = _crate_root(source, tree.crates)
    if crate and tree.exists(f"{crate}/{path}"):
        return f"{crate}/{path}"
    return None


def _lines(source: str, text: str):
    """`(number, line)` to scan. In Markdown, fenced blocks and link targets are
    `check_docs`'s (its command paths and links), so they are left out here."""
    lines = text.splitlines()
    if not source.endswith(".md"):
        yield from enumerate(lines, 1)
        return
    for index, line, info, _ in iter_fenced_lines(lines):
        if info is None:
            yield index + 1, LINK_RE.sub(lambda match: match.group(0).split("](")[0], line)


Reference = tuple[str, int, str, "str | None", str]


def scan(tree: Tree, sources: Iterable[str],
         exempt_table: dict[str, frozenset[str] | None]) -> tuple[list[Reference], int]:
    """Every reference `sources` make, as `(source, line, path, anchor, kind)`.

    `kind` is "path" for a repository path, "bare" for a bare file name and
    "heading" for a quoted section name, whose text is in `anchor`. The count
    is of the paths and file names read; quoted sections are not counted.
    """
    found: list[Reference] = []
    count = 0
    for source in sources:
        exempt = exempt_table.get(source, frozenset())
        if exempt is None:
            continue
        try:
            text = (tree.root / source).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for number, line in _lines(source, text):
            for match in _PREFIXED_RE.finditer(line):
                first, *rest = _RANGE_RE.split(match["path"].rstrip("."))
                paths = [first] + [f"{first.rsplit('/', 1)[0]}/{last}" for last in rest]
                if match["after"]:
                    paths[-1] = paths[-1].rsplit("/", 1)[0]
                for path in paths:
                    if path in exempt:
                        continue
                    count += 1
                    anchor = match["anchor"] if path is paths[-1] else None
                    found.append((source, number, path, anchor, "path"))
            for match in _BARE_RE.finditer(line):
                path = match["path"]
                if path.startswith(tuple(p + "/" for p in PATH_PREFIXES)) or path in exempt:
                    continue
                count += 1
                found.append((source, number, path, match["anchor"], "bare"))
            for match in _QUOTED_HEADING_RE.finditer(line):
                found.append((source, number, match["path"], match["heading"], "heading"))
    return found, count


def check(root: Path | str = Path("."),
          exempt_table: dict[str, frozenset[str] | None] | None = None) -> tuple[list[str], int]:
    """Every problem in the change at `root`, and the number of paths read.

    `exempt_table` replaces `EXEMPT`. The table describes the repository this
    checker ships in, so its entries are verified against that tree only (any
    other tree, such as a test's throwaway repository, would fail every one);
    a table passed in is always verified.
    """
    root = Path(root)
    tree = Tree(root)
    verify = exempt_table is not None or root.resolve() == Path(__file__).resolve().parents[1]
    exempt_table = EXEMPT if exempt_table is None else exempt_table
    sources = sorted(repo_files(root, SOURCE_PATTERNS))
    references, count = scan(tree, sources, exempt_table)

    problems = []
    for source, exempt in sorted(exempt_table.items()) if verify else ():
        if source not in tree.files:
            problems.append(f"tools/check_paths.py: EXEMPT names a missing file: {source}")
            continue
        text = (root / source).read_text(encoding="utf-8", errors="replace")
        for token in sorted(exempt or ()):
            if token not in text:
                problems.append(f"tools/check_paths.py: EXEMPT token not in {source}: {token}")

    unresolved = []
    for source, line, path, anchor, kind in references:
        target = _resolve(tree, source, path)
        if target is None and kind != "path":
            target = tree.suffix_match(path)
        if target is None and kind == "heading":
            continue  # the same text's path or file name reports the missing file
        if target is None:
            unresolved.append((source, line, path, anchor, kind))
            continue
        if anchor and target.endswith(".md"):
            if not tree.names_section(target, anchor, quoted=kind == "heading"):
                shown = f'"{anchor}"' if kind == "heading" else f"#{anchor}"
                problems.append(f"{source}:{line}: missing heading anchor: {target} {shown}")

    ignored = _ignored((path for _, _, path, _, _ in unresolved), root)
    for source, line, path, anchor, kind in unresolved:
        if path.rstrip("/") in ignored:
            continue
        what = "file name matches no file" if kind != "path" else "path not found"
        problems.append(f"{source}:{line}: {what}: {path}")
    return problems, count


def main() -> int:
    try:
        problems, count = check(Path("."))
    except RepoFilesError as error:
        print(f"check_paths: {error}", file=sys.stderr)
        return 2
    for problem in problems:
        print(problem)
    print(f"checked {count} code paths; {len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
