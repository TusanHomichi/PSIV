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
entries and abandoned staging, judging each under its entry's lock.
`ReadOnlyCase` shows a built entry refuses writes through the link and an
in-place `regenerate` replaces the link instead. `FramesCase` shows the pack's key
and snapshot leave out the evidence frames it no longer reads. `SnapshotCase` shows a build reads a copy of
what its key names, so an edit while it runs does not reach it, and
`WaitCase` that a job checks its inputs again once it holds the lock.
`BuildCommandCase` checks the command, directory and lock each build runs
with.
"""

import fcntl
import json
import os
import shutil
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
FRAMES = Path("oracle") / "fixtures" / "battle_animation_art"

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
    """A fake builder: records each build, which worktree built each entry and
    what it read from its snapshot (the extractor, the ROM and any frames).

    It keeps the record itself rather than writing a marker to read back
    through the link: a read under `generated/` is `tests/test_extract_stamp.py`'s
    to refuse, and these entries are only stand-ins."""

    def __init__(self, gate=None, started=None, contents=False):
        self.calls = []
        self.contents = contents
        self.builders = {}
        self.read = {}
        self.lock = threading.Lock()
        self.gate = gate
        self.started = started

    def __call__(self, entry, worktree, source, output):
        frames = source / FRAMES
        read = {"extractor": (source / "psiv_tools" / "core.py").read_text(),
                "rom": (source / ROM_NAME).read_bytes(),
                "frames": {path.name: path.read_bytes() for path in sorted(frames.glob("*.png"))}}
        with self.lock:
            self.calls.append((entry.name, Path(worktree).name))
            # The staging directory, `.<label>.build-<pid>`, becomes the entry `<label>`.
            label = output.parent.name[1:].rpartition(".build-")[0]
            self.builders[label] = Path(worktree).name
            self.read[label] = read
        if self.started is not None:
            self.started.set()
        if self.gate is not None:
            self.gate.wait(TIMEOUT)
        output.mkdir(parents=True)
        if self.contents:
            (output / "tables").mkdir()
            (output / "tables" / "table.json").write_text("{}\n")

    def names(self):
        return sorted(name for name, _ in self.calls)

    def builder(self, worktree, name):
        """The worktree whose build `worktree`'s `name` links to."""
        return self.builders[(Path(worktree) / name).resolve().parent.name]

    def inputs(self, worktree, name):
        """What the build `worktree`'s `name` links to read."""
        return self.read[(Path(worktree) / name).resolve().parent.name]


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
            self.assertEqual(builds.builder(second, name), "first")
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
            self.assertEqual(builds.builder(new, name), "new")

    def test_a_changed_fixture_builds_only_the_pack(self):
        builds = Builds()
        link(self.worktree("old"), build=builds)
        self.commit("oracle/fixtures/receipt.json", '{"frames": 1}\n')
        new = self.worktree("new")
        link(new, build=builds)
        self.assertEqual(builds.calls[2:], [("runtime-pack", "new")])
        self.assertEqual(builds.builder(new, "generated"), "old")


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
        self.assertEqual(builds.builder(second, "generated"), "first")


class FailureCase(ScratchCase):
    def test_a_failed_build_caches_nothing(self):
        def failing(entry, worktree, source, output):
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
                with self.assertRaisesRegex(Refused, f"uncommitted changes .*\\({relative}\\)"):
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
        builds = Builds()
        link(worktree, build=builds)
        self.assertEqual(builds.builder(worktree, "generated"), "first")
        self.assertFalse((worktree / "runtime-pack").is_symlink())
        self.assertFalse((worktree / "reference").is_symlink())
        self.assertTrue(any("runtime-pack: kept this worktree's own" in line for line in self.said))


class PruneCase(ScratchCase):
    def entries(self, **used):
        """Entries under the cache, each last used at the given time."""
        self.cache.mkdir(parents=True, exist_ok=True)
        for name, when in used.items():
            name = name.replace("_", "-")
            (self.cache / name).mkdir()
            (self.cache / name / USED_FILE).touch()
            os.utime(self.cache / name / USED_FILE, (when, when))

    def held(self, label):
        """Hold `label`'s entry lock, as a build or a reuse does."""
        handle = open(self.cache / f".{label}.lock", "w")
        self.addCleanup(handle.close)
        fcntl.flock(handle, fcntl.LOCK_EX)

    def test_only_unused_unlocked_entries_and_abandoned_staging_go(self):
        now = time.time()
        old = now - local_inputs.PRUNE_AFTER_S - 60
        self.entries(generated_old=old, generated_recent=now, runtime_pack_locked=old,
                     runtime_pack_kept=old)
        (self.cache / ".generated-x.build-1").mkdir()
        (self.cache / ".generated-y.build-2").mkdir()
        (self.cache / ".generated-z.removing-3").mkdir()
        self.held("runtime-pack-locked")
        self.held("generated-y")
        removed = prune(self.cache, keep=self.cache / "runtime-pack-kept", now=now)
        self.assertEqual(sorted(removed), [".generated-x.build-1", ".generated-z.removing-3",
                                           "generated-old"])
        self.assertEqual(sorted(p.name for p in self.cache.iterdir() if p.is_dir()),
                         [".generated-y.build-2", "generated-recent", "runtime-pack-kept",
                          "runtime-pack-locked"])

    def test_a_use_before_the_lock_is_taken_keeps_the_entry(self):
        # A reuse that refreshes `used` and releases the lock just before the
        # pruner takes it: the age has to be read under the lock.
        now = time.time()
        self.entries(generated_old=now - local_inputs.PRUNE_AFTER_S - 60)
        take = local_inputs._try_lock

        def reused_first(path):
            os.utime(self.cache / "generated-old" / USED_FILE, (now, now))
            return take(path)

        with mock.patch.object(local_inputs, "_try_lock", reused_first):
            removed = prune(self.cache, keep=self.cache / "kept", now=now)
        self.assertEqual(removed, [])
        self.assertTrue((self.cache / "generated-old").is_dir())

    def test_an_entry_another_pruner_removed_is_passed_over(self):
        now = time.time()
        self.entries(generated_old=now - local_inputs.PRUNE_AFTER_S - 60)
        take = local_inputs._try_lock

        def removed_first(path):
            shutil.rmtree(self.cache / "generated-old", ignore_errors=True)
            return take(path)

        with mock.patch.object(local_inputs, "_try_lock", removed_first):
            self.assertEqual(prune(self.cache, keep=self.cache / "kept", now=now), [])

    def test_a_built_read_only_entry_is_removed(self):
        builds = Builds()
        worktree = self.worktree("first")
        link(worktree, build=builds)
        entry = self.entry_of(worktree, "generated")
        os.utime(entry / USED_FILE, (0, 0))
        removed = prune(self.cache, keep=self.entry_of(worktree, "runtime-pack"))
        self.assertEqual(removed, [entry.name])
        self.assertFalse(os.path.lexists(entry))


class ReadOnlyCase(ScratchCase):
    def test_an_entry_is_read_only_once_built(self):
        worktree = self.worktree("first")
        link(worktree, build=Builds(contents=True))
        for name in ("generated", "runtime-pack"):
            for path in [worktree / name, *(worktree / name).rglob("*")]:
                with self.subTest(path=path):
                    self.assertFalse(path.stat().st_mode & 0o222, path)

    @unittest.skipIf(os.geteuid() == 0, "root writes through any permission")
    def test_a_write_through_the_link_is_refused(self):
        worktree = self.worktree("first")
        link(worktree, build=Builds(contents=True))
        entry = self.entry_of(worktree, "generated")
        before = sorted(p.name for p in (entry / "generated").rglob("*"))
        for relative in ("new.json", "tables/new.json", "tables/table.json"):
            with self.subTest(relative=relative), self.assertRaises(PermissionError):
                (worktree / "generated" / relative).write_text("overwritten\n")
        self.assertEqual(sorted(p.name for p in (entry / "generated").rglob("*")), before)

    def test_regenerate_in_place_replaces_the_link_and_leaves_the_entry(self):
        from psiv_tools import __main__ as cli
        from psiv_tools import battle_art, gfx, planes

        worktree = self.worktree("first")
        link(worktree, build=Builds(contents=True))
        entry = self.entry_of(worktree, "generated")
        before = sorted(p.name for p in (entry / "generated").rglob("*"))

        def extract(_data, output):
            output.mkdir(parents=True, exist_ok=True)
            (output / "rebuilt.json").write_text("{}\n")

        with mock.patch.object(cli, "_extract", extract), \
                mock.patch.object(gfx, "export_art_pngs", lambda _data, _out: []), \
                mock.patch.object(planes, "export_plane_pngs", lambda _data, _out: []), \
                mock.patch.object(battle_art, "export_battle_art_pngs", lambda _data, _out: []), \
                mock.patch("builtins.print"):
            cli._regenerate(b"", worktree / "generated")
        self.assertFalse((worktree / "generated").is_symlink())
        self.assertTrue((worktree / "generated" / "rebuilt.json").is_file())
        self.assertEqual(sorted(p.name for p in (entry / "generated").rglob("*")), before)


class FramesCase(ScratchCase):
    """No pack input reads the ignored evidence frames (#214): a worktree still
    links them for the tests, and they neither key nor reach a build."""

    def test_a_worktrees_own_frame_shares_the_pack(self):
        builds = Builds()
        linked = self.worktree("linked")
        link(linked, build=builds)
        own = self.worktree("own")
        (own / FRAMES).mkdir(parents=True, exist_ok=True)
        (own / FRAMES / "frame.png").write_bytes(b"another png")
        link(own, build=builds)
        self.assertEqual(len(builds.calls), 2)
        self.assertEqual(self.entry_of(linked, "runtime-pack"), self.entry_of(own, "runtime-pack"))
        self.assertEqual((own / FRAMES / "frame.png").read_bytes(), b"another png")

    def test_a_frame_changed_added_or_removed_in_the_main_checkout_builds_nothing(self):
        builds = Builds()
        link(self.worktree("before"), build=builds)
        changes = {
            "changed": lambda: (self.main / FRAMES / "frame.png").write_bytes(b"new png"),
            "added": lambda: (self.main / FRAMES / "frame2.png").write_bytes(b"png 2"),
            "removed": lambda: (self.main / FRAMES / "frame.png").unlink(),
        }
        for change, apply in changes.items():
            with self.subTest(change=change):
                apply()
                link(self.worktree(change), build=builds)
                self.assertEqual(len(builds.calls), 2)

    def test_a_build_does_not_get_the_frames(self):
        builds = Builds()
        first = self.worktree("first")
        link(first, build=builds)
        self.assertTrue((first / FRAMES / "frame.png").is_file())
        for name in ("generated", "runtime-pack"):
            with self.subTest(name=name):
                self.assertEqual(builds.inputs(first, name)["frames"], {})


class SnapshotCase(ScratchCase):
    """A build reads a copy of what its key names, never the worktree's files."""

    def test_an_edit_during_the_build_does_not_reach_it(self):
        builds = Builds()

        def editing(entry, worktree, source, output):
            extractor = Path(worktree) / "psiv_tools" / "core.py"
            committed = extractor.read_text()
            extractor.write_text("EXTRACTOR = 'uncommitted'\n")
            builds(entry, worktree, source, output)
            extractor.write_text(committed)

        worktree = self.worktree("first")
        link(worktree, build=editing)
        for name in ("generated", "runtime-pack"):
            with self.subTest(name=name):
                self.assertEqual(builds.inputs(worktree, name),
                                 {"extractor": "EXTRACTOR = 1\n", "rom": b"not a cartridge",
                                  "frames": {}})
                self.assertEqual(sorted(p.name for p in self.entry_of(worktree, name).iterdir()),
                                 sorted([name, "key.json", USED_FILE]))


class WaitCase(ScratchCase):
    """Whatever a job waited for, it checks its inputs again holding the lock."""

    def link_changing_while_waiting(self, worktree, name, change, builds):
        """Link `worktree`, applying `change` while it waits for its first `name` entry."""
        take = local_inputs._lock
        pending = [change]

        def waited(path, label):
            if label.startswith(f"{name}-") and pending:
                pending.pop()()
            return take(path, label)

        with mock.patch.object(local_inputs, "_lock", waited):
            link(worktree, build=builds)

    def test_a_head_moved_while_waiting_is_served_its_own_extract(self):
        builds = Builds()
        first = self.worktree("first")
        link(first, build=builds)
        newer = self.commit("psiv_tools/core.py", "EXTRACTOR = 2\n")
        second = self.worktree("second", commit=git(first, "rev-parse", "HEAD"))
        self.link_changing_while_waiting(
            second, "generated", lambda: git(second, "checkout", "-q", "--detach", newer), builds)
        self.assertEqual(builds.inputs(second, "generated")["extractor"], "EXTRACTOR = 2\n")

    def test_an_edit_while_waiting_is_refused(self):
        builds = Builds()
        link(self.worktree("first"), build=builds)
        second = self.worktree("second")
        with self.assertRaisesRegex(Refused, "uncommitted changes"):
            self.link_changing_while_waiting(
                second, "generated",
                lambda: (second / "psiv_tools" / "core.py").write_text("EXTRACTOR = 3\n"),
                builds)
        self.assertFalse(os.path.lexists(second / "generated"))


class BuildCommandCase(ScratchCase):
    def setUp(self):
        super().setUp()
        # The worktree's heavy_lock.py prints the lock's path with no arguments
        # (the override, else a default relative to where it runs) and records
        # each `run` with the directory and lock it ran with.
        self.tree = self.worktree("first")
        (self.tree / "tools").mkdir()
        self.calls = self.scratch / "calls.json"
        (self.tree / "tools" / "heavy_lock.py").write_text(
            "import json, os, pathlib, sys\n"
            "if len(sys.argv) == 1:\n"
            "    print(os.environ.get('PSIV_HEAVY_LOCK') or 'build/heavy.lock')\n"
            "    sys.exit(0)\n"
            f"calls = pathlib.Path({str(self.calls)!r})\n"
            "seen = json.loads(calls.read_text()) if calls.exists() else []\n"
            "calls.write_text(json.dumps(seen + [[pathlib.Path.cwd().name,\n"
            "                                     os.environ['PSIV_HEAVY_LOCK'], sys.argv[1:]]]))\n")
        self.source, self.output = self.scratch / "source", self.scratch / "out"
        self.source.mkdir()

    def runs(self):
        for entry in ENTRIES:
            build_with_lock(entry, self.tree, self.source, self.output)
        return json.loads(self.calls.read_text())

    def test_each_build_runs_in_its_snapshot_under_the_worktrees_heavy_lock(self):
        with mock.patch.dict(os.environ):
            os.environ.pop("PSIV_HEAVY_LOCK", None)
            runs = self.runs()
        rom, lock = str(self.source / ROM_NAME), str((self.tree / "build" / "heavy.lock").resolve())
        self.assertEqual(runs, [
            ["source", lock, ["run", "--class", "python", "--", "python3", "-m", "psiv_tools",
                              "regenerate", rom, str(self.output)]],
            ["source", lock, ["run", "--class", "pack", "--", "python3", "-m", "psiv_tools",
                              "pack", rom, str(self.output)]],
        ])

    def test_an_override_keeps_naming_the_callers_lock_from_the_snapshot(self):
        caller = self.scratch / "caller"
        caller.mkdir()
        for override, expected in (("/elsewhere/heavy.lock", "/elsewhere/heavy.lock"),
                                   ("custom.lock", str((caller / "custom.lock").resolve()))):
            with self.subTest(override=override), mock.patch.dict(
                    os.environ, {"PSIV_HEAVY_LOCK": override}):
                self.calls.unlink(missing_ok=True)
                cwd = os.getcwd()
                os.chdir(caller)
                try:
                    runs = self.runs()
                finally:
                    os.chdir(cwd)
                self.assertEqual([lock for _, lock, _ in runs], [expected, expected])


if __name__ == "__main__":
    unittest.main()
