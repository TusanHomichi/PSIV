"""The extract stamp: how `generated/` refuses to be read after the extractor changed.

`python3 -m psiv_tools extract` writes `generated/*.json`; `art`, `planes` and the
battle-art and layout exporters write PNG directories under it. Every one of
them is a function of the `psiv_tools` source, and none of them used to say
which source. A reader of an older extract got the old tables without a word
(#103: `formations.json` without the inline formations, `graphics.json` and
`planes.json` with the old 3-bit palette expansion).

This module owns the fix, and nothing else decides it.

* **The stamp** is a SHA-256 over the source bytes of every `psiv_tools` module
  the producing entry point can reach through its imports (`import`, `from .x
  import y`, lazy imports inside functions included). The per-module hashes are
  stored beside it so a refusal can name the modules that changed.
* **One stamp for the whole JSON extract**, not one per file. `core.extract_all`
  produces all of the tables in one run and `core` imports every extractor, so
  the module graph reaches the same set from each of them: a per-file
  attribution would claim a precision the graph does not have, and a missed
  drift is worse than a spurious refusal. A pure refactor of an extractor
  module changes the stamp too; that costs a re-extract and is accepted.
* **Tables** carry their stamp in `metadata.json` (`extract_stamp`). **PNG
  directories** carry `extract_stamp.json` beside the images, written by the
  function that writes them.
* **Reading** goes through `load_table` / `load_table_file`. A directory that is
  absent stays an ordinary `FileNotFoundError`, so a reader that skips when the
  local input is missing keeps skipping. A directory that is present but
  unstamped or stamped by other source raises `StaleExtractError`, whose message
  names the file, both stamps, the changed modules and the rebuild command.
  `tests/test_extract_stamp.py` fails when any other module reads `generated/`.

`python3 -m psiv_tools regenerate <rom> generated` rebuilds all of it.
"""
from __future__ import annotations

import ast
import hashlib
import json
from functools import lru_cache
from pathlib import Path
from typing import Any

PACKAGE = "psiv_tools"
PACKAGE_DIR = Path(__file__).resolve().parent

#: Bumped when this module's own hashing changes; it is part of every stamp.
#: The module itself is left out of the hashed graph (`EXCLUDED_MODULES`): it
#: decides the stamp, it does not shape a table or a pixel.
STAMP_FORMAT = 1
EXCLUDED_MODULES = frozenset({"psiv_tools.extract_stamp"})

#: Beside the PNGs of a directory; `metadata.json` carries the JSON extract's.
STAMP_FILE = "extract_stamp.json"
METADATA_FILE = "metadata.json"
METADATA_KEY = "extract_stamp"

#: What each kind of output is produced from: the module whose import graph is
#: hashed. `core` writes every JSON table; each exporter writes its PNG directory.
KIND_ENTRY_MODULES = {
    "tables": "psiv_tools.core",
    "gfx": "psiv_tools.gfx",
    "planes": "psiv_tools.planes",
    "layouts": "psiv_tools.layouts",
    "battle_art": "psiv_tools.battle_art",
}

#: Shown in every refusal. The ROM stays a local input, so its path is the reader's.
REBUILD_COMMAND = 'python3 -m psiv_tools regenerate "<path to Phantasy Star IV (USA).md>" generated'


class StaleExtractError(RuntimeError):
    """A present `generated/` output that this checkout's extractor did not write."""

    def __init__(self, path: Path, kind: str, found: dict[str, Any] | None,
                 expected: dict[str, Any], command: str = REBUILD_COMMAND) -> None:
        self.path = Path(path)
        self.kind = kind
        self.found = found
        self.expected = expected
        self.command = command
        super().__init__(self._message())

    def _message(self) -> str:
        if self.found is None:
            seen = "no stamp (an extract from before stamping existed, or one written by hand)"
        else:
            seen = f"stamp {self.found.get('stamp', '?')[:16]}"
        lines = [
            f"{self.path} is a stale {self.kind} extract: it carries {seen}, "
            f"but this checkout's extractor is stamp {self.expected['stamp'][:16]}.",
        ]
        changed = describe_difference(self.found, self.expected)
        if changed:
            lines.append("Extractor modules that differ: " + ", ".join(changed) + ".")
        lines.append(f"Rebuild it with: {self.command}")
        return " ".join(lines)


# ---------------------------------------------------------------------------
# The stamp: a hash over the modules an entry point reaches
# ---------------------------------------------------------------------------
def _module_file(package_dir: Path, name: str) -> Path | None:
    """The file of module `name` (`psiv_tools.maps.x`) under `package_dir`, if it is ours."""
    parts = name.split(".")
    if parts[0] != PACKAGE:
        return None
    base = package_dir.joinpath(*parts[1:])
    for candidate in (base.with_suffix(".py"), base / "__init__.py"):
        if candidate.is_file():
            return candidate
    return None


def _imported_names(source: str, module: str, is_package: bool) -> set[str]:
    """Every dotted name `source` imports, resolved against `module`, anywhere in the file."""
    package = module if is_package else module.rpartition(".")[0]
    found: set[str] = set()
    for node in ast.walk(ast.parse(source)):
        if isinstance(node, ast.Import):
            found.update(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom):
            if node.level:
                parts = package.split(".")
                base = parts[:len(parts) - (node.level - 1)]
                if node.module:
                    base = base + node.module.split(".")
                target = ".".join(base)
            else:
                target = node.module or ""
            found.add(target)
            # `from . import x` and `from .pkg import x` may name submodules.
            found.update(f"{target}.{alias.name}" for alias in node.names)
    return found


def source_modules(entry: str, package_dir: Path = PACKAGE_DIR) -> dict[str, Path]:
    """`{module name: file}` for `entry` and every module of the package it can reach.

    A module's parent packages are included: importing `psiv_tools.maps.x` runs
    `psiv_tools/maps/__init__.py` first.
    """
    reached: dict[str, Path] = {}
    pending = [entry]
    while pending:
        name = pending.pop()
        if name in reached or name in EXCLUDED_MODULES:
            continue
        path = _module_file(package_dir, name)
        if path is None:
            continue
        reached[name] = path
        parent = name.rpartition(".")[0]
        if parent:
            pending.append(parent)
        pending.extend(sorted(_imported_names(
            path.read_text(encoding="utf-8"), name, path.name == "__init__.py")))
    return reached


def _record(kind: str, modules: dict[str, Path], package_dir: Path) -> dict[str, Any]:
    hashes = {
        path.relative_to(package_dir.parent).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in modules.values()
    }
    hashes = dict(sorted(hashes.items()))
    digest = hashlib.sha256(
        json.dumps({"format": STAMP_FORMAT, "modules": hashes}, sort_keys=True).encode("utf-8")
    ).hexdigest()
    return {"kind": kind, "stamp": digest, "modules": hashes}


def source_stamp(kind: str, package_dir: Path = PACKAGE_DIR) -> dict[str, Any]:
    """The stamp record this checkout's source gives output of `kind`."""
    if package_dir == PACKAGE_DIR:
        return _cached_stamp(kind)
    return _record(kind, source_modules(KIND_ENTRY_MODULES[kind], package_dir), package_dir)


@lru_cache(maxsize=None)
def _cached_stamp(kind: str) -> dict[str, Any]:
    return _record(kind, source_modules(KIND_ENTRY_MODULES[kind]), PACKAGE_DIR)


def describe_difference(found: dict[str, Any] | None, expected: dict[str, Any]) -> list[str]:
    """The module paths whose hash differs between two stamp records."""
    if not found or not isinstance(found.get("modules"), dict):
        return []
    old, new = found["modules"], expected["modules"]
    return sorted(name for name in set(old) | set(new) if old.get(name) != new.get(name))


# ---------------------------------------------------------------------------
# Writing the stamp (the producers call these)
# ---------------------------------------------------------------------------
def table_stamp() -> dict[str, Any]:
    """The record `metadata.json` carries under `extract_stamp`."""
    return source_stamp("tables")


def write_png_stamp(directory: str | Path, kind: str) -> Path:
    """Stamp a PNG directory. The exporter that writes the images calls this."""
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / STAMP_FILE
    path.write_text(json.dumps(source_stamp(kind), indent=2) + "\n", encoding="utf-8")
    return path


def discard_png_stamp(directory: str | Path) -> None:
    """Remove a PNG directory's stamp before its images are rewritten.

    An exporter that stops half way then leaves images nothing vouches for.
    """
    (Path(directory) / STAMP_FILE).unlink(missing_ok=True)


def write_table_stamp(directory: str | Path) -> Path:
    """Stamp a table directory by merging into its `metadata.json`.

    `write_extract` embeds the stamp in the metadata it writes; this exists for
    a directory that is not a full extract (a test's synthetic one).
    """
    path = Path(directory) / METADATA_FILE
    metadata = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else {}
    metadata[METADATA_KEY] = table_stamp()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return path


# ---------------------------------------------------------------------------
# Reading (the only door into generated/)
# ---------------------------------------------------------------------------
def _read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def check_tables(directory: str | Path, command: str = REBUILD_COMMAND) -> None:
    """Raise `StaleExtractError` unless `directory`'s tables carry this source's stamp."""
    directory = Path(directory)
    expected = table_stamp()
    metadata_path = directory / METADATA_FILE
    found = None
    if metadata_path.is_file():
        try:
            value = _read_json(metadata_path)
        except ValueError:
            value = None
        if isinstance(value, dict) and isinstance(value.get(METADATA_KEY), dict):
            found = value[METADATA_KEY]
    if found is None or found.get("stamp") != expected["stamp"]:
        raise StaleExtractError(metadata_path, "table", found, expected, command)


def load_table(directory: str | Path, name: str, command: str = REBUILD_COMMAND) -> Any:
    """The parsed `<directory>/<name>.json`, refused unless its extract is current.

    A missing table is a `FileNotFoundError` and is never reported as stale, so a
    caller that skips on an absent local input keeps skipping.
    """
    path = Path(directory) / f"{name}.json"
    if not path.is_file():
        raise FileNotFoundError(f"{path} is not present (local input; build it with: {command})")
    try:
        check_tables(directory, command)
    except StaleExtractError as error:
        raise StaleExtractError(path, "table", error.found, error.expected, command) from None
    return _read_json(path)


def load_table_file(path: str | Path, command: str = REBUILD_COMMAND) -> Any:
    """`load_table` for a caller that holds the table's own path."""
    path = Path(path)
    return load_table(path.parent, path.stem, command)


def check_accumulating_png_directory(
    directory: str | Path, kind: str, rewriting: set[str], command: str = REBUILD_COMMAND
) -> None:
    """For an exporter that adds images to a directory it does not rebuild.

    One stamp cannot vouch for images the exporter is not rewriting, so images
    from another source must be cleared (or rebuilt) first: this raises
    `StaleExtractError` when the directory holds PNGs other than `rewriting`
    and does not carry this source's stamp.
    """
    directory = Path(directory)
    if any(path.name not in rewriting for path in directory.glob("*.png")):
        check_png_directory(directory, kind, command)


def check_png_directory(directory: str | Path, kind: str, command: str = REBUILD_COMMAND) -> None:
    """Raise `StaleExtractError` unless a PNG directory carries this source's stamp."""
    directory = Path(directory)
    expected = source_stamp(kind)
    stamp_path = directory / STAMP_FILE
    found = None
    if stamp_path.is_file():
        try:
            value = _read_json(stamp_path)
        except ValueError:
            value = None
        if isinstance(value, dict):
            found = value
    if found is None or found.get("kind") != kind or found.get("stamp") != expected["stamp"]:
        raise StaleExtractError(stamp_path, f"{kind} PNG", found, expected, command)
