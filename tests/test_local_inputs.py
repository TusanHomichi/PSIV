"""tools/local_inputs.py: a worktree's local inputs, and generated/ and the
runtime pack built once per extractor and shared between worktrees.

    PYTHONPATH=. python3 -m unittest tests.test_local_inputs -v

Each case makes a scratch repository with a stand-in ROM and inputs, and a
fake builder that records what it built. `SharedCacheCase` links two
worktrees at one commit: each input is built once and both link the same
entry; its negative control, a commit that changes `psiv_tools/`, builds both
again, and one that changes only `oracle/fixtures/` builds only the pack.
`ConcurrentCase` starts two worktrees at once: the second waits on the
entry's lock, saying so, and links what the first built. `FailureCase` shows a
failed build caches nothing. `RefusalCase` refuses a worktree with
uncommitted changes under the keyed paths (and not one with changes
elsewhere) and the main checkout, also from main's copy read on stdin, as the
steward runs it. `LinkCase` repoints a stale link to a cached input and keeps
a worktree's own directory. `PruneCase` removes only unused, unlocked
entries and abandoned staging. `BuildCommandCase` checks the command and lock
class each build runs under.
"""

import fcntl
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from tools import local_inputs
from tools.local_inputs import (CACHE_DIR, ENTRIES, ROM_NAME, USED_FILE, Refused, build_with_lock,
                                link, prune)

TOOL = Path(__file__).resolve().parents[1] / "tools" / "local_inputs.py"
TIMEOUT = 30

IGNORED = """/generated
/runtime-pack
/reference
/build
Phantasy Star IV*.md
oracle/fixtures/battle_animation_art/*.png
"""


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True,
                          text=True).stdout.strip()


class Builds:
    """A fake builder: records each build and writes a marker into it."""

    def __init__(self, gate=None, started=None):
        self.calls = []
        self.lock = threading.Lock()
        self.gate = gate
        self.started = started

    def __call__(self, entry, worktree, rom, output):
        with self.lock:
            self.calls.append((entry.name, Path(worktree).name))
        if self.started is not None:
            self.started.set()
        if self.gate is not None:
            self.gate.wait(TIMEOUT)
        output.mkdir(parents=True)
        (output / "built-by").write_text(Path(worktree).name)

    def names(self):
        return sorted(name for name, _ in self.calls)


class ScratchCase(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.scratch = Path(scratch.name)
        self.main = self.scratch / "main"
        self.main.mkdir()
        git(self.main, "init", "-q", "-b", "main")
        git(self.main, "config", "user.email", "test@example.invalid")
        git(self.main, "config", "user.name", "test")
        (self.main / ".gitignore").write_text(IGNORED)
        (self.main / "psiv_tools").mkdir()
        (self.main / "psiv_tools" / "core.py").write_text("EXTRACTOR = 1\n")
        (self.main / "oracle" / "fixtures" / "battle_animation_art").mkdir(parents=True)
        (self.main / "oracle" / "fixtures" / "receipt.json").write_text("{}\n")
        (self.main / "README.md").write_text("scratch\n")
        git(self.main, "add", "-A")
        git(self.main, "commit", "-q", "-m", "scratch")
        (self.main / ROM_NAME).write_bytes(b"not a cartridge")
        (self.main / "reference").mkdir()
        (self.main / "oracle" / "fixtures" / "battle_animation_art" / "frame.png").write_bytes(b"png")
        self.said = []
        patcher = mock.patch.object(local_inputs, "_say", self.said.append)
        patcher.start()
        self.addCleanup(patcher.stop)

    def worktree(self, name, commit="HEAD"):
        path = self.scratch / name
        git(self.main, "worktree", "add", "-q", "--detach", str(path), commit)
        return path

    def commit(self, relative, text):
        (self.main / relative).write_text(text)
        git(self.main, "commit", "-q", "-am", f"change {relative}")
        return git(self.main, "rev-parse", "HEAD")

    def entry_of(self, worktree, name):
        return (worktree / name).resolve().parent

    @property
    def cache(self):
        return self.main / CACHE_DIR


class SharedCacheCase(ScratchCase):
    def test_two_worktrees_at_one_commit_build_each_input_once(self):
        builds = Builds()
        first, second = self.worktree("first"), self.worktree("second")
        link(first, build=builds)
        link(second, build=builds)
        self.assertEqual(builds.names(), ["generated", "runtime-pack"])
        for name in ("generated", "runtime-pack"):
            self.assertTrue((second / name).is_symlink())
            self.assertEqual(self.entry_of(first, name), self.entry_of(second, name))
            self.assertEqual((second / name / "built-by").read_text(), "first")
            self.assertTrue((self.entry_of(second, name) / USED_FILE).is_file())
        self.assertEqual((second / "reference").resolve(), (self.main / "reference").resolve())
        frame = second / "oracle" / "fixtures" / "battle_animation_art" / "frame.png"
        self.assertTrue(frame.is_symlink())
        self.assertEqual(frame.read_bytes(), b"png")
        self.assertFalse(git(second, "status", "--porcelain"))

    def test_negative_control_a_changed_extractor_builds_both_again(self):
        builds = Builds()
        old = self.worktree("old")
        link(old, build=builds)
        self.commit("psiv_tools/core.py", "EXTRACTOR = 2\n")
        new = self.worktree("new")
        link(new, build=builds)
        self.assertEqual(builds.names(), ["generated", "generated", "runtime-pack", "runtime-pack"])
        for name in ("generated", "runtime-pack"):
            self.assertNotEqual(self.entry_of(old, name), self.entry_of(new, name))
            self.assertEqual((new / name / "built-by").read_text(), "new")

    def test_a_changed_fixture_builds_only_the_pack(self):
        builds = Builds()
        link(self.worktree("old"), build=builds)
        self.commit("oracle/fixtures/receipt.json", '{"frames": 1}\n')
        new = self.worktree("new")
        link(new, build=builds)
        self.assertEqual(builds.calls[2:], [("runtime-pack", "new")])
        self.assertEqual((new / "generated" / "built-by").read_text(), "old")


class ConcurrentCase(ScratchCase):
    def test_a_second_job_waits_for_the_first_build_and_links_it(self):
        gate, started = threading.Event(), threading.Event()
        builds = Builds(gate=gate, started=started)
        first, second = self.worktree("first"), self.worktree("second")
        errors = []

        def run(worktree):
            try:
                link(worktree, build=builds)
            except Exception as error:  # reported by the assertion below
                errors.append(error)

        with mock.patch.object(local_inputs, "WAIT_REPORT_S", 0.05):
            one = threading.Thread(target=run, args=(first,))
            one.start()
            self.assertTrue(started.wait(TIMEOUT))
            two = threading.Thread(target=run, args=(second,))
            two.start()
            deadline = time.monotonic() + TIMEOUT
            while not any("still waiting" in line for line in self.said):
                self.assertLess(time.monotonic(), deadline, self.said)
                time.sleep(0.01)
            gate.set()
            one.join(TIMEOUT)
            two.join(TIMEOUT)
        self.assertEqual(errors, [])
        self.assertEqual(builds.names(), ["generated", "runtime-pack"])
        self.assertEqual((second / "generated" / "built-by").read_text(), "first")


class FailureCase(ScratchCase):
    def test_a_failed_build_caches_nothing(self):
        def failing(entry, worktree, rom, output):
            output.mkdir(parents=True)
            raise subprocess.CalledProcessError(1, ["build"])

        worktree = self.worktree("first")
        with self.assertRaises(subprocess.CalledProcessError):
            link(worktree, build=failing)
        self.assertEqual(sorted(p.name for p in self.cache.iterdir() if p.is_dir()), [])
        builds = Builds()
        link(worktree, build=builds)
        self.assertEqual(builds.names(), ["generated", "runtime-pack"])


class RefusalCase(ScratchCase):
    def test_uncommitted_changes_under_a_keyed_path_are_refused(self):
        for relative in ("psiv_tools/core.py", "oracle/fixtures/new.json"):
            with self.subTest(relative=relative):
                worktree = self.worktree(f"dirty-{relative.replace('/', '-')}")
                (worktree / relative).write_text("uncommitted\n")
                with self.assertRaisesRegex(Refused, "uncommitted changes"):
                    link(worktree, build=Builds())
                self.assertFalse(os.path.lexists(worktree / "reference"))
                self.assertFalse(self.cache.exists())

    def test_negative_control_changes_elsewhere_are_not(self):
        worktree = self.worktree("elsewhere")
        (worktree / "README.md").write_text("edited\n")
        link(worktree, build=Builds())
        self.assertTrue((worktree / "generated").is_symlink())

    def test_the_main_checkout_is_refused(self):
        with self.assertRaisesRegex(Refused, "main checkout"):
            link(self.main, build=Builds())

    def test_main_copy_read_on_stdin_refuses_with_exit_2(self):
        for command, source in (([sys.executable, str(TOOL), "link"], None),
                                ([sys.executable, "-", "link"], TOOL.read_text())):
            with self.subTest(command=command[1]):
                result = subprocess.run(command, cwd=self.main, input=source, text=True,
                                        capture_output=True, timeout=TIMEOUT, check=False)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn("main checkout", result.stderr)

    def test_a_missing_rom_is_refused(self):
        (self.main / ROM_NAME).unlink()
        with self.assertRaisesRegex(Refused, "no ROM"):
            link(self.worktree("first"), build=Builds())


class LinkCase(ScratchCase):
    def test_a_stale_link_is_repointed_and_an_own_directory_kept(self):
        (self.main / "generated").mkdir()
        worktree = self.worktree("first")
        (worktree / "generated").symlink_to(self.main / "generated")
        (worktree / "runtime-pack").mkdir()
        (worktree / "reference").mkdir()
        link(worktree, build=Builds())
        self.assertEqual((worktree / "generated" / "built-by").read_text(), "first")
        self.assertFalse((worktree / "runtime-pack").is_symlink())
        self.assertFalse((worktree / "reference").is_symlink())
        self.assertTrue(any("runtime-pack: kept this worktree's own" in line for line in self.said))


class PruneCase(ScratchCase):
    def test_only_unused_unlocked_entries_and_abandoned_staging_go(self):
        self.cache.mkdir(parents=True)
        now = time.time()
        old = now - local_inputs.PRUNE_AFTER_S - 60
        for name, used in (("generated-old", old), ("generated-recent", now),
                           ("runtime-pack-locked", old), ("runtime-pack-kept", old)):
            (self.cache / name).mkdir()
            (self.cache / name / USED_FILE).touch()
            os.utime(self.cache / name / USED_FILE, (used, used))
        dead = subprocess.Popen([sys.executable, "-c", "pass"])
        dead.wait()
        (self.cache / f".generated-x.build-{dead.pid}").mkdir()
        (self.cache / f".generated-y.build-{os.getpid()}").mkdir()
        with open(self.cache / ".runtime-pack-locked.lock", "w") as held:
            fcntl.flock(held, fcntl.LOCK_EX)
            removed = prune(self.cache, keep=self.cache / "runtime-pack-kept", now=now)
        self.assertEqual(sorted(removed), [f".generated-x.build-{dead.pid}", "generated-old"])
        self.assertEqual(sorted(p.name for p in self.cache.iterdir() if p.is_dir()),
                         [f".generated-y.build-{os.getpid()}", "generated-recent",
                          "runtime-pack-kept", "runtime-pack-locked"])


class BuildCommandCase(ScratchCase):
    def test_each_build_runs_the_worktrees_builder_under_its_heavy_lock(self):
        worktree = self.worktree("first")
        (worktree / "tools").mkdir()
        (worktree / "tools" / "heavy_lock.py").write_text(
            "import json, pathlib, sys\n"
            "calls = pathlib.Path('calls.json')\n"
            "seen = json.loads(calls.read_text()) if calls.exists() else []\n"
            "calls.write_text(json.dumps(seen + [sys.argv[1:]]))\n")
        rom, output = self.main / ROM_NAME, worktree / "out"
        for entry in ENTRIES:
            build_with_lock(entry, worktree, rom, output)
        calls = json.loads((worktree / "calls.json").read_text())
        self.assertEqual(calls, [
            ["run", "--class", "python", "--", "python3", "-m", "psiv_tools", "regenerate",
             str(rom), str(output)],
            ["run", "--class", "pack", "--", "python3", "-m", "psiv_tools", "pack",
             str(rom), str(output)],
        ])


if __name__ == "__main__":
    unittest.main()
