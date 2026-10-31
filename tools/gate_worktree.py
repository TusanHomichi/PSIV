#!/usr/bin/env python3
"""Move the gate's kept worktree to the commit a gate is about to run on.

    python3 tools/gate_worktree.py DIR COMMIT [--cap-gib N]

Run it anywhere in the repository. A gate in a fresh worktree builds the Rust
workspace cold every time, so the steward's gate job keeps one worktree for
gates instead and moves it from commit to commit, and its `rust/target` stays
warm (docs/DEVELOPMENT.md, "Gate and coverage"):

* a DIR that doesn't exist is added as a detached worktree at COMMIT, marked
  as the gate's in its own Git directory (`MARKER`);
* a DIR this added before is checked out at COMMIT
  (`--force`) and cleaned of every untracked and ignored file but `rust/target`
  (`git clean -ffdx -e /rust/target/`): the last gate's linked inputs, which
  are links (the files behind them are left alone), and whatever it built in
  place go, and `tools/local_inputs.py link` links this commit's;
* the library a Godot run loads, `rust/target/<profile>/libpsiv_godot.so`
  (`LIBRARIES`), is removed: nothing in the gate builds it, and the Python
  suite, which runs before the gate's first cargo command, runs its Godot tests
  against whatever library is there, which would be another commit's;
* `rust/target` is removed whole once it takes more than the cap on disk
  (`--cap-gib`, default `CAP_GIB`), and that gate builds cold.

Cargo judges freshness by file times, and this is why moving one worktree is
safe where sharing a target dir between worktrees is not: Git's checkout
writes every file that differs between the two commits anew, so each is newer
than anything the last gate built, and a file that doesn't differ keeps its
time and its contents. A worktree checked out before another worktree's build
looks older than that build, and cargo reuses the other commit's crates for
its changed sources (FieldmouseWorks/remi#386).

One gate at a time moves it: the steward runs one gate job at once. The job
runs main's copy on any commit (`git show origin/main:tools/gate_worktree.py |
python3 - DIR COMMIT`), so this file imports nothing from `tools/`. Exit 0
ready, 1 a Git command failed, 2 refused: DIR is something other than the
gate's worktree of this repository (the main checkout, a lane's worktree, any
other directory), which this never cleans.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

EXIT_OK = 0
EXIT_FAILED = 1
EXIT_REFUSED = 2

#: The target dir the gate's cargo commands build into, relative to the root.
TARGET = Path("rust") / "target"

#: The libraries a Godot run loads from the worktree (godot/psiv.gdextension).
LIBRARIES = (TARGET / "debug" / "libpsiv_godot.so", TARGET / "release" / "libpsiv_godot.so")

#: The file, in a worktree's own Git directory (`git rev-parse --git-dir`),
#: that marks it as the gate's: only a worktree this added is ever cleaned.
MARKER = "psiv-gate-worktree"

#: The most `rust/target` may take on disk, in GiB, before it is removed. A
#: cold build takes about 2 GiB on remi's compressed ZFS and one commit's warm
#: build added about 0.8 GiB (FieldmouseWorks/remi#386); LoK-web's gate runners
#: keep theirs under the same 20.
CAP_GIB = 20


class Refused(Exception):
    """DIR is not a worktree this may move and clean."""


class GitFailed(Exception):
    """A Git command exited nonzero."""


def _git(cwd: Path, *args: str) -> str:
    result = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        raise GitFailed(f"git {' '.join(args)} failed in {cwd}: {result.stderr.strip()}")
    return result.stdout.strip()


def common_dir(path: Path) -> Path:
    """The `.git` directory the checkout or worktree at `path` shares."""
    return Path(_git(path, "rev-parse", "--path-format=absolute", "--git-common-dir")).resolve()


def disk_bytes(root: Path) -> int:
    """What `root` takes on disk, as `du` counts it: links are not followed."""
    total = 0
    for parent, dirs, files in os.walk(root):
        for name in dirs + files:
            total += os.lstat(os.path.join(parent, name)).st_blocks * 512
    return total


def _git_dir(worktree: Path) -> Path:
    return Path(_git(worktree, "rev-parse", "--absolute-git-dir")).resolve()


def is_gate_worktree(repo: Path, path: Path) -> bool:
    """Whether `path` is the root of a worktree of `repo` that this added."""
    try:
        if Path(_git(path, "rev-parse", "--show-toplevel")).resolve() != path.resolve():
            return False
        git_dir = _git_dir(path)
        return (common_dir(path) == common_dir(repo) and git_dir != common_dir(repo)
                and (git_dir / MARKER).is_file())
    except GitFailed:
        return False


def prepare(repo: Path, worktree: Path, commit: str, cap_bytes: int) -> list[str]:
    """Make `worktree` the gate's clean worktree of `repo` at `commit`; return what was done."""
    done = []
    sha = _git(repo, "rev-parse", "--verify", f"{commit}^{{commit}}")
    if not worktree.exists():
        # --force: a gate worktree whose directory was deleted is still
        # registered, and `add` would refuse its path.
        _git(repo, "worktree", "add", "--force", "--detach", str(worktree), sha)
        (_git_dir(worktree) / MARKER).write_text("the gate's kept worktree: tools/gate_worktree.py\n")
        done.append(f"added {worktree} at {sha[:12]}")
    else:
        if not is_gate_worktree(repo, worktree):
            raise Refused(f"{worktree} is not the gate's worktree of {repo} "
                          "(tools/gate_worktree.py adds it); it is left as it is")
        _git(worktree, "checkout", "--quiet", "--detach", "--force", sha)
        _git(worktree, "clean", "-ffdxq", "-e", f"/{TARGET.as_posix()}/")
        done.append(f"moved {worktree} to {sha[:12]}, keeping only {TARGET.as_posix()}")
    for library in LIBRARIES:
        path = worktree / library
        if path.is_file() or path.is_symlink():
            path.unlink()
            done.append(f"removed {library.as_posix()}")
    target = worktree / TARGET
    if target.is_dir():
        used = disk_bytes(target)
        if used > cap_bytes:
            shutil.rmtree(target)
            done.append(f"removed {TARGET.as_posix()}: {used / 2**30:.1f} GiB, over the "
                        f"{cap_bytes / 2**30:g} GiB cap; this gate builds cold")
        else:
            done.append(f"kept {TARGET.as_posix()}: {used / 2**30:.1f} GiB")
    else:
        done.append(f"no {TARGET.as_posix()}: this gate builds cold")
    return done


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 tools/gate_worktree.py",
        description="Move the gate's kept worktree to a commit, keeping only its warm rust/target.")
    parser.add_argument("dir", type=Path, help="the kept worktree (added if it doesn't exist)")
    parser.add_argument("commit", help="the commit the gate runs on")
    parser.add_argument("--cap-gib", type=float, default=CAP_GIB,
                        help=f"remove rust/target past this many GiB on disk (default {CAP_GIB})")
    args = parser.parse_args(argv)
    try:
        repo = common_dir(Path.cwd()).parent
        for line in prepare(repo, args.dir.absolute(), args.commit, int(args.cap_gib * 2**30)):
            print(f"gate worktree: {line}")
    except Refused as error:
        print(f"gate worktree: refused: {error}", file=sys.stderr)
        return EXIT_REFUSED
    except GitFailed as error:
        print(f"gate worktree: {error}", file=sys.stderr)
        return EXIT_FAILED
    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
