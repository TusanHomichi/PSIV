#!/usr/bin/env python3
"""tools/heavy_lock.py - the repository-wide heavy-run lock: its path and its holder.

    python3 tools/heavy_lock.py [--gate]          # print the lock path
    python3 tools/heavy_lock.py run -- <cmd...>   # run <cmd> holding the heavy lock

Heavy runs (Godot, Xvfb, the gate's pack-loading suites) are serialized by
project rule across every worktree, so their locks must name one file for the
whole repository, not one per checkout. Both locks live under the main
checkout's `build/`, found through `git rev-parse --git-common-dir`, which a
linked worktree and the main checkout answer identically:

- `heavy_lock_path()`: `<main>/build/continuation-heavy.lock`.
  `PSIV_HEAVY_LOCK` overrides it.
- `gate_lock_path()`: `<main>/build/gate/.lock`, which `tools/gate.py` holds
  for a whole gate run.

`hold_heavy_lock(path)` is the one way in-repo code takes the heavy lock, and
`run -- <cmd>` the one way to wrap a command in it. The holder exports
`PSIV_HEAVY_LOCK_HELD=<resolved path>`, so a descendant that asks for the same
lock (the gate under `run`, verify_candidate's certify step) proceeds without
blocking on its own ancestor. A raw `flock(1)` wrapper sets no marker; a child
that finds the lock held by one of its ancestors (by inode in /proc/locks)
exits 2 naming the parent instead of waiting forever. Any other holder is
waited for, with one "waiting for" line naming it.

A directory git cannot describe (a fixture, an exported tree) falls back to its
own `build/`. Stdlib only, Python 3, Linux (/proc).
"""

from __future__ import annotations

import argparse
import fcntl
import os
import subprocess
import sys
from contextlib import contextmanager
from pathlib import Path

HEAVY_LOCK_ENV = "PSIV_HEAVY_LOCK"
HELD_ENV = "PSIV_HEAVY_LOCK_HELD"
HEAVY_LOCK_NAME = "continuation-heavy.lock"
GATE_LOCK = Path("gate") / ".lock"
EXIT_HELD_BY_PARENT = 2


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


def lock_holders(path, proc_locks=Path("/proc/locks")):
    """PIDs holding a lock on `path`'s inode, from /proc/locks.

    Matched by inode alone: on btrfs the device field names the superblock's
    device, not the file's `st_dev`. Waiters (`->` lines) are not holders.
    """
    inode = os.stat(path).st_ino
    pids = []
    try:
        lines = Path(proc_locks).read_text().splitlines()
    except OSError:
        return pids
    for line in lines:
        fields = line.split()
        if len(fields) < 6 or fields[1] == "->":
            continue
        try:
            pid = int(fields[4])
            line_inode = int(fields[5].rsplit(":", 1)[1])
        except (ValueError, IndexError):
            continue
        if line_inode == inode and pid not in pids:
            pids.append(pid)
    return pids


def ancestor_pids(pid=None):
    """`pid` (default: this process) and every ancestor up to init, nearest first."""
    pid = os.getpid() if pid is None else pid
    chain = []
    while pid > 0 and pid not in chain:
        chain.append(pid)
        try:
            stat = Path(f"/proc/{pid}/stat").read_text()
        except OSError:
            break
        pid = int(stat.rsplit(")", 1)[1].split()[1])  # field 4, after the comm
    return chain


@contextmanager
def hold_heavy_lock(path=None, environ=None, report=None):
    """Hold the heavy lock `path` (default: `heavy_lock_path()`) for the block.

    A no-op when this process already runs under a holder of the same file
    (`PSIV_HEAVY_LOCK_HELD`). Otherwise it takes the lock, exporting the marker
    for the block's subprocesses. When the lock is busy: if an ancestor holds
    it (a raw `flock(1)` wrapper), it prints why and raises SystemExit(2)
    rather than wait on itself; else it prints one "waiting for" line and blocks.
    """
    environ = os.environ if environ is None else environ
    report = report or sys.stderr
    path = Path(path or heavy_lock_path(environ=environ))
    path.parent.mkdir(parents=True, exist_ok=True)
    resolved = str(path.resolve())
    if environ.get(HELD_ENV) == resolved:
        yield path
        return
    handle = os.open(resolved, os.O_RDWR | os.O_CREAT, 0o644)
    try:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            holders = lock_holders(resolved)
            ancestors = ancestor_pids()
            parent = next((pid for pid in holders if pid in ancestors), None)
            if parent is not None:
                print(f"heavy lock {resolved} is already held by your parent pid "
                      f"{parent}; use `python3 tools/heavy_lock.py run -- <command>` "
                      "or run without a wrapper", file=report, flush=True)
                raise SystemExit(EXIT_HELD_BY_PARENT) from None
            named = ", ".join(str(pid) for pid in holders) or "an unknown process"
            print(f"waiting for {resolved} held by pid {named}", file=report, flush=True)
            fcntl.flock(handle, fcntl.LOCK_EX)
        previous = environ.get(HELD_ENV)
        environ[HELD_ENV] = resolved
        try:
            yield path
        finally:
            if previous is None:
                environ.pop(HELD_ENV, None)
            else:
                environ[HELD_ENV] = previous
    finally:
        os.close(handle)


def _git(cwd, *args):
    """Stripped stdout of a git command run in `cwd`, or "" when it fails."""
    try:
        completed = subprocess.run(["git", *args], cwd=str(cwd), stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, text=True)
    except OSError:
        return ""
    return completed.stdout.strip() if completed.returncode == 0 else ""


def main(argv=None):
    """Print a lock path, or `run -- <cmd>` under the heavy lock (its exit code)."""
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv[:1] == ["run"]:
        command = argv[2:] if argv[1:2] == ["--"] else argv[1:]
        if not command:
            print("usage: python3 tools/heavy_lock.py run -- <command...>", file=sys.stderr)
            return 2
        with hold_heavy_lock():
            return subprocess.run(command).returncode
    parser = argparse.ArgumentParser(
        prog="python3 tools/heavy_lock.py",
        description="Print the repository-wide heavy-run lock path, or "
                    "`run -- <command>` holding it.")
    parser.add_argument("--gate", action="store_true",
                        help="print the gate lock instead of the heavy lock")
    args = parser.parse_args(argv)
    print(gate_lock_path() if args.gate else heavy_lock_path())
    return 0


if __name__ == "__main__":
    sys.exit(main())
