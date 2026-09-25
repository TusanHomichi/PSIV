#!/usr/bin/env python3
"""The files in the change: one answer to "which files belong to the repository".

`git ls-files` lists the index, so a new file nobody has staged used to be
invisible to the guards that answered this question for themselves
(tools/check_docs.py, tools/size_guard.py and tests/test_feature_map.py), and a
lane worker - who cannot stage - could not check the file just written (issue
#30). They all read `repo_files` now, which is the question's one owner and
looks at the working tree as well:

    python3 -c "from tools.repo_files import repo_files; print(*repo_files())"

Stdlib only, Python 3.
"""

from __future__ import annotations

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
    ignores is not. The result is de-duplicated and sorted, and a path that no
    longer exists on disk is dropped, so a tracked file deleted from the
    working tree is not in it either.

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
    return sorted(path for path in listed if (root / path).exists())
