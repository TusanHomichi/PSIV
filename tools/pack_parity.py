#!/usr/bin/env python3
"""tools/pack_parity.py - hold `psiv-pack` to the Python pack builder (#216).

Until the switch, the Python builder (`python3 -m psiv_tools pack`) is the
pack the game, the lanes and the gate use, and `rust/psiv-pack` ports it a
section at a time. This step, part of the full gate, builds `psiv-pack` in
release and checks it against the Python builder from the same ROM:

1. The wrong-ROM refusal: both builders run on a file that is not the US
   retail ROM. Both must exit non-zero, leave the output alone, and print the
   same message after their program prefix (`psiv-tools: error: ` and
   `psiv-pack: error: `), which the setup screen shows to the player.
2. Every path `psiv-pack --claims` names (its sections, each with the paths
   it writes): both builders build a full pack from
   the ROM, and each file under a claimed path must be in both packs, with
   byte-identical bytes, or for a PNG the same `image_sha256` (its decoded
   content: zlib implementations deflate alike rows differently,
   `psiv_tools/png.py`). A claim the Python pack lacks fails, and so does a
   file `psiv-pack` writes outside its claims. With no claims, the Python
   build is skipped, and the output is still checked: it must be an
   installed directory holding no link and nothing outside the claims.
3. `psiv-pack`'s run: one `progress <done>/<total> <section>` line per
   section, then `installed <pack>`, exit 0. Its wall time is printed with
   the CPU clock and load, the number each port PR records.

`--write-fixtures` instead rewrites `rust/psiv-pack/tests/fixtures/harness.json`,
the cases Python's `json.dumps(..., indent=2, sort_keys=True)` and PNG encoder
produce that the crate's own tests hold its writers to;
`tests/test_pack_parity.py` fails when the committed file is stale.

The cargo build takes the heavy lock's `build` class and each pack build its
`pack` class; under the gate's exclusive hold both are no-ops. Exit 0 on
parity, 1 on any difference or missing input, 2 on bad usage.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import random
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "tools"))

from heavy_lock import BUILD, PACK, hold_heavy_lock  # noqa: E402
from psiv_tools.png import encode_indexed, encode_rgb, image_sha256  # noqa: E402
from tree_rss import cpu_mhz, load_1m  # noqa: E402

DEFAULT_ROM = ROOT / "Phantasy Star IV (USA).md"
FIXTURE = ROOT / "rust" / "psiv-pack" / "tests" / "fixtures" / "harness.json"
BINARY = ROOT / "rust" / "target" / "release" / ("psiv-pack.exe" if os.name == "nt" else "psiv-pack")
CARGO_BUILD = ["cargo", "build", "--release", "--manifest-path", "rust/Cargo.toml", "-p", "psiv-pack"]
PYTHON_PREFIX = "psiv-tools: error: "
RUST_PREFIX = "psiv-pack: error: "
PROGRESS = re.compile(r"\Aprogress (\d+)/(\d+) (\S+)\Z")


# ---------------------------------------------------------------------------
# Comparison
# ---------------------------------------------------------------------------
def _files(root: Path) -> dict[str, Path]:
    """Every file under `root`, by its root-relative POSIX path."""
    found = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink() or path.is_file():
            found[path.relative_to(root).as_posix()] = path
    return found


def _under(relative: str, claim: str) -> bool:
    return relative == claim or relative.startswith(claim.rstrip("/") + "/")


def check_output(rust_root: Path, claims: list[str]) -> list[str]:
    """`psiv-pack`'s installed output on its own: a directory, holding no
    link and no file outside its claims. Checked on every run, claims or
    none."""
    if not rust_root.is_dir():
        return [f"psiv-pack left no pack directory at {rust_root.name}"]
    problems = []
    for relative, path in _files(rust_root).items():
        if path.is_symlink():
            problems.append(f"{relative}: psiv-pack wrote a symbolic link")
        elif not any(_under(relative, claim) for claim in claims):
            problems.append(f"{relative}: psiv-pack wrote it outside its claims")
    return problems


def compare(rust_root: Path, python_root: Path, claims: list[str]) -> tuple[list[str], int]:
    """The differences between the two packs under `claims`, and how many
    claimed files matched. Empty problems is parity."""
    problems = check_output(rust_root, claims)
    rust = _files(rust_root)
    python = _files(python_root)
    for claim in claims:
        if not (python_root / claim).exists():
            problems.append(f"claim {claim}: the Python pack has no such path")
    claimed = sorted({relative for relative in [*rust, *python]
                      if any(_under(relative, claim) for claim in claims)})
    matched = 0
    for relative in claimed:
        if relative not in python:
            if relative in rust and not rust[relative].is_symlink():
                problems.append(f"{relative}: psiv-pack wrote it, the Python pack has none")
            continue
        if relative not in rust:
            problems.append(f"{relative}: missing from psiv-pack's pack")
            continue
        if rust[relative].is_symlink():
            continue
        ours, theirs = rust[relative].read_bytes(), python[relative].read_bytes()
        if relative.endswith(".png"):
            try:
                same = image_sha256(ours) == image_sha256(theirs)
            except ValueError as error:
                problems.append(f"{relative}: not a readable PNG ({error})")
                continue
            if not same:
                problems.append(f"{relative}: the decoded image differs")
                continue
        elif ours != theirs:
            offset = next((i for i, (a, b) in enumerate(zip(ours, theirs)) if a != b),
                          min(len(ours), len(theirs)))
            problems.append(f"{relative}: bytes differ from offset {offset} "
                            f"({len(ours)} bytes against {len(theirs)})")
            continue
        matched += 1
    return problems, matched


# ---------------------------------------------------------------------------
# Writer fixtures
# ---------------------------------------------------------------------------
def _tag(value):
    """`value` in a form that keeps what JSON text loses: int against float,
    a float's exact bits, an object's key order."""
    if value is None:
        return {"null": None}
    if isinstance(value, bool):
        return {"bool": value}
    if isinstance(value, int):
        return {"int": str(value)}
    if isinstance(value, float):
        return {"float": str(struct.unpack("<Q", struct.pack("<d", value))[0])}
    if isinstance(value, str):
        return {"str": value}
    if isinstance(value, list):
        return {"array": [_tag(item) for item in value]}
    return {"object": [[key, _tag(item)] for key, item in value.items()]}


def _json_cases():
    rng = random.Random(216)
    floats = [0.0, -0.0, 1.0, 0.5, 2.0, 0.1, 0.1 + 0.2, 1e-4, 1e-5, 9.999e-5, 1e15, 1e16,
              9.999999999999998e15, 1e22, 1e100, 5e-324, 1.7976931348623157e308,
              float("inf"), float("-inf"), 123456789.125, -2.5e-7]
    floats += [rng.uniform(-1000, 1000) for _ in range(60)]
    floats += [rng.random() * 10 ** rng.randint(-30, 30) for _ in range(120)]
    while len(floats) < 400:
        (bits,) = struct.unpack("<d", rng.getrandbits(64).to_bytes(8, "little"))
        if bits == bits and abs(bits) != float("inf"):
            floats.append(bits)
    strings = ["", "plain", "quote \" backslash \\ slash /", "\n\r\t\b\f", "\x00\x01\x1f\x7f",
               "café ÿ Ā € ￿", "\U0001F600 \U00010000 \U0010FFFF", "~ !"]
    unsorted = {"b": 1, "a": [], "C": {}, "é": None, "aa": True, "_": False,
                "10": 1, "9": 2, "": "empty"}
    nested = {"z": [1, [2, [3, {}]], {"y": {"x": [None]}}], "a": {"k": -12345678901234567890}}
    values = [None, True, False, 0, -1, 2 ** 63, -(2 ** 63), [], {}, [[]], [{}], {"a": []},
              strings, floats, unsorted, nested, {"pairs": [unsorted, nested]}]
    # Both orders only where there is an object to order.
    return [{"sort_keys": sort_keys, "value": _tag(value),
             "text": json.dumps(value, indent=2, sort_keys=sort_keys)}
            for value in values
            for sort_keys in ((True, False) if "{" in json.dumps(value) else (True,))]


def _png_cases():
    rng = random.Random(2160)
    cases = []
    shapes = [(1, 1, 1, []), (8, 8, 16, [0]), (37, 5, 3, [1, 2]), (300, 2, 256, [255]),
              (16, 3, 2, []), (5, 7, 64, [3, 60])]
    for width, height, colours, transparent in shapes:
        palette = [tuple(rng.randrange(256) for _ in range(3)) for _ in range(colours)]
        pixels = bytes(rng.randrange(colours) for _ in range(width * height))
        data = encode_indexed(width, height, pixels, palette, transparent)
        cases.append({"kind": "indexed", "width": width, "height": height,
                      "pixels": base64.b64encode(pixels).decode(),
                      "palette": [list(c) for c in palette], "transparent": transparent,
                      "png": base64.b64encode(data).decode(), "image_sha256": image_sha256(data)})
    for width, height in [(1, 1), (13, 4), (2, 90)]:
        pixels = bytes(rng.randrange(256) for _ in range(width * height * 3))
        data = encode_rgb(width, height, pixels)
        cases.append({"kind": "rgb", "width": width, "height": height,
                      "pixels": base64.b64encode(pixels).decode(),
                      "png": base64.b64encode(data).decode(), "image_sha256": image_sha256(data)})
    return cases


def harness_fixture():
    """What `--write-fixtures` writes: synthetic cases, no ROM content."""
    return {"json": _json_cases(), "png": _png_cases(),
            "source": "tools/pack_parity.py --write-fixtures"}


def fixture_text():
    return json.dumps(harness_fixture(), indent=2, sort_keys=True) + "\n"


# ---------------------------------------------------------------------------
# The parity run
# ---------------------------------------------------------------------------
def _error_line(stderr: str, prefix: str) -> str | None:
    lines = [line for line in stderr.splitlines() if line.startswith(prefix)]
    return lines[-1][len(prefix):] if lines else None


def check_wrong_rom(binary: Path, scratch: Path) -> list[str]:
    """Both builders refuse a wrong ROM with the same message."""
    wrong = scratch / "wrong-rom"
    wrong.write_bytes(bytes(0x400))
    relative = os.path.relpath(wrong, ROOT)
    python = subprocess.run([sys.executable, "-m", "psiv_tools", "pack", relative,
                             os.path.relpath(scratch / "wrong-python", ROOT)],
                            cwd=ROOT, capture_output=True, text=True)
    rust = subprocess.run([str(binary), relative, os.path.relpath(scratch / "wrong-rust", ROOT)],
                          cwd=ROOT, capture_output=True, text=True)
    problems = []
    for name, run in (("python", python), ("psiv-pack", rust)):
        if run.returncode == 0:
            problems.append(f"wrong ROM: {name} exited 0")
    for name in ("wrong-python", "wrong-rust"):
        if os.path.lexists(scratch / name):
            problems.append(f"wrong ROM: {name} was created")
    ours, theirs = _error_line(rust.stderr, RUST_PREFIX), _error_line(python.stderr, PYTHON_PREFIX)
    if ours is None or theirs is None or ours != theirs:
        problems.append(f"wrong ROM: psiv-pack says {ours!r}, the Python builder {theirs!r}")
    return problems


def check_progress(stdout: str, sections: int) -> list[str]:
    """`psiv-pack`'s stdout: a progress line per section, then `installed`."""
    lines = stdout.splitlines()
    if not lines or not lines[-1].startswith("installed "):
        return [f"psiv-pack's last line is not `installed <pack>`: {lines[-1:]!r}"]
    problems, seen = [], []
    for line in lines[:-1]:
        match = PROGRESS.match(line)
        if not match:
            problems.append(f"not a progress line: {line!r}")
            continue
        seen.append(int(match.group(1)))
        if int(match.group(2)) != sections:
            problems.append(f"progress total {match.group(2)}, not {sections}: {line!r}")
    if seen != list(range(1, sections + 1)):
        problems.append(f"progress counted {seen}, not 1..{sections}")
    return problems


def _clock():
    mhz = cpu_mhz()
    return f"load {load_1m()}, {mhz:.0f} MHz" if mhz else f"load {load_1m()}, clock unknown"


def run_parity(rom: Path, keep: Path | None) -> int:
    if not rom.is_file():
        print(f"pack parity: missing input, the ROM at {rom}", flush=True)
        return 1
    with hold_heavy_lock(lock_class=BUILD):
        built = subprocess.run(CARGO_BUILD, cwd=ROOT)
    if built.returncode != 0:
        print("pack parity: psiv-pack did not build", flush=True)
        return 1
    sections = json.loads(subprocess.run([str(BINARY), "--claims"], cwd=ROOT, check=True,
                                         capture_output=True, text=True).stdout)["sections"]
    claims = [claim for section in sections for claim in section["claims"]]
    build_dir = ROOT / "build" / "pack-parity"
    build_dir.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix="run-", dir=build_dir)) if keep is None else keep
    scratch.mkdir(parents=True, exist_ok=True)
    try:
        problems = check_wrong_rom(BINARY, scratch)
        with hold_heavy_lock(lock_class=PACK):
            print(f"pack parity: psiv-pack, {_clock()}", flush=True)
            start = time.monotonic()
            rust = subprocess.run([str(BINARY), str(rom), str(scratch / "rust")], cwd=ROOT,
                                  capture_output=True, text=True)
            seconds = time.monotonic() - start
            print(f"pack parity: psiv-pack took {seconds:.2f} s (release, {os.cpu_count()} cores, "
                  f"{_clock()})", flush=True)
            if rust.returncode != 0:
                problems.append(f"psiv-pack exited {rust.returncode}: {rust.stderr.strip()}")
            elif not claims:
                problems += check_output(scratch / "rust", claims)
            problems += check_progress(rust.stdout, len(sections))
            matched = 0
            if claims:
                start = time.monotonic()
                python = subprocess.run([sys.executable, "-m", "psiv_tools", "pack", str(rom),
                                         str(scratch / "python")], cwd=ROOT)
                print(f"pack parity: the Python builder took {time.monotonic() - start:.1f} s",
                      flush=True)
                if python.returncode != 0:
                    problems.append(f"the Python builder exited {python.returncode}")
                else:
                    found, matched = compare(scratch / "rust", scratch / "python", claims)
                    problems += found
            else:
                print("pack parity: psiv-pack claims no sections yet; Python build skipped",
                      flush=True)
    finally:
        if keep is None:
            shutil.rmtree(scratch, ignore_errors=True)
    for problem in problems:
        print(f"pack parity: {problem}", flush=True)
    print(f"pack parity: {len(claims)} claimed paths, {matched} files matched, "
          f"{len(problems)} problems", flush=True)
    return 1 if problems else 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--keep", type=Path, help="build into this directory and keep it")
    parser.add_argument("--write-fixtures", action="store_true",
                        help=f"rewrite {FIXTURE.relative_to(ROOT)} and exit")
    args = parser.parse_args(argv)
    if args.write_fixtures:
        FIXTURE.parent.mkdir(parents=True, exist_ok=True)
        FIXTURE.write_text(fixture_text(), encoding="utf-8", newline="\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
        return 0
    return run_parity(args.rom, args.keep)


if __name__ == "__main__":
    raise SystemExit(main())
