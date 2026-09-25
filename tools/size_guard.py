#!/usr/bin/env python3
"""tools/size_guard.py - the file-size rule (docs/DEVELOPMENT.md, File size).

Run from the repository root:

    python3 tools/size_guard.py
    python3 tools/size_guard.py --write-baseline

Every file `git ls-files` lists that is text - no NUL byte in its first 8 KiB -
is counted the way an editor counts it, a final line without a newline
included, and a file over `MAX_LINES` fails the run. Files matching `EXEMPT`,
the generated and data files that grow with their content rather than with
anyone's editing, are passed over; the list is the rule's own, and
docs/DEVELOPMENT.md names the same globs. The rule is checked inside the Python
suite the gate runs (tests/test_size_guard.py), so it is a gate check and not a
reviewer's memory; the lane harness reports the same limit on its own commits.

`tools/size_baseline.txt` is the ratchet: the files that were already over the
limit when the guard landed, one `<lines> <path>` per line, sorted by path.
Against it, all of these fail:

- a non-exempt file over the limit that the baseline does not list;
- a baselined file that grew past its recorded count;
- a baselined file now at or under the limit - the baseline's entry has to go;
- a baselined path that is gone, or that the rule no longer counts.

A baselined file that shrank but is still over the limit passes, and the report
suggests lowering its recorded count.

Problems print one per line, either `<path>: <lines> lines (limit 1000)` or the
baseline's reason, then suggestions as `note: ...`, then a summary line. Exit
status is 1 when a problem was found, 0 when the tree is clean, and 2 when the
guard cannot run at all: no repository, or not started from its root.

`--write-baseline` rewrites `tools/size_baseline.txt` from the current tree. It
is for seeding the file and for lowering a recorded count only, never for
accepting growth: it refuses to record a path the baseline does not already
know, and refuses to raise a recorded count, so a file that grew is
reorganized instead of baselined away. Stdlib only, Python 3.
"""

from __future__ import annotations

import argparse
import fnmatch
import re
import subprocess
import sys
from pathlib import Path

MAX_LINES = 1000
BINARY_SNIFF_BYTES = 8192
# Generated and data files (owner decision, 2026-09-24): a manifest, a transcript or a lock
# file grows with its content, and splitting one is not the reorganization the
# rule asks a change to make. Globs are fnmatch patterns, with `**` also
# crossing directories.
EXEMPT = ("*.json", "*.tsv", "*.csv", "*.lock", "**/replay_fixtures/**")
BASELINE = Path("tools") / "size_baseline.txt"
BASELINE_RE = re.compile(r"\A(?P<lines>\d+) (?P<path>\S.*)\Z")

EXIT_OK = 0
EXIT_PROBLEMS = 1
EXIT_REFUSED = 2


def git(*args: str) -> tuple[bool, str]:
    """`(ok, stdout)` of `git args` run in the working directory."""
    proc = subprocess.run(["git", *args], capture_output=True, text=True, check=False)
    return proc.returncode == 0, proc.stdout


def git_lines(*args: str) -> list[str]:
    """Non-empty NUL-separated records from a git command run at the root."""
    return [record for record in git(*args)[1].split("\0") if record]


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
    """`(sizes, skipped)` over `files` (default: `git ls-files`).

    `sizes` holds one entry per non-exempt text file, `path` -> lines; a tree's
    over-limit files are `over_limit(sizes)`. `skipped` counts the paths the
    rule passes over, by reason, so a report can show what it never looked at.
    """
    if files is None:
        files = git_lines("ls-files", "-z")
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


def parse_baseline(text: str) -> tuple[dict[str, int], list[str]]:
    """`(entries, problems)` for a baseline's text: `<lines> <path>` lines."""
    entries: dict[str, int] = {}
    problems: list[str] = []
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        match = BASELINE_RE.match(line)
        if match is None:
            problems.append(f"{BASELINE}:{number}: malformed baseline line: {line}")
            continue
        path = match["path"]
        if path in entries:
            problems.append(f"{BASELINE}:{number}: duplicate baseline entry: {path}")
            continue
        entries[path] = int(match["lines"])
    return entries, problems


def read_baseline(root: Path) -> tuple[dict[str, int], bool, list[str]]:
    """`(entries, exists, problems)`: the recorded counts, and whether it read."""
    path = root / BASELINE
    if not path.is_file():
        return {}, False, []
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        return {}, True, [f"{BASELINE}: cannot be read: {error}"]
    entries, problems = parse_baseline(text)
    return entries, True, problems


def baseline_text(entries: list[tuple[str, int]]) -> str:
    """The baseline's content for `entries`: `<lines> <path>`, sorted by path."""
    return "".join(f"{lines} {path}\n" for path, lines in sorted(entries))


def check(root: Path) -> tuple[list[str], list[str], dict[str, int]]:
    """`(problems, notes, counts)` for the tree at `root`."""
    baseline, _, problems = read_baseline(root)
    files = git_lines("ls-files", "-z")
    sizes, counts = scan(root, files)
    notes: list[str] = []
    for path, lines in over_limit(sizes):
        recorded = baseline.get(path)
        if recorded is None:
            problems.append(f"{path}: {lines} lines (limit {MAX_LINES})")
        elif lines > recorded:
            problems.append(
                f"{path}: {lines} lines, over its baseline of {recorded} "
                f"(limit {MAX_LINES})"
            )
    tracking = set(files)
    for path, recorded in sorted(baseline.items()):
        lines = sizes.get(path)
        if lines is None:  # the rule counts no lines for the path at all
            if path in tracking:
                problems.append(
                    f"{path}: recorded as {recorded} lines but the rule counts no "
                    "lines for it (exempt, binary or missing); "
                    "remove it from the baseline"
                )
            else:
                problems.append(
                    f"{path}: in the baseline but not tracked; "
                    "remove it from the baseline"
                )
        elif lines <= MAX_LINES:
            problems.append(
                f"{path}: {lines} lines, at or under the limit; "
                "remove it from the baseline"
            )
        elif lines < recorded:  # still over the limit, but closer: suggest a lower count
            notes.append(f"{path}: {lines} lines, down from {recorded}; lower its count")
    counts["over"] = len(over_limit(sizes))
    counts["baselined"] = len(baseline)
    return problems, notes, counts


def summary(counts: dict[str, int], problems: int) -> str:
    """The one-line report: what was scanned, and how many problems it found."""
    return (
        f"scanned {counts['files']} files ({counts['exempt']} exempt, "
        f"{counts['not_text']} binary or missing), {counts['over']} over the limit, "
        f"{counts['baselined']} baselined; {problems} problem(s)"
    )


def refusals(
    entries: list[tuple[str, int]], previous: dict[str, int], seeding: bool
) -> list[str]:
    """Why `--write-baseline` must not record `entries`, empty when it may.

    Seeding is writing a baseline no tree had yet (`seeding`, the file is
    absent): every entry is new and writing it is how the file is created.
    After that the only edits allowed are lowering a count and dropping a path.
    """
    if seeding:
        return []
    refused = []
    for path, lines in entries:
        recorded = previous.get(path)
        if recorded is None:
            refused.append(
                f"{path}: {lines} lines (limit {MAX_LINES}) is not in the baseline"
            )
        elif lines > recorded:
            refused.append(f"{path}: {lines} lines, up from {recorded}")
    if not refused:
        return []
    return [
        f"size_guard: refusing to write {BASELINE}: a file over the limit is "
        "reorganized (docs/DEVELOPMENT.md, File size), not baselined",
        *refused,
    ]


def rewrite(root: Path) -> int:
    """`--write-baseline`: write the current tree's over-limit files, or refuse."""
    previous, exists, problems = read_baseline(root)
    sizes, _ = scan(root)
    entries = over_limit(sizes)
    if problems:
        for problem in problems:
            print(problem)
        return EXIT_PROBLEMS
    refused = refusals(entries, previous, seeding=not exists)
    if refused:
        for line in refused:
            print(line)
        return EXIT_PROBLEMS
    path = root / BASELINE
    text = baseline_text(entries)
    if exists and path.read_text(encoding="utf-8") == text:
        print(f"{BASELINE} is unchanged: {len(entries)} entries")
        return EXIT_OK
    path.write_text(text, encoding="utf-8")
    print(f"wrote {BASELINE}: {len(entries)} entries")
    return EXIT_OK


def parser() -> argparse.ArgumentParser:
    """The command line: the check, and the baseline it ratchets against."""
    parser = argparse.ArgumentParser(
        description="Fail on tracked files over the file-size limit.",
        epilog="Run from the repository root; see docs/DEVELOPMENT.md, File size.",
    )
    parser.add_argument(
        "--write-baseline",
        action="store_true",
        help=f"rewrite {BASELINE} from the current tree (seeding and lowering only)",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)

    ok, top = git("rev-parse", "--show-toplevel")
    if not ok:
        print("size_guard: not a git repository", file=sys.stderr)
        return EXIT_REFUSED
    root = Path(top.strip()).resolve()
    if Path.cwd().resolve() != root:
        print(f"size_guard: run from the repository root ({root})", file=sys.stderr)
        return EXIT_REFUSED
    if args.write_baseline:
        return rewrite(root)
    problems, notes, counts = check(root)
    for problem in problems:
        print(problem)
    for note in notes:
        print(f"note: {note}")
    print(summary(counts, len(problems)))
    return EXIT_PROBLEMS if problems else EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
