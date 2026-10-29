#!/usr/bin/env python3
"""Link the main checkout's local inputs into a worktree, with `generated/`
and `runtime-pack` built once per extractor and shared.

    python3 tools/local_inputs.py link

Run it anywhere in a worktree. Every local input the main checkout has
(`LINKED_INPUTS`, and the ignored attack frames `LINKED_INPUT_GLOBS` names) is
linked in where the worktree has none of its own. `generated/` and
`runtime-pack` are not the main checkout's: they are functions of this
commit's extractor, so they link to an entry of the shared cache under the
main checkout's `build/inputs/`, keyed by what builds them:

* `generated-<key>`: the Git tree of `psiv_tools/` and the ROM's SHA-256, built
  with `python3 -m psiv_tools regenerate`.
* `runtime-pack-<key>`: the trees of `psiv_tools/` and `oracle/fixtures/` and
  the ROM's SHA-256, built with `python3 -m psiv_tools pack`.

The ignored captures under `oracle/` are linked from the main checkout, the
same files for every worktree, so they key nothing.

A missing entry is built by the first worktree that needs it, with this
commit's `psiv_tools` under the heavy-run lock, into a staging directory that
is renamed into place only once the build succeeded. A second worktree that
needs the same entry meanwhile waits on the entry's lock, saying so every
`WAIT_REPORT_S`, and then links it: a stale pack is built once, not once per
gate job. Each link records the use; an entry unused for `PRUNE_AFTER_S` is
removed after the next build. A worktree with uncommitted changes under the
keyed paths has no key, so it is refused, and builds in place instead.

The steward's gate job runs main's copy on any commit
(`git show origin/main:tools/local_inputs.py | python3 - link`), so this file
imports nothing from `tools/`; the builds go through the worktree's own
`tools/heavy_lock.py`. Exit 0 linked, 1 a build failed (nothing cached), 2
refused (nothing changed).
"""

from __future__ import annotations

import argparse
import errno
import fcntl
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

EXIT_OK = 0
EXIT_FAILED = 1
EXIT_REFUSED = 2

#: Every local input a worktree links from the main checkout.
LINKED_INPUTS = (
    "runtime-pack", "reference", "generated", "saves", "build", "roms",
    "oracle/gpgx-src", "oracle/core", "oracle/bin", "oracle/frames",
    "oracle/states", "oracle/layouts", "oracle/logs",
    "Phantasy Star IV (USA).md",
)

#: Ignored files inside tracked directories, linked one by one.
LINKED_INPUT_GLOBS = ("oracle/fixtures/battle_animation_art/*.png",)

ROM_NAME = "Phantasy Star IV (USA).md"

#: Bumped when what goes into a key changes.
KEY_FORMAT = 1

#: Where the entries live, under the main checkout.
CACHE_DIR = Path("build") / "inputs"

#: Seconds between "still waiting" lines while another job builds an entry.
WAIT_REPORT_S = 60

#: An entry not linked for this long is removed after the next build.
PRUNE_AFTER_S = 3 * 24 * 3600

USED_FILE = "used"
KEY_FILE = "key.json"


class Refused(Exception):
    """Nothing was linked; the message says why."""


@dataclass(frozen=True)
class Entry:
    """One cached input: `name` is the worktree path it is linked as."""

    name: str
    trees: tuple[str, ...]
    lock_class: str
    command: tuple[str, ...]

    def key(self, worktree: Path, rom_sha256: str) -> dict:
        return {
            "format": KEY_FORMAT,
            "input": self.name,
            "rom_sha256": rom_sha256,
            "trees": {tree: _git(worktree, "rev-parse", f"HEAD:{tree}") for tree in self.trees},
        }


#: What each cached input is built from and by. The command's `{rom}` and
#: `{output}` are filled in; it runs in the worktree, under the heavy lock.
ENTRIES = (
    Entry("generated", ("psiv_tools",), "python",
          ("python3", "-m", "psiv_tools", "regenerate", "{rom}", "{output}")),
    Entry("runtime-pack", ("psiv_tools", "oracle/fixtures"), "pack",
          ("python3", "-m", "psiv_tools", "pack", "{rom}", "{output}")),
)


def _git(cwd: Path, *args: str) -> str:
    result = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True,
                            check=False)
    if result.returncode != 0:
        raise Refused(f"git {' '.join(args)} failed in {cwd}: {result.stderr.strip()}")
    return result.stdout.strip()


def main_checkout(worktree: Path) -> Path:
    """The checkout whose `.git` the worktree shares."""
    common = _git(worktree, "rev-parse", "--path-format=absolute", "--git-common-dir")
    return Path(common).parent


def key_digest(key: dict) -> str:
    return hashlib.sha256(json.dumps(key, sort_keys=True).encode("utf-8")).hexdigest()[:24]


def build_with_lock(entry: Entry, worktree: Path, rom: Path, output: Path) -> None:
    """Build `entry` into `output` with the worktree's `psiv_tools`, under its heavy lock."""
    command = [part.format(rom=rom, output=output) for part in entry.command]
    subprocess.run([sys.executable, "tools/heavy_lock.py", "run", "--class", entry.lock_class,
                    "--", *command], cwd=worktree, check=True)


def _say(message: str) -> None:
    print(f"local inputs: {message}", flush=True)


def _duration(seconds: float) -> str:
    minutes, seconds = divmod(int(seconds), 60)
    return f"{minutes}m{seconds:02d}s" if minutes else f"{seconds}s"


def _lock(path: Path, label: str) -> int:
    """Hold `path`'s flock, saying every `WAIT_REPORT_S` that it still waits."""
    descriptor = os.open(path, os.O_RDWR | os.O_CREAT, 0o644)
    started = time.monotonic()
    reported = started
    try:
        while True:
            try:
                fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
                return descriptor
            except OSError as error:
                if error.errno not in (errno.EAGAIN, errno.EACCES):
                    raise
            now = time.monotonic()
            if now - reported >= WAIT_REPORT_S:
                _say(f"still waiting for {label}, which another job is building "
                     f"({_duration(now - started)} so far)")
                reported = now
            time.sleep(min(1.0, WAIT_REPORT_S))
    except BaseException:
        os.close(descriptor)
        raise


def _try_lock(path: Path) -> int | None:
    descriptor = os.open(path, os.O_RDWR | os.O_CREAT, 0o644)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return descriptor
    except OSError:
        os.close(descriptor)
        return None


def _alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def ensure_entry(entry: Entry, worktree: Path, cache: Path, rom: Path, rom_sha256: str,
                 build: Callable[[Entry, Path, Path, Path], None] = build_with_lock) -> Path:
    """The entry directory for this commit's `entry`, built first if no job has."""
    key = entry.key(worktree, rom_sha256)
    label = f"{entry.name}-{key_digest(key)}"
    directory = cache / label
    cache.mkdir(parents=True, exist_ok=True)
    descriptor = _lock(cache / f".{label}.lock", label)
    try:
        if directory.is_dir():
            _say(f"{entry.name}: reusing {label}")
        else:
            staging = cache / f".{label}.build-{os.getpid()}"
            if staging.exists():
                shutil.rmtree(staging)
            staging.mkdir()
            started = time.monotonic()
            _say(f"{entry.name}: building {label} for this commit")
            try:
                build(entry, worktree, rom, staging / entry.name)
                (staging / KEY_FILE).write_text(json.dumps(key, indent=2, sort_keys=True) + "\n")
                os.rename(staging, directory)
            except BaseException:
                shutil.rmtree(staging, ignore_errors=True)
                raise
            _say(f"{entry.name}: built {label} in {_duration(time.monotonic() - started)}")
            prune(cache, keep=directory)
        (directory / USED_FILE).touch()
    finally:
        os.close(descriptor)
    return directory


def prune(cache: Path, keep: Path, now: float | None = None) -> list[str]:
    """Remove entries unused for `PRUNE_AFTER_S` and abandoned staging; returns their names."""
    now = time.time() if now is None else now
    removed = []
    for path in sorted(cache.iterdir()):
        if path == keep or not path.is_dir():
            continue
        if path.name.startswith("."):
            pid = path.name.rpartition(".build-")[2]
            if pid.isdigit() and not _alive(int(pid)):
                shutil.rmtree(path, ignore_errors=True)
                removed.append(path.name)
            continue
        used = path / USED_FILE
        last = used.stat().st_mtime if used.exists() else path.stat().st_mtime
        if now - last < PRUNE_AFTER_S:
            continue
        descriptor = _try_lock(cache / f".{path.name}.lock")
        if descriptor is None:
            continue
        try:
            shutil.rmtree(path, ignore_errors=True)
            removed.append(path.name)
        finally:
            os.close(descriptor)
    for name in removed:
        _say(f"removed {name}, unused for {PRUNE_AFTER_S // 86400} days or abandoned")
    return removed


def _link(worktree: Path, relative: str, target: Path, repoint: bool = False) -> bool:
    """Link `relative` in the worktree to `target`; False when the worktree has its own.

    With `repoint`, a link already there is pointed at `target`: a cached
    input's old link names another commit's entry, the very staleness this
    tool exists to stop."""
    path = worktree / relative
    if path.is_symlink() and repoint:
        if path.resolve() == target.resolve():
            return True
        path.unlink()
    elif os.path.lexists(path):
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    path.symlink_to(target, target_is_directory=target.is_dir())
    return True


def _dirty(worktree: Path, trees: tuple[str, ...]) -> list[str]:
    status = _git(worktree, "status", "--porcelain", "--untracked-files=all", "--", *trees)
    return [line[3:] for line in status.splitlines()]


def link(worktree: Path, build: Callable[[Entry, Path, Path, Path], None] = build_with_lock) -> None:
    worktree = Path(_git(worktree, "rev-parse", "--show-toplevel")).resolve()
    main = main_checkout(worktree)
    if main.resolve() == worktree:
        raise Refused("this is the main checkout, which keeps its own local inputs; "
                      "run this from a worktree")
    rom = main / ROM_NAME
    if not rom.is_file():
        raise Refused(f"the main checkout has no ROM ({ROM_NAME}); generated/ and "
                      "runtime-pack cannot be built")
    dirty = _dirty(worktree, tuple(sorted({tree for entry in ENTRIES for tree in entry.trees})))
    if dirty:
        raise Refused("uncommitted changes under the paths that key generated/ and runtime-pack "
                      f"({', '.join(dirty[:5])}{', ...' if len(dirty) > 5 else ''}), so this "
                      "tree has no cache entry. Commit them, or build both in place: "
                      "docs/DEVELOPMENT.md, \"Prepare the local pack\"")

    cached = {entry.name for entry in ENTRIES}
    for relative in LINKED_INPUTS:
        source = main / relative
        if relative in cached or not os.path.lexists(source):
            continue
        if _link(worktree, relative, source):
            _say(f"{relative}: linked from the main checkout")
        else:
            _say(f"{relative}: kept this worktree's own")
    for pattern in LINKED_INPUT_GLOBS:
        sources = sorted(main.glob(pattern))
        own = sum(not _link(worktree, str(source.relative_to(main)), source) for source in sources)
        if sources:
            _say(f"{pattern}: {len(sources) - own} linked from the main checkout"
                 + (f", {own} kept" if own else ""))

    rom_sha256 = hashlib.sha256(rom.read_bytes()).hexdigest()
    cache = main / CACHE_DIR
    for entry in ENTRIES:
        directory = ensure_entry(entry, worktree, cache, rom, rom_sha256, build)
        if not _link(worktree, entry.name, directory / entry.name, repoint=True):
            _say(f"{entry.name}: kept this worktree's own (a directory, not a link); "
                 "remove it to use the shared one")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 tools/local_inputs.py",
        description="Link the main checkout's local inputs into a worktree, with generated/ "
                    "and runtime-pack built once per extractor and shared.")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("link", help="link the inputs into the worktree the current directory is in")
    parser.parse_args(argv)
    try:
        link(Path.cwd())
    except Refused as error:
        print(f"local inputs: refused: {error}", file=sys.stderr)
        return EXIT_REFUSED
    except subprocess.CalledProcessError as error:
        print(f"local inputs: the build exited {error.returncode}; its output is above, "
              "and nothing was cached", file=sys.stderr)
        return EXIT_FAILED
    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
