#!/usr/bin/env python3
"""The Godot executable PSIV runs: one resolver for every launcher and verifier.

    python3 tools/godot.py

prints the executable's path, or the reason there is none on stderr with exit
status 1. `PSIV_GODOT` names the executable; when it is unset or empty, `godot`
on `PATH` is used. Either way the executable must report the version that
`.godot-version` at the repository root pins (`godot --version` prints
`4.7.1.stable.official.<commit>`), so a different Godot is refused rather than
run. That file is the one place the version is written: the shell launchers
call this script and the Python tools import `resolve`, so changing the
version is a one-line edit there.

Stdlib only, Python 3.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
from collections.abc import Callable, Mapping
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

#: The pinned version, one line such as `4.7.1`.
VERSION_FILE = ROOT / ".godot-version"

#: The environment variable that names the executable.
ENV = "PSIV_GODOT"

#: Seconds `godot --version` may take.
VERSION_TIMEOUT = 30

_VERSION_RE = re.compile(r"\A\d+\.\d+(?:\.\d+)?\Z")


class GodotError(RuntimeError):
    """No usable Godot: none named or found, not runnable, or the wrong version."""


def required_version(path: Path = VERSION_FILE) -> str:
    """The version `.godot-version` pins, such as `4.7.1`."""
    try:
        text = path.read_text(encoding="utf-8").strip()
    except OSError as error:
        raise GodotError(f"cannot read the pinned Godot version: {error}") from error
    if not _VERSION_RE.match(text):
        raise GodotError(f"{path} must hold one version such as 4.7.1, not {text!r}")
    return text


def reported_version(output: str) -> str | None:
    """The version in `godot --version` output: its leading numeric fields.

    `4.7.1.stable.official.a13da4feb` is 4.7.1; a `.0` patch release prints
    as `4.7.stable...`, which is 4.7.
    """
    words = output.strip().split()
    if not words:
        return None
    numbers = []
    for field in words[0].split("."):
        if not field.isdigit():
            break
        numbers.append(field)
    return ".".join(numbers) if len(numbers) >= 2 else None


def same_version(left: str, right: str) -> bool:
    """True when two versions name the same release (`4.7` is `4.7.0`)."""
    def fields(version: str) -> list[int]:
        values = [int(part) for part in version.split(".")]
        return values + [0] * (3 - len(values))
    return fields(left) == fields(right)


def find(env: Mapping[str, str] | None = None,
         which: Callable[..., str | None] = shutil.which) -> Path | None:
    """The executable `PSIV_GODOT` names, else `godot` on PATH, unchecked.

    Cheap and silent, so a tool can look it up at import; `check` it before
    running it. None when neither names one.
    """
    env = os.environ if env is None else env
    named = env.get(ENV, "")
    if named:
        return Path(named).expanduser()
    found = which("godot", path=env.get("PATH"))
    return Path(found) if found else None


def check(path: Path | None, version_file: Path = VERSION_FILE) -> Path:
    """`path`, once it is an executable reporting the pinned version.

    Raises `GodotError` naming the cause otherwise.
    """
    required = required_version(version_file)
    if path is None:
        raise GodotError(f"no Godot: set {ENV} to the Godot {required} executable, "
                         "or put it on PATH as `godot`")
    if not (path.is_file() and os.access(path, os.X_OK)):
        raise GodotError(f"{path} is not an executable file ({ENV}, or `godot` on PATH)")
    try:
        proc = subprocess.run([str(path), "--version"], capture_output=True, text=True,
                              timeout=VERSION_TIMEOUT, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise GodotError(f"cannot run {path} --version: {error}") from error
    version = reported_version(proc.stdout)
    if proc.returncode != 0 or version is None:
        raise GodotError(f"{path} --version reported no version (exit {proc.returncode})")
    if not same_version(version, required):
        raise GodotError(f"{path} is Godot {version}; PSIV needs {required} "
                         f"({version_file.name})")
    return path


def resolve(env: Mapping[str, str] | None = None,
            which: Callable[..., str | None] = shutil.which,
            version_file: Path = VERSION_FILE) -> Path:
    """The Godot executable to run: `find`, then `check`."""
    return check(find(env, which), version_file)


def main() -> int:
    try:
        print(resolve())
    except GodotError as error:
        print(f"godot: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
