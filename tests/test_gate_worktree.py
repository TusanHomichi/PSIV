"""tools/gate_worktree.py: the gate's kept worktree, moved from commit to
commit with only its `rust/target` kept.

    PYTHONPATH=. python3 -m unittest tests.test_gate_worktree -v

Each case makes a scratch repository with two commits that differ in one Rust
source. `MoveCase` adds the worktree, then moves it: the tracked files are the
new commit's, every untracked and ignored file but `rust/target` is gone (a
linked input's target is left alone), the changed source is newer than the
kept target dir and an unchanged one keeps its time, which is what lets cargo
reuse the warm build safely. `LibraryCase` removes a Godot library a gate
would otherwise test against. `CapCase` removes `rust/target` past the cap and
keeps it at the cap, on a measurement it stubs: a file just written can show
almost none of its blocks until the filesystem allocates them (on ZFS, seconds
later), so measuring one here would race. `DiskBytesCase` checks that
`disk_bytes` counts blocks rather than sizes, on a sparse file, and leaves
links unfollowed, on stat results it hands it, since what a link's target
takes depends on the host and its timing. `RefusalCase`
leaves the main checkout, a lane's worktree and a plain directory as they are,
also from main's copy read on stdin, as the steward runs it.
"""

import os
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

from tools import gate_worktree
from tools.gate_worktree import LIBRARIES, TARGET, Refused, prepare

TOOL = Path(__file__).resolve().parents[1] / "tools" / "gate_worktree.py"
CAP = 1 << 30
MIB = 1 << 20

IGNORED = """/rust/target
/generated
/build
"""


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True,
                          text=True).stdout.strip()


class ScratchRepo(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="gate-worktree-")
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        self.repo = self.tmp / "repo"
        (self.repo / "rust" / "core").mkdir(parents=True)
        git(self.tmp, "init", "-q", "-b", "main", str(self.repo))
        git(self.repo, "config", "user.email", "test@example.invalid")
        git(self.repo, "config", "user.name", "test")
        (self.repo / ".gitignore").write_text(IGNORED)
        (self.repo / "rust" / "core" / "lib.rs").write_text("pub fn a() {}\n")
        (self.repo / "rust" / "core" / "kept.rs").write_text("pub fn kept() {}\n")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "-m", "first")
        self.first = git(self.repo, "rev-parse", "HEAD")
        (self.repo / "rust" / "core" / "lib.rs").write_text("pub fn b() {}\n")
        git(self.repo, "commit", "-q", "-am", "second")
        self.second = git(self.repo, "rev-parse", "HEAD")
        self.gate = self.tmp / "gate"

    def prepare(self, commit, cap=CAP):
        return prepare(self.repo, self.gate, commit, cap)


class MoveCase(ScratchRepo):
    def test_adds_the_worktree_at_the_commit(self):
        done = self.prepare(self.first)
        self.assertEqual(git(self.gate, "rev-parse", "HEAD"), self.first)
        self.assertEqual((self.gate / "rust/core/lib.rs").read_text(), "pub fn a() {}\n")
        self.assertIn("no rust/target: this gate builds cold", done)

    def test_moves_it_keeping_only_rust_target(self):
        self.prepare(self.first)
        built = self.gate / TARGET / "debug" / "deps" / "libcore.rlib"
        built.parent.mkdir(parents=True)
        built.write_bytes(b"built")
        (self.gate / "generated").mkdir()
        (self.gate / "generated" / "stale.json").write_text("{}")
        (self.gate / "notes.txt").write_text("untracked")
        (self.gate / "rust/core/kept.rs").write_text("edited in place")
        linked = self.tmp / "main-build"
        linked.mkdir()
        (linked / "receipt.json").write_text("{}")
        (self.gate / "build").symlink_to(linked, target_is_directory=True)

        done = self.prepare(self.second)

        self.assertEqual(git(self.gate, "rev-parse", "HEAD"), self.second)
        self.assertEqual((self.gate / "rust/core/lib.rs").read_text(), "pub fn b() {}\n")
        self.assertEqual((self.gate / "rust/core/kept.rs").read_text(), "pub fn kept() {}\n")
        self.assertEqual(built.read_bytes(), b"built")
        for gone in ("generated", "notes.txt", "build"):
            self.assertFalse(os.path.lexists(self.gate / gone), gone)
        self.assertEqual((linked / "receipt.json").read_text(), "{}")
        self.assertEqual(git(self.gate, "status", "--porcelain", "--ignored"), "!! rust/target/")
        self.assertTrue(any(line.startswith("kept rust/target") for line in done), done)

    def test_a_changed_source_is_newer_than_the_kept_build(self):
        """Cargo rebuilds a crate whose sources are newer than its build: the
        checkout must make every changed file newer than what the last gate
        built, and leave an unchanged one's time alone."""
        self.prepare(self.first)
        built = self.gate / TARGET / "debug" / "core.stamp"
        built.parent.mkdir(parents=True)
        built.write_text("built")
        past = time.time() - 60
        for path in (self.gate / "rust/core/lib.rs", self.gate / "rust/core/kept.rs", built):
            os.utime(path, (past, past))
        os.utime(built, (past + 30, past + 30))
        # As after Git wrote them: the index's times match the files'.
        git(self.gate, "update-index", "-q", "--refresh")

        self.prepare(self.second)

        build_time = built.stat().st_mtime
        self.assertGreater((self.gate / "rust/core/lib.rs").stat().st_mtime, build_time)
        self.assertEqual((self.gate / "rust/core/kept.rs").stat().st_mtime, past)


class LibraryCase(ScratchRepo):
    def test_removes_the_godot_library_and_keeps_the_rest(self):
        self.prepare(self.first)
        for library in LIBRARIES:
            (self.gate / library).parent.mkdir(parents=True, exist_ok=True)
            (self.gate / library).write_bytes(b"\x7fELF another commit's")
        other = self.gate / TARGET / "debug" / "libpsiv_core.rlib"
        other.write_bytes(b"kept")

        done = self.prepare(self.second)

        for library in LIBRARIES:
            self.assertFalse((self.gate / library).exists(), library)
            self.assertIn(f"removed {library.as_posix()}", done)
        self.assertEqual(other.read_bytes(), b"kept")


class CapCase(ScratchRepo):
    def prepare_measuring(self, used, cap):
        self.prepare(self.first)
        built = self.gate / TARGET / "debug" / "deps" / "libcore.rlib"
        built.parent.mkdir(parents=True)
        built.write_bytes(b"built")
        with mock.patch.object(gate_worktree, "disk_bytes", return_value=used) as measured:
            done = self.prepare(self.second, cap=cap)
        measured.assert_called_once_with(self.gate / TARGET)
        return done

    def test_removes_rust_target_past_the_cap(self):
        done = self.prepare_measuring(used=CAP + 1, cap=CAP)
        self.assertFalse((self.gate / TARGET).exists())
        self.assertTrue(any("over the" in line and "builds cold" in line for line in done), done)

    def test_keeps_it_at_the_cap(self):
        done = self.prepare_measuring(used=CAP, cap=CAP)
        self.assertTrue((self.gate / TARGET / "debug" / "deps" / "libcore.rlib").is_file())
        self.assertTrue(any(line.startswith("kept rust/target") for line in done), done)


class DiskBytesCase(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="gate-worktree-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)

    def test_counts_blocks_not_sizes(self):
        """A sparse file's size is all hole: none of it is on disk."""
        with open(self.root / "sparse", "wb") as sparse:
            sparse.truncate(CAP)
        self.assertLess(gate_worktree.disk_bytes(self.root), MIB)

    def test_does_not_follow_links(self):
        """Each file but a link is made to take 1 MiB, so a link followed to
        the directory or file behind it would count that too."""
        outside = self.root / "outside"
        (outside / "inputs").mkdir(parents=True)
        for name in ("a", "b", "c"):
            (outside / "inputs" / name).write_bytes(b"input")
        measured = self.root / "measured"
        measured.mkdir()
        (measured / "built").write_bytes(b"built")
        (measured / "inputs").symlink_to(outside / "inputs", target_is_directory=True)
        (measured / "library").symlink_to(outside / "inputs" / "a")

        real_lstat = os.lstat

        def lstat(path):
            mode = real_lstat(path).st_mode
            return SimpleNamespace(st_mode=mode, st_blocks=0 if stat.S_ISLNK(mode) else MIB // 512)

        def followed(path):
            return lstat(os.path.realpath(path))

        with mock.patch.object(os, "lstat", lstat), mock.patch.object(os, "stat", followed):
            self.assertEqual(gate_worktree.disk_bytes(measured), MIB)


class RefusalCase(ScratchRepo):
    def assertLeftAlone(self, directory, commit):
        (directory / "precious.txt").write_text("evidence")
        with self.assertRaises(Refused):
            prepare(self.repo, directory, commit, CAP)
        self.assertEqual((directory / "precious.txt").read_text(), "evidence")

    def test_refuses_the_main_checkout(self):
        self.assertLeftAlone(self.repo, self.first)
        self.assertEqual(git(self.repo, "rev-parse", "HEAD"), self.second)

    def test_refuses_a_worktree_it_did_not_add(self):
        lane = self.tmp / "lane"
        git(self.repo, "worktree", "add", "-q", "--detach", str(lane), self.second)
        self.assertLeftAlone(lane, self.first)
        self.assertEqual(git(lane, "rev-parse", "HEAD"), self.second)

    def test_refuses_a_plain_directory(self):
        plain = self.tmp / "plain"
        plain.mkdir()
        self.assertLeftAlone(plain, self.first)

    def test_main_copy_on_stdin_refuses_with_exit_2(self):
        result = subprocess.run([sys.executable, "-", str(self.repo), self.first],
                                input=TOOL.read_text(), cwd=self.repo, capture_output=True,
                                text=True, check=False)
        self.assertEqual(result.returncode, gate_worktree.EXIT_REFUSED, result.stderr)
        self.assertIn("is not the gate's worktree", result.stderr)

    def test_main_copy_on_stdin_adds_and_moves_it(self):
        for commit in (self.first, self.second):
            result = subprocess.run([sys.executable, "-", str(self.gate), commit],
                                    input=TOOL.read_text(), cwd=self.repo, capture_output=True,
                                    text=True, check=False)
            self.assertEqual(result.returncode, gate_worktree.EXIT_OK, result.stderr)
        self.assertEqual(git(self.gate, "rev-parse", "HEAD"), self.second)

    def test_re_adds_a_deleted_worktree(self):
        self.prepare(self.first)
        shutil.rmtree(self.gate)
        self.prepare(self.second)
        self.assertEqual(git(self.gate, "rev-parse", "HEAD"), self.second)


if __name__ == "__main__":
    unittest.main()
