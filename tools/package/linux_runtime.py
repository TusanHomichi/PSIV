#!/usr/bin/env python3
"""Configure Linux Mesa/GLX/EGL/nix-ld runtime env for exported PSIV binaries.

Stock Linux Godot export templates (`psiv.x86_64`) dynamically load X11,
libglvnd (`libGLX`, `libEGL`), and Mesa drivers at startup. Standard FHS
distributions (Ubuntu, SteamOS/Steam Deck, Fedora) provide those libraries and
`/usr/share/glvnd/egl_vendor.d/50_mesa.json` in system paths, so this module is
a no-op there.

On NixOS hosts without `/run/opengl-driver` (headless builders and CI sandboxes
running exported binaries via `nix-ld`), the flake's `godot` wrapper sets
`LD_LIBRARY_PATH`, `LIBGL_DRIVERS_PATH`, `__GLX_VENDOR_LIBRARY_NAME`, and
`__EGL_VENDOR_LIBRARY_DIRS`, and writes the same environment plus its matching
glibc loader (`NIX_LD`) to `godot-runtime-env.json` (exposed as
`PSIV_LINUX_RUNTIME_ENV` in `nix develop`). Without `__EGL_VENDOR_LIBRARY_DIRS`,
`libglvnd` loads zero EGL vendor ICDs and Godot 4.7.2 segfaults on a null
pointer in `drivers/egl/egl_manager.cpp:470` whenever EGL is initialized
(directly via `opengl3_es` or via fallback after GLX fails).

Stdlib only, Python 3.
"""

from __future__ import annotations

import json
import os
import sys
from collections.abc import Iterable, Mapping
from pathlib import Path

#: Environment variable set by `flake.nix` pointing to `godot-runtime-env.json`.
RUNTIME_ENV_VAR = "PSIV_LINUX_RUNTIME_ENV"

#: Standard locations where libglvnd looks for EGL vendor ICD JSON files.
DEFAULT_SYSTEM_EGL_DIRS: tuple[Path, ...] = (
    Path("/etc/glvnd/egl_vendor.d"),
    Path("/usr/share/glvnd/egl_vendor.d"),
    Path("/run/opengl-driver/share/glvnd/egl_vendor.d"),
)

#: Runtime environment keys recorded in smoke `failure.json` diagnostics.
DIAGNOSTIC_ENV_KEYS: tuple[str, ...] = (
    "DISPLAY",
    "LD_LIBRARY_PATH",
    "LIBGL_DRIVERS_PATH",
    "LIBGL_ALWAYS_SOFTWARE",
    "__GLX_VENDOR_LIBRARY_NAME",
    "__EGL_VENDOR_LIBRARY_DIRS",
    "__EGL_VENDOR_LIBRARY_FILENAMES",
    "NIX_LD",
)


def _split_paths(value: str | None) -> list[str]:
    if not value:
        return []
    return [part for part in value.split(":") if part]


def _prepend_colon_paths(preferred: str, existing: str | None) -> str:
    merged: list[str] = []
    seen: set[str] = set()
    for part in (*_split_paths(preferred), *_split_paths(existing)):
        if part not in seen:
            seen.add(part)
            merged.append(part)
    return ":".join(merged)


def _has_json_files(directory: Path) -> bool:
    try:
        return directory.is_dir() and any(
            entry.is_file() and entry.name.endswith(".json")
            for entry in directory.iterdir()
        )
    except OSError:
        return False


def _has_dri_drivers(directory: Path) -> bool:
    try:
        return directory.is_dir() and any(
            entry.name.endswith("_dri.so") for entry in directory.iterdir()
        )
    except OSError:
        return False


def _derive_egl_vendor_dirs(env: Mapping[str, str]) -> list[str]:
    """Find Mesa `share/glvnd/egl_vendor.d` dirs matching active GLX/LD paths."""
    candidates: list[Path] = []
    for dri_dir in _split_paths(env.get("LIBGL_DRIVERS_PATH")):
        candidates.append(Path(dri_dir).parent.parent / "share" / "glvnd" / "egl_vendor.d")
    for lib_dir in _split_paths(env.get("LD_LIBRARY_PATH")):
        candidates.append(Path(lib_dir).parent / "share" / "glvnd" / "egl_vendor.d")
    found: list[str] = []
    seen: set[str] = set()
    for candidate in candidates:
        text = str(candidate)
        if text not in seen and _has_json_files(candidate):
            seen.add(text)
            found.append(text)
    return found


def _derive_dri_dirs(env: Mapping[str, str]) -> list[str]:
    """Find Mesa `lib/dri` dirs beside entries already on `LD_LIBRARY_PATH`."""
    found: list[str] = []
    seen: set[str] = set()
    for lib_dir in _split_paths(env.get("LD_LIBRARY_PATH")):
        candidate = Path(lib_dir) / "dri"
        text = str(candidate)
        if text not in seen and _has_dri_drivers(candidate):
            seen.add(text)
            found.append(text)
    return found


def _load_runtime_env_file(path: Path | None) -> dict[str, str]:
    if path is None or not path.is_file():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    if not isinstance(data, dict):
        return {}
    return {
        key: value
        for key, value in data.items()
        if isinstance(key, str) and isinstance(value, str) and value
    }


def discover_runtime_env_file(source_env: Mapping[str, str]) -> Path | None:
    """Locate `godot-runtime-env.json` via `PSIV_LINUX_RUNTIME_ENV` or `tools.godot`."""
    named = source_env.get(RUNTIME_ENV_VAR, "").strip()
    if named:
        candidate = Path(named).expanduser()
        if candidate.is_file():
            return candidate
    if sys.platform.startswith("linux"):
        try:
            from tools.godot import linux_runtime_env_file
        except ImportError:
            return None
        return linux_runtime_env_file(source_env)
    return None


def apply_linux_runtime_env(
    env: Mapping[str, str],
    *,
    source_env: Mapping[str, str] | None = None,
    system_egl_dirs: Iterable[Path] = DEFAULT_SYSTEM_EGL_DIRS,
    runtime_env_file: Path | None = None,
    is_linux: bool | None = None,
) -> dict[str, str]:
    """Return a copy of `env` with Nix/Mesa GLX and EGL runtime settings on Linux.

    Leaves standard FHS Linux systems (with system glvnd EGL vendor JSONs and no
    Nix `godot-runtime-env.json`) completely unchanged.
    """
    result = dict(env)
    if is_linux is None:
        is_linux = sys.platform.startswith("linux")
    if not is_linux:
        return result

    lookup_env = env if source_env is None else source_env
    spec_path = (
        runtime_env_file
        if runtime_env_file is not None
        else discover_runtime_env_file(lookup_env)
    )
    spec = _load_runtime_env_file(spec_path)

    if spec:
        if "LD_LIBRARY_PATH" in spec:
            result["LD_LIBRARY_PATH"] = _prepend_colon_paths(
                spec["LD_LIBRARY_PATH"], result.get("LD_LIBRARY_PATH")
            )
        for key in ("LIBGL_DRIVERS_PATH", "__GLX_VENDOR_LIBRARY_NAME"):
            if key in spec and not result.get(key):
                result[key] = spec[key]
        nix_ld = spec.get("NIX_LD")
        if nix_ld and result.get("NIX_LD") and Path(nix_ld).is_file():
            result["NIX_LD"] = nix_ld

    has_system_egl = any(_has_json_files(directory) for directory in system_egl_dirs)

    if not result.get("__EGL_VENDOR_LIBRARY_DIRS") and not result.get(
        "__EGL_VENDOR_LIBRARY_FILENAMES"
    ):
        derived_egl = _derive_egl_vendor_dirs(result)
        if spec and "__EGL_VENDOR_LIBRARY_DIRS" in spec:
            # Prefer an EGL vendor dir matching a caller-chosen LIBGL_DRIVERS_PATH
            # if present, otherwise use the flake's matching Mesa EGL vendor dir.
            if env.get("LIBGL_DRIVERS_PATH") and derived_egl:
                result["__EGL_VENDOR_LIBRARY_DIRS"] = ":".join(derived_egl)
            else:
                result["__EGL_VENDOR_LIBRARY_DIRS"] = spec["__EGL_VENDOR_LIBRARY_DIRS"]
        elif not has_system_egl and derived_egl:
            result["__EGL_VENDOR_LIBRARY_DIRS"] = ":".join(derived_egl)

    if not result.get("LIBGL_DRIVERS_PATH") and not has_system_egl:
        derived_dri = _derive_dri_dirs(result)
        if derived_dri:
            result["LIBGL_DRIVERS_PATH"] = ":".join(derived_dri)
            result.setdefault("__GLX_VENDOR_LIBRARY_NAME", "mesa")

    return result


def diagnostic_runtime_env(env: Mapping[str, str]) -> dict[str, str | None]:
    """Safe loader and graphics driver env snapshot for smoke failure reports."""
    return {key: env.get(key) for key in DIAGNOSTIC_ENV_KEYS}


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    if args[:1] == ["--"]:
        args = args[1:]
    if not args:
        print(
            "usage: python3 tools/package/linux_runtime.py -- <executable> [args...]",
            file=sys.stderr,
        )
        return 2
    env = apply_linux_runtime_env(os.environ, source_env=os.environ)
    os.execvpe(args[0], args, env)


if __name__ == "__main__":
    sys.exit(main())
