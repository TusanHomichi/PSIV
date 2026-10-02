"""Build `oracle/bin/psiv_oracle`, the headless host the oracle runs on.

    python3 -m oracle.build_host

The host's compile command lives here and nowhere else: the same eight
translation units and the same flags `oracle/verify.sh` spelled out inline
before this module existed, plus `-DPSIV_ORACLE_BUILD_ID="<sha256>"` - the
fingerprint of `oracle/host/*.c` and `*.h` as they stand at compile time
(`oracle/host_binary.py`'s `source_fingerprint`). The built host prints that
fingerprint for `--build-id`, which is what lets a launcher notice that the
binary in front of it came from other sources (#61: a checkout's host dated
from three weeks earlier and had no `--rng-trace`, so a capture lane rebuilt it
privately).

A Python module rather than a shell script, because the fingerprint compiled in
here and the fingerprint `oracle/host_binary.py` compares have to be one
implementation, and because the rebuilding helper is Python: it calls this
module's command line, so the entry point a launcher exercises is exactly the
one a person runs by hand. `oracle/verify.sh` runs it too.

The compile is one `gcc` call over ~3.4k lines and takes about a second, so
this entry point always builds - it is the build, not a cache. A caller that
wants a binary current for the sources in front of it, without rebuilding what
is already current, wants `oracle/host_binary.py`'s `ensure()`.
"""
from __future__ import annotations

import argparse
import os
import pathlib
import subprocess
import sys

from oracle import host_binary

#: The host's translation units: those `oracle/verify.sh` listed inline, plus
#: `core_options.c`, split out of `psiv_oracle.c` when the build id took that
#: file past the repository's size limit (#61). The fingerprint covers every
#: `*.c` and `*.h` of `oracle/host/`, so a source added to the directory
#: without a line here fails `tests/test_oracle_host_binary.py`'s compile-list
#: case rather than quietly dropping out of the build.
SOURCES = (
    "psiv_oracle.c",
    "core_options.c",
    "frame_dump.c",
    "ram_patch.c",
    "ram_dump.c",
    "core_vdp.c",
    "rng_trace.c",
    "state_dump.c",
    "tape.c",
)

#: What `oracle/verify.sh` compiled with, and what it linked against.
CC = "gcc"
CFLAGS = ("-O2", "-Wall", "-Wextra")
LDLIBS = ("-ldl",)


class BuildError(RuntimeError):
    """The host could not be compiled, or did not come out of it as asked."""


def compile_command(binary, host_dir, fingerprint, cc=CC) -> list[str]:
    """The one gcc invocation, as an argv list.

    `fingerprint` is compiled in rather than computed by the host, because the
    host has no business reading its own sources: what the binary reports has
    to be what the build saw, so the comparison is between two numbers that a
    caller can reproduce from the tree.
    """
    return [cc, *CFLAGS, f'-D{host_binary.BUILD_ID_DEFINE}="{fingerprint}"',
            "-o", str(binary),
            *[str(pathlib.Path(host_dir) / name) for name in SOURCES],
            *LDLIBS]


def build(binary=None, host_dir=None, cc=CC) -> tuple[pathlib.Path, str]:
    """Compile the host into place; returns `(binary, fingerprint)`.

    The compile writes to a temporary name beside the target and then renames
    it, so a launcher running concurrently never sees a half-written binary and
    a compile that fails leaves the previous one where it was. The new binary
    is asked for its build id before the rename, so a compiler that ignored the
    define fails the build instead of installing a host that would be reported
    as stale forever.
    """
    binary = pathlib.Path(binary or host_binary.BINARY)
    host_dir = pathlib.Path(host_dir or host_binary.HOST_DIR)
    fingerprint = host_binary.source_fingerprint(host_dir)
    missing = [name for name in SOURCES if not (host_dir / name).is_file()]
    if missing:
        raise BuildError(
            f"{host_binary.display(host_dir)} is missing {', '.join(missing)}")

    binary.parent.mkdir(parents=True, exist_ok=True)
    temp = binary.with_name(f".{binary.name}.{os.getpid()}.tmp")
    try:
        proc = subprocess.run(compile_command(temp, host_dir, fingerprint, cc),
                              capture_output=True, text=True)
        if proc.returncode != 0:
            raise BuildError(f"gcc exited {proc.returncode}:\n{proc.stderr.strip()}")
        reported = host_binary.reported_build_id(temp)
        if reported != fingerprint:
            raise BuildError(
                f"{host_binary.display(binary)} reports build id {reported} "
                f"instead of the {fingerprint} it was compiled with")
        os.replace(temp, binary)
    finally:
        temp.unlink(missing_ok=True)
    return binary, fingerprint


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m oracle.build_host",
        description="Compile the headless oracle host and install it.")
    parser.add_argument("--binary", default=None,
                        help="where to write it "
                             f"(default: {host_binary.display(host_binary.BINARY)})")
    parser.add_argument("--host-dir", default=None,
                        help="the sources to compile "
                             f"(default: {host_binary.display(host_binary.HOST_DIR)})")
    args = parser.parse_args(argv)
    try:
        binary, fingerprint = build(binary=args.binary, host_dir=args.host_dir)
    except (BuildError, host_binary.HostBinaryError) as error:
        print(f"oracle/build_host.py: {error}", file=sys.stderr)
        return 1
    print(f"host built: {host_binary.display(binary)} build-id {fingerprint} "
          f"({len(SOURCES)} sources)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
