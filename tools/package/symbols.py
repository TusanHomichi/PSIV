"""ELF build-id, separate debug symbols, and crash-artifact helpers for releases.

Linux release builds (`tools/package/release.py build --platform linux`) emit
DWARF line tables (`rust/Cargo.toml` `[profile.release]`) and an explicit GNU
build-id. `split_linux_symbols` verifies both, extracts `<binary>.debug` beside
the build output, strips `.debug_*` from the shipped binary while keeping
`.note.gnu.build-id` and `.symtab`, attaches `.gnu_debuglink`, and writes
`build-symbols.json`. Export and smoke steps reuse these helpers to record
Godot template metadata and preserve crash diagnostics without shipping debug
payloads in player archives.

Stdlib only, Python 3.
"""

from __future__ import annotations

import hashlib
import json
import re
import shutil
import signal
from collections.abc import Callable
from pathlib import Path

CHUNK = 1 << 20
ELF_MAGIC = b"\x7fELF"
BUILD_SYMBOLS_FILE = "build-symbols.json"
EXPORT_METADATA_FILE = "export-metadata.json"
FAILURE_REPORT_FILE = "failure.json"

_BUILD_ID_RE = re.compile(r"Build ID:\s*([0-9a-fA-F]+)\b")
_SECTION_RE = re.compile(r"^\s*\[\s*\d+\]\s+(\S+)\s+", re.M)


class SymbolError(RuntimeError):
    """A release binary lacked a build-id, debug info, or a valid split symbol file."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(CHUNK):
            digest.update(chunk)
    return digest.hexdigest()


def read_build_id(path: Path, runner: Callable) -> str:
    """Return the ELF GNU build-id hex string of `path`, or raise `SymbolError`."""
    result = runner(["readelf", "--notes", "--wide", str(path)])
    match = _BUILD_ID_RE.search(result.stdout)
    if not match:
        raise SymbolError(f"{path.name}: missing ELF GNU build-id (.note.gnu.build-id)")
    return match.group(1).lower()


def try_read_build_id(path: Path, runner: Callable) -> str | None:
    """Best-effort ELF GNU build-id for metadata; None when non-ELF or unreadable."""
    try:
        with path.open("rb") as handle:
            if handle.read(len(ELF_MAGIC)) != ELF_MAGIC:
                return None
        return read_build_id(path, runner)
    except (OSError, RuntimeError):
        return None


def read_elf_sections(path: Path, runner: Callable) -> set[str]:
    """Return ELF section names reported by `readelf --section-headers --wide`."""
    result = runner(["readelf", "--section-headers", "--wide", str(path)])
    return set(_SECTION_RE.findall(result.stdout))


def split_linux_symbols(
    paths: list[Path],
    out: Path | None = None,
    runner: Callable | None = None,
) -> tuple[list[Path], Path]:
    """Extract `<binary>.debug` beside each Linux binary and write `build-symbols.json`.

    Verifies that every input binary carries a GNU build-id and DWARF `.debug_line`
    before splitting, strips `.debug_*` from the shipped binary while keeping its
    build-id and `.symtab`, adds `.gnu_debuglink`, and verifies that both files
    keep the exact same build-id afterward.
    """
    if not paths:
        raise SymbolError("no Linux binaries provided for symbol extraction")
    if runner is None:
        raise SymbolError("runner is required for symbol extraction")
    out_dir = out or paths[0].parent
    out_dir.mkdir(parents=True, exist_ok=True)
    debug_paths: list[Path] = []
    manifest: dict[str, dict[str, str]] = {}
    for binary in paths:
        build_id = read_build_id(binary, runner)
        sections = read_elf_sections(binary, runner)
        if ".debug_line" not in sections:
            raise SymbolError(f"{binary.name}: missing DWARF debug symbols (.debug_line)")
        debug_path = out_dir / f"{binary.name}.debug"
        debug_path.unlink(missing_ok=True)
        runner(["objcopy", "--only-keep-debug", str(binary), str(debug_path)])
        if debug_path.is_file():
            debug_path.chmod(0o644)
        runner(["objcopy", "--strip-debug", str(binary)])
        runner(["objcopy", f"--add-gnu-debuglink={debug_path}", str(binary)])

        post_build_id = read_build_id(binary, runner)
        if post_build_id != build_id:
            raise SymbolError(
                f"{binary.name}: build-id changed after stripping ({build_id} -> {post_build_id})"
            )
        post_sections = read_elf_sections(binary, runner)
        if ".gnu_debuglink" not in post_sections:
            raise SymbolError(f"{binary.name}: missing .gnu_debuglink after symbol split")
        if ".debug_line" in post_sections:
            raise SymbolError(f"{binary.name}: still contains .debug_line after stripping")

        debug_build_id = read_build_id(debug_path, runner)
        if debug_build_id != build_id:
            raise SymbolError(
                f"{debug_path.name}: build-id {debug_build_id} does not match "
                f"{binary.name} ({build_id})"
            )
        debug_sections = read_elf_sections(debug_path, runner)
        if ".debug_line" not in debug_sections:
            raise SymbolError(f"{debug_path.name}: missing .debug_line in extracted debug file")

        manifest[binary.name] = {
            "file": binary.name,
            "debug_file": debug_path.name,
            "build_id": build_id,
            "sha256": sha256(binary) if binary.is_file() else "",
            "debug_sha256": sha256(debug_path) if debug_path.is_file() else "",
        }
        debug_paths.append(debug_path)

    manifest_path = out_dir / BUILD_SYMBOLS_FILE
    manifest_path.write_text(
        json.dumps({"binaries": manifest}, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    return debug_paths, manifest_path


def copy_sidecar_symbols(source_dir: Path, dest_dir: Path) -> list[dict[str, str]]:
    """Copy `.debug` files and `build-symbols.json` from `source_dir` into `dest_dir`."""
    if not source_dir.is_dir():
        return []
    candidates = sorted(
        path for path in source_dir.iterdir()
        if path.is_file() and (path.name.endswith(".debug") or path.name == BUILD_SYMBOLS_FILE)
    )
    if not candidates:
        return []
    dest_dir.mkdir(parents=True, exist_ok=True)
    copied = []
    for path in candidates:
        target = dest_dir / path.name
        shutil.copy2(path, target)
        copied.append({"file": path.name, "sha256": sha256(target)})
    return copied


def signal_name(returncode: int | None) -> str | None:
    """Unix signal name (such as `'SIGSEGV'`) for a negative returncode, else None."""
    if returncode is None or returncode >= 0:
        return None
    try:
        return signal.Signals(-returncode).name
    except ValueError:
        return f"SIG{-returncode}"


def move_failure_dir(scratch: Path, failure_dir: Path | None) -> Path:
    """Keep `scratch` in place, or move it into `failure_dir` when requested."""
    if failure_dir is None:
        return scratch
    target = failure_dir.resolve()
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        shutil.rmtree(target)
    shutil.move(str(scratch), str(target))
    return target
