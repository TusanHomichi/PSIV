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
- inside a `bash`, `sh`, `shell` or `console` block, every token naming a path
  under `docs/`, `godot/`, `oracle/`, `psiv_tools/`, `rust/`, `tests/` or
  `tools/` (optionally behind `./` or `$PWD/`) must exist, and a token with a
  `*` must match at least one file of the change.

A path git ignores is a local input (`AGENTS.md`, "Protect local inputs and
evidence"), not repository content - and not a file of the change either: the
ROM, `reference/`, generated packs, saves and captures live outside Git, so a
link or a command path naming one resolves only in a prepared checkout. The
checker skips those wherever it meets them, and stays green in a fresh clone.

Problems print one per line as `path:line: message`, followed by a summary
count. The counts are of the links, anchors and paths inspected; a link with a
scheme or host is passed over without being counted. Exit status is 1 when a
problem was found, 0 when the tree is clean, and 2 when the checker cannot run
at all: no repository, or not started from its root.
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

SCHEME_RE = re.compile(r"\A[A-Za-z][A-Za-z0-9+.\-]*:")
FENCE_OPEN_RE = re.compile(r"\A {0,3}(?P<fence>`{3,}|~{3,})(?P<info>.*)\Z")
HEADING_RE = re.compile(r"\A {0,3}(?P<hashes>#{1,6})(?:[ \t]+(?P<text>.*?))?[ \t]*\Z")
CLOSING_HASHES_RE = re.compile(r"[ \t]+#+[ \t]*\Z")
INLINE_CODE_RE = re.compile(r"(`+).+?\1")
LINK_RE = re.compile(r"\[[^\]]*\]\((?P<target>[^()]*(?:\([^()]*\)[^()]*)*)\)")
SHELL_ASSIGNMENT_RE = re.compile(r"\A[A-Za-z_][A-Za-z0-9_]*=(?P<value>.+)\Z", re.DOTALL)
SHELL_PUNCTUATION = ".,;:)]}\"'"


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
    """`(index, text, info)` per line; `info` is None outside a fenced block.

    A fence delimiter line reports the info string of the block it belongs to,
    so no content is ever read off a delimiter.
    """
    fence = None  # (character, length, info string)
    for index, text in enumerate(lines):
        current = None if fence is None else fence[2]
        if fence is None:
            open_match = FENCE_OPEN_RE.match(text)
            if open_match:
                fence_text, info = open_match["fence"], open_match["info"]
                # An info string may not contain a backtick in a backtick fence.
                if not (fence_text[0] == "`" and "`" in info):
                    fence = (fence_text[0], len(fence_text), info.strip())
                    current = fence[2]
        elif re.match(
            rf"\A {{0,3}}{re.escape(fence[0])}{{{fence[1]},}}[ \t]*\Z", text
        ):
            fence = None
        yield index, text, current


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
    for _, text, info in iter_fenced_lines(lines):
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
    counts = [len(texts), 0, 0, 0]  # files, links, anchors, command paths
    for name, lines in texts.items():
        for index, text, info in iter_fenced_lines(lines):
            line = index + 1
            if info is None:
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
    problems, counts = check_tree()
    for problem in problems:
        print(problem)
    files, links, anchors, paths = counts
    summary = (
        f"checked {files} files, {links} links, {anchors} anchors, {paths} command paths"
    )
    print(f"{summary}; {len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
