"""Reuse the pack's map-independent sections across builds in one process.

Most of a `build_pack` call is spent on sections that `map_ids` does not
touch: battle records and art, presentation panels, dialogue, the title, sound,
travel and the ship menu. A process that builds several packs from one ROM (the
Python suite builds about 25 small packs) used to emit each of them from
scratch every time (#130).

`for_rom(rom)` returns the sections of that ROM, keyed by the SHA-256 of its
bytes. `Sections.emit` runs a section the first time into a private temporary
directory, keeps the files it wrote and the value it returned, and from then on
writes those same bytes into each build's directory and hands out a deep copy
of the value. `Sections.value` does the same for a section that writes no file.
What a section emits is a function of the ROM bytes and of checked-in inputs
under `oracle/`, which a process already treats as constant
(`battle_animation_receipts` caches its receipt file the same way), so a reused
section is the section a fresh emit would write.

Nothing is written to disk outside the build directory and nothing outlives
the process. Only the most recent `ROM_LIMIT` ROMs are held, so a test that
builds from a mutated copy cannot grow the store without bound. `fresh()`
turns reuse off for a block: the build-twice tests use it so that both of
their builds really run every emitter.
"""

from __future__ import annotations

import copy
import hashlib
import tempfile
from collections import OrderedDict
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterator, TypeVar

T = TypeVar("T")

#: How many ROMs' sections one process keeps.
ROM_LIMIT = 2


@dataclass(frozen=True)
class EmittedSection:
    """What one section wrote, relative to the pack root, and what it returned."""

    directories: tuple[str, ...]
    files: tuple[tuple[str, bytes], ...]
    result: Any

    def install(self, root: Path) -> None:
        for relative in self.directories:
            (root / relative).mkdir(parents=True, exist_ok=True)
        for relative, data in self.files:
            (root / relative).write_bytes(data)


def _snapshot(root: Path, result: Any) -> EmittedSection:
    directories: list[str] = []
    files: list[tuple[str, bytes]] = []
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            raise ValueError(f"a pack section wrote a symlink: {relative}")
        if path.is_dir():
            directories.append(relative)
        else:
            files.append((relative, path.read_bytes()))
    return EmittedSection(tuple(directories), tuple(files), copy.deepcopy(result))


class Sections:
    """The reusable sections of one ROM."""

    def __init__(self, digest: str, reuse: bool) -> None:
        self.digest = digest
        self.reuse = reuse
        self._emitted: dict[tuple[Any, ...], EmittedSection] = {}
        self._values: dict[tuple[Any, ...], Any] = {}
        #: Section emitters actually run, per key; read by the tests.
        self.runs: dict[tuple[Any, ...], int] = {}

    def _ran(self, key: tuple[Any, ...]) -> None:
        self.runs[key] = self.runs.get(key, 0) + 1

    def emit(
        self,
        directory: Path,
        key: tuple[Any, ...],
        emitter: Callable[[Path], T],
    ) -> T:
        """Run `emitter(pack_root)` once per `key`; replay its files after that.

        `key` names the section and every argument the emitter takes besides
        the ROM and the directory.
        """
        if not self.reuse:
            self._ran(key)
            return emitter(directory)
        section = self._emitted.get(key)
        if section is None:
            with tempfile.TemporaryDirectory(prefix="psiv-pack-section-") as scratch:
                self._ran(key)
                result = emitter(Path(scratch))
                section = _snapshot(Path(scratch), result)
            self._emitted[key] = section
        section.install(directory)
        return copy.deepcopy(section.result)

    def value(self, key: tuple[Any, ...], compute: Callable[[], T]) -> T:
        """`compute()` once per `key`; a deep copy of its value after that."""
        if not self.reuse:
            self._ran(key)
            return compute()
        if key not in self._values:
            self._ran(key)
            self._values[key] = copy.deepcopy(compute())
        return copy.deepcopy(self._values[key])


_store: OrderedDict[str, Sections] = OrderedDict()
_reuse = [True]


def rom_digest(rom: bytes) -> str:
    return hashlib.sha256(rom).hexdigest()


def for_rom(rom: bytes) -> Sections:
    """The sections of `rom`: shared across builds unless `fresh()` is active."""
    digest = rom_digest(rom)
    if not _reuse[-1]:
        return Sections(digest, reuse=False)
    sections = _store.get(digest)
    if sections is None:
        sections = _store[digest] = Sections(digest, reuse=True)
        while len(_store) > ROM_LIMIT:
            _store.popitem(last=False)
    else:
        _store.move_to_end(digest)
    return sections


@contextmanager
def fresh() -> Iterator[None]:
    """Run every section emitter inside the block; read and keep nothing."""
    _reuse.append(False)
    try:
        yield
    finally:
        _reuse.pop()


def clear() -> None:
    """Forget every kept section."""
    _store.clear()
