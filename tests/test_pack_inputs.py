"""The pack builds from the ROM and the tracked tree alone (#214).

A pack builder that quietly reads an ignored file (an emulator capture, a
local layout dump) works on the machine that has the file and nowhere else,
and its output silently depends on evidence nobody can rebuild. This guard
copies only the files of the change (`tools/repo_files.py`: every tracked
file plus any unignored new one, never an ignored input) into a scratch tree,
adds the ROM, runs
the normal entry point (`python3 -m psiv_tools pack ROM OUT`) and records every
file the builder process opens. It fails, naming each path, when the builder
opens anything other than:

* the ROM;
* a tracked file under `psiv_tools/` or `oracle/fixtures/`;
* the interpreter's own files (its prefixes and the import path it starts
  with: the standard library and installed packages);
* its own output directory (the pack, its staging sibling).

Why an audit hook rather than strace: `sys.addaudithook` sees the `open`
event that every Python-level open raises (`open`, `io.open_code` for imports,
`os.open`), before the open is attempted, so a read of a file that is absent
from the scratch tree is still named instead of only surfacing as the
traceback it causes. It is stdlib, needs no ptrace permission and runs the
same on every host the suite runs on. What it cannot see is a C extension
opening a file without going through Python; the builder imports none that
do (its only non-stdlib reads are the ROM and its own JSON and PNG inputs).
A probe that only stats a path (`Path.is_file()`) raises no `open` event, and
a build that probes for an absent input aborts before opening it, so the
bootstrap also records every stat that finds nothing (through `os.stat`,
which `os.path` and `pathlib` call); the build's own failure is reported
beside the list. Two more bookkeeping rules keep the list to inputs: opens
inside `shutil.rmtree` (deletion; its names are relative to a descriptor the
event does not carry) and directory or exclusive-create opens are not reads,
and a scratch directory the build makes with `tempfile` is its own output.

The hook is installed by a `-c` bootstrap that then runs the package exactly
as `-m` would (`runpy.run_module(..., alter_sys=True)`), under `-I -B`, so
neither the caller's environment nor a bytecode write leaks in.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

from tools.repo_files import repo_files

REPO = Path(__file__).resolve().parents[1]
ROM_NAME = "Phantasy Star IV (USA).md"
ROM = REPO / ROM_NAME

#: Tracked prefixes the builder may read.
ALLOWED_TRACKED_PREFIXES = ("psiv_tools/", "oracle/fixtures/")

#: One map keeps the guard to the map-independent sections plus one map's
#: emitters; the full pack's extra maps run the same per-map code.
GUARD_MAP_ARGS = ("--map", "0x10")

#: Runs in the build process: hook first, then the package as `-m` runs it.
#: argv: log path, then the psiv_tools command line.
BOOTSTRAP = r"""
import os, runpy, sys
log = open(sys.argv[1], "w", encoding="utf-8", buffering=1)
bases = [p for p in (sys.prefix, sys.base_prefix, sys.exec_prefix, sys.base_exec_prefix) if p]
bases += [p for p in sys.path if p and os.path.isabs(p)]
roots = {os.path.abspath(p) for p in bases} | {os.path.realpath(p) for p in bases}
log.write("ROOTS\t" + "\0".join(sorted(roots)) + "\n")
SKIP = os.O_DIRECTORY
FRESH = os.O_CREAT | os.O_EXCL
# shutil.rmtree opens each subdirectory by a name relative to a dir_fd the
# event does not carry, so its opens cannot be placed; it only deletes, so
# they are not recorded. Wrapped before psiv_tools is imported.
import shutil, threading
removing = threading.local()
_rmtree = shutil.rmtree
def rmtree(*a, **k):
    removing.depth = getattr(removing, "depth", 0) + 1
    try:
        return _rmtree(*a, **k)
    finally:
        removing.depth -= 1
rmtree.avoids_symlink_attacks = _rmtree.avoids_symlink_attacks
shutil.rmtree = rmtree
# A probe (`Path.is_file()`, `exists()`) stats without opening, and a build
# that probes for an absent input aborts before it opens anything; a failed
# stat is recorded too, so that input is named. os.stat is looked up at call
# time by os.path and pathlib; the import system uses posix.stat directly.
_stat = os.stat
def stat(path, *a, **k):
    try:
        return _stat(path, *a, **k)
    except FileNotFoundError:
        if not isinstance(path, int) and not k.get("dir_fd"):
            log.write("OPEN\t" + os.path.abspath(os.fsdecode(path)) + "\n")
        raise
os.stat = stat
def hook(event, args, _write=log.write, _fsdecode=os.fsdecode, _abspath=os.path.abspath):
    if getattr(removing, "depth", 0):
        return
    if event in ("tempfile.mkdtemp", "tempfile.mkstemp"):
        _write("TEMP\t" + _abspath(_fsdecode(args[0])) + "\n")
        return
    if event != "open" or not args or args[0] is None or isinstance(args[0], int):
        return
    flags = args[2] if len(args) > 2 and isinstance(args[2], int) else 0
    # A directory open lists or walks (shutil.rmtree opens names relative to
    # a dir_fd the event does not carry); an exclusive create cannot read an
    # existing file. Neither is a read of an input.
    if flags & SKIP or flags & FRESH == FRESH:
        return
    _write("OPEN\t" + _abspath(_fsdecode(args[0])) + "\n")
sys.addaudithook(hook)
sys.argv = ["psiv_tools", *sys.argv[2:]]
sys.path.insert(0, os.getcwd())
runpy.run_module("psiv_tools", run_name="__main__", alter_sys=True)
"""


def tracked_files(repo: Path = REPO) -> list[str]:
    """The files of the change, repository-relative (the one owner of that
    list is `tools/repo_files.py`); an ignored local input is never one."""
    return repo_files(repo)


def copy_tracked_tree(repo: Path, names: list[str], tree: Path) -> None:
    """Copy each tracked regular file (a tracked symlink is copied as one)."""
    for name in names:
        source = repo / name
        target = tree / name
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_symlink():
            target.symlink_to(os.readlink(source))
        elif source.is_file():
            shutil.copyfile(source, target)
        # A tracked path deleted in the working tree is not copied.


def audited_build(tree: Path, out_root: Path, args: list[str],
                  timeout: float = 1500) -> tuple[subprocess.CompletedProcess, list[str], list[str]]:
    """Run `psiv_tools ARGS` from `tree` with the open hook; return
    `(process, allowed roots, opened absolute paths)`. The allowed roots are
    the interpreter's and every scratch directory or file the build itself
    created through `tempfile` (a section's staging directory)."""
    log = out_root / "opens.log"
    process = subprocess.run(
        [sys.executable, "-I", "-B", "-c", BOOTSTRAP, str(log), *args],
        cwd=tree, capture_output=True, text=True, timeout=timeout,
    )
    roots: list[str] = []
    opened: list[str] = []
    for line in log.read_text(encoding="utf-8").splitlines():
        kind, _, value = line.partition("\t")
        if kind == "ROOTS":
            roots += [root for root in value.split("\0") if root]
        elif kind == "TEMP":
            roots += [value, os.path.realpath(value)]
        elif kind == "OPEN":
            opened.append(value)
    return process, roots, opened


def _allowed_tracked(relative: str, tracked: set[str]) -> bool:
    """A tracked file under an allowed prefix, or the bytecode cache the
    import system probes for one (`-B` stops writes, not that probe)."""
    if relative in tracked:
        return relative.startswith(ALLOWED_TRACKED_PREFIXES)
    parent, _, name = relative.rpartition("/__pycache__/")
    if parent and name.endswith(".pyc") and name.count(".") == 2:
        source = f"{parent}/{name.split('.', 1)[0]}.py"
        return source in tracked and source.startswith(ALLOWED_TRACKED_PREFIXES)
    return False


def offending_opens(opened: list[str], *, tree: Path, rom: Path, out_dir: Path,
                    tracked: set[str], roots: list[str]) -> list[str]:
    """The opened paths outside the allowed inputs, sorted, each once.

    `tree` paths are judged by their tree-relative name, so a path the build
    tried to open but the scratch tree lacks (an untracked input) is named."""
    tree = Path(os.path.realpath(tree))
    out_dir = Path(os.path.realpath(out_dir))
    rom = Path(os.path.realpath(rom))
    # A root that holds the tree would excuse every untracked read in it.
    root_paths = [Path(root) for root in roots if not tree.is_relative_to(root)]
    bad: set[str] = set()
    for raw in opened:
        # A Nix-style environment links each library file into its prefix, so
        # the interpreter's own files are judged by the path it opened too.
        if any(Path(raw).is_relative_to(root) for root in root_paths):
            continue
        path = Path(os.path.realpath(raw))
        if path == rom or path.is_relative_to(out_dir):
            continue
        if path.is_relative_to(tree):
            relative = path.relative_to(tree).as_posix()
            if _allowed_tracked(relative, tracked):
                continue
            bad.add(relative)
            continue
        if any(path.is_relative_to(root) for root in root_paths):
            continue
        bad.add(str(path))
    return sorted(bad)


def guard_report(process: subprocess.CompletedProcess, offenders: list[str]) -> str:
    lines = [f"the pack builder opened or probed {len(offenders)} path(s) outside the ROM, "
             "tracked psiv_tools/ and oracle/fixtures/, the interpreter and its output:"]
    lines += [f"  {name}" for name in offenders]
    if process.returncode != 0:
        tail = "\n".join(process.stderr.strip().splitlines()[-4:])
        lines.append(f"the build exited {process.returncode}; its last stderr lines:\n{tail}")
    return "\n".join(lines)


def _have_git_checkout() -> bool:
    return shutil.which("git") is not None and (REPO / ".git").exists()


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(_have_git_checkout(), "needs a git checkout to list its files")
class TestPackInputs(unittest.TestCase):

    def setUp(self):
        self._scratch = tempfile.TemporaryDirectory(prefix="psiv-pack-inputs-")
        self.addCleanup(self._scratch.cleanup)
        scratch = Path(self._scratch.name)
        self.tree = scratch / "tree"
        self.out_root = scratch / "out"
        self.out_root.mkdir()
        self.tracked = tracked_files()
        copy_tracked_tree(REPO, self.tracked, self.tree)
        shutil.copyfile(ROM, self.tree / ROM_NAME)

    def _run(self, extra: list[str]) -> tuple[subprocess.CompletedProcess, list[str]]:
        rom = self.tree / ROM_NAME
        out_dir = self.out_root / "pack-root"
        process, roots, opened = audited_build(
            self.tree, self.out_root, ["pack", str(rom), str(out_dir / "pack"), *extra])
        self.assertTrue(roots, "the bootstrap wrote no interpreter roots")
        self.assertIn(os.path.realpath(rom), {os.path.realpath(p) for p in opened},
                      "the hook saw no open of the ROM; it is not recording")
        offenders = offending_opens(opened, tree=self.tree, rom=rom, out_dir=out_dir,
                                    tracked=set(self.tracked), roots=roots)
        return process, offenders

    def test_pack_reads_only_the_rom_and_tracked_inputs(self):
        started = time.monotonic()
        process, offenders = self._run(list(GUARD_MAP_ARGS))
        elapsed = time.monotonic() - started
        self.assertEqual(offenders, [], guard_report(process, offenders))
        self.assertEqual(process.returncode, 0, guard_report(process, offenders))
        print(f"\n[pack-inputs] audited one-map pack build: {elapsed:.1f} s",
              file=sys.stderr)

    def test_planted_untracked_read_is_named(self):
        # Negative control: an untracked file the builder package reads at
        # import time. `pack --help` exits after argument parsing, so the
        # control costs an import, not a build; the ROM is not opened then,
        # so this run is judged without the ROM-open check.
        # The second name is built from pieces: it exists only in the scratch
        # tree, and the path checker reads string literals.
        planted = ("oracle/layouts/planted_capture.json",
                   "/".join(("psiv_tools", "untracked_table.json")))
        for name in planted:
            (self.tree / name).parent.mkdir(parents=True, exist_ok=True)
            (self.tree / name).write_text("{}")
        init = self.tree / "psiv_tools/__init__.py"
        init.write_text(
            init.read_text()
            + "\nfrom pathlib import Path as _P\n"
            + "".join(f"_P(__file__).resolve().parents[1].joinpath({name!r}).read_text()\n"
                      for name in planted)
        )
        rom = self.tree / ROM_NAME
        out_dir = self.out_root / "pack-root"
        process, roots, opened = audited_build(
            self.tree, self.out_root,
            ["pack", str(rom), str(out_dir / "pack"), "--help"], timeout=120)
        self.assertEqual(process.returncode, 0, process.stderr)
        offenders = offending_opens(opened, tree=self.tree, rom=rom, out_dir=out_dir,
                                    tracked=set(self.tracked), roots=roots)
        # An untracked file outside the allowed prefixes, and one inside
        # psiv_tools/ that is not tracked: both are named, nothing else is.
        self.assertEqual(
            offenders,
            list(planted),
        )
        self.assertIn("oracle/layouts/planted_capture.json",
                      guard_report(process, offenders))

    def test_tracked_inputs_outside_the_allowed_prefixes_are_named(self):
        # A tracked file is still an offender outside psiv_tools/ and
        # oracle/fixtures/ (a doc or a tool's data is not a pack input).
        tree = Path(os.path.realpath(self.tree))
        opened = [str(tree / "docs/README.md"), str(tree / "psiv_tools/pack.py"),
                  str(tree / "oracle/fixtures/battle_animation_remainder.json")]
        offenders = offending_opens(
            opened, tree=tree, rom=tree / ROM_NAME, out_dir=self.out_root,
            tracked={"docs/README.md", "psiv_tools/pack.py",
                     "oracle/fixtures/battle_animation_remainder.json"},
            roots=[])
        self.assertEqual(offenders, ["docs/README.md"])

    def test_paths_outside_tree_and_interpreter_are_named(self):
        opened = ["/etc/hostname", str(Path(sys.prefix) / "lib" / "module.txt")]
        offenders = offending_opens(
            opened, tree=self.tree, rom=self.tree / ROM_NAME, out_dir=self.out_root,
            tracked=set(), roots=[os.path.realpath(sys.prefix)])
        self.assertEqual(offenders, [os.path.realpath("/etc/hostname")])


if __name__ == "__main__":
    unittest.main()
