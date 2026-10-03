"""The native verifier must reject changed code, even with a passing game."""

from pathlib import Path
import hashlib
import os
import signal
import subprocess
import sys
import tempfile
import time
import unittest

from tools.verify_native_tape import (file_identity, identities_stable,
                                      source_identity, terminate_process_group)


class NativeTapeIdentityGuard(unittest.TestCase):
    def test_changed_extension_or_driver_cannot_keep_a_stable_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = {"extension": root / "libpsiv_godot.so",
                     "driver": root / "native_tape.gd"}
            for name, path in paths.items():
                path.write_bytes(f"stub {name}".encode())
            before = {name: file_identity(path) for name, path in paths.items()}
            git = {"head": "fixed", "dirty_paths": [], "diff_sha256": "fixed"}
            self.assertTrue(identities_stable(before, before, git, git,
                                              "pack", "pack", "tape", "tape"))
            for changed in paths:
                with self.subTest(changed=changed):
                    before = {name: file_identity(path) for name, path in paths.items()}
                    original = paths[changed].read_bytes()
                    paths[changed].write_bytes(original + b" changed")
                    after = {name: file_identity(path) for name, path in paths.items()}
                    self.assertNotEqual(before[changed]["sha256"], after[changed]["sha256"])
                    self.assertFalse(identities_stable(before, after, git, git,
                                                       "pack", "pack", "tape", "tape"))
                    paths[changed].write_bytes(original)

    def test_untracked_directory_symlink_is_hashed_without_following_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "-c", "user.name=Test",
                            "-c", "user.email=test@example.invalid", "commit",
                            "--allow-empty", "-qm", "base"], check=True)
            link = root / "runtime-pack"
            first = root / "missing-protected-assets"
            link.symlink_to(first, target_is_directory=True)
            before = source_identity(root)
            self.assertEqual(before["untracked_entries"]["runtime-pack"]["kind"],
                             "symlink")
            self.assertEqual(before["untracked_entries"]["runtime-pack"]["link_sha256"],
                             hashlib.sha256(os.fsencode(str(first))).hexdigest())
            link.unlink()
            link.symlink_to(root / "another-missing-directory", target_is_directory=True)
            after = source_identity(root)
            self.assertNotEqual(before["untracked_entries"], after["untracked_entries"])
            self.assertFalse(identities_stable({}, {}, before, after,
                                               "pack", "pack", "tape", "tape"))

    def test_timeout_cleanup_kills_owned_child_not_only_shell(self):
        with tempfile.TemporaryDirectory() as directory:
            pid_file = Path(directory) / "child.pid"
            parent_code = (
                "import pathlib, subprocess, sys, time; "
                "child=subprocess.Popen([sys.executable, '-c', "
                "'import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(60)']); "
                "pathlib.Path(sys.argv[1]).write_text(str(child.pid)); time.sleep(60)"
            )
            parent = subprocess.Popen([sys.executable, "-c", parent_code,
                                       str(pid_file)], start_new_session=True)
            child_pid = None
            try:
                deadline = time.monotonic() + 5
                while not pid_file.is_file() and time.monotonic() < deadline:
                    time.sleep(0.02)
                self.assertTrue(pid_file.is_file())
                child_pid = int(pid_file.read_text())
                terminate_process_group(parent, grace_s=0.2)
                self.assertIsNotNone(parent.returncode)
                child_stat = Path(f"/proc/{child_pid}/stat")
                if child_stat.is_file():
                    self.assertEqual(child_stat.read_text().split(") ", 1)[1][0], "Z")
            finally:
                if parent.poll() is None:
                    terminate_process_group(parent, grace_s=0.1)
                if child_pid is not None and Path(f"/proc/{child_pid}/stat").is_file():
                    state = Path(f"/proc/{child_pid}/stat").read_text().split(") ", 1)[1][0]
                    if state != "Z":
                        os.kill(child_pid, signal.SIGKILL)


if __name__ == "__main__":
    unittest.main()
