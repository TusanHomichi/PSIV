"""psiv_tools/pack_swap.py: a pack build swaps its output in whole (#145).

    PYTHONPATH=. python3 -m unittest tests.test_pack_swap -v

Every case builds a small stand-in pack in a scratch directory: `manifest.json`
names a version and the files, and every file holds that version, so a reader
can tell a complete pack from a half-written or mixed one. No ROM and no real
pack is touched. Each drift class the swap closes has a case and a negative
control that runs the old way (a build into the output in place, a reader that
reopens the output per file, no cleanup) and shows the failure:

- a half-written pack: a reader during a build;
- a killed build: SIGKILL part-way, in a child process;
- a reader spanning a swap: one that resolved the output once;
- a stale symbolic link: an output link naming a version that is gone, and the
  link and staging directory a killed build left.
"""
import json
import os
import signal
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from unittest import mock

from psiv_tools import pack_swap
from psiv_tools.pack_swap import KEEP_VERSIONS, PackSwapError, install_atomically

ROOT = Path(__file__).resolve().parents[1]
FILES = [f"part{n}.json" for n in range(6)]


def write_pack(directory, version, pause=None):
    """A stand-in pack build: the files first, the manifest last, like
    `build_pack`. `pause` is called after half the files."""
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    for index, name in enumerate(FILES):
        if pause is not None and index == len(FILES) // 2:
            pause()
        (directory / name).write_text(json.dumps({"version": version}))
    (directory / "manifest.json").write_text(json.dumps({"version": version, "files": FILES}))
    return version


def consistent(directory):
    """The pack's version when every file matches its manifest, else None."""
    directory = Path(directory)
    try:
        manifest = json.loads((directory / "manifest.json").read_text())
        found = {json.loads((directory / name).read_text())["version"]
                 for name in manifest["files"]}
    except (OSError, ValueError, KeyError):
        return None
    return manifest["version"] if found == {manifest["version"]} else None


class Scratch(unittest.TestCase):

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="pack-swap-")
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name)
        self.output = self.dir / "runtime-pack"

    def install(self, version, pause=None):
        return install_atomically(self.output, lambda d: write_pack(d, version, pause))


class Install(Scratch):

    def test_the_output_becomes_a_link_to_a_complete_version(self):
        self.assertEqual(self.install("v1"), "v1")
        self.assertTrue(self.output.is_symlink())
        self.assertEqual(consistent(self.output), "v1")
        self.assertEqual(self.install("v2"), "v2")
        self.assertEqual(consistent(self.output), "v2")
        self.assertEqual(len(pack_swap.versions(self.output)), 2)
        self.assertEqual([entry.name for entry in self.dir.iterdir()
                          if ".build-" in entry.name or ".link-" in entry.name], [])

    def test_a_real_directory_is_exchanged_and_kept_as_the_older_version(self):
        write_pack(self.output, "legacy")
        self.install("v1")
        self.assertTrue(self.output.is_symlink())
        self.assertEqual(consistent(self.output), "v1")
        older, newer = pack_swap.versions(self.output)
        self.assertEqual(consistent(older), "legacy")
        self.assertEqual(os.path.realpath(self.output), str(newer))

    def test_a_link_to_a_pack_elsewhere_is_replaced_and_that_pack_untouched(self):
        shared = self.dir / "shared-pack"
        write_pack(shared, "shared")
        self.output.symlink_to(shared)
        self.install("v1")
        self.assertEqual(consistent(self.output), "v1")
        self.assertEqual(consistent(shared), "shared")
        self.assertEqual(pack_swap.versions(self.output)[0].parent, self.dir)

    def test_a_failing_build_leaves_the_output_and_no_staging(self):
        self.install("v1")

        def failing(directory):
            write_pack(directory, "v2", pause=lambda: 1 / 0)

        with self.assertRaises(ZeroDivisionError):
            install_atomically(self.output, failing)
        self.assertEqual(consistent(self.output), "v1")
        self.assertEqual(len(pack_swap.versions(self.output)), 1)
        self.assertEqual([entry.name for entry in self.dir.iterdir()
                          if ".build-" in entry.name], [])

    def test_a_file_at_the_output_is_refused(self):
        self.output.write_text("not a pack")
        with self.assertRaises(PackSwapError):
            self.install("v1")
        self.assertEqual(self.output.read_text(), "not a pack")

    def test_old_versions_are_pruned_but_never_the_current_one(self):
        for n in range(KEEP_VERSIONS + 2):
            self.install(f"v{n}")
        kept = pack_swap.versions(self.output)
        self.assertEqual(len(kept), KEEP_VERSIONS)
        self.assertEqual(os.path.realpath(self.output), str(kept[-1]))
        self.assertEqual([consistent(path) for path in kept],
                         [f"v{n}" for n in range(2, KEEP_VERSIONS + 2)])
        # The current version survives a prune that would keep none.
        pack_swap.prune(self.output, keep=0)
        self.assertEqual(consistent(self.output), f"v{KEEP_VERSIONS + 1}")


class HalfWritten(Scratch):
    """A reader looking at the output while a build is half-way through."""

    def read_during_build(self, build_in_place):
        self.install("v1")
        seen = []

        def look():
            seen.append(consistent(self.output))

        if build_in_place:  # the old `pack`: straight into the output
            write_pack(os.path.realpath(self.output), "v2", pause=look)
        else:
            self.install("v2", pause=look)
        return seen[0]

    def test_a_reader_during_a_build_sees_the_old_pack_whole(self):
        self.assertEqual(self.read_during_build(build_in_place=False), "v1")
        self.assertEqual(consistent(self.output), "v2")

    def test_negative_control_an_in_place_build_shows_a_half_written_pack(self):
        self.assertIsNone(self.read_during_build(build_in_place=True))

    def test_a_reader_polling_through_a_whole_build_never_sees_a_mix(self):
        # A reader held off the CPU across KEEP_VERSIONS swaps loses the
        # version it resolved to pruning, a missing file and not a mix (the
        # module's documented limit), so pruning, tested above, is off here.
        no_prune = mock.patch.object(pack_swap, "prune")
        no_prune.start()
        self.addCleanup(no_prune.stop)
        self.install("v1")
        stop = threading.Event()
        seen = set()

        def poll():
            while not stop.is_set():
                seen.add(consistent(os.path.realpath(self.output)))

        reader = threading.Thread(target=poll)
        reader.start()
        try:
            for n in range(2, 12):
                self.install(f"v{n}")
        finally:
            stop.set()
            reader.join()
        self.assertNotIn(None, seen)


#: A child that builds into the output, half-way, then waits to be killed.
KILLED_CHILD = """
import json, sys, time
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from tests.test_pack_swap import write_pack
from psiv_tools.pack_swap import install_atomically

def pause():
    print("half", flush=True)
    time.sleep(120)

output = Path(sys.argv[2])
if sys.argv[3] == "in-place":
    import os
    write_pack(os.path.realpath(output), "v2", pause)
else:
    install_atomically(output, lambda d: write_pack(d, "v2", pause))
"""


class KilledBuild(Scratch):

    def kill_part_way(self, mode):
        self.install("v1")
        child = subprocess.Popen([sys.executable, "-c", KILLED_CHILD, str(ROOT),
                                  str(self.output), mode],
                                 stdout=subprocess.PIPE, text=True)
        self.addCleanup(lambda: child.poll() is None and child.kill())
        self.assertEqual(child.stdout.readline().strip(), "half")
        child.send_signal(signal.SIGKILL)
        child.wait(timeout=30)
        child.stdout.close()

    def test_a_killed_build_leaves_the_old_pack_and_the_next_build_cleans_up(self):
        self.kill_part_way("atomic")
        self.assertEqual(consistent(self.output), "v1")
        left = [entry.name for entry in self.dir.iterdir() if ".build-" in entry.name]
        self.assertEqual(len(left), 1)
        self.install("v3")
        self.assertEqual(consistent(self.output), "v3")
        self.assertFalse((self.dir / left[0]).exists())

    def test_negative_control_a_killed_in_place_build_leaves_a_broken_pack(self):
        self.kill_part_way("in-place")
        self.assertIsNone(consistent(self.output))

    def test_a_live_builders_staging_directory_is_kept(self):
        live = self.dir / f".runtime-pack.build-{os.getppid()}-abcd0123"
        live.mkdir()
        self.install("v1")
        self.assertTrue(live.is_dir())


class SpanningReader(Scratch):
    """A reader that reads the pack's files one by one while a swap lands."""

    def read_across_swap(self, resolve_once):
        self.install("v1")
        base = Path(os.path.realpath(self.output)) if resolve_once else self.output
        manifest = json.loads((base / "manifest.json").read_text())
        found = [json.loads((base / name).read_text())["version"]
                 for name in manifest["files"][:3]]
        self.install("v2")  # lands between the reader's opens
        found += [json.loads((base / name).read_text())["version"]
                  for name in manifest["files"][3:]]
        return manifest["version"], set(found)

    def test_a_reader_that_resolved_once_reads_one_version(self):
        self.assertEqual(self.read_across_swap(resolve_once=True), ("v1", {"v1"}))

    def test_negative_control_a_reader_reopening_the_output_mixes_versions(self):
        self.assertEqual(self.read_across_swap(resolve_once=False), ("v1", {"v1", "v2"}))


class StaleLinks(Scratch):

    def test_a_dangling_output_and_a_killed_builds_link_are_repaired(self):
        self.output.symlink_to(".runtime-pack.v20260101T000000000000Z-1")  # gone
        dead = self.dead_pid()
        stale = self.dir / f".runtime-pack.link-{dead}-0badc0de"
        stale.symlink_to("nowhere")
        self.install("v1")
        self.assertEqual(consistent(self.output), "v1")
        self.assertFalse(os.path.lexists(stale))

    def test_negative_control_without_the_cleanup_the_stale_link_stays(self):
        stale = self.dir / f".runtime-pack.link-{self.dead_pid()}-0badc0de"
        stale.symlink_to("nowhere")
        with mock.patch.object(pack_swap, "remove_abandoned", lambda output: []):
            self.install("v1")
        self.assertTrue(os.path.lexists(stale))

    def test_a_reader_that_resolved_a_kept_version_keeps_reading_it(self):
        self.install("v0")
        resolved = os.path.realpath(self.output)
        for n in range(1, KEEP_VERSIONS):
            self.install(f"v{n}")
        self.assertEqual(consistent(resolved), "v0")
        self.install(f"v{KEEP_VERSIONS}")  # one swap past the kept window
        self.assertIsNone(consistent(resolved))

    def test_a_build_killed_between_exchange_and_rename_keeps_the_old_pack(self):
        """After the exchange the `.link-` name holds the old real directory;
        cleanup keeps it as a version rather than deleting the only copy."""
        self.install("v0")
        stale = self.dir / f".runtime-pack.link-{self.dead_pid()}-0badc0de"
        stale.mkdir()
        (stale / "manifest.json").write_text("v0")
        removed = pack_swap.remove_abandoned(self.output)
        self.assertFalse(stale.exists())
        self.assertEqual(removed, [stale.name])
        self.assertIn("v0", [consistent(version) for version in pack_swap.versions(self.output)])

    @staticmethod
    def dead_pid():
        child = subprocess.Popen([sys.executable, "-c", "pass"])
        child.wait()
        return child.pid


class Ignored(unittest.TestCase):
    """The checkout's pack versions and staging are local inputs, never staged."""

    @staticmethod
    def ignored(name):
        return subprocess.run(["git", "check-ignore", "--no-index", "-q", name],
                              cwd=ROOT).returncode == 0

    def test_versions_staging_and_links_beside_any_pack_are_ignored(self):
        for name in (".runtime-pack.v20261018T114840029832Z-1549809",
                     ".runtime-pack.build-1549809-0a1b2c3d", ".runtime-pack.link-1-0a1b2c3d",
                     ".my-mod-pack.v20261018T114840029832Z-1549809",
                     ".my-mod-pack.build-1549809-0a1b2c3d",
                     ".my-mod-pack.link-1549809-0a1b2c3d"):
            with self.subTest(name=name):
                self.assertTrue(self.ignored(name))

    def test_negative_control_tracked_dotfiles_and_other_names_are_not_ignored(self):
        for name in (".runtime-packs.v1", ".my-mod-pack.v2", ".gitignore", ".godot-version",
                     ".github/ISSUE_TEMPLATE/bug.yml", ".github/pull_request_template.md",
                     ".my-mod-pack.v20261018T114840029832Z", ".my-mod-pack.build-1-0a1b2c3"):
            with self.subTest(name=name):
                self.assertFalse(self.ignored(name))


if __name__ == "__main__":
    unittest.main()
