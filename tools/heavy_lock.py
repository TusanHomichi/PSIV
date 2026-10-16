#!/usr/bin/env python3
"""tools/heavy_lock.py - the repository-wide heavy-run lock: its path, classes and queue.

    python3 tools/heavy_lock.py [--gate]          # print the lock path
    python3 tools/heavy_lock.py status            # print holders and waiters
    python3 tools/heavy_lock.py run [--priority] [--class exclusive|godot] -- <cmd...>

Heavy runs are serialized by project rule across every worktree, so their locks
must name one file for the whole repository, not one per checkout. Both locks
live under the main checkout's `build/`, found through
`git rev-parse --git-common-dir`, which a linked worktree and the main checkout
answer identically:

- `heavy_lock_path()`: `<main>/build/continuation-heavy.lock`.
  `PSIV_HEAVY_LOCK` overrides it.
- `gate_lock_path()`: `<main>/build/gate/.lock`, which `tools/gate.py` holds
  for a whole gate run.

Lock order, everywhere: the gate lock first, then the heavy lock. Every
deployed gate takes them in that order, so a process that took the heavy lock
and then waited for the gate lock would deadlock against one of them.

The heavy lock has two classes. `exclusive` (cargo, the Python suite, the gate,
pack builds) holds the lock file with LOCK_EX and runs alone. `godot` (one
Godot/Xvfb run, about 350 MB) holds it with LOCK_SH plus one of
`PSIV_GODOT_SLOTS` (default 3) slot files beside it, so several Godot runs
share the machine and an exclusive holder excludes them all. The cap must be
the same for every process; one that finds a slot beyond its own cap held
refuses (exit 2) rather than overfill the machine.

Waiters queue in `<lock>.queue/`, one file per waiter that the waiter holds
flocked for as long as it waits (a killed waiter's entry is pruned, never
waited for). Every grant is decided under `<lock>.mutex`, and only the head of
the queue may take the lock, so the order is deterministic: priority waiters
(`--priority`, or `PSIV_LOCK_PRIORITY=1` for code that calls
`hold_heavy_lock`) before normal ones, each in arrival order. A holder is never
preempted; the next grant goes to the head, which waits in flock(2) itself, so
a release wakes it together with any raw `flock(1)` waiter instead of that
waiter winning every race against a poll. While waiting, a line names the
holders and how many waiters (and priority waiters) are ahead, again whenever
that changes.

`hold_heavy_lock(path)` is the one way in-repo code takes the heavy lock, and
`run -- <cmd>` the one way to wrap a command in it. The holder exports
`PSIV_HEAVY_LOCK_HELD=<resolved path>` with the class and slot count, so a
descendant that asks for the same lock (the gate under `run`, certify under
verify_candidate) proceeds without blocking on its own ancestor; an exclusive
request under a godot holder is refused instead, because the holder never
excluded anyone. A raw `flock(1)` wrapper sets no marker; a child that finds
the lock held by one of its ancestors (by inode in /proc/locks) exits 2 naming
the parent instead of waiting forever (`raw_flock_parent`).

A directory git cannot describe (a fixture, an exported tree) falls back to its
own `build/`. Stdlib only, Python 3, Linux (/proc).
"""

from __future__ import annotations

import argparse
import fcntl
import os
import re
import subprocess
import sys
import threading
import time
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path

HEAVY_LOCK_ENV = "PSIV_HEAVY_LOCK"
HELD_ENV = "PSIV_HEAVY_LOCK_HELD"
HELD_CLASS_ENV = "PSIV_HEAVY_LOCK_CLASS"
HELD_SLOTS_ENV = "PSIV_HEAVY_LOCK_SLOTS"
PRIORITY_ENV = "PSIV_LOCK_PRIORITY"
GODOT_SLOTS_ENV = "PSIV_GODOT_SLOTS"
HEAVY_LOCK_NAME = "continuation-heavy.lock"
GATE_LOCK = Path("gate") / ".lock"
EXIT_HELD_BY_PARENT = 2

EXCLUSIVE = "exclusive"
GODOT = "godot"
LOCK_CLASSES = (EXCLUSIVE, GODOT)
DEFAULT_GODOT_SLOTS = 3
#: Seconds between a waiter's looks at the queue. A heavy run lasts minutes.
POLL_S = 0.2


@dataclass(frozen=True)
class Hold:
    """What a `hold_heavy_lock` block holds: its class and godot slots (an
    exclusive hold covers every slot), and whether an ancestor holds it."""

    path: Path
    lock_class: str
    slots: int
    nested: bool


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


def godot_slots(environ=None):
    """How many godot-class holders may share the machine: `PSIV_GODOT_SLOTS`, default 3."""
    environ = os.environ if environ is None else environ
    raw = environ.get(GODOT_SLOTS_ENV)
    if not raw:
        return DEFAULT_GODOT_SLOTS
    value = int(raw)
    if value < 1:
        raise ValueError(f"{GODOT_SLOTS_ENV}={raw!r}: need at least one slot")
    return value


def lock_modes(path, proc_locks=Path("/proc/locks")):
    """`[(pid, class)]` holding a lock on `path`'s inode, from /proc/locks:
    a WRITE lock is `exclusive`, a READ lock `godot`.

    Matched by inode alone: on btrfs the device field names the superblock's
    device, not the file's `st_dev`. Waiters (`->` lines) are not holders.
    """
    inode = os.stat(path).st_ino
    found = []
    try:
        lines = Path(proc_locks).read_text().splitlines()
    except OSError:
        return found
    for line in lines:
        fields = line.split()
        if len(fields) < 6 or fields[1] == "->":
            continue
        try:
            pid = int(fields[4])
            line_inode = int(fields[5].rsplit(":", 1)[1])
        except (ValueError, IndexError):
            continue
        mode = EXCLUSIVE if fields[3] == "WRITE" else GODOT
        if line_inode == inode and (pid, mode) not in found:
            found.append((pid, mode))
    return found


def lock_holders(path, proc_locks=Path("/proc/locks")):
    """PIDs holding a lock on `path`'s inode, from /proc/locks (see `lock_modes`)."""
    pids = []
    for pid, _ in lock_modes(path, proc_locks):
        if pid not in pids:
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


def raw_flock_parent(path=None, environ=None):
    """The pid of an ancestor holding the heavy lock `path` without exporting the
    marker (a raw `flock(1)` wrapper), or None.

    A process under such a parent can never take the lock, so an entry point
    refuses at once (exit 2) instead of waiting on itself.
    """
    environ = os.environ if environ is None else environ
    path = Path(path or heavy_lock_path(environ=environ))
    if not path.exists():
        return None
    resolved = str(path.resolve())
    if environ.get(HELD_ENV) == resolved:
        return None
    ancestors = ancestor_pids()
    return next((pid for pid in lock_holders(resolved) if pid in ancestors), None)


def refuse_message(path, parent):
    """What a child of a raw `flock(1)` holder prints before exiting 2."""
    return (f"heavy lock {path} is already held by your parent pid {parent}; "
            "use `python3 tools/heavy_lock.py run -- <command>` or run without a wrapper")


class _Queue:
    """The waiters of one lock: `<lock>.queue/<rank>-<seq>-<pid>-<class>`, each
    held flocked by its waiter, read and changed only under `<lock>.mutex`."""

    def __init__(self, lock):
        self.dir = Path(f"{lock}.queue")
        self.mutex = Path(f"{lock}.mutex")
        self.dir.mkdir(exist_ok=True)

    @contextmanager
    def locked(self):
        handle = os.open(self.mutex, os.O_RDWR | os.O_CREAT, 0o644)
        try:
            fcntl.flock(handle, fcntl.LOCK_EX)
            yield
        finally:
            os.close(handle)

    def entries(self):
        """Live waiters, head first; entries whose waiter died are removed."""
        live = []
        for name in os.listdir(self.dir):
            if not ENTRY_RE.match(name):
                continue  # not a waiter: never ordered, never removed
            try:
                handle = os.open(self.dir / name, os.O_RDWR)
            except FileNotFoundError:
                continue
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                live.append(name)
            else:
                _unlink(self.dir / name)
            finally:
                os.close(handle)
        return sorted(live, key=_entry_key)

    def join(self, priority, lock_class, waiting):
        """Enqueue this process behind `waiting`; returns (name, held handle)."""
        seq = max((_entry_key(name)[1] for name in waiting), default=0) + 1
        name = f"{0 if priority else 1}-{seq:012d}-{os.getpid()}-{lock_class}"
        handle = os.open(self.dir / name, os.O_RDWR | os.O_CREAT | os.O_EXCL, 0o644)
        fcntl.flock(handle, fcntl.LOCK_EX)
        return name, handle

    def leave(self, entry):
        name, handle = entry
        _unlink(self.dir / name)
        os.close(handle)


def _unlink(path):
    """Remove `path`; one that a concurrent scan or its owner removed first is fine."""
    try:
        os.unlink(path)
    except FileNotFoundError:
        pass


#: A waiter's file in `<lock>.queue/`: rank, sequence, pid and class.
ENTRY_RE = re.compile(r"^[01]-\d{12}-\d+-(?:exclusive|godot)$")


def _entry_key(name):
    """(rank, seq) of a queue entry: priority rank 0 first, then arrival."""
    rank, seq = name.split("-")[:2]
    return int(rank), int(seq)


def _entry_pid(name):
    return int(name.split("-")[2])


class SlotCapMismatch(RuntimeError):
    """A godot slot beyond this process's cap is held: another process runs with
    a larger `PSIV_GODOT_SLOTS`, so together they would exceed either cap."""


def _check_slot_cap(lock, cap):
    """Raise SlotCapMismatch when a slot file at an index >= `cap` is held."""
    prefix = Path(f"{lock}.godot")
    for path in prefix.parent.glob(prefix.name + "*"):
        index = path.name[len(prefix.name):]
        if not index.isdecimal() or int(index) < cap:
            continue
        handle = os.open(path, os.O_RDWR)
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise SlotCapMismatch(
                f"heavy lock {lock}: godot slot {index} is held, so another process runs "
                f"with {GODOT_SLOTS_ENV} above this one's {cap}; every process must use "
                "the same value") from None
        finally:
            os.close(handle)


def _take_slots(lock, slots, cap):
    """Handles holding `slots` free godot slot files of `lock`, or None (nothing
    held) when fewer are free. Never blocks. Raises SlotCapMismatch when a
    process with a larger cap holds a slot this one cannot see."""
    _check_slot_cap(lock, cap)
    held = []
    for index in range(cap):
        if len(held) == slots:
            break
        handle = os.open(f"{lock}.godot{index}", os.O_RDWR | os.O_CREAT, 0o644)
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            os.close(handle)
        else:
            held.append(handle)
    if len(held) == slots:
        return held
    for handle in held:
        os.close(handle)
    return None


def _main_mode(lock_class):
    return fcntl.LOCK_EX if lock_class == EXCLUSIVE else fcntl.LOCK_SH


def _try_grant(lock, lock_class, slots, cap):
    """The handles that hold `lock` in `lock_class` with `slots` godot slots, or
    None (nothing held) when it is not free now. Never blocks."""
    main = os.open(lock, os.O_RDWR | os.O_CREAT, 0o644)
    try:
        fcntl.flock(main, _main_mode(lock_class) | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(main)
        return None
    taken = [] if lock_class == EXCLUSIVE else _take_slots(lock, slots, cap)
    if taken is None:
        os.close(main)
        return None
    return [main, *taken]


class _Blocker:
    """A thread blocked in flock(2) on its own handle of the lock file.

    The queue's head waits in the kernel, as a raw `flock(1)` waiter does, so
    it is woken with them when the holder releases instead of losing every
    such race between two polls. The handle is the caller's once `done` is set;
    `abandon()` gives it up, now or as soon as the flock returns.
    """

    def __init__(self, lock, mode):
        self.handle = os.open(lock, os.O_RDWR | os.O_CREAT, 0o644)
        self.done = threading.Event()
        self._guard = threading.Lock()
        self._abandoned = False
        threading.Thread(target=self._wait, args=(mode,), daemon=True).start()

    def _wait(self, mode):
        fcntl.flock(self.handle, mode)
        with self._guard:
            if self._abandoned:
                os.close(self.handle)
            else:
                self.done.set()

    def abandon(self):
        with self._guard:
            if self.done.is_set():
                os.close(self.handle)
            else:
                self._abandoned = True


def _wait_line(lock, lock_class, priority, ahead):
    """One "waiting for" line: the holders with their class, the queue ahead."""
    holders = ", ".join(f"{pid} ({mode})" for pid, mode in lock_modes(lock)
                        if pid != os.getpid()) or "none"  # a godot head holds it shared
    urgent = sum(1 for name in ahead if _entry_key(name)[0] == 0)
    me = f"{lock_class}, priority" if priority else lock_class
    return (f"waiting for {lock} held by pid {holders}; {len(ahead)} waiter(s) ahead, "
            f"{urgent} of them priority; this waiter: {me}")


def _acquire(lock, lock_class, slots, cap, priority, report):
    """Queue for `lock` and return the granting handles once this waiter is the
    head and the lock is free in its class.

    The head blocks in the kernel for the lock file in its class (`_Blocker`);
    a godot head then takes its slots as they free. A waiter displaced from
    the head (a priority waiter arrived) gives the lock file back at once.
    """
    queue = _Queue(lock)
    entry = None
    blocker = None
    shown = None
    try:
        while True:
            with queue.locked():
                waiting = queue.entries()
                if entry is None:
                    if not waiting:
                        held = _try_grant(lock, lock_class, slots, cap)
                        if held:
                            return held
                    parent = raw_flock_parent(lock, environ={})
                    if parent is not None:
                        print(refuse_message(lock, parent), file=report, flush=True)
                        raise SystemExit(EXIT_HELD_BY_PARENT)
                    entry = queue.join(priority, lock_class, waiting)
                    waiting = queue.entries()
                ahead = waiting[:waiting.index(entry[0])]
                if ahead and blocker is not None:
                    blocker.abandon()
                    blocker = None
                elif not ahead:
                    blocker = blocker or _Blocker(lock, _main_mode(lock_class))
                    if blocker.done.is_set():
                        taken = [] if lock_class == EXCLUSIVE else _take_slots(lock, slots,
                                                                               cap)
                        if taken is not None:
                            queue.leave(entry)
                            entry = None
                            held, blocker = [blocker.handle, *taken], None
                            return held
                line = _wait_line(lock, lock_class, priority, ahead)
            if line != shown:
                print(line, file=report, flush=True)
                shown = line
            if blocker is not None and not blocker.done.is_set():
                blocker.done.wait(POLL_S)
            else:
                time.sleep(POLL_S)
    finally:
        if blocker is not None:
            blocker.abandon()
        if entry is not None:
            with queue.locked():  # a scan never sees a half-removed entry
                queue.leave(entry)


@contextmanager
def hold_heavy_lock(path=None, environ=None, report=None, lock_class=EXCLUSIVE, slots=1,
                    priority=None):
    """Hold the heavy lock `path` (default: `heavy_lock_path()`) for the block,
    yielding a `Hold`.

    `lock_class` is `exclusive` or `godot`; a godot hold takes `slots` slots
    (1 to `PSIV_GODOT_SLOTS`). `priority` defaults to `PSIV_LOCK_PRIORITY=1`.
    A no-op when this process already runs under a holder of the same file
    (`PSIV_HEAVY_LOCK_HELD`), yielding that holder's class and slots; an
    exclusive request under a godot holder exits 2. Otherwise it queues, takes
    the lock and exports the markers for the block's subprocesses. If an
    ancestor holds the lock without the marker (a raw `flock(1)` wrapper), it
    prints why and raises SystemExit(2) rather than wait on itself.
    """
    environ = os.environ if environ is None else environ
    report = report or sys.stderr
    if lock_class not in LOCK_CLASSES:
        raise ValueError(f"lock class {lock_class!r}: expected one of {LOCK_CLASSES}")
    path = Path(path or heavy_lock_path(environ=environ))
    path.parent.mkdir(parents=True, exist_ok=True)
    resolved = str(path.resolve())
    cap = godot_slots(environ)
    if environ.get(HELD_ENV) == resolved:
        held_class = environ.get(HELD_CLASS_ENV) or EXCLUSIVE
        if lock_class == EXCLUSIVE and held_class != EXCLUSIVE:
            print(f"heavy lock {resolved}: an exclusive run was started under a godot-class "
                  "holder, which excludes nobody; run it under an exclusive holder",
                  file=report, flush=True)
            raise SystemExit(EXIT_HELD_BY_PARENT)
        held_slots = int(environ.get(HELD_SLOTS_ENV) or cap)
        yield Hold(path, held_class, held_slots, nested=True)
        return
    if lock_class == GODOT and not 1 <= slots <= cap:
        raise ValueError(f"{slots} godot slots requested; {GODOT_SLOTS_ENV} allows 1 to {cap}")
    if priority is None:
        priority = environ.get(PRIORITY_ENV) == "1"
    try:
        held = _acquire(resolved, lock_class, slots, cap, priority, report)
    except SlotCapMismatch as mismatch:
        print(mismatch, file=report, flush=True)
        raise SystemExit(EXIT_HELD_BY_PARENT) from None
    granted = cap if lock_class == EXCLUSIVE else slots
    markers = {HELD_ENV: resolved, HELD_CLASS_ENV: lock_class, HELD_SLOTS_ENV: str(granted)}
    previous = {key: environ.get(key) for key in markers}
    try:
        environ.update(markers)
        yield Hold(path, lock_class, granted, nested=False)
    finally:
        for key, value in previous.items():
            if value is None:
                environ.pop(key, None)
            else:
                environ[key] = value
        for handle in held:
            os.close(handle)


def _cmdline(pid):
    try:
        raw = Path(f"/proc/{pid}/cmdline").read_bytes()
    except OSError:
        return "?"
    return " ".join(part.decode(errors="replace") for part in raw.split(b"\0") if part)


def status(path=None, out=None):
    """Print the lock's holders and waiters, each with its pid and command line."""
    out = out or sys.stdout
    path = Path(path or heavy_lock_path())
    print(f"heavy lock {path} (godot slots: {godot_slots()})", file=out)
    if not path.exists():
        print("holders: none", file=out)
        return
    holders = lock_modes(path)
    print("holders:" + ("" if holders else " none"), file=out)
    for pid, mode in holders:
        print(f"  {pid} {mode}: {_cmdline(pid)}", file=out)
    queue = _Queue(path.resolve())
    with queue.locked():
        waiting = queue.entries()
    print("waiters:" + ("" if waiting else " none"), file=out)
    for name in waiting:
        pid = _entry_pid(name)
        rank = "priority " if _entry_key(name)[0] == 0 else ""
        print(f"  {pid} {rank}{name.split('-', 3)[3]}: {_cmdline(pid)}", file=out)


def _git(cwd, *args):
    """Stripped stdout of a git command run in `cwd`, or "" when it fails."""
    try:
        completed = subprocess.run(["git", *args], cwd=str(cwd), stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, text=True)
    except OSError:
        return ""
    return completed.stdout.strip() if completed.returncode == 0 else ""


def run_parser():
    parser = argparse.ArgumentParser(
        prog="python3 tools/heavy_lock.py run",
        description="Run a command holding the heavy lock.")
    parser.add_argument("--priority", action="store_true",
                        help="go ahead of every normal waiter (speedup lanes, a yield)")
    parser.add_argument("--class", dest="lock_class", choices=LOCK_CLASSES, default=EXCLUSIVE,
                        help="exclusive (default: cargo, the Python suite, the gate, pack "
                             "builds) or godot (one Godot/Xvfb run)")
    return parser


def main(argv=None):
    """Print a lock path or the lock's status, or `run -- <cmd>` under the heavy
    lock (its exit code)."""
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv[:1] == ["run"]:
        options, command = (argv[1:argv.index("--")], argv[argv.index("--") + 1:]) \
            if "--" in argv else ([], argv[1:])
        args = run_parser().parse_args(options)
        if not command:
            print("usage: python3 tools/heavy_lock.py run [--priority] "
                  "[--class exclusive|godot] -- <command...>", file=sys.stderr)
            return 2
        with hold_heavy_lock(lock_class=args.lock_class, priority=args.priority or None):
            return subprocess.run(command).returncode
    if argv[:1] == ["status"]:
        status()
        return 0
    parser = argparse.ArgumentParser(
        prog="python3 tools/heavy_lock.py",
        description="Print the repository-wide heavy-run lock path; `status` for its "
                    "holders and waiters, or `run -- <command>` holding it.")
    parser.add_argument("--gate", action="store_true",
                        help="print the gate lock instead of the heavy lock")
    args = parser.parse_args(argv)
    print(gate_lock_path() if args.gate else heavy_lock_path())
    return 0


if __name__ == "__main__":
    sys.exit(main())
