"""Install a runtime pack atomically: build beside it, then swap it in.

`python3 -m psiv_tools pack <rom> <dir>` used to write into `<dir>` in place,
and every lane, suite and Godot run reads that directory, so a reader alongside
a build could load a half-written pack, and a killed build left one behind
(issue #145). `install_atomically(output, build)` makes a pack build safe to
run beside its readers:

1. `build(staging)` writes the whole pack into a new sibling directory,
   `.<name>.build-<pid>-<random>`, on the same filesystem as `<output>`.
2. The finished directory is renamed to a version,
   `.<name>.v<UTC stamp>-<pid>`, which is never written again.
3. `<output>` becomes a symbolic link to that version in one `rename(2)`: a
   new link is made beside it and renamed over it. Where `<output>` is still
   a real directory (a pack built before this module), the new link and the
   directory are exchanged in one `renameat2(RENAME_EXCHANGE)`, and the old
   directory is kept as a version too.

So a reader that opens `<output>/manifest.json` sees the old pack or the new
one, never a half-written one, and a reader that resolves `<output>` once
(`os.path.realpath`; a run root's `runtime-pack` link, `tools/godot.py`; the
game itself canonicalizes the pack path when it starts) reads one version from
start to finish. A reader that reopens `<output>/...` file by file across a
swap can mix two complete versions; resolve once where that matters.

Windows lets an ordinary user make a symbolic link only in developer mode, so
there `<output>` stays a real directory: step 3 renames the old directory to
the `.link-` name and the new version to `<output>`, two renames that never
show a half-written pack but leave a moment with none, and then removes the
old one. A reader holding a file of the old pack open can make the first
rename fail, which leaves `<output>` as it was. A reader there has no version
of its own to resolve to, so one that reopens `<output>/...` across a swap can
mix two versions, and no older version is kept.

A build that fails or is killed never touches `<output>`. Its staging
directory is removed on failure; one a killed build left behind is removed by
the next build, which removes any `.<name>.build-<pid>-*` whose process is
gone, and any `.<name>.link-*` left between steps 3's link and its rename.
After a swap the newest `KEEP_VERSIONS` versions are kept (the current one and
the ones before it, which a run that resolved them earlier may still be
reading) and older ones are removed; the version `<output>` names is never
removed, nor one another build that is still running made and has yet to
install, so two builds of one output never delete each other's work. A run
that outlives `KEEP_VERSIONS` swaps loses its pack, and fails loudly on a
missing file rather than reading a mix.

Stdlib only, Python 3; Linux (`renameat2`), macOS (`renamex_np`) and
Windows.
"""

from __future__ import annotations

import ctypes
import os
import re
import secrets
import shutil
import sys
from collections.abc import Callable
from datetime import datetime, timezone
from pathlib import Path
from typing import TypeVar

T = TypeVar("T")

#: Versions kept after a swap: the current one and the two before it.
KEEP_VERSIONS = 3

#: Whether `<output>` is a symbolic link to its version, or (Windows) the
#: real directory.
LINKS = os.name != "nt"

#: renameat2's flag that swaps two names atomically (linux/fs.h), and
#: renamex_np's (macOS sys/stdio.h).
RENAME_EXCHANGE = 2
RENAME_SWAP = 2
AT_FDCWD = -100

#: OpenProcess's right to read a process's exit code, and what it reads while
#: the process runs (Windows).
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
STILL_ACTIVE = 259
ERROR_ACCESS_DENIED = 5


class PackSwapError(RuntimeError):
    """The pack could not be installed; `<output>` is unchanged."""


def _names(output: Path) -> tuple[re.Pattern, re.Pattern, re.Pattern]:
    name = re.escape(output.name)
    return (re.compile(rf"\A\.{name}\.build-(\d+)-[0-9a-f]+\Z"),
            re.compile(rf"\A\.{name}\.v\d{{8}}T\d{{12}}Z-(\d+)\Z"),
            re.compile(rf"\A\.{name}\.link-(\d+)-[0-9a-f]+\Z"))


def _alive(pid: int) -> bool:
    """True while process `pid` exists, whoever owns it."""
    if pid == os.getpid():
        return True
    if os.name == "nt":
        return _alive_windows(pid)
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:  # another user's process
        return True
    return True


def _alive_windows(pid: int) -> bool:
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.GetExitCodeProcess.argtypes = (wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD))
    kernel32.CloseHandle.argtypes = (wintypes.HANDLE,)
    handle = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not handle:
        # Refused means it exists; any other error (no such process) means gone.
        return ctypes.get_last_error() == ERROR_ACCESS_DENIED
    try:
        code = wintypes.DWORD()
        if not kernel32.GetExitCodeProcess(handle, ctypes.byref(code)):
            return True
        return code.value == STILL_ACTIVE
    finally:
        kernel32.CloseHandle(handle)


def _remove(path: Path) -> None:
    if path.is_symlink() or not path.is_dir():
        path.unlink()
    else:
        shutil.rmtree(path)


def remove_abandoned(output: Path) -> list[str]:
    """Remove the staging directories and links that a build of `output`
    killed part-way left beside it; returns their names.

    A real directory under a `.link-` name is the old pack between the
    exchange and its rename (a build killed in that window); it is renamed to
    a version, stamped by its mtime so it sorts where it belongs, not deleted.
    """
    building, _, linking = _names(output)
    removed = []
    for entry in sorted(output.parent.iterdir()):
        match = building.match(entry.name) or linking.match(entry.name)
        if not match or _alive(int(match.group(1))):
            continue
        if linking.match(entry.name) and entry.is_dir() and not entry.is_symlink():
            stamp = datetime.fromtimestamp(entry.stat().st_mtime, timezone.utc).strftime(
                "%Y%m%dT%H%M%S%fZ")
            entry.rename(output.parent / f".{output.name}.v{stamp}-{match.group(1)}")
        else:
            _remove(entry)
        removed.append(entry.name)
    return removed


def versions(output: Path) -> list[Path]:
    """`output`'s installed versions beside it, oldest first."""
    _, version, _ = _names(output)
    return sorted(entry for entry in output.parent.iterdir() if version.match(entry.name))


def _exchange(first: Path, second: Path) -> None:
    libc = ctypes.CDLL(None, use_errno=True)
    name = "renamex_np" if sys.platform == "darwin" else "renameat2"
    call = getattr(libc, name, None)
    if call is None:
        raise PackSwapError(f"this C library has no {name}; cannot swap a real "
                            f"directory at {second} atomically")
    if sys.platform == "darwin":
        failed = call(os.fsencode(first), os.fsencode(second), RENAME_SWAP) != 0
    else:
        failed = call(AT_FDCWD, os.fsencode(first), AT_FDCWD, os.fsencode(second),
                      RENAME_EXCHANGE) != 0
    if failed:
        error = ctypes.get_errno()
        raise PackSwapError(f"cannot exchange {first} and {second}: {os.strerror(error)}")


def _stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")


def install_atomically(output: Path, build: Callable[[Path], T]) -> T:
    """Run `build(staging)` into a new sibling directory of `output`, then make
    `output` name it in one atomic step; returns what `build` returned.

    `output` may be absent, a symbolic link (to an earlier version, or
    anywhere: only the link is replaced, never what it names), or a real
    directory. On any failure `output` is untouched and the staging
    directory is removed."""
    output = Path(output).absolute()
    parent = output.parent
    parent.mkdir(parents=True, exist_ok=True)
    if output.exists() and not output.is_dir():
        raise PackSwapError(f"{output} exists and is not a directory")
    remove_abandoned(output)
    # Taken first, so a real directory replaced below sorts before the new
    # version (`versions` orders by name).
    before = _stamp()
    token = f"{os.getpid()}-{secrets.token_hex(4)}"
    staging = parent / f".{output.name}.build-{token}"
    staging.mkdir()
    try:
        result = build(staging)
        version = parent / f".{output.name}.v{_stamp()}-{os.getpid()}"
        os.rename(staging, version)
    except BaseException:
        if staging.exists():
            shutil.rmtree(staging)
        raise
    link = parent / f".{output.name}.link-{token}"
    if LINKS:
        _link(output, version, link, parent / f".{output.name}.v{before}-{os.getpid()}")
        prune(output)
    else:
        _replace_directory(output, version, link)
        prune(output, keep=0)
    return result


def _link(output: Path, version: Path, link: Path, kept: Path) -> None:
    """Make `output` a link to `version`, through the new link `link`; a real
    directory at `output` is kept as the version `kept`."""
    link.symlink_to(version.name, target_is_directory=True)
    try:
        if output.is_dir() and not output.is_symlink():
            _exchange(link, output)  # `link` is now the old real directory
            os.rename(link, kept)
        else:
            os.replace(link, output)
    except BaseException:
        if link.is_symlink():
            link.unlink()
        raise


def _replace_directory(output: Path, version: Path, old: Path) -> None:
    """Rename `version` to `output`, moving a directory already there to `old`
    first and removing it after."""
    if os.path.lexists(output):
        os.rename(output, old)
    try:
        os.rename(version, output)
    except BaseException:
        if os.path.lexists(old):
            os.rename(old, output)
        raise
    if os.path.lexists(old):
        _remove(old)


def prune(output: Path, keep: int = KEEP_VERSIONS) -> list[str]:
    """Remove all but the newest `keep` versions of `output`, never the one
    `output` names nor one another live process made (a concurrent build
    between its version and its install); returns the removed names."""
    current = os.path.realpath(output)
    _, version, _ = _names(output)
    installed = versions(output)

    def pending(path: Path) -> bool:
        pid = int(version.match(path.name).group(1))
        return pid != os.getpid() and _alive(pid)

    old = [path for path in installed[:max(0, len(installed) - keep)]
           if os.path.realpath(path) != current and not pending(path)]
    for path in old:
        shutil.rmtree(path)
    return [path.name for path in old]
