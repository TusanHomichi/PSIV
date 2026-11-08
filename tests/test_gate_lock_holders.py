"""tools/flock_holders.py, as the gate names its lock's holders: a /proc/locks
line counts only on the lock's own file system.

    PYTHONPATH=. python3 -m unittest tests.test_gate_lock_holders -v

/proc/locks lists every file system's locks, and an inode number is unique
only within one (#380: a lock on another ZFS dataset read as a held heavy-lock
unit, 5 units for 3). `HolderCase` holds a gate lock in this process and reads
its holders (`tools/gate.py` names them when it refuses) from a copy of
/proc/locks with one line added; `DeviceCase` reads `lock_devices` against a
mountinfo written here. Hermetic: temp files only, so the flake's gate-tests
check runs it. Each case has a negative control.
"""

import fcntl
import os
import tempfile
import unittest
from pathlib import Path

from tools import heavy_lock as heavy_lock_module
from tools.heavy_lock import lock_holders

#: The module heavy_lock reads holders through (it imports `tools/` by path).
flock_holders = heavy_lock_module.flock_holders

#: A device no lock here is on (each case checks), and a pid no process here has.
FOREIGN = (0xfe, 0xfe)
OTHER_PID = 4194000


def device_of(path):
    device = os.stat(path).st_dev
    return os.major(device), os.minor(device)


class HolderCase(unittest.TestCase):
    """The gate's lock, held by this process."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.lock = Path(tmp.name).resolve() / "build" / "gate" / ".lock"
        self.lock.parent.mkdir(parents=True)
        handle = os.open(self.lock, os.O_RDWR | os.O_CREAT, 0o644)
        self.addCleanup(os.close, handle)
        fcntl.flock(handle, fcntl.LOCK_EX)

    def holders_with_a_line_on(self, device):
        """`lock_holders` from /proc/locks plus one WRITE lock by OTHER_PID on
        the lock's inode number, on `device`."""
        line = (f"99: FLOCK  ADVISORY  WRITE {OTHER_PID} "
                f"{device[0]:02x}:{device[1]:02x}:{os.stat(self.lock).st_ino} 0 EOF")
        proc_locks = self.lock.parent.parent / "proc-locks"
        proc_locks.write_text(Path("/proc/locks").read_text() + line + "\n")
        return lock_holders(self.lock, proc_locks)

    def test_a_lock_on_another_file_system_with_its_inode_number_is_no_holder(self):
        self.assertNotEqual(FOREIGN, device_of(self.lock.parent))
        self.assertEqual(self.holders_with_a_line_on(FOREIGN), [os.getpid()])

    def test_negative_control_the_same_line_on_the_locks_file_system_is_a_holder(self):
        self.assertEqual(self.holders_with_a_line_on(device_of(self.lock.parent)),
                         [os.getpid(), OTHER_PID])


class DeviceCase(unittest.TestCase):
    """`flock_holders.lock_devices`: the devices a /proc lock line names for a
    lock directory, its `st_dev` and its mount's, read from a mountinfo file
    written here (on btrfs the kernel prints the mount's, not a subvolume's
    `st_dev`)."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.base = Path(tmp.name).resolve()
        self.lockdir = self.base / "sub vol" / "build"
        self.lockdir.mkdir(parents=True)
        device = os.stat(self.lockdir).st_dev
        self.own = (os.major(device), os.minor(device))

    def devices(self, *mounts):
        """`lock_devices` under a mountinfo of `(device, mount point)` lines, in order."""
        mountinfo = self.base / "mountinfo"
        mountinfo.write_text("".join(
            f"{index} 1 {device} / {point.replace(' ', chr(92) + '040')} rw - zfs pool rw\n"
            for index, (device, point) in enumerate(mounts, 30)))
        return flock_holders.lock_devices(self.lockdir / "heavy.lock", mountinfo)

    def test_the_longest_mount_point_above_the_directory_names_the_device(self):
        found = self.devices(("0:1", "/"), ("0:2", str(self.base / "sub vol")),
                             ("0:3", str(self.base / "sub")), ("0:4", str(self.lockdir / "x")))
        self.assertEqual(found, {self.own, (0, 2)})

    def test_negative_control_a_string_prefix_is_not_a_mount_above_it(self):
        """`<base>/sub` is a prefix of `<base>/sub vol`'s text, not of its path."""
        self.assertEqual(self.devices(("0:1", "/"), ("0:3", str(self.base / "sub"))),
                         {self.own, (0, 1)})

    def test_a_later_mount_over_the_same_point_names_the_device(self):
        point = str(self.base / "sub vol")
        self.assertEqual(self.devices(("0:2", point), ("0:5", point)), {self.own, (0, 5)})

    def test_a_line_on_the_mounts_device_is_held_and_one_elsewhere_is_not(self):
        devices = self.devices(("0:2", str(self.base / "sub vol")))
        lines = ["1: FLOCK  ADVISORY  WRITE 10 00:02:77 0 EOF",
                 "2: FLOCK  ADVISORY  READ 11 00:09:77 0 EOF",
                 "3: FLOCK  ADVISORY  READ 12 00:02:78 0 EOF"]
        self.assertEqual(flock_holders.parse_lock_lines(lines, devices=devices),
                         [(10, 77, True), (12, 78, False)])
        self.assertEqual(len(flock_holders.parse_lock_lines(lines)), 3, "no devices: every line")

    def mountinfo_bytes(self, *lines):
        """`lock_devices` for the lock directory under a mountinfo of raw lines."""
        mountinfo = self.base / "mountinfo"
        mountinfo.write_bytes(b"".join(line + b"\n" for line in lines))
        return flock_holders.lock_devices(self.lockdir / "heavy.lock", mountinfo)

    def raw_line(self, device, point):
        """A mountinfo line mounting `point` (bytes, already escaped) from `device`."""
        return b"30 1 " + device + b" / " + point + b" rw - zfs pool rw"

    def test_an_unrelated_mount_name_that_is_not_utf8_is_read_past(self):
        """#381: the kernel prints a name's bytes raw; byte 0xff is no UTF-8."""
        found = self.mountinfo_bytes(self.raw_line(b"0:7", b"/media/\xff\xfename"),
                                     self.raw_line(b"0:2", os.fsencode(self.base) + b"/sub\\040vol"),
                                     self.raw_line(b"0:8", b"/mnt/\xff"))
        self.assertEqual(found, {self.own, (0, 2)})

    def test_a_lock_directory_whose_name_is_not_utf8_is_matched(self):
        try:
            lockdir = Path(os.fsdecode(os.fsencode(self.base) + b"/vol\xff"))
            (lockdir / "build").mkdir(parents=True)
        except (OSError, UnicodeError) as error:  # a file system that takes UTF-8 names only
            self.skipTest(f"no non-UTF-8 directory here: {error}")
        mountinfo = self.base / "mountinfo"
        mountinfo.write_bytes(self.raw_line(b"0:2", os.fsencode(lockdir)) + b"\n")
        self.assertEqual(flock_holders.lock_devices(lockdir / "build" / "heavy.lock", mountinfo),
                         {self.own, (0, 2)})

    def test_whitespace_the_kernel_does_not_escape_stays_in_the_name(self):
        """Fields are split on a space only: a form feed or vertical tab in a name
        is printed raw, and splitting on any whitespace would cut the name short."""
        lockdir = self.base / "ff\x0cvt\x0bdir"
        (lockdir / "build").mkdir(parents=True)
        mountinfo = self.base / "mountinfo"
        mountinfo.write_bytes(self.raw_line(b"0:1", b"/") + b"\n"
                              + self.raw_line(b"0:2", os.fsencode(lockdir)) + b"\n")
        self.assertEqual(flock_holders.lock_devices(lockdir / "build" / "heavy.lock", mountinfo),
                         {self.own, (0, 2)})

    def test_negative_control_a_raw_form_feed_name_is_not_the_directory(self):
        """`<base>/sub\\fvol` (raw form feed) is another path than `<base>/sub vol`."""
        found = self.mountinfo_bytes(self.raw_line(b"0:1", b"/"),
                                     self.raw_line(b"0:9", os.fsencode(self.base) + b"/sub\x0cvol"))
        self.assertEqual(found, {self.own, (0, 1)})

    def test_escaped_tab_newline_and_backslash_are_decoded(self):
        """The kernel escapes space, tab, newline and backslash as octal; an escaped
        backslash before digits is a backslash, then the digits."""
        try:
            lockdir = self.base / "a\tb\nc\\040d"
            (lockdir / "build").mkdir(parents=True)
        except OSError as error:
            self.skipTest(f"no such directory here: {error}")
        mountinfo = self.base / "mountinfo"
        mountinfo.write_bytes(
            self.raw_line(b"0:1", b"/") + b"\n"
            + self.raw_line(b"0:4", os.fsencode(self.base) + b"/a\\011b\\012c\\134040d") + b"\n"
            + self.raw_line(b"0:6", os.fsencode(self.base) + b"/a\\011b\\012c d") + b"\n")
        self.assertEqual(flock_holders.lock_devices(lockdir / "build" / "heavy.lock", mountinfo),
                         {self.own, (0, 4)})

    def test_a_directory_that_does_not_exist_holds_no_locks(self):
        self.assertEqual(flock_holders.lock_devices(self.base / "gone" / "heavy.lock"), set())


if __name__ == "__main__":
    unittest.main()
