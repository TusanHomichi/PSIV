#!/usr/bin/env python3
"""The Godot executable PSIV runs: one resolver for every launcher and verifier.

    python3 tools/godot.py

prints the executable's path, or the reason there is none on stderr with exit
status 1. `PSIV_GODOT` names the executable; when it is unset or empty, `godot`
on `PATH` is used. Either way the executable must report the version that
`.godot-version` at the repository root pins (`godot --version` prints
`4.7.1.stable.official.<commit>`) and be a `stable` build, so a different
Godot, or a release candidate of the pinned one, is refused rather than run. That file is the one place the version is written: the shell launchers
call this script and the Python tools import `resolve`, so changing the
version is a one-line edit there.

    python3 tools/godot.py prepare

prepares the Godot project for its first run. Godot finds the game's
GDExtension through `godot/.godot/extension_list.cfg`, which its editor
import writes and Git ignores, so a fresh clone has none: the game then
starts on a black screen with "Cannot get class 'Field'". `prepare` runs that
import headless once (`--headless --path godot --editor --quit`) and checks
the file; when the file already lists the extension it does nothing. Every
launcher and verifier calls it before starting the game.

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

#: The Godot project, the extension index its import writes, and the entry
#: that index must hold for the game's classes to load.
PROJECT = ROOT / "godot"
EXTENSION_INDEX = Path(".godot/extension_list.cfg")
EXTENSION_ENTRY = "res://psiv.gdextension"

#: Seconds the one-shot editor import may take (about 5 on a fresh project).
IMPORT_TIMEOUT = 300

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


def reported_version(output: str) -> tuple[str, str] | None:
    """The version and build status in `godot --version` output: its leading
    numeric fields and the field after them.

    `4.7.1.stable.official.a13da4feb` is (4.7.1, stable); a `.0` patch release
    prints as `4.7.stable...`, which is 4.7; a release candidate prints
    `4.7.1.rc1.official...`, status `rc1`.
    """
    words = output.strip().split()
    if not words:
        return None
    fields = words[0].split(".")
    numbers = []
    for field in fields:
        if not field.isdigit():
            break
        numbers.append(field)
    if len(numbers) < 2:
        return None
    status = fields[len(numbers)] if len(fields) > len(numbers) else ""
    return ".".join(numbers), status


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
    reported = reported_version(proc.stdout)
    if proc.returncode != 0 or reported is None:
        raise GodotError(f"{path} --version reported no version (exit {proc.returncode})")
    version, status = reported
    if status != "stable":
        raise GodotError(f"{path} is Godot {version} {status or 'with no build status'}; "
                         f"PSIV needs the stable {required} release ({version_file.name})")
    if not same_version(version, required):
        raise GodotError(f"{path} is Godot {version}; PSIV needs {required} "
                         f"({version_file.name})")
    return path


def resolve(env: Mapping[str, str] | None = None,
            which: Callable[..., str | None] = shutil.which,
            version_file: Path = VERSION_FILE) -> Path:
    """The Godot executable to run: `find`, then `check`."""
    return check(find(env, which), version_file)


def prepared(project: Path = PROJECT) -> bool:
    """True when `project`'s extension index lists the game's extension."""
    try:
        return EXTENSION_ENTRY in (project / EXTENSION_INDEX).read_text(encoding="utf-8")
    except OSError:
        return False


def prepare(godot: Path, project: Path = PROJECT, log: Path | None = None,
            timeout: float = IMPORT_TIMEOUT) -> bool:
    """Make `project` ready to run: once, Godot's headless editor import.

    Returns True when it ran the import and False when the project was already
    prepared. `log` receives the import's output (otherwise it is kept for the
    error message). Raises `GodotError` when the import fails or leaves no
    extension index.
    """
    if prepared(project):
        return False
    command = [str(godot), "--headless", "--path", str(project), "--editor", "--quit"]
    try:
        proc = subprocess.run(command, capture_output=True, text=True, timeout=timeout,
                              check=False)
        output, code = proc.stdout + proc.stderr, proc.returncode
    except subprocess.TimeoutExpired as error:
        output, code = f"timed out after {timeout} s\n{error.output or ''}", 124
    except OSError as error:
        raise GodotError(f"cannot run the {project.name}/ import: {error}") from error
    if log is not None:
        log.write_text(output, encoding="utf-8")
    if code != 0 or not prepared(project):
        where = f"log={log}" if log is not None else "\n" + "\n".join(output.splitlines()[-20:])
        raise GodotError(f"the {project.name}/ import (exit {code}) wrote no "
                         f"{EXTENSION_ENTRY} to {EXTENSION_INDEX}; {where}")
    return True


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if argv not in ([], ["prepare"]):
        print("usage: python3 tools/godot.py [prepare]", file=sys.stderr)
        return 2
    try:
        godot = resolve()
        if argv:
            ran = prepare(godot)
            print(f"godot: {PROJECT.name}/ {'prepared' if ran else 'already prepared'}",
                  file=sys.stderr)
        else:
            print(godot)
    except GodotError as error:
        print(f"godot: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
