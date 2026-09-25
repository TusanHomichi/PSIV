#!/usr/bin/env python3
"""tools/size_guard.py - the file-size rule (docs/DEVELOPMENT.md, File size).

Run from the repository root:

    python3 tools/size_guard.py

Every file of the change that is text - the paths `tools/repo_files.py` lists,
what a commit made with `git add -A` would contain, a new unstaged file
included, and no NUL byte in the first 8 KiB - is counted the way an editor
counts it, a final line without a newline included, and a file over `MAX_LINES`
fails the run. Files matching `EXEMPT`, the generated and data files that grow
with their content rather than with anyone's editing, are passed over; the list
is the rule's own, and docs/DEVELOPMENT.md names the same globs. The rule is
checked inside the Python suite the gate runs (tests/test_size_guard.py), so it
is a gate check and not a reviewer's memory; the lane harness reports the same
limit on its own commits.

The rule has one form and no exceptions list: `EXEMPT` is the only carve-out,
and a non-exempt file over the limit is reorganized into cohesive modules
rather than let through. The command takes no arguments, so there is nothing to
configure and no way to record a file the rule should pass.

Problems print one per line as `<path>: <lines> lines (limit 1000)`, then a
summary line saying what was scanned and what was found. Exit status is 1 when
a problem was found, 0 when the tree is clean, and 2 when the guard cannot run
at all: no repository, not started from its root, or an argument it does not
take. Stdlib only, Python 3.
"""

from __future__ import annotations

import argparse
import fnmatch
import re
import subprocess
import sys
from pathlib import Path

try:  # imported as `tools.size_guard`: the suite, and the gate's `PYTHONPATH=.`
    from tools.repo_files import repo_files
except ModuleNotFoundError:  # `python3 tools/size_guard.py` puts `tools/` itself on sys.path
    from repo_files import repo_files

MAX_LINES = 1000
BINARY_SNIFF_BYTES = 8192
# Generated and data files (owner decision, 2026-09-24): a manifest, a transcript or a lock
# file grows with its content, and splitting one is not the reorganization the
# rule asks a change to make. Globs are fnmatch patterns, with `**` also
# crossing directories.
EXEMPT = ("*.json", "*.tsv", "*.csv", "*.lock", "**/replay_fixtures/**")

EXIT_OK = 0
EXIT_PROBLEMS = 1
EXIT_REFUSED = 2


def git(*args: str) -> tuple[bool, str]:
    """`(ok, stdout)` of `git args` run in the working directory."""
    proc = subprocess.run(["git", *args], capture_output=True, text=True, check=False)
    return proc.returncode == 0, proc.stdout


def glob_re(pattern: str) -> re.Pattern:
    """`pattern` as fnmatch reads it, with `**` also matching across `/`."""
    out, index = "", 0
    while index < len(pattern):
        char = pattern[index]
        if char == "*":
            if pattern.startswith("**", index):
                index += 2
                if pattern.startswith("/", index):  # `**/` matches no directory
                    index += 1
                    out += "(?:.*/)?"
                else:
                    out += ".*"
                continue
            out += "[^/]*"
        elif char == "?":
            out += "[^/]"
        else:
            out += re.escape(char)
        index += 1
    return re.compile(out + r"\Z")


def exempt(path: str) -> bool:
    """True when `path` is a generated or data file the rule passes over."""
    return any(
        fnmatch.fnmatchcase(path, pattern) or glob_re(pattern).match(path)
        for pattern in EXEMPT
    )


def counted_lines(path: Path) -> int | None:
    """Lines in `path`, or None when it is absent or binary (a NUL in 8 KiB).

    Content is bytes: any encoding may be committed and only a count is
    wanted. The count is the one an editor shows, so a final line without a
    newline still counts.
    """
    try:
        data = path.read_bytes()
    except OSError:
        return None
    if b"\0" in data[:BINARY_SNIFF_BYTES]:
        return None
    return len(data.splitlines())


def scan(root: Path, files: list[str] | None = None):
    """`(sizes, skipped)` over `files` (default: the change at `root`).

    `sizes` holds one entry per non-exempt text file, `path` -> lines; a tree's
    over-limit files are `over_limit(sizes)`. `skipped` counts the paths the
    rule passes over, by reason, so a report can show what it never looked at.
    """
    if files is None:
        files = repo_files(root)
    sizes: dict[str, int] = {}
    skipped = {"files": len(files), "exempt": 0, "not_text": 0}
    for path in sorted(files):
        if exempt(path):
            skipped["exempt"] += 1
            continue
        lines = counted_lines(root / path)
        if lines is None:
            skipped["not_text"] += 1  # a binary file, or a path staged but gone
        else:
            sizes[path] = lines
    return sizes, skipped


def over_limit(sizes: dict[str, int]) -> list[tuple[str, int]]:
    """The `(path, lines)` of every file in `sizes` over `MAX_LINES`, sorted."""
    return sorted(
        (path, lines) for path, lines in sizes.items() if lines > MAX_LINES
    )


def check(root: Path) -> tuple[list[str], dict[str, int]]:
    """`(problems, counts)` for the tree at `root`, one problem per offender."""
    sizes, counts = scan(root, repo_files(root))
    problems = [
        f"{path}: {lines} lines (limit {MAX_LINES})" for path, lines in over_limit(sizes)
    ]
    counts["over"] = len(problems)
    return problems, counts


def summary(counts: dict[str, int], problems: int) -> str:
    """The one-line report: what was scanned, and how many problems it found."""
    return (
        f"scanned {counts['files']} files ({counts['exempt']} exempt, "
        f"{counts['not_text']} binary or missing), {counts['over']} over the limit; "
        f"{problems} problem(s)"
    )


def parser() -> argparse.ArgumentParser:
    """The command line: the check, and nothing to tell it to skip."""
    return argparse.ArgumentParser(
        description="Fail on every non-exempt file over the file-size limit.",
        epilog="Run from the repository root; see docs/DEVELOPMENT.md, File size.",
    )


def main(argv: list[str] | None = None) -> int:
    parser().parse_args(argv)  # no argument is taken: an unknown one exits 2

    ok, top = git("rev-parse", "--show-toplevel")
    if not ok:
        print("size_guard: not a git repository", file=sys.stderr)
        return EXIT_REFUSED
    root = Path(top.strip()).resolve()
    if Path.cwd().resolve() != root:
        print(f"size_guard: run from the repository root ({root})", file=sys.stderr)
        return EXIT_REFUSED
    problems, counts = check(root)
    for problem in problems:
        print(problem)
    print(summary(counts, len(problems)))
    return EXIT_PROBLEMS if problems else EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
