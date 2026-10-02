"""The host binary, and the one rule that keeps it from being stale.

`oracle/bin/psiv_oracle` is compiled from `oracle/host/` by
`oracle/build_host.py`. This module is how a launcher gets the path: `ensure()`
asks the binary what it was built from (`--build-id`, the fingerprint compiled
in as `-DPSIV_ORACLE_BUILD_ID`), compares that with the fingerprint of
`oracle/host/*.c` and `*.h` as they are now, and - when the two differ, or
there is no binary - rebuilds through the entry point and hands back a path
that is current by construction.

That is the whole point (#61): the host used to be built only as a side effect
of `oracle/verify.sh`, so a lane that ran `python3 -m oracle.force` in a
checkout whose last `verify.sh` was three weeks old ran a three-week-old host -
one that predated `--rng-trace` - and had to rebuild it privately, by hand, mid
capture. A launcher that asks this module cannot do that: the sources in front
of it and the binary it runs are the same by definition.

Rebuilding is the default rather than a refusal because it costs about a
second, needs nothing but `gcc`, and is what the caller wants anyway. Two
escape hatches, both documented in `oracle/README.md`:

* `PSIV_ORACLE_NO_REBUILD=1` turns a mismatch into a refusal that names both
  fingerprints, for a tree that must not be written to;
* `python3 -m oracle.host_binary --path` prints the path a launcher would use,
  and `--build-id` the fingerprint it reports, for a shell script or a log.

Concurrent launchers serialize on a lock beside the binary, so a sweep at
`--jobs 3` compiles the host once instead of three times.

The fingerprint is a SHA-256 over every `oracle/host/*.c` and `*.h` file: each
file's name, its length and its bytes, in name order, behind a scheme tag. File
order, mtimes and where the tree lives cannot change it, and one byte anywhere
in the sources does - `tests/test_oracle_host_binary.py` owns both cases.
"""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import os
import pathlib
import subprocess
import sys

ORACLE = pathlib.Path(__file__).resolve().parent
ROOT = ORACLE.parent
HOST_DIR = ORACLE / "host"
BINARY = ORACLE / "bin" / "psiv_oracle"
LOCK_NAME = ".psiv_oracle.lock"

#: The define `oracle/build_host.py` passes and `oracle/host/psiv_oracle.c`
#: answers `--build-id` with. A host compiled without it answers `unknown`,
#: which reads as "not these sources" and is rebuilt.
BUILD_ID_DEFINE = "PSIV_ORACLE_BUILD_ID"

#: Set to any of `OFF_VALUES` to refuse a stale host instead of rebuilding it.
NO_REBUILD_ENV = "PSIV_ORACLE_NO_REBUILD"
OFF_VALUES = ("1", "yes", "true", "on")

#: `FINGERPRINT_SCHEME` prefixes the digest input so that a later change to
#: what a fingerprint covers cannot produce a value an earlier one also
#: produces.
FINGERPRINT_SCHEME = b"psiv-oracle-host-v1\0"

SOURCE_SUFFIXES = (".c", ".h")


class HostBinaryError(RuntimeError):
    """The host binary in front of a launcher is not one it may run."""


class StaleHostError(HostBinaryError):
    """The host was built from other sources, and rebuilding was refused."""


def display(path) -> str:
    """A path for a message: relative to the repository when it is inside it."""
    path = pathlib.Path(path)
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def host_sources(host_dir=HOST_DIR) -> list[pathlib.Path]:
    """The host's own sources: every `*.c` and `*.h`, by name.

    By name, not in directory order, because the fingerprint must not care
    which order a filesystem hands the entries over in.
    """
    host_dir = pathlib.Path(host_dir)
    return sorted((p for p in host_dir.iterdir()
                   if p.is_file() and p.suffix in SOURCE_SUFFIXES),
                  key=lambda p: p.name)


def source_fingerprint(host_dir=HOST_DIR) -> str:
    """SHA-256 over the host's sources, in name order.

    Each file contributes its name, its length and its bytes, so neither the
    order of the files nor a file boundary moving can change the result. A
    directory with no sources is an error rather than the digest of nothing,
    which a binary built from nothing would then match.
    """
    host_dir = pathlib.Path(host_dir)
    suffixes = " or ".join(SOURCE_SUFFIXES)
    if not host_dir.is_dir():
        raise HostBinaryError(
            f"no host sources: {display(host_dir)} is not a directory")
    paths = host_sources(host_dir)
    if not paths:
        raise HostBinaryError(
            f"no host sources: {display(host_dir)} holds no {suffixes} file")
    digest = hashlib.sha256()
    digest.update(FINGERPRINT_SCHEME)
    for path in paths:
        data = path.read_bytes()
        digest.update(f"{path.name}\0{len(data)}\0".encode())
        digest.update(data)
    return digest.hexdigest()


def reported_build_id(binary=BINARY) -> str | None:
    """What `binary` says it was built from, or None if it cannot be asked.

    A missing file, a binary that would not start, one built without the
    define (`unknown`), or something that is not this host at all all read as
    "not the sources in front of me" - which is exactly what a caller about to
    run it needs to know, and needs to be true rather than clever.
    """
    path = pathlib.Path(binary)
    if not path.exists():
        return None
    try:
        proc = subprocess.run([str(path), "--build-id"], capture_output=True,
                              text=True)
    except OSError:
        return None
    if proc.returncode != 0:
        return None
    return proc.stdout.strip() or None


def mismatch(binary, reported, want) -> str:
    """The sentence a refusal or a rebuild notice carries.

    Both fingerprints when the binary has one, and what is missing instead when
    it has none - the two cases a launcher can meet.
    """
    if reported is None:
        return (f"{display(binary)} is missing, or does not report a build id "
                f"(it was not built by oracle/build_host.py); the host sources "
                f"hash to {want}")
    return (f"{display(binary)} was built from {reported}, but the host "
            f"sources hash to {want}")


def rebuilding_allowed(env=None) -> bool:
    """True unless `PSIV_ORACLE_NO_REBUILD` says otherwise.

    Absent, empty, `0`, `no`, `false` - and any other spelling that is not one
    of `OFF_VALUES` - allow a rebuild; `1`, `yes`, `true` and `on` refuse it.
    """
    env = os.environ if env is None else env
    return env.get(NO_REBUILD_ENV, "").strip().lower() not in OFF_VALUES


def ensure(binary=BINARY, host_dir=HOST_DIR,
           rebuild: bool | None = None) -> pathlib.Path:
    """A host built from `host_dir` as it stands, rebuilding it if it is not.

    Returns the path every launcher should run. `rebuild=False` (or
    `PSIV_ORACLE_NO_REBUILD=1` in the environment) refuses a stale host with a
    `StaleHostError` naming both fingerprints instead of compiling one.
    """
    binary = pathlib.Path(binary)
    host_dir = pathlib.Path(host_dir)
    if rebuild is None:
        rebuild = rebuilding_allowed()

    want = source_fingerprint(host_dir)
    reported = reported_build_id(binary)
    if reported == want:
        return binary
    problem = mismatch(binary, reported, want)
    if not rebuild:
        raise StaleHostError(
            f"{problem}; unset {NO_REBUILD_ENV} (or rebuild with "
            f"`python3 -m oracle.build_host`) to let a launcher do it")

    with _build_lock(binary.parent / LOCK_NAME):
        # The lock is held by one builder at a time: whoever waited asks again
        # and finds the host the winner built instead of compiling a second
        # one. A locked rebuild is the only reason the check runs twice.
        if reported_build_id(binary) != want:
            sys.stderr.write(f"oracle: rebuilding the host; {problem}\n")
            _rebuild(binary, host_dir, problem)
    if reported_build_id(binary) != want:
        raise HostBinaryError(
            f"{display(binary)} still does not report the fingerprint of the "
            f"host sources ({want}) after a rebuild")
    return binary


def _rebuild(binary, host_dir, problem) -> None:
    """Run the build entry point, as a module, from the repository root."""
    argv = [sys.executable, "-m", "oracle.build_host",
            "--binary", str(binary), "--host-dir", str(host_dir)]
    proc = subprocess.run(argv, cwd=str(ROOT), capture_output=True, text=True)
    # The entry point says what it built (or what the compiler said); a
    # launcher's log is where that belongs, stale or not.
    sys.stderr.write(proc.stdout)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise HostBinaryError(
            f"`python3 -m oracle.build_host` exited {proc.returncode}; {problem}")


@contextlib.contextmanager
def _build_lock(path):
    """Hold an exclusive lock on `path` for a rebuild."""
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        import fcntl
    except ImportError:  # not POSIX: nothing to serialize with, so no lock
        yield
        return
    with open(path, "a+") as handle:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m oracle.host_binary",
        description="Print the host binary a launcher should run, rebuilding it "
                    "if it is not the current sources.")
    parser.add_argument("--path", action="store_true",
                        help="print the binary's path (the default)")
    parser.add_argument("--build-id", action="store_true",
                        help="print the fingerprint the binary reports")
    parser.add_argument("--binary", default=None,
                        help=f"the host to check (default: {display(BINARY)})")
    parser.add_argument("--host-dir", default=None,
                        help="the sources to compare it with "
                             f"(default: {display(HOST_DIR)})")
    args = parser.parse_args(argv)
    try:
        binary = ensure(binary=args.binary or BINARY,
                        host_dir=args.host_dir or HOST_DIR)
    except HostBinaryError as error:
        print(f"oracle: {error}", file=sys.stderr)
        return 1
    print(reported_build_id(binary) if args.build_id else binary)
    return 0


if __name__ == "__main__":
    sys.exit(main())
