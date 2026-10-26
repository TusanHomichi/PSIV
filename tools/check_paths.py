#!/usr/bin/env python3
r"""Check that tracked code names only repository paths that exist (issue #114).

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
- a climbing path (`../battle/BATTLE_RULES.md`, `../../runtime-pack`), read
  beside its file and from its crate; one that climbs out of the repository
  is not checked;
- a path built on a Rust crate's directory, `concat!(env!("CARGO_MANIFEST_DIR"),
  "/routes/main.json")` or `...(env!("CARGO_MANIFEST_DIR")).join("../x")`,
  on one line or several, and a Godot `res://` path, read from `godot/`;
- a bare file name (`native_alshline.gd`, `RUNNER_LOG.md`,
  `battle/BATTLE_RULES.md`, `rust-toolchain.toml`) ending in `.md`, `.gd`,
  `.rs`, `.py` or `.toml`, or a hyphenated dotfile name (`.godot-version`),
  that resolves nowhere as above and that no file of the change ends with;
- a section of a Markdown file that the file does not have. `path.md#anchor`
  must be a heading's slug, slugged as `check_docs` slugs it. A quoted name
  after the path, `` `path.md` "Heading" ``, `` `path.md`'s "Heading" ``,
  `path.md, "Heading"` or JSON's `path.md \"Heading\"`, must give a
  heading's leading words (its section number and a leading "The" may be
  left out) or open a bold paragraph label (`**Heading`). The quoted form is
  matched across wrapped lines: consecutive comment lines (`//`, `///`,
  `//!`, `#`, `#:`) with their markers stripped, and consecutive prose lines
  of Markdown.

In Python, a regular expression's escaped dot is read as a dot, so
`r"tools/godot\.py"` and `"tools/godot\\.py"` name `tools/godot.py`.

A file that no longer exists is cited by its commit instead: `RUNNER_LOG
(commit 0faa91b^)` names the text `git show 0faa91b^:<path>` prints.

A path git ignores is a local input (`AGENTS.md`, "Protect local inputs and
evidence") and passes, as in `check_docs`; so does a crate-relative spelling of
an ignored root path (`rust/psiv-core/build/x`: a test run from the crate's
directory writes the root's `build/x` there).

What it does not read, so a stale reference there still passes: a section
cited without quotes (`§ Root` alone, or `section 10`); JSON outside the
campaign routes; `.toml`, `.cfg`, `.yml` and other configuration files as
sources; a path assembled at run time from pieces (`ROOT / "docs" / name`);
and a quoted section name wrapped across lines of a string literal. Some sources name paths outside
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
import sys
from collections.abc import Iterable
from pathlib import Path

try:  # imported as `tools.check_paths`: the suite, and the gate's `PYTHONPATH=.`
    from tools.check_docs import LINK_RE, heading_slugs, iter_fenced_lines, slugify
    from tools.repo_files import RepoFilesError, ignored_paths, repo_files
except ModuleNotFoundError:  # `python3 tools/check_paths.py` puts `tools/` itself on sys.path
    from check_docs import LINK_RE, heading_slugs, iter_fenced_lines, slugify
    from repo_files import RepoFilesError, ignored_paths, repo_files

#: The sources scanned: code, and the routes whose note strings cite docs.
SOURCE_PATTERNS = ("*.rs", "*.gd", "*.py", "*.sh", "*.c", "*.h", "*.md",
                   "rust/psiv-campaign/routes/*.json")

#: The top-level directories a repository path starts with.
PATH_PREFIXES = (".github", "docs", "godot", "oracle", "psiv_tools", "rust", "tests", "tools")

#: The extensions a bare file name is recognized by.
BARE_EXTENSIONS = ("md", "gd", "rs", "py", "toml")

#: Sources that name paths outside the change on purpose. A value of None exempts the
#: whole file; a set exempts those exact tokens only. Each token must still
#: occur in its file, and each file must exist.
EXEMPT: dict[str, frozenset[str] | None] = {
    # This checker's own docstring and this table name examples, and so do
    # check_docs's rules for run-record file names.
    "tools/check_paths.py": None,
    "tools/check_docs.py": frozenset({"RUNNER_LOG_KURAN.md", "LANE_LEDGER.md", "MESSAGE_LOG.md",
                                     "CHANGELOG.md"}),
    # These build throwaway repositories and name the files they write there.
    "tests/test_check_docs.py": None,
    "tests/test_check_paths.py": None,
    "tests/test_repo_files.py": None,
    "tests/test_feature_map.py": None,
    # The launcher census's cases: launchers it reads from strings, not files.
    "tests/test_run_root.py": None,
    "tests/test_size_guard.py": None,
    "tests/test_extract_stamp.py": frozenset({"a.py", "b.py", "unused.py", "sub/__init__.py",
                                              "sub/c.py", "../precious.txt",
                                              "../outside.json"}),
    "tests/test_route_abilities.py": frozenset({"scene.md", "parent.rs", "child.rs"}),
    "tests/test_oracle_host_binary.py": frozenset({"oracle/new_launcher.py", "new_launcher.py"}),
    # Another extension's entry the test writes into a throwaway project's index.
    "tests/test_godot.py": frozenset({"res://addons/other/other.gdextension"}),
    # Scripts and test modules the test writes and names in captured output.
    "tests/test_certify.py": frozenset({"fake_xvfb.py"}),
    "tests/test_gate.py": frozenset({".../capture/gate_sample_fail.py",
                                     "python3-tools-check_docs.py", "sample_tests.py"}),
    # Scratch files a test or tool writes into its own output directory.
    "tests/test_player_abilities.py": frozenset({"ledger.md"}),
    "tools/verify_native_route.py": frozenset({"summary.md"}),
    "docs/campaign/CAMPAIGN_RUNNER.md": frozenset({"summary.md"}),
    # The optional, untracked local override the agent guidance reads, and
    # the house block's source file, in FieldmouseWorks/burrow.
    "AGENTS.md": frozenset({"AGENTS.override.md", "house/rules.md"}),
    "docs/AGENT_WORKFLOW.md": frozenset({"AGENTS.override.md"}),
    "rust/psiv-core/src/battle/replay/stream.rs": frozenset({"state.md"}),
    # Rust syntax quoted as an example: `#[path = "x.rs"] mod x;`.
    "oracle/sweep/route_abilities.py": frozenset({"x.rs"}),
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
#: A hyphenated dotfile name, `.godot-version`: the root's pin files.
_DOTFILE_RE = re.compile(_START + r"(?P<path>\.[a-z]\w*(?:-\w+)+)(?![\w.-])")
#: A path that climbs first, `../battle/BATTLE_RULES.md`, `../../runtime-pack`.
_PARENT_RE = re.compile(
    _START + r"(?P<path>(?:\.\./)+[\w.-]+(?:/[\w.-]+)*)(?P<after>[{*<%\[$]?)"
    + r"(?:#(?P<anchor>[\w-]+))?"
)
#: A Godot resource path, `res://field.tscn`; `res://` is the `godot/` directory.
_RES_RE = re.compile(r"\bres://(?P<path>[\w./-]*[\w])(?P<after>[{*<%\[$]?)")
#: A Rust path built on the crate's directory, on one line or several:
#: `concat!(env!("CARGO_MANIFEST_DIR"), "/routes/main.json")` and
#: `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack")`.
_MANIFEST_RE = re.compile(
    r'env!\(\s*"CARGO_MANIFEST_DIR"\s*\)\s*\)?\s*(?:,|\.join\()\s*"(?P<path>[^"\n]*)"'
)
#: A range of sibling files, `docs/scenes/03_A.md..11_B.md`; `../` is a parent.
_RANGE_RE = re.compile(r"\.\.(?!/)")
_QUOTED_HEADING_RE = re.compile(
    _START + r"`?(?P<path>[\w./-]+\.md)`?(?:'s)?,? (?:§ ?)?\\?\"(?P<heading>[^\"\\]{2,80})\\?\""
)


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
        or open a bold paragraph label (`**Fade correction:**`), and may leave
        out the heading's section number and leading "The" (`"How a shop opens"`
        for "2. How a shop opens", `"per-frame tick structure"` for "The
        per-frame tick structure - ...").
        """
        if path not in self._slugs:
            self._slugs[path] = heading_slugs(self.text(path).splitlines())
        slugs = self._slugs[path]
        if not quoted:
            return anchor in slugs
        slug = slugify(anchor)
        bare = {re.sub(r"\A(?:\d+[a-z]?-)?(?:the-)?", "", found) for found in slugs}
        return (any(found == slug or found.startswith(slug + "-") for found in slugs | bare)
                or f"**{anchor}" in self.text(path))


def _candidates(tree: Tree, source: str, path: str, kind: str) -> list[str]:
    """The repository paths `path` may name from `source`, in resolution order:
    beside `source` (a Markdown link's way), from the root, and, inside a Rust
    crate, from the crate (cargo runs a crate's tests from its directory). A
    climbing path (`../x`) is read beside its file and from its crate only, and
    a "root" path (`res://`, `CARGO_MANIFEST_DIR`) is already from the root.
    Spellings that climb out of the repository are dropped."""
    path = path.rstrip("/")
    if kind == "root":
        found = [os.path.normpath(path)]
    else:
        found = [os.path.normpath(os.path.join(os.path.dirname(source), path))]
        if kind != "parent":
            found.append(os.path.normpath(path))
        crate = _crate_root(source, tree.crates)
        if crate:
            found.append(os.path.normpath(f"{crate}/{path}"))
    unique = []
    for candidate in found:
        if candidate != ".." and not candidate.startswith("../") and candidate not in unique:
            unique.append(candidate)
    return unique


def _resolve(tree: Tree, candidates: list[str]) -> str | None:
    """The first of `candidates` that names a file or directory of the change."""
    return next((path for path in candidates if path == "." or tree.exists(path)), None)


def _ignore_spellings(tree: Tree, candidates: list[str]) -> list[str]:
    """The paths whose being ignored makes `candidates` a local input: each
    candidate, and for one inside a crate, its crate-relative rest read from
    the root (`rust/psiv-core/build/x` is where a test run from the crate puts
    the root's ignored `build/x`)."""
    found = list(candidates)
    for candidate in candidates:
        crate = _crate_root(candidate, tree.crates)
        if crate:
            found.append(candidate[len(crate) + 1:])
    return found


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


#: The comment markers a wrapped citation continues behind, longest first.
_COMMENT_RE = re.compile(r"\A\s*(?://[/!]?|#:?)\s?")


def _wrapped(source: str, lines: list[tuple[int, str]]):
    """`(line starts, text)` of each run a citation can wrap across; the starts
    are `(offset in text, line number)` pairs.

    A quoted section name often wraps (`` `path.md` "Deliberate fixes to `` on
    one comment line, `` retail battle bugs" `` on the next), so the quoted
    form is matched over joined runs: consecutive comment lines of code with
    their markers stripped, and consecutive non-blank prose lines of Markdown.
    Other lines (code, JSON) are their own run.
    """
    run: list[tuple[int, str]] = []

    def flush():
        if run:
            starts, offset = [], 0
            for number, text in run:
                starts.append((offset, number))
                offset += len(text) + 1
            yield starts, " ".join(text for _, text in run)
            run.clear()

    for number, line in lines:
        if source.endswith(".md"):
            if line.strip():
                run.append((number, line.strip()))
                continue
        else:
            marker = _COMMENT_RE.match(line)
            if marker:
                run.append((number, line[marker.end():].strip()))
                continue
        yield from flush()
        yield [(0, number)], line
    yield from flush()


Reference = tuple[str, int, str, "str | None", str]


def scan(tree: Tree, sources: Iterable[str],
         exempt_table: dict[str, frozenset[str] | None]) -> tuple[list[Reference], int]:
    """Every reference `sources` make, as `(source, line, path, anchor, kind)`.

    `kind` is "path" for a repository path, "bare" for a bare file name or a
    dotfile, "parent" for a climbing path, "root" for a path already read from
    the root (`res://`, `CARGO_MANIFEST_DIR`) and "heading" for a quoted
    section name, whose text is in `anchor`. The count
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
        if source.endswith(".rs"):
            crate = _crate_root(source, tree.crates)
            for match in _MANIFEST_RE.finditer(text):
                # Read here, and blanked so the line scan does not read it again.
                start, end = match.span("path")
                text = text[:start] + " " * (end - start) + text[end:]
                if crate and match["path"].strip("/") and match["path"] not in exempt:
                    count += 1
                    found.append((source, text.count("\n", 0, start) + 1,
                                  f"{crate}/{match['path'].lstrip('/')}", None, "root"))
        for number, line in _lines(source, text):
            if source.endswith(".py"):
                # A regular expression's escaped dot names the same path as the
                # plain one, raw (`r"tools/godot\.py"`) or not (`"tools/godot\\.py"`).
                line = re.sub(r"\\+\.", ".", line)
            for match in _PARENT_RE.finditer(line):
                path = match["path"]
                if path in exempt:
                    continue
                if match["after"]:
                    path = path.rsplit("/", 1)[0]
                count += 1
                found.append((source, number, path, match["anchor"], "parent"))
            for match in _RES_RE.finditer(line):
                path = f"godot/{match['path']}"
                if match["after"]:
                    path = path.rsplit("/", 1)[0]
                if match.group(0) in exempt:
                    continue
                count += 1
                found.append((source, number, path, None, "root"))
            for match in _DOTFILE_RE.finditer(line):
                if match["path"] in exempt:
                    continue
                count += 1
                found.append((source, number, match["path"], None, "bare"))
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
                if (path.startswith(tuple(p + "/" for p in PATH_PREFIXES) + ("../",))
                        or path in exempt):
                    continue
                count += 1
                found.append((source, number, path, match["anchor"], "bare"))
        for starts, block in _wrapped(source, list(_lines(source, text))):
            for match in _QUOTED_HEADING_RE.finditer(block):
                number = max(line for offset, line in starts if offset <= match.start())
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
        candidates = _candidates(tree, source, path, kind)
        if not candidates:
            continue  # it climbs out of the repository
        target = _resolve(tree, candidates)
        if target is None and kind in ("bare", "heading"):
            target = tree.suffix_match(path)
        if target is None and kind == "heading":
            continue  # the same text's path or file name reports the missing file
        if target is None:
            unresolved.append((source, line, path, kind, _ignore_spellings(tree, candidates)))
            continue
        if anchor and target.endswith(".md"):
            if not tree.names_section(target, anchor, quoted=kind == "heading"):
                shown = f'"{anchor}"' if kind == "heading" else f"#{anchor}"
                problems.append(f"{source}:{line}: missing heading anchor: {target} {shown}")

    ignored = ignored_paths((path for *_, spellings in unresolved for path in spellings), root)
    for source, line, path, kind, spellings in unresolved:
        if ignored.intersection(spellings):
            continue
        what = "file name matches no file" if kind == "bare" else "path not found"
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
