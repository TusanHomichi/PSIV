#!/usr/bin/env python3
"""The files in the change: one answer to "which files belong to the repository".

`git ls-files` lists the index, so a new file nobody has staged used to be
invisible to the guards that answered this question for themselves
(tools/check_docs.py, tools/size_guard.py and tests/test_feature_map.py), and a
lane worker - who cannot stage - could not check the file just written (issue
#30). They all read `repo_files` now, which is the question's one owner and
looks at the working tree as well:

    python3 -c "from tools.repo_files import repo_files; print(*repo_files())"

Its other half is `ignored_paths`, the one answer to "is this path a local
input Git ignores", which the documentation and code-path guards
(tools/check_docs.py, tools/check_paths.py) ask about the paths they cannot
resolve.

Stdlib only, Python 3.
"""

from __future__ import annotations

import os
import subprocess
from collections.abc import Iterable
from pathlib import Path


class RepoFilesError(RuntimeError):
    """`git` could not list the files: no repository at the root, or a refusal."""


def repo_files(root: Path | str = Path("."), patterns: Iterable[str] = ()) -> list[str]:
    """The repository-relative paths of the files in the change at `root`.

    That is what `git ls-files -z --cached --others --exclude-standard
    [-- patterns]` lists: every tracked file, plus every working-tree file Git
    does not ignore. It is exactly the set a commit made with `git add -A`
    would contain, so a file nobody has staged is in it, and a local input Git
    ignores is not. The result is de-duplicated and sorted. A tracked file
    deleted from the working tree is dropped, but a dangling symlink remains:
    its directory entry and link text still belong to the change.

    `patterns` filters the paths the way pathspecs do: `("*.md",)` is the
    Markdown of the change. `RepoFilesError` is raised when Git cannot list
    `root` at all.
    """
    root = Path(root)
    patterns = list(patterns)
    command = ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"]
    if patterns:
        command += ["--", *patterns]
    try:
        proc = subprocess.run(
            command, cwd=root, capture_output=True, text=True, check=False
        )
    except OSError as error:
        raise RepoFilesError(f"cannot run git in {root}: {error}") from error
    if proc.returncode != 0:
        raise RepoFilesError(
            f"git {' '.join(command[1:])} failed in {root}: {proc.stderr.strip()}"
        )
    listed = {record for record in proc.stdout.split("\0") if record}
    return sorted(path for path in listed if os.path.lexists(root / path))


def ignored_paths(paths: Iterable[str], root: Path | str = Path(".")) -> set[str]:
    """The members of `paths` (repository-relative) that Git ignores at `root`,
    themselves or through a parent directory.

    A parent is asked first: in a checkout whose ignored input directories are
    symbolic links (a lane worktree's `oracle/states`), Git refuses to answer
    for a path beyond one ("is beyond a symbolic link") and fails the whole
    question. A directory-only pattern (`oracle/frames/`) matches a path that
    does not exist only when it is spelled with a trailing slash, so both
    spellings are asked, one at a time for the same reason. A path outside the
    repository (`../x`) is never ignored.
    """
    root = Path(root)
    answers: dict[str, bool] = {}

    def ignored(path: str) -> bool:
        if path not in answers:
            answers[path] = any(
                subprocess.run(
                    ["git", "check-ignore", "-q", spelling], cwd=root,
                    capture_output=True, check=False,
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
