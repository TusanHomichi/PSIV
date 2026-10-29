#!/usr/bin/env python3
"""tools/heavy_lock.py - the repository-wide heavy-run lock: its path, classes and queue.

    python3 tools/heavy_lock.py [--gate]          # print the lock path
    python3 tools/heavy_lock.py status            # print holders and waiters
    python3 tools/heavy_lock.py run [--priority] --class CLASS -- <cmd...>

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
and then waited for the gate lock would deadlock against one of them. Never
wrap the gate in `run`: the full gate queues first by itself, a priority fast
tier is `PSIV_LOCK_PRIORITY=1 python3 tools/gate.py --fast`, and a gate nested
under a heavy holder refuses a busy gate lock rather than wait.

The heavy lock's classes are split by peak memory (the measured table is in
docs/DEVELOPMENT.md, "Heavy-run lock"). `exclusive` (the gate as a whole and
the opt-level-3 extension build) holds the lock file with LOCK_EX and runs alone. The
shared classes hold it with LOCK_SH plus units of a machine memory budget,
`PSIV_LOCK_UNITS` (default `DEFAULT_LOCK_UNITS`) unit files of `UNIT_MB` each
beside it, taken all or none, `CLASS_UNITS` each: `build` (a cargo build,
test or clippy run that does not compile psiv-godot: compilation is its peak),
`extension` (one that does, in dev: `-p psiv-godot` or the workspace), `godot` (one
Godot/Xvfb run, per slot, plus one of `PSIV_GODOT_SLOTS` (default 3) slot
files), `python` (the Python suite) and `pack` (a pack build, which swaps
the new pack in whole, psiv_tools/pack_swap.py, so its readers need no
exclusion). The caller declares the class and is
trusted: a build in `build` that compiles psiv-godot costs memory headroom,
never a crash, because every Godot a repository tool starts loads its own copy
of the library (tools/godot.py `create_run_root`). So shared holders run side by side
while their units fit, and an exclusive holder excludes them all. The slot cap
and the budget must be the same for every process: each holder writes them
into its slot and unit files, and a process that finds a held file recording
another value refuses (exit 2) rather than overfill the machine.

Waiters queue in `<lock>.queue/`, one file per waiter that the waiter holds
flocked for as long as it waits (a killed waiter's entry is pruned, never
waited for). Every grant is decided under `<lock>.mutex`, and only the head of
the queue may take the lock, so the order is deterministic: a full gate
(`tools/gate.py` without `--fast`, which holds the lock with `gate=True`)
first, then priority waiters (`--priority`, or `PSIV_LOCK_PRIORITY=1` for code
that calls `hold_heavy_lock`), then the rest, each in arrival order. A holder
is never preempted; the next grant goes to the head, which waits in flock(2)
itself, so a release wakes it together with any raw `flock(1)` waiter instead
of that waiter winning every race against a poll. While waiting, a line names
the holders and how many waiters (gates and priority waiters among them) are
ahead, again whenever that changes, and every `WAIT_REPORT_S` while nothing
does, a line says how long it has waited and each holder's class, age and
command line (`wait_reporting` does the same for any flock, such as the gate
lock), so a job runner that stops a job gone quiet sees it waiting, and on
whom.

`hold_heavy_lock(path)` is the one way in-repo code takes the heavy lock, and
`run --class CLASS -- <cmd>` the one way to wrap a command in it. The class is
required: without one, `run` prints the classes and exits 2, so no command
takes the whole machine by default. The holder exports
`PSIV_HEAVY_LOCK_HELD=<resolved path>` with the class and slot count, so a
descendant that asks for the same lock (certify under an exclusive `run`)
proceeds without blocking on its own ancestor; a request under a shared
holder of another class (an exclusive run under godot, a build under python) is
refused instead, because the holder neither excluded anyone nor reserved the
request's units. A raw `flock(1)` wrapper sets no marker; a child that finds
the lock held by one of its ancestors (by inode, from /proc) exits 2 naming
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
UNITS_ENV = "PSIV_LOCK_UNITS"
HEAVY_LOCK_NAME = "continuation-heavy.lock"
GATE_LOCK = Path("gate") / ".lock"
EXIT_HELD_BY_PARENT = 2

EXCLUSIVE = "exclusive"
GODOT = "godot"
BUILD = "build"
EXTENSION = "extension"
PYTHON = "python"
PACK = "pack"
LOCK_CLASSES = (EXCLUSIVE, GODOT, BUILD, EXTENSION, PYTHON, PACK)
#: How /proc/locks shows a LOCK_SH holder that holds no slot or unit yet (a
#: shared head between its flock and its grant).
SHARED = "shared"
DEFAULT_GODOT_SLOTS = 3
#: Megabytes per budget unit, and the budget: 16 units are 8 GB of the 13.5 GB
#: development host. The safety margin is the budget itself: the 5.5 GB it
#: leaves covers the desktop, the agent sessions and unlocked headless runs,
#: and any job that peaks above its measured RSS.
UNIT_MB = 512
DEFAULT_LOCK_UNITS = 16
#: Units each shared holder takes (godot: per slot), one rule for every class:
#: the fewest units that keep at least 15% of the holding above the class's
#: largest measured peak process-tree RSS, ceil(peak / (0.85 * UNIT_MB)), from
#: the table in docs/DEVELOPMENT.md: a cold `cargo test --workspace --exclude
#: psiv-godot --no-run` 2,964 MB (7 units, 17% headroom), a cold `cargo test
#: --workspace --no-run` at 8 jobs 4,979 MB (`extension`: 12 units, 19%), a
#: certify pair 418 MB (1, 18%), the Python suite 926 MB in one process and
#: 1,256 MB on four workers (3 units, 18%), a full pack
#: build 129 MB (1, 75%). The
#: opt-level-3 extension build (5,582 MB, 13 units) is exclusive, so it does
#: not size `extension`.
CLASS_UNITS = {BUILD: 7, EXTENSION: 12, GODOT: 1, PYTHON: 3, PACK: 1}
#: Seconds between a waiter's looks at the queue. A heavy run lasts minutes.
POLL_S = 0.2
#: A queue entry's rank, the lowest served first: a full gate, which a pull
#: request waits on to merge, then a priority waiter, then the rest.
GATE_RANK, PRIORITY_RANK, NORMAL_RANK = 0, 1, 2
#: Seconds between the lines a waiter prints while nothing about its wait
#: changes. A job runner that stops a job whose output has gone quiet must see
#: that it is waiting, and on whom.
WAIT_REPORT_S = 60
#: How much of a holder's command line a waiting line shows.
COMMAND_CHARS = 160


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


def lock_units(environ=None):
    """The machine budget shared holders draw on: `PSIV_LOCK_UNITS` units of
    `UNIT_MB`, default `DEFAULT_LOCK_UNITS`. A budget below one holder of the
    largest shared class could never grant it, so it is refused."""
    environ = os.environ if environ is None else environ
    raw = environ.get(UNITS_ENV)
    if not raw:
        return DEFAULT_LOCK_UNITS
    value = int(raw)
    if value < max(CLASS_UNITS.values()):
        raise ValueError(f"{UNITS_ENV}={raw!r}: need at least {max(CLASS_UNITS.values())} "
                         "units, one holder of the largest shared class")
    return value


def _parse_lock_lines(lines, prefix=""):
    """`[(pid, inode, is_write)]` for the held flocks among /proc lock lines
    (`<prefix>1: FLOCK  ADVISORY  WRITE <pid> <maj>:<min>:<inode> 0 EOF`).

    Matched by inode alone: on btrfs the device field names the superblock's
    device, not the file's `st_dev`. Waiters (`->` lines) are not holders.
    """
    held = []
    for line in lines:
        if not line.startswith(prefix):
            continue
        fields = line[len(prefix):].split()
        if len(fields) < 6 or fields[1] == "->":
            continue
        try:
            held.append((int(fields[4]), int(fields[5].rsplit(":", 1)[1]),
                         fields[3] == "WRITE"))
        except (ValueError, IndexError):
            continue
    return held


def _fdinfo_locks(lock):
    """`[(pid, inode, is_write)]` for every flock held through an fd, of a
    process this user can read, on `lock` or a file named after it.

    The walks over /proc and /proc/<pid>/fd go by pid and fd number, and one
    fd's fdinfo is printed in one read, so a lock held for the whole scan is
    always found. An fd is matched by its link text, which never touches the
    file, so a hung mount elsewhere cannot stall the scan.
    """
    prefix = os.path.realpath(lock)
    held = []
    for pid in os.listdir("/proc"):
        if not pid.isdecimal():
            continue
        try:
            fds = os.listdir(f"/proc/{pid}/fd")
        except OSError:  # gone, or another user's
            continue
        for fd in fds:
            try:
                if not os.readlink(f"/proc/{pid}/fd/{fd}").startswith(prefix):
                    continue
                lines = Path(f"/proc/{pid}/fdinfo/{fd}").read_text().splitlines()
            except OSError:  # closed since the listing
                continue
            held += _parse_lock_lines(lines, "lock:")
    return held


def _held_locks(lock, proc_locks):
    """`[(pid, inode, is_write)]` holding a flock: every one on `lock` and the
    files named after it (`_fdinfo_locks`), plus what `proc_locks` shows.

    /proc/locks alone misses held locks: it is printed a page per read(2),
    each from a fresh walk of the kernel's lock list to the reader's position,
    so a lock released earlier in the list between two reads shifts held ones
    past that position unseen (#204: 11 of 12 units, with other tests taking
    and dropping locks). It still names holders whose fds this user cannot
    read; a lock released mid-scan may appear in one source and not the other.
    """
    try:
        lines = Path(proc_locks).read_text().splitlines()
    except OSError:
        lines = []
    return list(dict.fromkeys(_fdinfo_locks(lock) + _parse_lock_lines(lines)))


def _numbered(lock, kind):
    """`{inode: path}` of `<lock>.<kind><index>` files (kind `godot` or `unit`)."""
    prefix = Path(f"{lock}.{kind}")
    found = {}
    for path in prefix.parent.glob(prefix.name + "*"):
        if path.name[len(prefix.name):].isdecimal():
            try:
                found[path.stat().st_ino] = path
            except FileNotFoundError:
                continue
    return found


def _recorded(path):
    """`(cap, class)` a slot or unit file records (`<cap>` or `<cap> <class>`);
    None for a field it does not hold."""
    try:
        fields = path.read_text(errors="replace").split()
    except OSError:
        return None, None
    cap = int(fields[0]) if fields and fields[0].isdecimal() else None
    return cap, fields[1] if len(fields) > 1 else None


def lock_modes(path, proc_locks=Path("/proc/locks")):
    """`[(pid, class)]` holding a lock on `path`'s inode (see `_held_locks`): a
    WRITE lock is `exclusive`; a READ lock is `godot` when the pid holds a
    godot slot, else the class its budget units record, else `shared`."""
    inode = os.stat(path).st_ino
    held = _held_locks(path, proc_locks)
    slots, units = _numbered(path, GODOT), _numbered(path, "unit")
    in_slots = {pid for pid, node, _ in held if node in slots}
    unit_class = {pid: _recorded(units[node])[1] or SHARED
                  for pid, node, _ in held if node in units}
    found = []
    for pid, node, write in held:
        if node != inode:
            continue
        mode = EXCLUSIVE if write else GODOT if pid in in_slots \
            else unit_class.get(pid, SHARED)
        if (pid, mode) not in found:
            found.append((pid, mode))
    return found


def units_in_use(path, proc_locks=Path("/proc/locks")):
    """How many of `path`'s budget unit files are held now (see `_held_locks`)."""
    units = _numbered(path, "unit")
    return len({node for _, node, _ in _held_locks(path, proc_locks) if node in units})


def lock_holders(path, proc_locks=Path("/proc/locks")):
    """PIDs holding a lock on `path`'s inode (see `lock_modes`)."""
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
            "use `python3 tools/heavy_lock.py run --class CLASS -- <command>` or run "
            "without a wrapper")


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

    def join(self, rank, lock_class, waiting):
        """Enqueue this process at `rank` behind `waiting`; returns (name, held handle)."""
        seq = max((_entry_key(name)[1] for name in waiting), default=0) + 1
        name = f"{rank}-{seq:012d}-{os.getpid()}-{lock_class}"
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
ENTRY_RE = re.compile(rf"^[{GATE_RANK}-{NORMAL_RANK}]-\d{{12}}-\d+-(?:"
                      + "|".join(LOCK_CLASSES) + ")$")


def _entry_key(name):
    """(rank, seq) of a queue entry: the lowest rank first, then arrival."""
    rank, seq = name.split("-")[:2]
    return int(rank), int(seq)


def _entry_pid(name):
    return int(name.split("-")[2])


class SlotCapMismatch(RuntimeError):
    """A godot slot or budget unit is held by a process with a different
    `PSIV_GODOT_SLOTS` or `PSIV_LOCK_UNITS`, so together they could exceed
    either cap."""


@dataclass(frozen=True)
class _Need:
    """What one grant takes beside the lock file: `slots` godot slot files out
    of `cap`, and `units` budget unit files out of `budget`."""

    lock_class: str
    slots: int
    cap: int
    units: int
    budget: int

    @classmethod
    def of(cls, lock_class, slots, cap, budget):
        if lock_class == EXCLUSIVE:
            return cls(lock_class, 0, cap, 0, budget)
        slots = slots if lock_class == GODOT else 0
        units = CLASS_UNITS[lock_class] * (slots if lock_class == GODOT else 1)
        return cls(lock_class, slots, cap, units, budget)

    def take(self, lock):
        """Handles holding every slot and unit, or None (nothing held) when
        they are not all free now. Never blocks; may raise SlotCapMismatch."""
        slots = _take_files(lock, GODOT, self.slots, self.cap, GODOT_SLOTS_ENV)
        if slots is None:
            return None
        units = None
        try:
            units = _take_files(lock, "unit", self.units, self.budget, UNITS_ENV,
                                self.lock_class)
        finally:
            if units is None:  # no units, or a mismatch raised: give the slots back
                for handle in slots:
                    os.close(handle)
        return None if units is None else slots + units


def _check_cap(lock, kind, cap, env):
    """Raise SlotCapMismatch when a held `<lock>.<kind>N` file records a cap
    other than `cap` (each holder writes its cap into the files it takes), or
    sits at an index >= `cap`."""
    prefix = Path(f"{lock}.{kind}")
    for path in prefix.parent.glob(prefix.name + "*"):
        index = path.name[len(prefix.name):]
        if not index.isdecimal():
            continue
        try:
            handle = os.open(path, os.O_RDWR)
        except FileNotFoundError:
            continue
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            fields = os.pread(handle, 64, 0).decode(errors="replace").split()
            recorded = fields[0] if fields else ""
            if int(index) >= cap or (recorded.isdecimal() and int(recorded) != cap):
                theirs = recorded if recorded.isdecimal() else f"above {cap}"
                what = "godot slot" if kind == GODOT else "budget unit"
                raise SlotCapMismatch(
                    f"heavy lock {lock}: {what} {index} is held by a process with "
                    f"{env}={theirs}, this one has {cap}; every process must "
                    "use the same value") from None
        finally:
            os.close(handle)


def _take_files(lock, kind, count, cap, env, label=None):
    """Handles holding `count` free `<lock>.<kind>N` files (N below `cap`), each
    recording `cap` (and the holder's class `label`, for `status`), or None
    (nothing held) when fewer are free: all or none. Never blocks. Raises
    SlotCapMismatch when a held file records another cap."""
    if count == 0:
        return []
    _check_cap(lock, kind, cap, env)
    held = []
    for index in range(cap):
        if len(held) == count:
            break
        handle = os.open(f"{lock}.{kind}{index}", os.O_RDWR | os.O_CREAT, 0o644)
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            os.close(handle)
        else:
            os.ftruncate(handle, 0)
            os.pwrite(handle, (f"{cap} {label}" if label else str(cap)).encode(), 0)
            held.append(handle)
    if len(held) == count:
        return held
    for handle in held:
        os.close(handle)
    return None


def _main_mode(lock_class):
    return fcntl.LOCK_EX if lock_class == EXCLUSIVE else fcntl.LOCK_SH


def _try_grant(lock, need):
    """The handles that hold `lock` in `need`'s class with its slots and units,
    or None (nothing held) when that is not free now. Never blocks."""
    main = os.open(lock, os.O_RDWR | os.O_CREAT, 0o644)
    try:
        fcntl.flock(main, _main_mode(need.lock_class) | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(main)
        return None
    taken = None
    try:
        taken = need.take(lock)
    finally:
        if taken is None:  # not free, or a mismatch raised: give the lock file back
            os.close(main)
    return None if taken is None else [main, *taken]


class _Blocker:
    """A thread blocked in flock(2) on its own handle of the lock file.

    The queue's head waits in the kernel, as a raw `flock(1)` waiter does, so
    it is woken with them when the holder releases instead of losing every
    such race between two polls. `done` is set when the flock returns or fails;
    `grant()` then hands over the handle, or raises the flock's error with the
    handle already closed. `abandon()` gives the handle up, now or as soon as
    the flock returns.
    """

    def __init__(self, lock, mode):
        self.handle = os.open(lock, os.O_RDWR | os.O_CREAT, 0o644)
        self.done = threading.Event()
        self._guard = threading.Lock()
        self._abandoned = False
        self._error = None
        threading.Thread(target=self._wait, args=(mode,), daemon=True).start()

    def _wait(self, mode):
        try:
            fcntl.flock(self.handle, mode)
        except OSError as exc:  # ENOLCK, say: the waiter must hear it, not wait forever
            error = exc
        else:
            error = None
        with self._guard:
            self._error = error
            if self._abandoned or error is not None:
                os.close(self.handle)
            if not self._abandoned:
                self.done.set()

    def grant(self):
        """The handle holding the lock, once `done` is set; raises the flock's
        error instead when it failed."""
        if self._error is not None:
            raise self._error
        return self.handle

    def abandon(self):
        with self._guard:
            if not self.done.is_set():
                self._abandoned = True
            elif self._error is None:
                os.close(self.handle)


def wait_reporting(lock, mode, report):
    """Block until this process holds `lock` in flock `mode`, waiting in the
    kernel (`_Blocker`), and print `_still_line` every `WAIT_REPORT_S` until
    then. Returns the holding handle, which the caller closes; raises when the
    flock fails."""
    began = time.monotonic()
    blocker = _Blocker(lock, mode)
    try:
        while not blocker.done.wait(WAIT_REPORT_S):
            print(_still_line(lock, time.monotonic() - began), file=report, flush=True)
    except BaseException:
        blocker.abandon()
        raise
    return blocker.grant()


_RANK_NAMES = {GATE_RANK: "gate", PRIORITY_RANK: "priority"}


def _wait_line(lock, lock_class, rank, ahead):
    """One "waiting for" line: the holders with their class, the queue ahead."""
    holders = ", ".join(f"{pid} ({mode})" for pid, mode in lock_modes(lock)
                        if pid != os.getpid()) or "none"  # a godot head holds it shared
    urgent = sum(1 for name in ahead if _entry_key(name)[0] == PRIORITY_RANK)
    gates = sum(1 for name in ahead if _entry_key(name)[0] == GATE_RANK)
    me = ", ".join([lock_class, *([_RANK_NAMES[rank]] if rank in _RANK_NAMES else [])])
    return (f"waiting for {lock} held by pid {holders}; {len(ahead)} waiter(s) ahead, "
            f"{urgent} of them priority, {gates} gate(s); this waiter: {me}")


def _still_line(lock, waited):
    """The line a waiter repeats while its wait does not change: how long it has
    waited, and each holder's class, age and command line (on one line)."""
    holders = "; ".join(
        f"pid {pid} ({mode}, up {_duration(_process_age(pid))}): "
        f"{' '.join(_cmdline(pid).split())[:COMMAND_CHARS]}"
        for pid, mode in lock_modes(lock) if pid != os.getpid())
    return f"still waiting for {lock} after {_duration(waited)}; held by {holders or 'nobody'}"


def _process_age(pid):
    """Seconds since `pid` started (/proc/<pid>/stat field 22), or None."""
    try:
        start = int(Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[19])
        uptime = float(Path("/proc/uptime").read_text().split()[0])
    except (OSError, ValueError, IndexError):
        return None
    return max(0.0, uptime - start / os.sysconf("SC_CLK_TCK"))


def _duration(seconds):
    """`seconds` for a person: `40 s`, `12 min`, `2 h 05 min`; `?` for None."""
    if seconds is None:
        return "?"
    if seconds < 60:
        return f"{seconds:.0f} s"
    minutes = int(seconds // 60)
    return f"{minutes} min" if minutes < 60 else f"{minutes // 60} h {minutes % 60:02d} min"


def _acquire(lock, need, rank, report):
    """Queue for `lock` and return the granting handles once this waiter is the
    head and the lock is free in its class.

    The head blocks in the kernel for the lock file in its class (`_Blocker`);
    a shared head then takes its slots and units as they free. A waiter
    displaced from the head (a priority waiter arrived) gives the lock file
    back at once. Prints `_wait_line` whenever it changes, and `_still_line`
    after every `WAIT_REPORT_S` it has not.
    """
    lock_class = need.lock_class
    queue = _Queue(lock)
    entry = None
    blocker = None
    shown = None
    began = reported = time.monotonic()
    try:
        while True:
            with queue.locked():
                waiting = queue.entries()
                if entry is None:
                    if not waiting:
                        held = _try_grant(lock, need)
                        if held:
                            return held
                    parent = raw_flock_parent(lock, environ={})
                    if parent is not None:
                        print(refuse_message(lock, parent), file=report, flush=True)
                        raise SystemExit(EXIT_HELD_BY_PARENT)
                    entry = queue.join(rank, lock_class, waiting)
                    waiting = queue.entries()
                ahead = waiting[:waiting.index(entry[0])]
                if ahead and blocker is not None:
                    blocker.abandon()
                    blocker = None
                elif not ahead:
                    blocker = blocker or _Blocker(lock, _main_mode(lock_class))
                    if blocker.done.is_set():
                        main = blocker.grant()
                        taken = need.take(lock)
                        if taken is not None:
                            queue.leave(entry)
                            entry = None
                            held, blocker = [main, *taken], None
                            return held
                line = _wait_line(lock, lock_class, rank, ahead)
            now = time.monotonic()
            if line != shown:
                print(line, file=report, flush=True)
                shown, reported = line, now
            elif now - reported >= WAIT_REPORT_S:
                print(_still_line(lock, now - began), file=report, flush=True)
                reported = now
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
                    priority=None, gate=False):
    """Hold the heavy lock `path` (default: `heavy_lock_path()`) for the block,
    yielding a `Hold`.

    `lock_class` is `exclusive`, `godot` or `build`; a godot hold takes `slots`
    slots (1 to `PSIV_GODOT_SLOTS`) and their units, a build hold its units.
    `gate` (a full gate) queues ahead of every other waiter; otherwise
    `priority`, default `PSIV_LOCK_PRIORITY=1`, ahead of the rest. A no-op when this process
    already runs under a holder of the same file (`PSIV_HEAVY_LOCK_HELD`) whose
    class is exclusive or the same, yielding that holder's class and slots; a
    request under a shared holder of another class exits 2. Otherwise it queues, takes
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
    budget = lock_units(environ)
    if environ.get(HELD_ENV) == resolved:
        held_class = environ.get(HELD_CLASS_ENV) or EXCLUSIVE
        if held_class not in (EXCLUSIVE, lock_class):
            article = "an" if lock_class == EXCLUSIVE else "a"
            print(f"heavy lock {resolved}: {article} {lock_class} run was started under a "
                  f"{held_class}-class holder, which excludes nobody and reserved no units "
                  "for it; run it under an exclusive holder or on its own",
                  file=report, flush=True)
            raise SystemExit(EXIT_HELD_BY_PARENT)
        held_slots = int(environ.get(HELD_SLOTS_ENV) or cap)
        yield Hold(path, held_class, held_slots, nested=True)
        return
    if lock_class == GODOT and not 1 <= slots <= cap:
        raise ValueError(f"{slots} godot slots requested; {GODOT_SLOTS_ENV} allows 1 to {cap}")
    need = _Need.of(lock_class, slots, cap, budget)
    if need.units > budget:
        raise ValueError(f"{lock_class} with {slots} slot(s) needs {need.units} units; "
                         f"{UNITS_ENV} is {budget}")
    if priority is None:
        priority = environ.get(PRIORITY_ENV) == "1"
    rank = GATE_RANK if gate else PRIORITY_RANK if priority else NORMAL_RANK
    try:
        held = _acquire(resolved, need, rank, report)
    except SlotCapMismatch as mismatch:
        print(mismatch, file=report, flush=True)
        raise SystemExit(EXIT_HELD_BY_PARENT) from None
    granted = {EXCLUSIVE: cap, GODOT: slots}.get(lock_class, 0)
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
    budget = lock_units()
    print(f"heavy lock {path} (godot slots: {godot_slots()}; budget: {budget} units of "
          f"{UNIT_MB} MB; units per holder: "
          + ", ".join(f"{name} {units}" for name, units in CLASS_UNITS.items()) + ")",
          file=out)
    if not path.exists():
        print("holders: none", file=out)
        return
    print(f"units in use: {units_in_use(path)} of {budget}", file=out)
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
        rank = _RANK_NAMES.get(_entry_key(name)[0])
        print(f"  {pid} {rank + ' ' if rank else ''}{name.split('-', 3)[3]}: {_cmdline(pid)}",
              file=out)


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
    parser.add_argument("--class", dest="lock_class", choices=LOCK_CLASSES,
                        help="required; see CLASS_HELP")
    return parser


#: What `run` prints when no class is given, one line per class.
CLASS_HELP = {
    EXCLUSIVE: "the whole machine: the opt-level-3 extension build",
    BUILD: "a cargo build, test or clippy run that does not compile psiv-godot",
    EXTENSION: "a cargo build, test or clippy run that compiles psiv-godot (or the workspace)",
    GODOT: "one Godot/Xvfb run",
    PYTHON: "the Python suite, or part of it",
    PACK: "a pack build (`python3 -m psiv_tools pack`)",
}


def class_usage():
    """`run`'s usage line and one line per class with its units."""
    lines = ["usage: python3 tools/heavy_lock.py run [--priority] --class CLASS "
             "-- <command...>", "classes (docs/DEVELOPMENT.md, \"Heavy-run lock\"):"]
    for name in LOCK_CLASSES:
        units = "every unit" if name == EXCLUSIVE else f"{CLASS_UNITS[name]} unit(s)"
        lines.append(f"  {name:<9} {units:<12} {CLASS_HELP[name]}")
    return "\n".join(lines)


def main(argv=None):
    """Print a lock path or the lock's status, or `run --class CLASS -- <cmd>`
    under the heavy lock (its exit code)."""
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv[:1] == ["run"]:
        options, command = (argv[1:argv.index("--")], argv[argv.index("--") + 1:]) \
            if "--" in argv else ([], argv[1:])
        args = run_parser().parse_args(options)
        if not command or args.lock_class is None:
            print(class_usage(), file=sys.stderr)
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
