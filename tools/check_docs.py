#!/usr/bin/env python3
"""Check this repository's Markdown: relative links, heading anchors and command paths.

Run from the repository root:

    python3 tools/check_docs.py

Every Markdown file of the change - the paths `tools/repo_files.py` lists, what
a commit made with `git add -A` would contain, a new unstaged file included -
is scanned, and fenced code blocks and inline code are skipped as content:

- a relative `[text](target)` link, `target#anchor` included, must resolve to an
  existing file or directory; a bare `#anchor` points at the current file;
  links carrying a scheme or host (`https:`, `mailto:`, `//host/...`) and
  site-absolute links (`/docs/...`) are skipped;
- an anchor must name a heading of the target Markdown file, slugged the way
  GitHub does it: lowercase, everything but letters, digits, spaces, hyphens
  and underscores dropped, spaces turned into hyphens, and a repeated slug
  numbered `-1`, `-2`, ... in document order;
- every `yaml` (or `yml`) block must parse (PyYAML `safe_load`): a broken quote
  turns a YAML record into text no tool can read;
- no document is a run record (issue #119). Runs, receipts and task graphs
  live in commits, issues and pull requests (`docs/AGENT_WORKFLOW.md`), so the
  checker refuses the three shapes the deleted records had: a file named as a
  log, ledger or receipt (a word of its name is one of `RECORD_NAME_WORDS`:
  `RUNNER_LOG_KURAN.md`, `LANE_LEDGER.md`); a table with
  `DATED_ROWS_LIMIT` or more rows that open with a date (`| 2026-09-23 |
  lane | ...`); and a `yaml` block that holds a task graph (a mapping anywhere
  in it with a `TASK_GRAPH_KEYS` key: `nodes`, `depends_on` or
  `next_action`). Research documents pass: a dated heading or a dated
  chronology entry (`## 2026-10-03 - Rounds that open no party command
  window`), a date inside a table cell, and `CHANGELOG.md` (one word) are all
  allowed;
- inside a `bash`, `sh`, `shell` or `console` block, every token naming a path
  under `docs/`, `godot/`, `oracle/`, `psiv_tools/`, `rust/`, `tests/` or
  `tools/` (optionally behind `./` or `$PWD/`) must exist, and a token with a
  `*` must match at least one file of the change;
- no line, fenced or not, names a host-absolute home path (`/home/<user>`,
  `/Users/<user>`, a `file://` URL's included; a network URL's path is not
  one): it works on one machine only. Write a `~/`-relative path,
  an environment variable such as `$PSIV_GODOT`, or a placeholder.

The code of the change is checked too: `tools/check_paths.py` (issue #114)
fails on a repository path, file name or Markdown section that Rust, GDScript,
Python, shell, C or a campaign route names and the tree does not have. It runs
here so both gate tiers carry it without a command of their own.

A path git ignores is a local input (`AGENTS.md`, "Protect local inputs and
evidence"), not repository content - and not a file of the change either: the
ROM, `reference/`, generated packs, saves and captures live outside Git, so a
link or a command path naming one resolves only in a prepared checkout. The
checker skips those wherever it meets them, and stays green in a fresh clone.

Problems print one per line as `path:line: message`, followed by a summary
count. The counts are of the links, anchors and paths inspected; a link with a
scheme or host is passed over without being counted. Exit status is 1 when a
problem was found, 0 when the tree is clean, and 2 when the checker cannot run
at all: no repository, not started from its root, or PyYAML missing.
"""

from __future__ import annotations

import fnmatch
import os
import re
import shlex
import subprocess
import sys
from pathlib import Path
from urllib.parse import unquote

try:
    import yaml
except ModuleNotFoundError:  # reported by `main`: the checker cannot run without it
    yaml = None

try:  # imported as `tools.check_docs`: the suite, and the gate's `PYTHONPATH=.`
    from tools.repo_files import repo_files
except ModuleNotFoundError:  # `python3 tools/check_docs.py` puts `tools/` itself on sys.path
    from repo_files import repo_files

CHECKED_PATH_PREFIXES = (
    "docs/",
    "godot/",
    "oracle/",
    "psiv_tools/",
    "rust/",
    "tests/",
    "tools/",
)

# `$PWD/` survives shell word splitting only when the quotes around it are
# stripped first; keep both spellings so either reading of a token works.
PATH_STRIPPED_PREFIXES = ("./", "$PWD/", "${PWD}/", '"$PWD/', '"${PWD}/')

COMMAND_FENCE_LANGS = frozenset({"bash", "sh", "shell", "console"})

#: Words of a file name (split on `_`, `-` and `.`, any case) that name a run
#: record: a run log, a receipt ledger (issue #119).
RECORD_NAME_WORDS = frozenset({"LOG", "LOGS", "RUNLOG", "JOURNAL", "LEDGER", "LEDGERS",
                               "RECEIPT", "RECEIPTS"})
#: Rows opening with a date that make a table a dated receipt ledger.
DATED_ROWS_LIMIT = 3
DATED_ROW_RE = re.compile(r"\A {0,3}\|\s*\d{4}-\d{2}-\d{2}\b")
#: Keys that make a `yaml` block a task graph.
TASK_GRAPH_KEYS = frozenset({"nodes", "depends_on", "next_action"})
YAML_FENCE_LANGS = frozenset({"yaml", "yml"})

SCHEME_RE = re.compile(r"\A[A-Za-z][A-Za-z0-9+.\-]*:")
FENCE_OPEN_RE = re.compile(r"\A {0,3}(?P<fence>`{3,}|~{3,})(?P<info>.*)\Z")
HEADING_RE = re.compile(r"\A {0,3}(?P<hashes>#{1,6})(?:[ \t]+(?P<text>.*?))?[ \t]*\Z")
CLOSING_HASHES_RE = re.compile(r"[ \t]+#+[ \t]*\Z")
INLINE_CODE_RE = re.compile(r"(`+).+?\1")
LINK_RE = re.compile(r"\[[^\]]*\]\((?P<target>[^()]*(?:\([^()]*\)[^()]*)*)\)")
SHELL_ASSIGNMENT_RE = re.compile(r"\A[A-Za-z_][A-Za-z0-9_]*=(?P<value>.+)\Z", re.DOTALL)
SHELL_PUNCTUATION = ".,;:)]}\"'"
# A home directory spelled out for one machine: `/home/<user>` or
# `/Users/<user>` ending in `/`, whitespace, punctuation or the line's end. A
# path inside a longer one (`a/home/b/`, `~/home/b/`) is not one; neither is the
# path of a network URL, which `host_home_paths` blanks out first.
HOST_HOME_RE = re.compile(
    r"(?<![\w.~/$-])/(?:home|Users)/[A-Za-z0-9._-]+?(?=[/\s.,;:)\]}>\"'`]|\Z)"
)
# A URL with a scheme; `file:` URLs name local paths and stay checked.
URL_RE = re.compile(r"\b(?P<scheme>[A-Za-z][A-Za-z0-9+.\-]*)://\S*")


def host_home_paths(text: str) -> list[str]:
    """The host-absolute home paths one line names.

    A network URL (`https://example.com/home/x`) is a remote path, so it is
    blanked before the scan; a `file://` URL keeps its path, which is local.
    """
    def blank(match: re.Match) -> str:
        if match["scheme"].lower() == "file":
            return " " * len("file://") + match.group(0)[len("file://"):]
        return " " * len(match.group(0))

    return [found.group(0) for found in HOST_HOME_RE.finditer(URL_RE.sub(blank, text))]


def git_stdout(*args: str) -> str:
    """`git args` output, run at the repository root."""
    proc = subprocess.run(
        ["git", *args], capture_output=True, text=True, cwd=Path.cwd(), check=False
    )
    if proc.returncode != 0:
        failure = f"check_docs: git {' '.join(args)} failed: {proc.stderr.strip()}"
        print(failure, file=sys.stderr)
        raise SystemExit(2)
    return proc.stdout


def is_ignored(path: str) -> bool:
    """True when git ignores `path`, which marks it a local input.

    A directory-only pattern (`oracle/frames/`) matches a path that does not
    exist on disk only when the path is spelled with a trailing slash, so both
    spellings go in. `-q` takes a single pathname; `--stdin` carries both.
    """
    proc = subprocess.run(
        ["git", "check-ignore", "-q", "--stdin"],
        input=f"{path}\n{path}/\n",
        capture_output=True,
        text=True,
        cwd=Path.cwd(),
        check=False,
    )
    return proc.returncode == 0


def iter_fenced_lines(lines: list[str]):
    """`(index, text, info, delimiter)` per line; `info` is None outside a fenced block.

    A fence delimiter line reports the info string of the block it belongs to,
    so no content is ever read off a delimiter; `delimiter` marks those lines.
    """
    fence = None  # (character, length, info string)
    for index, text in enumerate(lines):
        current = None if fence is None else fence[2]
        delimiter = fence is not None
        if fence is None:
            open_match = FENCE_OPEN_RE.match(text)
            if open_match:
                fence_text, info = open_match["fence"], open_match["info"]
                # An info string may not contain a backtick in a backtick fence.
                if not (fence_text[0] == "`" and "`" in info):
                    fence = (fence_text[0], len(fence_text), info.strip())
                    current = fence[2]
                    delimiter = True
        elif re.match(
            rf"\A {{0,3}}{re.escape(fence[0])}{{{fence[1]},}}[ \t]*\Z", text
        ):
            fence = None
        else:
            delimiter = False
        yield index, text, current, delimiter


def fence_language(info: str) -> str:
    """The language an info string names, lowercase; `''` for a bare block."""
    return info.split()[0].lower() if info.split() else ""


def slugify(text: str) -> str:
    """GitHub's anchor slug for one heading's rendered text."""
    kept = [char for char in text.lower() if char.isalnum() or char in " -_"]
    return "".join(kept).replace(" ", "-")


def heading_slugs(lines: list[str]) -> set[str]:
    """Every anchor a file's headings define, duplicates numbered in order."""
    seen: dict[str, int] = {}
    slugs: set[str] = set()
    for _, text, info, _ in iter_fenced_lines(lines):
        if info is not None:
            continue
        match = HEADING_RE.match(text)
        if match is None:
            continue
        heading = CLOSING_HASHES_RE.sub("", match["text"] or "")
        slug = slugify(heading)
        if not slug:
            continue
        if slug in seen:
            seen[slug] += 1
            slug = f"{slug}-{seen[slug]}"
        else:
            seen[slug] = 0
        slugs.add(slug)
    return slugs


def strip_inline_code(text: str) -> str:
    """`text` with code spans blanked out, so their content is not read."""
    return INLINE_CODE_RE.sub(lambda match: " " * len(match.group(0)), text)


def link_targets(text: str) -> list[str]:
    """The raw targets of every inline `[text](target)` link on one line."""
    return [match["target"] for match in LINK_RE.finditer(text)]


def split_link_target(raw: str) -> tuple[str, str] | None:
    """`(path, fragment)` of a link target, or None when it is not checked.

    A target with a scheme, a host or a leading slash is not repository
    content, so it is skipped rather than resolved.
    """
    target = raw.strip()
    words = target.split()
    bracketed = target.startswith("<") and target.endswith(">")
    if bracketed:
        target = target[1:-1].strip()
    elif len(words) > 1:
        target = words[0]  # drop a `"title"` after the path
    if not target or target.startswith("/") or SCHEME_RE.match(target):
        return None  # `//host/...` starts with a slash and is skipped here too
    path, _, fragment = target.partition("#")
    return unquote(path), unquote(fragment)


def shell_words(line: str) -> list[str]:
    """One command line's words, quotes and comments removed."""
    try:
        return shlex.split(line, comments=True)
    except ValueError:  # an unbalanced quote: fall back to a plain split
        return line.split()


def path_token(word: str) -> str | None:
    """The repository path a shell word names, or None when it names none."""
    for prefix in PATH_STRIPPED_PREFIXES:
        if word.startswith(prefix):
            word = word[len(prefix) :]
            break
    word = word.rstrip(SHELL_PUNCTUATION)
    return word if word.startswith(CHECKED_PATH_PREFIXES) else None


def command_paths(line: str) -> list[str]:
    """The repository paths a command line names, in order."""
    found = []
    for word in shell_words(line):
        candidates = [word]
        assignment = SHELL_ASSIGNMENT_RE.match(word)
        if assignment:
            candidates.append(assignment["value"])
        for candidate in candidates:
            path = path_token(candidate)
            if path is not None:
                found.append(path)
    return found


def check_path(path: str, files: list[str]) -> str | None:
    """A message when `path` names nothing in the change, else None."""
    if "*" in path or "?" in path:
        if any(fnmatch.fnmatchcase(name, path) for name in files):
            return None
        if is_ignored(path):
            return None
        return f"command glob matches no tracked file: {path}"
    if Path(path).exists() or is_ignored(path):
        return None
    return f"command path not found: {path}"


def check_link(
    source: str, line: int, raw: str, slugs: dict[str, set[str]]
) -> tuple[list[str], int, int]:
    """Problems and the link/anchor counts for one link target."""
    parsed = split_link_target(raw)
    if parsed is None:
        return [], 0, 0
    path, fragment = parsed
    target = os.path.normpath(os.path.join(os.path.dirname(source), path)) if path else source
    if path and not Path(target).exists():
        if is_ignored(target):
            return [], 1, 0
        return [f"{source}:{line}: broken link target: {raw.strip()}"], 1, 0
    anchors = slugs.get(target)
    if fragment and anchors is not None:
        if fragment not in anchors:
            where = f"{target}#{fragment}" if path else f"#{fragment}"
            return [f"{source}:{line}: broken anchor: {where}"], 1, 1
        return [], 1, 1
    return [], 1, 0


def check_yaml(name: str, line: int, body: list[str]) -> str | None:
    """A problem line when the `yaml` block opened at `line` does not parse,
    or parses to a task graph."""
    try:
        data = yaml.safe_load("\n".join(body))
    except yaml.YAMLError as error:
        # The context mark is where the broken construct opened (a quote's
        # first character), which is the line to fix; the problem mark can sit
        # at the end of the block.
        mark = getattr(error, "context_mark", None) or getattr(error, "problem_mark", None)
        at = line + 1 + mark.line if mark is not None else line
        reason = "; ".join(
            part for part in (getattr(error, "context", None), getattr(error, "problem", None))
            if part
        ) or str(error).splitlines()[0]
        return f"{name}:{at}: yaml block does not parse: {reason}"
    keys = task_graph_keys(data)
    if keys:
        return (f"{name}:{line}: yaml block is a task graph ({', '.join(sorted(keys))}): "
                "task graphs belong in issues and pull requests")
    return None


def record_name_problem(name: str) -> str | None:
    """A message when the file `name` is named as a run record, else None."""
    stem = Path(name).name.rsplit(".", 1)[0]
    words = {word.upper() for word in re.split(r"[_.\-]+", stem) if word}
    found = sorted(words & RECORD_NAME_WORDS)
    if found:
        return (f"{name}:1: run record file name ({found[0]}): runs and receipts "
                "belong in commits, issues and pull requests")
    return None


def task_graph_keys(data) -> set[str]:
    """The `TASK_GRAPH_KEYS` that any mapping inside parsed yaml `data` holds."""
    found: set[str] = set()
    if isinstance(data, dict):
        found |= TASK_GRAPH_KEYS & {key for key in data if isinstance(key, str)}
        for value in data.values():
            found |= task_graph_keys(value)
    elif isinstance(data, list):
        for value in data:
            found |= task_graph_keys(value)
    return found


def check_tree() -> tuple[list[str], list[int]]:
    """Every problem in the tree, and the files/links/anchors/paths counts.

    The tree is the working directory, which `main` has checked is the
    repository root; `repo_files` lists the files of the change there.
    """
    root = Path.cwd()
    markdown = repo_files(root, ("*.md",))
    files = repo_files(root)
    texts = {}
    for name in markdown:
        try:
            texts[name] = Path(name).read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeDecodeError):
            continue  # not readable text: a binary blob, or a permission
    slugs = {name: heading_slugs(lines) for name, lines in texts.items()}

    problems: list[str] = []
    counts = [len(texts), 0, 0, 0, 0]  # files, links, anchors, command paths, yaml blocks
    for name, lines in texts.items():
        message = record_name_problem(name)
        if message:
            problems.append(message)
        for index, text in enumerate(lines):
            for path in host_home_paths(text):
                problems.append(f"{name}:{index + 1}: host-absolute home path: {path}")
        dated = [index + 1 for index, text, info, _ in iter_fenced_lines(lines)
                 if info is None and DATED_ROW_RE.match(text)]
        if len(dated) >= DATED_ROWS_LIMIT:
            problems.append(f"{name}:{dated[0]}: dated receipt ledger ({len(dated)} table rows "
                            "open with a date): runs and receipts belong in commits, issues "
                            "and pull requests")
        block = None  # (opening line, body) of the yaml block being read
        for index, text, info, delimiter in iter_fenced_lines(lines):
            line = index + 1
            if info is not None and fence_language(info) in YAML_FENCE_LANGS:
                if not delimiter:
                    block[1].append(text)
                elif block is None:
                    block = (line, [])
                else:
                    counts[4] += 1
                    message = check_yaml(name, *block)
                    if message:
                        problems.append(message)
                    block = None
            elif info is None:
                for raw in link_targets(strip_inline_code(text)):
                    found, links, anchors = check_link(name, line, raw, slugs)
                    problems += found
                    counts[1] += links
                    counts[2] += anchors
            elif fence_language(info) in COMMAND_FENCE_LANGS:
                for path in command_paths(text):
                    counts[3] += 1
                    message = check_path(path, files)
                    if message:
                        problems.append(f"{name}:{line}: {message}")
    return problems, counts


def main() -> int:
    top = git_stdout("rev-parse", "--show-toplevel").strip()
    root = Path(".").resolve()
    if Path(top).resolve() != root:
        print(f"check_docs: run from the repository root ({top})", file=sys.stderr)
        return 2
    if yaml is None:
        print("check_docs: PyYAML is required to parse yaml blocks (docs/DEVELOPMENT.md)",
              file=sys.stderr)
        return 2
    problems, counts = check_tree()
    # Imported here: check_paths reads this module's slug rules at its import.
    try:
        from tools.check_paths import check as check_code_paths
    except ModuleNotFoundError:
        from check_paths import check as check_code_paths
    code_problems, code_paths = check_code_paths(root)
    problems += code_problems
    for problem in problems:
        print(problem)
    files, links, anchors, paths, yaml_blocks = counts
    summary = (
        f"checked {files} files, {links} links, {anchors} anchors, {paths} command paths, "
        f"{yaml_blocks} yaml blocks, {code_paths} code paths"
    )
    print(f"{summary}; {len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
