#!/usr/bin/env python3
"""tools/heavy_lock.py - the lock paths every worktree of this repository shares.

    python3 tools/heavy_lock.py [--gate]

Heavy runs (Godot, Xvfb, the gate's pack-loading suites) are serialized by
project rule across every worktree, so their locks must name one file for the
whole repository, not one per checkout. Both locks live under the main
checkout's `build/`, found through `git rev-parse --git-common-dir`, which a
linked worktree and the main checkout answer identically:

- `heavy_lock_path()`: `<main>/build/continuation-heavy.lock`, which
  `tools/verify_candidate.py` takes around certify and which shell runs take
  with `flock -x "$(python3 tools/heavy_lock.py)" <command>`.
  `PSIV_HEAVY_LOCK` overrides it.
- `gate_lock_path()`: `<main>/build/gate/.lock`, which `tools/gate.py` holds
  for a whole gate run.

A directory git cannot describe (a fixture, an exported tree) falls back to its
own `build/`. The command line prints the heavy lock path, or the gate lock's
with `--gate`. Stdlib only, Python 3.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

HEAVY_LOCK_ENV = "PSIV_HEAVY_LOCK"
HEAVY_LOCK_NAME = "continuation-heavy.lock"
GATE_LOCK = Path("gate") / ".lock"


def main_checkout(start=None):
    """The main checkout's root for the repository containing `start` (default: cwd).

    A linked worktree's common git directory is the main checkout's `.git`, so
    its parent is the main checkout. When git cannot describe `start`, or the
    common directory is not a `.git` directory (a bare repository), the answer
    is `start`'s own top level, or `start` itself.
    """
    start = Path(start or Path.cwd()).resolve()
    common = _git(start, "rev-parse", "--path-format=absolute", "--git-common-dir")
    if common:
        common = Path(common)
        if common.name == ".git":
            return common.parent
    return Path(_git(start, "rev-parse", "--show-toplevel") or start)


def heavy_lock_path(start=None, environ=None):
    """`PSIV_HEAVY_LOCK` when set, else `<main checkout>/build/continuation-heavy.lock`."""
    environ = os.environ if environ is None else environ
    override = environ.get(HEAVY_LOCK_ENV)
    if override:
        return Path(override)
    return main_checkout(start) / "build" / HEAVY_LOCK_NAME


def gate_lock_path(start=None):
    """`<main checkout>/build/gate/.lock`, held for the length of one gate run."""
    return main_checkout(start) / "build" / GATE_LOCK


def _git(cwd, *args):
    """Stripped stdout of a git command run in `cwd`, or "" when it fails."""
    try:
        completed = subprocess.run(["git", *args], cwd=str(cwd), stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, text=True)
    except OSError:
        return ""
    return completed.stdout.strip() if completed.returncode == 0 else ""


def main(argv=None):
    """Print the heavy lock path (or the gate lock's with `--gate`)."""
    parser = argparse.ArgumentParser(
        prog="python3 tools/heavy_lock.py",
        description="Print the repository-wide heavy-run lock path.")
    parser.add_argument("--gate", action="store_true",
                        help="print the gate lock instead of the heavy lock")
    args = parser.parse_args(argv)
    print(gate_lock_path() if args.gate else heavy_lock_path())
    return 0


if __name__ == "__main__":
    sys.exit(main())
