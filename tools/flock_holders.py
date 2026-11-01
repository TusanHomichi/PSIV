"""Who holds a flock on a lock file, or on the files named after it, read from
/proc: the holder side of tools/heavy_lock.py (its waiting lines, `status` and
the units and slots in use) and of the gate's lock.

Stdlib only, Python 3, Linux (/proc).
"""

from __future__ import annotations

import os
from pathlib import Path


def parse_lock_lines(lines, prefix=""):
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


def held_locks(lock, proc_locks):
    """`[(pid, inode, is_write)]` holding a flock: every one on `lock` and the
    files named after it (`fdinfo_locks`), plus what `proc_locks` shows.

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
    return list(dict.fromkeys(fdinfo_locks(lock) + parse_lock_lines(lines)))
