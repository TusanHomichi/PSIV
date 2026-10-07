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
* **The stamp binds contents, not just the directory.** It lists every file the
  producer wrote with its SHA-256 (`files`). A table that is not listed, or whose
  bytes differ, is refused, so an old file left in a freshly stamped directory
  (a table a later extractor dropped, a stale file copied over a current one)
  cannot load. A PNG directory that holds a file the stamp does not list is
  refused as a whole. Producers remove the files their *previous* stamp listed
  and they no longer write, and nothing else: they never clear a directory.
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
from collections.abc import Iterable
from functools import lru_cache
from pathlib import Path
from typing import Any

PACKAGE = "psiv_tools"
PACKAGE_DIR = Path(__file__).resolve().parent

#: Bumped when this module's own hashing changes; it is part of every stamp.
#: The module itself is left out of the hashed graph (`EXCLUDED_MODULES`): it
#: decides the stamp, it does not shape a table or a pixel.
STAMP_FORMAT = 2
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
                 expected: dict[str, Any], command: str = REBUILD_COMMAND,
                 reason: str | None = None) -> None:
        self.path = Path(path)
        self.kind = kind
        self.found = found
        self.expected = expected
        self.command = command
        #: Set when the stamp matches but the file or directory does not (contents).
        self.reason = reason
        super().__init__(self._message())

    def _message(self) -> str:
        if self.reason:
            lines = [f"{self.path} {self.reason}; this {self.kind} extract's stamp "
                     f"{self.expected['stamp'][:16]} does not vouch for it."]
            lines.append(f"Rebuild it with: {self.command}")
            return " ".join(lines)
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
    """The source half of the record `metadata.json` carries under `extract_stamp`."""
    return source_stamp("tables")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def metadata_digest(metadata: dict[str, Any]) -> str:
    """Digest of a `metadata.json` document without the stamp it carries.

    The stamp lives in `metadata.json` and cannot list itself; the rest of the
    document is bound the same way as any other table.
    """
    body = {key: value for key, value in metadata.items() if key != METADATA_KEY}
    return digest(json.dumps(body, sort_keys=True).encode("utf-8"))


def _listed(record: Any) -> dict[str, str]:
    files = record.get("files") if isinstance(record, dict) else None
    return dict(files) if isinstance(files, dict) else {}


def _safe_relative(name: str) -> Path | None:
    path = Path(name)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        return None
    return path


def _remove_listed(directory: Path, names: Iterable[str]) -> None:
    """Delete exactly these files of an earlier stamp. Never a directory, never a glob."""
    for name in names:
        relative = _safe_relative(name)
        if relative is not None and (directory / relative).is_file():
            (directory / relative).unlink()


def _previous_tables_record(directory: Path) -> Any:
    try:
        metadata = _read_json(directory / METADATA_FILE)
    except (OSError, ValueError):
        return None
    return metadata.get(METADATA_KEY) if isinstance(metadata, dict) else None


def begin_table_export(directory: str | Path) -> set[str]:
    """Before an extract rewrites `directory`: drop the old stamp, return the files it listed.

    `metadata.json` carries the stamp, so removing it first leaves any table
    nothing vouches for until `finish_table_export` writes it again.
    """
    directory = Path(directory)
    previous = set(_listed(_previous_tables_record(directory)))
    (directory / METADATA_FILE).unlink(missing_ok=True)
    return previous


def finish_table_export(directory: str | Path, metadata: dict[str, Any],
                        written: dict[str, bytes], previous: set[str]) -> None:
    """Write `metadata.json` last, listing `written` (`{file name: bytes}`).

    Files the previous stamp listed that this run did not write are removed: a
    table a later extractor drops does not stay behind to load.
    """
    directory = Path(directory)
    files = {name: digest(data) for name, data in sorted(written.items())}
    files[METADATA_FILE] = metadata_digest(metadata)
    _remove_listed(directory, previous - set(files))
    document = {**metadata, METADATA_KEY: {**table_stamp(), "files": files}}
    (directory / METADATA_FILE).write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")


def write_table_stamp(directory: str | Path) -> Path:
    """Stamp a table directory that is not a full extract (a test's synthetic one).

    Lists every `*.json` table in it, so a table written afterwards must be
    stamped again.
    """
    directory = Path(directory)
    path = directory / METADATA_FILE
    metadata = _read_json(path) if path.is_file() else {}
    files = {table.name: digest(table.read_bytes())
             for table in sorted(directory.glob("*.json")) if table.name != METADATA_FILE}
    files[METADATA_FILE] = metadata_digest(metadata)
    metadata[METADATA_KEY] = {**table_stamp(), "files": files}
    directory.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return path


def begin_png_export(directory: str | Path) -> None:
    """Before an exporter rewrites a PNG directory: remove the files its last stamp listed.

    Only those files and the stamp. An exporter that stops half way then leaves
    a directory nothing vouches for, and an image a later exporter no longer
    writes does not stay behind.
    """
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    stamp_path = directory / STAMP_FILE
    try:
        previous = _listed(_read_json(stamp_path))
    except (OSError, ValueError):
        previous = {}
    _remove_listed(directory, previous)
    stamp_path.unlink(missing_ok=True)


def write_png_stamp(directory: str | Path, kind: str, written: Iterable[str | Path],
                    keep_previous: bool = False) -> Path:
    """Stamp a PNG directory, listing the files the exporter wrote (with their hashes).

    `keep_previous` is for an exporter that adds to a directory it does not
    rebuild (`export_map_pngs`): the files the existing stamp lists stay listed.
    """
    directory = Path(directory)
    stamp_path = directory / STAMP_FILE
    names: set[str] = set()
    if keep_previous and stamp_path.is_file():
        try:
            names.update(_listed(_read_json(stamp_path)))
        except ValueError:
            pass
    for item in written:
        item = Path(item)
        try:
            item = item.relative_to(directory)
        except ValueError:
            pass  # already relative to the directory
        names.add(item.as_posix())
    files = {}
    for name in sorted(names):
        relative = _safe_relative(name)
        if relative is not None and (directory / relative).is_file():
            files[name] = digest((directory / relative).read_bytes())
    stamp_path.write_text(json.dumps({**source_stamp(kind), "files": files}, indent=2) + "\n",
                          encoding="utf-8")
    return stamp_path


# ---------------------------------------------------------------------------
# Reading (the only door into generated/)
# ---------------------------------------------------------------------------
def _read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _tables_record(directory: Path) -> dict[str, Any] | None:
    metadata_path = directory / METADATA_FILE
    if not metadata_path.is_file():
        return None
    try:
        value = _read_json(metadata_path)
    except ValueError:
        return None
    if isinstance(value, dict) and isinstance(value.get(METADATA_KEY), dict):
        return value[METADATA_KEY]
    return None


def check_tables(directory: str | Path, command: str = REBUILD_COMMAND) -> dict[str, Any]:
    """Raise `StaleExtractError` unless `directory`'s tables carry this source's stamp.

    Returns the stamp record. This checks the source half only; `load_table`
    also binds the file it opens to the record's list.
    """
    directory = Path(directory)
    expected = table_stamp()
    found = _tables_record(directory)
    if found is None or found.get("stamp") != expected["stamp"]:
        raise StaleExtractError(directory / METADATA_FILE, "table", found, expected, command)
    return found


def _table_bytes(directory: Path, name: str, command: str) -> tuple[bytes, str]:
    """`(bytes, sha256)` of a table the current stamp lists and the bytes match."""
    path = directory / f"{name}.json"
    if not path.is_file():
        raise FileNotFoundError(f"{path} is not present (local input; build it with: {command})")
    try:
        record = check_tables(directory, command)
    except StaleExtractError as error:
        raise StaleExtractError(path, "table", error.found, error.expected, command) from None
    expected = table_stamp()
    listed = _listed(record).get(path.name)
    if listed is None:
        raise StaleExtractError(
            path, "table", record, expected, command,
            reason="is not one of the files the extract's stamp lists "
                   "(left over from an older extract, or added by hand)")
    data = path.read_bytes()
    if path.name != METADATA_FILE and digest(data) != listed:
        raise StaleExtractError(
            path, "table", record, expected, command,
            reason="has changed since the extract was stamped "
                   f"(sha256 {digest(data)[:16]}, the stamp lists {listed[:16]})")
    return data, listed


def load_table(directory: str | Path, name: str, command: str = REBUILD_COMMAND) -> Any:
    """The parsed `<directory>/<name>.json`, refused unless the current extractor wrote it.

    Refused when the directory's stamp is another source's, when the file is not
    one the stamp lists, or when its bytes are not the ones that were listed. A
    missing table is a `FileNotFoundError` and is never reported as stale, so a
    caller that skips on an absent local input keeps skipping.
    """
    directory = Path(directory)
    data, _ = _table_bytes(directory, name, command)
    document = json.loads(data.decode("utf-8"))
    if name == "metadata":
        # The stamp cannot list itself: the rest of the document is what is bound.
        listed = _listed(_tables_record(directory)).get(METADATA_FILE)
        if not isinstance(document, dict) or metadata_digest(document) != listed:
            raise StaleExtractError(
                directory / METADATA_FILE, "table", _tables_record(directory), table_stamp(),
                command, reason="has changed since the extract was stamped")
    return document


def table_sha256(directory: str | Path, name: str, command: str = REBUILD_COMMAND) -> str:
    """The SHA-256 the stamp lists for a table, after the same checks as `load_table`.

    For provenance records that want the hash of an input without reading it.
    """
    if name == "metadata":
        raise ValueError("metadata.json is bound by its contents without the stamp; load it instead")
    return _table_bytes(Path(directory), name, command)[1]


def load_table_file(path: str | Path, command: str = REBUILD_COMMAND) -> Any:
    """`load_table` for a caller that holds the table's own path."""
    path = Path(path)
    return load_table(path.parent, path.stem, command)


def check_png_directory(directory: str | Path, kind: str, command: str = REBUILD_COMMAND) -> None:
    """Raise `StaleExtractError` unless a PNG directory is what this source's exporter wrote.

    The stamp must be this source's; every file it lists must be present with
    the listed hash; and no file the stamp does not list may be present.
    """
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
    listed = _listed(found)

    def refuse(reason: str) -> StaleExtractError:
        return StaleExtractError(directory, f"{kind} PNG", found, expected, command, reason=reason)

    for name, want in sorted(listed.items()):
        relative = _safe_relative(name)
        path = directory / relative if relative is not None else None
        if path is None or not path.is_file():
            raise refuse(f"is missing {name}, which its stamp lists")
        if digest(path.read_bytes()) != want:
            raise refuse(f"holds {name} changed since the stamp was written")
    present = {path.relative_to(directory).as_posix() for path in directory.rglob("*") if path.is_file()}
    extra = sorted(present - set(listed) - {STAMP_FILE})
    if extra:
        shown = ", ".join(extra[:5]) + (f" and {len(extra) - 5} more" if len(extra) > 5 else "")
        raise refuse(f"holds files its stamp does not list ({shown}); remove them")


def check_accumulating_png_directory(
    directory: str | Path, kind: str, rewriting: set[str], command: str = REBUILD_COMMAND
) -> None:
    """For an exporter that adds images to a directory it does not rebuild.

    One stamp cannot vouch for images the exporter is not rewriting, so this
    raises `StaleExtractError` when the directory holds files other than
    `rewriting` and is not wholly what this source's exporter wrote.
    """
    directory = Path(directory)
    others = [path for path in directory.rglob("*")
              if path.is_file() and path.name != STAMP_FILE
              and path.relative_to(directory).as_posix() not in rewriting]
    if others:
        check_png_directory(directory, kind, command)
