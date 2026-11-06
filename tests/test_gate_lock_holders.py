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

    def test_a_directory_that_does_not_exist_holds_no_locks(self):
        self.assertEqual(flock_holders.lock_devices(self.base / "gone" / "heavy.lock"), set())


if __name__ == "__main__":
    unittest.main()
