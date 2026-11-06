"""Start a test's throwaway Git repository: `init(path)`, the one way a test
makes one (tests/test_scratch_git.py holds every test module to it).

A repository it starts never runs Git's automatic maintenance. Each commit,
merge or rebase otherwise starts a detached `git maintenance run --auto`
that outlives the command: it takes `.git/objects/maintenance.lock`, and
when it finds a worktree entry without its `gitdir` file it runs
`git worktree prune`. `git worktree add` makes the entry's directory a moment
before it writes the `locked` file that keeps prune off, so a test that adds a
worktree right after a commit can lose that race and die with
"could not open '.git/worktrees/NAME/locked' for writing" (#371). The
setting lives in the repository's own config, so it holds for every Git a
test or the tool it tests runs there, whatever its environment.
"""
from __future__ import annotations

import os
import subprocess
from pathlib import Path


def init(path: str | os.PathLike, *, branch: str | None = "main",
         env: dict[str, str] | None = None) -> Path:
    """`git init -q [-b BRANCH] PATH` with automatic maintenance off; returns PATH."""
    path = Path(path)
    command = ["git", "init", "-q", *(["-b", branch] if branch else []), str(path)]
    for args in (command, ["git", "-C", str(path), "config", "maintenance.auto", "false"]):
        done = subprocess.run(args, capture_output=True, text=True, env=env)
        if done.returncode:
            raise RuntimeError(f"{args} exited {done.returncode}: {done.stderr.strip()}")
    return path
