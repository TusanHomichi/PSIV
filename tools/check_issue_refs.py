#!/usr/bin/env python3
"""Report deferrals to closed issues in the change (issue #159).

Run from the repository root (needs `gh`, authenticated; `--states FILE`
replaces the network with a `{"N": "OPEN"|"CLOSED"}` map, which the tests and
an offline run use):

    python3 tools/check_issue_refs.py [--repo OWNER/NAME] [--cache FILE] [--states FILE]

Every text file in the change with a scanned suffix (`tools/repo_files.py`;
the disassembly and `build/` are not) is read line by line. A `#N` reference
with a deferral phrase within 80 characters on the same line (`until`,
`deferred`, `pending`, `tracked in`, `follow-up`, `stopgap`, `not yet`) is a
*deferral*: a statement that still points at the issue for future work. When
the issue is CLOSED the deferral is stale — the class behind the "closing a
deferral updates every statement" rule (`docs/AGENT_WORKFLOW.md`). A history
mention ("fixed in #142", "PR #151 merged") carries no deferral phrase and is
not reported; "tracked code" and "every later draw" are adjectives, not
deferrals; the window is line-local, so a section heading like "Still open:"
is not a deferral phrase.

The issue states come from one paginated `gh api` call over the repository's
issues; the run writes them to its cache path as a record and always fetches,
so a cache left by an earlier run cannot hide an issue that closed since.
Without `gh` the check says so and exits 0. Pull requests are skipped; a
deferral to a merged PR is outside this check.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

try:  # imported as `tools.check_issue_refs` by tests
    from tools.repo_files import repo_files
except ModuleNotFoundError:  # direct `python3 tools/check_issue_refs.py`
    from repo_files import repo_files

#: The deferral phrases: "tracked" alone is an adjective ("tracked code") and
#: "later" one ("every later draw"), so only the deferral construction counts,
#: and only within `NEAR` characters of the reference.
DEFERRAL = re.compile(
    r"\b(?:until|deferred|pending|tracked\s+in|follow-up|stopgap|not\s+yet)\b",
    re.IGNORECASE)
#: How close (characters, either side) the deferral word and `#N` must be.
NEAR = 80

#: Text files worth scanning. The disassembly is the evidence source, not a
#: deferral record, and build/ is not tracked.
TEXT_SUFFIXES = {".md", ".rs", ".py", ".json", ".toml", ".yml", ".yaml", ".gd",
                 ".sh", ".cfg", ".txt", ".tscn", ".gdextension"}
SKIP_PREFIXES = ("reference/ps4disasm/", "build/", ".git/")

REFERENCE = re.compile(r"#(\d+)\b")


def tracked_text(root: Path = Path(".")) -> dict[Path, str]:
    """Every text file in the change under `root`, path to text."""
    files = {}
    for name in repo_files(root):
        path = Path(name)
        if name.startswith(SKIP_PREFIXES) or path.suffix not in TEXT_SUFFIXES:
            continue
        try:
            files[path] = (root / path).read_text(errors="replace")
        except OSError:
            continue
    return files


def scan_text(path: Path, text: str, states: dict[str, str]) -> list[dict]:
    """The stale deferrals in one file: `{path, line, issue, line_text}`.

    A reference is a deferral when a deferral word sits within `NEAR`
    characters of it on the same line; the deferral words never match across
    lines, so a line of JSON cannot borrow a word from a thousand bytes away.
    """
    hits = []
    for number, line in enumerate(text.splitlines(), start=1):
        for match in REFERENCE.finditer(line):
            window = line[max(0, match.start() - NEAR):match.end() + NEAR]
            if not DEFERRAL.search(window):
                continue
            if states.get(match.group(1)) != "CLOSED":
                continue
            hits.append({"path": path.as_posix(), "line": number,
                         "issue": int(match.group(1)),
                         "line_text": " ".join(line.split())[:220]})
    return hits


def fetch_states(repo: str | None, cache: Path) -> dict[str, str] | None:
    """`{"N": "OPEN"|"CLOSED"}` for the repository's issues, from one paginated
    `gh api` call, written to `cache` as the run's record. None when `gh` is
    unavailable."""
    if repo is None:
        try:
            url = subprocess.run(["git", "remote", "get-url", "origin"], capture_output=True,
                                 text=True, check=True).stdout.strip()
        except (OSError, subprocess.CalledProcessError):
            return None
        repo = re.sub(r"^.*[:/]([^/]+/[^/]+?)(?:\.git)?$", r"\1", url)
    try:
        pages = subprocess.run(
            ["gh", "api", "--paginate", "--slurp",
             f"repos/{repo}/issues?state=all&per_page=100"],
            capture_output=True, text=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    states = {}
    for page in json.loads(pages or "[]"):
        for issue in page:
            if "pull_request" in issue:
                continue
            states[str(issue["number"])] = issue["state"].upper()
    cache.parent.mkdir(parents=True, exist_ok=True)
    cache.write_text(json.dumps(states, sort_keys=True) + "\n")
    return states


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", help="OWNER/NAME (default: from the origin remote)")
    parser.add_argument("--cache", type=Path, default=Path("build/issue-states.json"),
                        help="the run's states cache (default build/issue-states.json)")
    parser.add_argument("--states", type=Path,
                        help="a JSON {N: OPEN|CLOSED} map instead of gh, for tests and offline runs")
    args = parser.parse_args(argv)
    if args.states is not None:
        states = {str(k): str(v).upper() for k, v in json.loads(args.states.read_text()).items()}
    else:
        states = fetch_states(args.repo, args.cache)
    if states is None:
        print("issue refs: skipped (gh is unavailable or not authenticated)")
        return 0
    hits = []
    for path, text in sorted(tracked_text().items()):
        hits.extend(scan_text(path, text, states))
    for hit in hits:
        print(f"{hit['path']}:{hit['line']}: #{hit['issue']} is closed but the line defers to it")
        print(f"  {hit['line_text']}")
    print(f"issue refs: {len(hits)} stale deferral(s) to closed issues")
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
