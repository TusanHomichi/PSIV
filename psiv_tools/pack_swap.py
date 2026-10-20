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

A build that fails or is killed never touches `<output>`. Its staging
directory is removed on failure; one a killed build left behind is removed by
the next build, which removes any `.<name>.build-<pid>-*` whose process is
gone, and any `.<name>.link-*` left between steps 3's link and its rename.
After a swap the newest `KEEP_VERSIONS` versions are kept (the current one and
the ones before it, which a run that resolved them earlier may still be
reading) and older ones are removed; the version `<output>` names is never
removed. A run that outlives `KEEP_VERSIONS` swaps loses its pack, and fails
loudly on a missing file rather than reading a mix.

Stdlib only, Python 3, Linux (`renameat2`, /proc).
"""

from __future__ import annotations

import ctypes
import os
import re
import secrets
import shutil
from collections.abc import Callable
from datetime import datetime, timezone
from pathlib import Path
from typing import TypeVar

T = TypeVar("T")

#: Versions kept after a swap: the current one and the two before it.
KEEP_VERSIONS = 3

#: renameat2's flag that swaps two names atomically (linux/fs.h).
RENAME_EXCHANGE = 2
AT_FDCWD = -100


class PackSwapError(RuntimeError):
    """The pack could not be installed; `<output>` is unchanged."""


def _names(output: Path) -> tuple[re.Pattern, re.Pattern, re.Pattern]:
    name = re.escape(output.name)
    return (re.compile(rf"\A\.{name}\.build-(\d+)-[0-9a-f]+\Z"),
            re.compile(rf"\A\.{name}\.v\d{{8}}T\d{{12}}Z-\d+\Z"),
            re.compile(rf"\A\.{name}\.link-(\d+)-[0-9a-f]+\Z"))


def _alive(pid: int) -> bool:
    return pid == os.getpid() or Path(f"/proc/{pid}").exists()


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
    renameat2 = getattr(libc, "renameat2", None)
    if renameat2 is None:
        raise PackSwapError("this C library has no renameat2; cannot swap a real "
                            f"directory at {second} atomically")
    if renameat2(AT_FDCWD, os.fsencode(first), AT_FDCWD, os.fsencode(second),
                 RENAME_EXCHANGE) != 0:
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
    link.symlink_to(version.name, target_is_directory=True)
    try:
        if output.is_dir() and not output.is_symlink():
            _exchange(link, output)  # `link` is now the old real directory
            os.rename(link, parent / f".{output.name}.v{before}-{os.getpid()}")
        else:
            os.replace(link, output)
    except BaseException:
        if link.is_symlink():
            link.unlink()
        raise
    prune(output)
    return result


def prune(output: Path, keep: int = KEEP_VERSIONS) -> list[str]:
    """Remove all but the newest `keep` versions of `output`, never the one
    `output` names; returns the removed names."""
    current = os.path.realpath(output)
    installed = versions(output)
    old = [path for path in installed[:max(0, len(installed) - keep)]
           if os.path.realpath(path) != current]
    for path in old:
        shutil.rmtree(path)
    return [path.name for path in old]
