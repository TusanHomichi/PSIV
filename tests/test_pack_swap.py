"""psiv_tools/pack_swap.py: a pack build swaps its output in whole (#145).

    PYTHONPATH=. python3 -m unittest tests.test_pack_swap -v

Every case builds a small stand-in pack in a scratch directory: `manifest.json`
names a version and the files, and every file holds that version, so a reader
can tell a complete pack from a half-written or mixed one. No ROM and no real
pack is touched. Each drift class the swap closes has a case and a negative
control that runs the old way (a build into the output in place, a reader that
reopens the output per file, no cleanup) and shows the failure:

- a half-written pack: a reader during a build;
- a killed build: killed part-way (SIGKILL; TerminateProcess on Windows), in a
  child process;
- a reader spanning a swap: one that resolved the output once;
- a stale symbolic link: an output link naming a version that is gone, and the
  link and staging directory a killed build left.

Where the output is a link (Linux, macOS) every case runs; on Windows, where it
stays a real directory, the cases that rely on the link are skipped and
`DirectoryOutput` covers that mode. `DirectoryOutput` runs on every system, so
the gate checks the Windows mode on Linux, and the `platforms` workflow runs
this module on macOS and Windows.
"""
import json
import os
import shutil
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

#: A case about the output being a link to its version.
links = unittest.skipUnless(pack_swap.LINKS, "the output is a real directory here "
                            "(DirectoryOutput covers that mode)")


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
        # Resolved, as `realpath` reports it: macOS's temp dir is a link.
        self.dir = Path(tmp.name).resolve()
        self.output = self.dir / "runtime-pack"

    def install(self, version, pause=None):
        return install_atomically(self.output, lambda d: write_pack(d, version, pause))


class Install(Scratch):

    @links
    def test_the_output_becomes_a_link_to_a_complete_version(self):
        self.assertEqual(self.install("v1"), "v1")
        self.assertTrue(self.output.is_symlink())
        self.assertEqual(consistent(self.output), "v1")
        self.assertEqual(self.install("v2"), "v2")
        self.assertEqual(consistent(self.output), "v2")
        self.assertEqual(len(pack_swap.versions(self.output)), 2)
        self.assertEqual([entry.name for entry in self.dir.iterdir()
                          if ".build-" in entry.name or ".link-" in entry.name], [])

    @links
    def test_a_real_directory_is_exchanged_and_kept_as_the_older_version(self):
        write_pack(self.output, "legacy")
        self.install("v1")
        self.assertTrue(self.output.is_symlink())
        self.assertEqual(consistent(self.output), "v1")
        older, newer = pack_swap.versions(self.output)
        self.assertEqual(consistent(older), "legacy")
        self.assertEqual(os.path.realpath(self.output), str(newer))

    @links
    def test_a_link_to_a_pack_elsewhere_is_replaced_and_that_pack_untouched(self):
        shared = self.dir / "shared-pack"
        write_pack(shared, "shared")
        self.output.symlink_to(shared)
        self.install("v1")
        self.assertEqual(consistent(self.output), "v1")
        self.assertEqual(consistent(shared), "shared")
        self.assertEqual(pack_swap.versions(self.output)[0].parent, self.dir)

    @links
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

    @links
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

    @links
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


class DirectoryOutput(Scratch):
    """Windows' mode: the output stays a real directory, replaced by renames.

    Runs on every system, with `LINKS` off where links work."""

    def setUp(self):
        super().setUp()
        mode = mock.patch.object(pack_swap, "LINKS", False)
        mode.start()
        self.addCleanup(mode.stop)

    def leftovers(self):
        return sorted(entry.name for entry in self.dir.iterdir() if entry != self.output)

    def test_the_output_stays_a_real_directory_holding_the_newest_pack(self):
        self.install("v1")
        self.install("v2")
        self.assertTrue(self.output.is_dir())
        self.assertFalse(self.output.is_symlink())
        self.assertEqual(consistent(self.output), "v2")
        self.assertEqual(self.leftovers(), [])

    def test_a_failing_build_leaves_the_old_directory(self):
        self.install("v1")

        def failing(directory):
            write_pack(directory, "v2", pause=lambda: 1 / 0)

        with self.assertRaises(ZeroDivisionError):
            install_atomically(self.output, failing)
        self.assertEqual(consistent(self.output), "v1")
        self.assertEqual(self.leftovers(), [])

    def test_a_refused_rename_puts_the_old_directory_back(self):
        self.install("v1")
        rename = os.rename

        def refuse_the_new_version(source, target):
            if Path(target) == self.output and ".v" in Path(source).name:
                raise PermissionError("in use")
            rename(source, target)

        with mock.patch.object(pack_swap.os, "rename", refuse_the_new_version):
            with self.assertRaises(PermissionError):
                self.install("v2")
        self.assertEqual(consistent(self.output), "v1")
        self.assertEqual([name for name in self.leftovers() if ".link-" in name], [])
        self.install("v3")  # the stranded version is pruned by the next build
        self.assertEqual(consistent(self.output), "v3")
        self.assertEqual(self.leftovers(), [])

    def test_a_build_killed_between_the_renames_is_cleaned_up_by_the_next(self):
        self.install("v0")
        os.rename(self.output, self.dir / f".runtime-pack.link-{StaleLinks.dead_pid()}-0badc0de")
        self.install("v1")
        self.assertEqual(consistent(self.output), "v1")
        self.assertEqual(self.leftovers(), [])

    def test_a_reader_during_a_build_sees_the_old_pack_whole(self):
        self.install("v1")
        seen = []
        self.install("v2", pause=lambda: seen.append(consistent(self.output)))
        self.assertEqual(seen, ["v1"])
        self.assertEqual(consistent(self.output), "v2")


class ConcurrentBuilds(Scratch):
    """Two builds of one output: one installs while the other has made its
    version and not yet installed it."""

    def other_builders_version(self):
        """A complete version named for a live process that is not this one,
        as a concurrent build leaves it between its rename and its install."""
        version = self.dir / f".runtime-pack.v20991231T235959000000Z-{os.getppid()}"
        write_pack(version, "theirs")
        return version

    def install_beside_a_pending_version(self):
        theirs = self.other_builders_version()
        self.install("mine")
        self.assertTrue(theirs.is_dir(), "the other build's version was pruned")
        link = self.dir / ".runtime-pack.link-1-0a1b2c3d"
        if pack_swap.LINKS:
            pack_swap._link(self.output, theirs, link, self.dir / "unused")
        else:
            pack_swap._replace_directory(self.output, theirs, link)
        self.assertEqual(consistent(self.output), "theirs")

    def test_the_other_build_still_installs_its_version(self):
        self.install_beside_a_pending_version()

    def test_the_other_build_still_installs_its_version_in_directory_mode(self):
        with mock.patch.object(pack_swap, "LINKS", False):
            self.install_beside_a_pending_version()

    def test_negative_control_pruning_without_the_check_deletes_it(self):
        with mock.patch.object(pack_swap, "LINKS", False), \
                mock.patch.object(pack_swap, "_alive", return_value=False):
            with self.assertRaises(AssertionError):
                self.install_beside_a_pending_version()

    def test_a_dead_builders_version_is_still_pruned(self):
        stale = self.dir / f".runtime-pack.v20000101T000000000000Z-{StaleLinks.dead_pid()}"
        write_pack(stale, "stale")
        with mock.patch.object(pack_swap, "LINKS", False):
            self.install("v1")
        self.assertFalse(stale.exists())


class PruneRace(Scratch):
    """A build that made an old version, paused, then installed it and exited
    while another build pruned: the pruner must not delete what the output
    names by then (#225 review)."""

    def late_version(self):
        for index in range(KEEP_VERSIONS):
            self.install(f"v{index}")
        # Oldest by name, so the next prune considers it.
        late = self.dir / f".runtime-pack.v20000101T000000000000Z-{StaleLinks.dead_pid()}"
        write_pack(late, "late")
        return late

    def installs_then_exits(self, late):
        def alive(pid):
            if pid == int(late.name.rsplit("-", 1)[1]):
                link = self.dir / ".runtime-pack.link-1-0a1b2c3d"
                pack_swap._link(self.output, late, link, self.dir / "unused")
            return False
        return alive

    @links
    def test_the_newly_current_version_survives(self):
        late = self.late_version()
        with mock.patch.object(pack_swap, "_alive", self.installs_then_exits(late)):
            pack_swap.prune(self.output)
        self.assertTrue(late.is_dir())
        self.assertEqual(consistent(self.output), "late")

    @links
    def test_negative_control_a_snapshot_taken_first_deletes_it(self):
        late = self.late_version()
        snapshot = os.path.realpath(self.output)
        real = os.path.realpath
        with mock.patch.object(pack_swap, "_alive", self.installs_then_exits(late)), \
                mock.patch.object(pack_swap.os.path, "realpath",
                                  lambda p: snapshot if Path(p) == self.output else real(p)):
            pack_swap.prune(self.output)
        self.assertFalse(late.exists())


class PendingInThisProcess(Scratch):
    """An install paused between its version and its commit while others in
    this process installed and pruned: its version carries this process's id,
    so liveness cannot protect it; it must survive and be installed (#225
    review)."""

    def paused_install(self, links, output=None):
        """`output`: the paused install's spelling of the output; the others
        use `self.output`."""
        real = pack_swap._link if links else pack_swap._replace_directory
        name = "_link" if links else "_replace_directory"
        paused = []

        def commit(output, version, *rest):
            if not paused:
                paused.append(version)
                for index in range(KEEP_VERSIONS + 1):
                    self.install(f"other{index}")
                self.assertTrue(version.is_dir(), "the paused version was pruned")
            return real(output, version, *rest)
        with mock.patch.object(pack_swap, "LINKS", links), \
                mock.patch.object(pack_swap, name, commit):
            paused_output = self.output if output is None else output
            self.assertEqual(install_atomically(paused_output, lambda d: write_pack(d, "paused")),
                             "paused")
        self.assertEqual(consistent(self.output), "paused")
        self.assertFalse(pack_swap._pending, pack_swap._pending)

    @links
    def test_the_paused_version_survives_and_installs(self):
        self.paused_install(links=True)

    def test_the_paused_version_survives_and_installs_in_directory_mode(self):
        self.paused_install(links=False)

    def test_the_paused_version_survives_spelled_through_dotdot(self):
        (self.dir / "sub").mkdir()
        self.paused_install(pack_swap.LINKS, output=self.dir / "sub" / ".." / self.output.name)

    def test_the_paused_version_survives_spelled_relative(self):
        self.paused_install(pack_swap.LINKS, output=Path(os.path.relpath(self.output)))

    @links
    def test_the_paused_version_survives_spelled_through_a_linked_parent(self):
        alias = self.dir.parent / f"{self.dir.name}-alias"
        alias.symlink_to(self.dir, target_is_directory=True)
        self.addCleanup(alias.unlink)
        self.paused_install(True, output=alias / self.output.name)

    def test_negative_control_a_spelling_key_misses_the_alias(self):
        (self.dir / "sub").mkdir()
        with mock.patch.object(pack_swap, "_pending_key", lambda v: str(v.absolute())):
            with self.assertRaises((AssertionError, OSError)):
                self.paused_install(pack_swap.LINKS,
                                    output=self.dir / "sub" / ".." / self.output.name)

    def test_negative_control_without_the_registry_it_is_pruned(self):
        with mock.patch.object(pack_swap, "_pending", mock.MagicMock()) as pending:
            pending.__contains__.return_value = False
            with self.assertRaises((AssertionError, OSError)):
                self.paused_install(links=False)

    def test_a_finished_version_of_this_process_is_still_pruned(self):
        for index in range(KEEP_VERSIONS + 1):
            self.install(f"v{index}")
        expected = KEEP_VERSIONS if pack_swap.LINKS else 0
        self.assertEqual(len(pack_swap.versions(self.output)), expected)


class CleanupAfterCommit(Scratch):
    """A failure after the output names the new pack is a warning, not an
    error that claims the output is unchanged (#225 review)."""

    def prune_fails_on(self, stuck):
        """`shutil.rmtree` refusing only `stuck`, the one pruning candidate."""
        real = shutil.rmtree

        def rmtree(path, *args, **kwargs):
            if Path(path) == stuck:
                raise PermissionError("held")
            return real(path, *args, **kwargs)
        return mock.patch.object(pack_swap.shutil, "rmtree", rmtree)

    def stuck_version(self):
        stuck = self.dir / f".runtime-pack.v20000101T000000000000Z-{StaleLinks.dead_pid()}"
        write_pack(stuck, "stuck")
        return stuck

    def pruning_failure_warns(self):
        for index in range(KEEP_VERSIONS):
            self.install(f"v{index}")
        stuck = self.stuck_version()
        with self.prune_fails_on(stuck):
            with self.assertWarnsRegex(pack_swap.PackSwapWarning,
                                       "pruning old versions failed") as caught:
                self.assertEqual(self.install("new"), "new")
        self.assertEqual(len(caught.warnings), 1, [str(w.message) for w in caught.warnings])
        self.assertEqual(consistent(self.output), "new")
        self.assertTrue(stuck.is_dir())

    @links
    def test_a_pruning_failure_warns_and_the_pack_is_installed(self):
        self.pruning_failure_warns()

    def test_a_pruning_failure_warns_in_directory_mode(self):
        with mock.patch.object(pack_swap, "LINKS", False):
            self.pruning_failure_warns()

    def test_an_old_directory_left_behind_warns_in_directory_mode(self):
        with mock.patch.object(pack_swap, "LINKS", False):
            self.install("old")
            with mock.patch.object(pack_swap.shutil, "rmtree",
                                   side_effect=PermissionError("held")):
                with self.assertWarnsRegex(pack_swap.PackSwapWarning,
                                           "cannot remove the old pack"):
                    self.assertEqual(self.install("new"), "new")
        self.assertEqual(consistent(self.output), "new")

    def test_negative_control_a_failure_before_the_commit_is_an_error(self):
        self.install("old")
        with self.assertRaises(RuntimeError):
            install_atomically(self.output, lambda d: (_ for _ in ()).throw(RuntimeError("x")))
        self.assertEqual(consistent(self.output), "old")


class RefusedLinks(Scratch):
    """Where links need a privilege (Windows without developer mode), only the
    directory mode installs a pack."""

    def install_without_links(self, links):
        refused = OSError(1314, "A required privilege is not held by the client")
        with mock.patch.object(pack_swap, "LINKS", links), \
                mock.patch.object(Path, "symlink_to", side_effect=refused):
            return self.install("v1")

    def test_the_directory_mode_installs_without_a_link(self):
        self.assertEqual(self.install_without_links(links=False), "v1")
        self.assertEqual(consistent(self.output), "v1")

    def test_negative_control_the_link_mode_fails_there(self):
        with self.assertRaises(OSError):
            self.install_without_links(links=True)
        self.assertFalse(os.path.lexists(self.output))


class Liveness(unittest.TestCase):
    """A staging directory's builder is looked up without /proc, which macOS
    and Windows lack."""

    def test_a_live_process_is_alive_without_proc(self):
        with mock.patch.object(Path, "exists", return_value=False):
            self.assertTrue(pack_swap._alive(os.getppid()))

    def test_an_exited_process_is_not(self):
        self.assertFalse(pack_swap._alive(StaleLinks.dead_pid()))


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
        child.kill()
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


@links
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

    @links
    def test_a_dangling_output_and_a_killed_builds_link_are_repaired(self):
        self.output.symlink_to(".runtime-pack.v20260101T000000000000Z-1")  # gone
        dead = self.dead_pid()
        stale = self.dir / f".runtime-pack.link-{dead}-0badc0de"
        stale.symlink_to("nowhere")
        self.install("v1")
        self.assertEqual(consistent(self.output), "v1")
        self.assertFalse(os.path.lexists(stale))

    @links
    def test_negative_control_without_the_cleanup_the_stale_link_stays(self):
        stale = self.dir / f".runtime-pack.link-{self.dead_pid()}-0badc0de"
        stale.symlink_to("nowhere")
        with mock.patch.object(pack_swap, "remove_abandoned", lambda output: []):
            self.install("v1")
        self.assertTrue(os.path.lexists(stale))

    @links
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
        dead = self.dead_pid()
        stale = self.dir / f".runtime-pack.link-{dead}-0badc0de"
        write_pack(stale, "stale")
        removed = pack_swap.remove_abandoned(self.output)
        self.assertFalse(stale.exists())
        self.assertEqual(removed, [stale.name])
        recovered = [version for version in pack_swap.versions(self.output)
                     if version.name.endswith(f"-{dead}")]
        self.assertEqual([consistent(version) for version in recovered], ["stale"])

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
