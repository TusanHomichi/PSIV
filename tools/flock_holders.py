"""Who holds a flock on a lock file, or on the files named after it, read from
/proc: the holder side of tools/heavy_lock.py (its waiting lines, `status`, the
units and slots in use and its raw flock(1) parent check) and of the gate's
lock, with the /proc facts about a process they show (its ancestors, age and
command line).

Stdlib only, Python 3, Linux (/proc).
"""

from __future__ import annotations

import os
import re
from pathlib import Path

#: Every file lock on the machine, as the kernel prints them.
PROC_LOCKS = Path("/proc/locks")
#: When `held_locks` reads it: always; only when fdinfo shows nobody else
#: holding the lock file; never.
ALWAYS, WHEN_UNSEEN, NEVER = "always", "when unseen", "never"


def parse_lock_lines(lines, prefix="", devices=None):
    """`[(pid, inode, is_write)]` for the held flocks among /proc lock lines
    (`<prefix>1: FLOCK  ADVISORY  WRITE <pid> <maj>:<min>:<inode> 0 EOF`).

    An inode number is unique only within one file system: given `devices`
    (`lock_devices`), a line on any other device is left out, since the same
    number there is another file. Waiters (`->` lines) are not holders.
    """
    held = []
    for line in lines:
        if not line.startswith(prefix):
            continue
        fields = line[len(prefix):].split()
        if len(fields) < 6 or fields[1] == "->":
            continue
        try:
            major, minor, inode = fields[5].split(":")
            if devices is not None and (int(major, 16), int(minor, 16)) not in devices:
                continue
            held.append((int(fields[4]), int(inode), fields[3] == "WRITE"))
        except ValueError:
            continue
    return held


def _unescape(field):
    """A mountinfo path field, as bytes, with the kernel's octal escapes undone
    (`\\040` for a space; it escapes space, tab, newline and backslash)."""
    return re.sub(rb"\\([0-7]{3})", lambda match: bytes([int(match.group(1), 8) & 0xff]), field)


def lock_devices(lock, mountinfo=Path("/proc/self/mountinfo")):
    """The `(major, minor)` devices a /proc lock line can name for `lock` and
    the files named after it: their directory's `st_dev`, and the device of the
    mount that holds it, which is the one the kernel prints (its superblock's;
    on btrfs a subvolume's `st_dev` is another). Empty when the directory does
    not exist: nothing in it is locked.
    """
    directory = os.path.realpath(os.path.dirname(os.path.abspath(lock)))
    try:
        device = os.stat(directory).st_dev
    except OSError:
        return set()
    devices = {(os.major(device), os.minor(device))}
    # Bytes throughout: a mount point is any pathname, printed raw except for
    # the four escaped characters, so another mount's name need not be UTF-8.
    # Fields are split on the kernel's one delimiter, a space, and lines on
    # newline: other whitespace in a name is printed as is.
    directory = os.fsencode(directory)
    try:
        lines = Path(mountinfo).read_bytes().split(b"\n")
    except OSError:
        lines = []
    point, mounted = b"", None
    for line in lines:  # the longest mount point above it; a later mount over an earlier one
        fields = line.split(b" ")
        if len(fields) < 5:
            continue
        candidate = _unescape(fields[4])
        if (directory == candidate or directory.startswith(candidate.rstrip(b"/") + b"/")) \
                and len(candidate) >= len(point):
            point, mounted = candidate, fields[2]
    if mounted is not None:
        major, _, minor = mounted.partition(b":")
        if major.isdigit() and minor.isdigit():
            devices.add((int(major), int(minor)))
    return devices


def fdinfo_locks(lock):
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
            held += parse_lock_lines(lines, "lock:")
    return held


def held_here(lock):
    """Whether this process holds a flock on `lock`'s file through a descriptor
    of its own, under any name: a lock record in its fdinfo on the file's
    inode and one of its devices (`lock_devices`), which a hard link shares.
    The record names the locked file itself, so a descriptor closed and reused
    meanwhile cannot vouch for another file's lock; and only /proc is read,
    never a descriptor's own file, which a hung mount could stall. Never reads
    /proc/locks either."""
    try:
        inode = os.stat(lock).st_ino
    except OSError:
        return False
    devices = lock_devices(lock)
    for fd in os.listdir("/proc/self/fdinfo"):
        try:
            lines = Path(f"/proc/self/fdinfo/{fd}").read_text().splitlines()
        except OSError:  # closed since the listing, or the listing's own
            continue
        if any(node == inode for _, node, _ in parse_lock_lines(lines, "lock:", devices)):
            return True
    return False


def held_locks(lock, proc_locks=None, read=ALWAYS):
    """`[(pid, inode, is_write)]` holding a flock: every one on `lock` and the
    files named after it (`fdinfo_locks`), plus what `proc_locks` (default
    `PROC_LOCKS`) shows on their file system (`lock_devices`), as `read` says
    (ALWAYS, WHEN_UNSEEN or NEVER).

    /proc/locks alone misses held locks: it is printed a page per read(2),
    each from a fresh walk of the kernel's lock list to the reader's position,
    so a lock released earlier in the list between two reads shifts held ones
    past that position unseen (#204: 11 of 12 units, with other tests taking
    and dropping locks). It still names holders whose fds this user cannot
    read; a lock released mid-scan may appear in one source and not the other.
    It lists every file system's locks, so a line is matched by its device and
    inode: by inode alone a lock on another dataset whose inode number equals a
    unit file's counted as that unit held (#380: 5 units for 3).

    Reading it takes time that grows faster than the machine's lock count,
    since each page is printed from a fresh walk of the list: 3 s a read while
    another job held 60,000 locks on another file system (#383), longer with
    several readers at once. A waiter names its holders again and again, so it
    passes NEVER for the line it prints on every change, and WHEN_UNSEEN for
    the line it repeats: that reads it only when fdinfo shows nobody else
    holding the lock file, as when the holder is another user's process. Its
    raw flock(1) parent check reads it ALWAYS, since a holder fdinfo sees says
    nothing about a parent it can't see, but beside the wait rather than in its
    way (`heavy_lock._ParentCheck`).
    """
    held = fdinfo_locks(lock)
    if read == NEVER:
        return held
    if read == WHEN_UNSEEN:
        try:
            inode = os.stat(lock).st_ino
        except OSError:
            inode = None
        if any(node == inode and pid != os.getpid() for pid, node, _ in held):
            return held
    try:
        lines = Path(PROC_LOCKS if proc_locks is None else proc_locks).read_text().splitlines()
    except OSError:
        lines = []
    return list(dict.fromkeys(held + parse_lock_lines(lines, devices=lock_devices(lock))))


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


def process_age(pid):
    """Seconds since `pid` started (/proc/<pid>/stat field 22), or None."""
    try:
        start = int(Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[19])
        uptime = float(Path("/proc/uptime").read_text().split()[0])
    except (OSError, ValueError, IndexError):
        return None
    return max(0.0, uptime - start / os.sysconf("SC_CLK_TCK"))


def cmdline(pid):
    """`pid`'s command line, its arguments joined by spaces, or `?`."""
    try:
        raw = Path(f"/proc/{pid}/cmdline").read_bytes()
    except OSError:
        return "?"
    return " ".join(part.decode(errors="replace") for part in raw.split(b"\0") if part)
