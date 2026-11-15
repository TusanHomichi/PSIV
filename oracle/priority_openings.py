"""Capture signed-priority openings with the existing oracle observation hooks.

Run as ``python3 -m oracle.priority_openings capture --kind preemptive --out NEW_DIRECTORY``.
Outputs are local cartridge evidence; never add them to Git.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import time

from oracle import host_binary
from oracle.force.tape import Step, emit_tape, expand_tape, trim_tape
from oracle.priority_measurement import analyze, require, verify_rom, WINDOWS
from oracle.rng_trace import load_rows

ROOT = Path(__file__).resolve().parents[1]
ROM = ROOT / "Phantasy Star IV (USA).md"
CORE = ROOT / "oracle/core/genesis_plus_gx_libretro.so"
WATCHES = (
    0xB62A, 0xB650, 0xB662, 0xB670,
    0x52D6, 0x530E, 0x5316, 0x5326, 0x5350, 0x52C2, 0x52CA,
    0x48F6, 0x491E, 0x4974, 0x497A, 0x4996, 0x499C,
    0xFBE, 0xFF8, 0x1228, 0x123C, 0x1246, 0x5380,
    0x54CC, 0x550A, 0x554E, 0x56A8, 0x56D8,
    0x4239E,
)
EXTRA_MAP = (
    ("opening_continuation", "FFFF4102", 2),
    ("opening_timer", "FFFF4104", 2),
    ("opening_window_width", "FFFF418C", 2),
    ("opening_message_timer", "FFFFEE46", 2),
    ("opening_message_delay", "FFFFEE66", 2),
    ("party_fighter_routine", "FFFF4402", 2),
    ("enemy_fighter_routine", "FFFF4542", 2),
    ("name_routine", "FFFF4642", 2),
    ("enemy_6_reaction", "FFFF456A", 1),
    ("enemy_7_reaction", "FFFF45AA", 1),
)


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def capture(out: Path, kind: str) -> None:
    verify_rom(ROM.read_bytes())
    patches = [] if kind == "normal" else [
        f"24840:FFFFEF0C:{'58B4' if kind == 'preemptive' else '58B3'}"]
    dump_frames = ",".join(str(f) for f in range(24835, 24904 if kind == "preemptive" else 24866))
    out = out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    tape = out / "input.tape"
    base = ROOT / "oracle/tapes/07_first_battle.tape"
    steps = trim_tape(expand_tape(base.read_text()), 24870)
    tape.write_text(emit_tape([*steps, Step(500, ".", "opening_idle")]))
    ram_map = out / "ram_map.tsv"
    ram_map.write_text((ROOT / "oracle/ram_map.tsv").read_text() + "\n" + "".join(
        f"{name}\t{address}\t{size}\topening\thex\n"
        for name, address, size in EXTRA_MAP))
    command = [str(host_binary.ensure()), "--core", str(CORE), "--rom", str(ROM),
               "--map", str(ram_map), "--tape", str(tape),
               "--groups", "core,rng,battle,opening,bcmd,enemy",
               "--out", str(out / "frames.csv"),
               "--rng-trace", str(out / "rng.csv"),
               "--pc-trace", str(out / "pc.csv"), "--pc-trace-from", "24800"]
    for pc in WATCHES:
        command += ["--pc-watch", f"{pc:06X}"]
    for patch in patches:
        command += ["--ram-patch", patch]
    if dump_frames:
        command += ["--dump-frames", dump_frames, "--dump-frames-dir", str(out / "video")]
    sources = [*sorted((ROOT / "oracle/host").glob("*.[ch]")),
               *sorted((ROOT / "oracle/patches").glob("*.patch")),
               ROOT / "oracle/priority_openings.py", ROOT / "oracle/priority_measurement.py",
               ROOT / "oracle/force/tape.py", ROOT / "oracle/rng_trace.py",
               ROOT / "oracle/build_core.sh", ROOT / "oracle/checks.py"]
    receipt = {
        "command": command, "cwd": str(ROOT), "patches": patches,
        "kind": kind, "us_windows": WINDOWS,
        "measurement": "tape07_prefix_then_idle_with_explicit_ram_fixture",
        "source_revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_status": subprocess.check_output(
            ["git", "status", "--porcelain"], cwd=ROOT, text=True),
        "input_sha256": {name: sha(path) for name, path in {
            "rom": ROM, "core": CORE, "host": Path(command[0]),
            "tape": tape, "base_tape": base, "ram_map": ram_map}.items()},
        "source_sha256": {str(p.relative_to(ROOT)): sha(p) for p in sources},
        "started_utc": datetime.now(timezone.utc).isoformat(),
    }
    started = time.monotonic()
    with (out / "stdout.log").open("wb") as handle:
        result = subprocess.run(command, cwd=ROOT, stdout=handle, stderr=subprocess.STDOUT)
    receipt.update(exit_status=result.returncode, elapsed_seconds=time.monotonic() - started,
                   ended_utc=datetime.now(timezone.utc).isoformat())
    receipt["output_sha256"] = {str(p.relative_to(out)): sha(p)
                                for p in sorted(out.rglob("*")) if p.is_file()}
    (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    if result.returncode:
        raise RuntimeError(f"oracle exited {result.returncode}; see {out / 'stdout.log'}")
    print(out)


def analyze_capture(directory: Path, kind: str) -> dict:
    receipt = json.loads((directory / "receipt.json").read_text())
    require(receipt["exit_status"] == 0, "capture failed")
    require(receipt["input_sha256"]["rom"] == sha(ROM), "capture ROM differs")
    expected_patch = f"24840:FFFFEF0C:{'58B4' if kind == 'preemptive' else '58B3'}"
    require(receipt["patches"] == [expected_patch], "unrecognized fixture assumptions")
    for name, expected in receipt["output_sha256"].items():
        require(sha(directory / name) == expected, f"capture artifact changed: {name}")
    for name, file in (("tape", "input.tape"), ("ram_map", "ram_map.tsv")):
        require(sha(directory / file) == receipt["input_sha256"][name], f"input changed: {name}")
    result = analyze(load_rows(directory / "frames.csv"), load_rows(directory / "pc.csv"),
                     load_rows(directory / "rng.csv"), kind)
    require(result["origin"] == 24840, "priority draw moved away from the seed fixture frame")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("capture")
    run.add_argument("--out", type=Path, required=True)
    run.add_argument("--kind", choices=("normal", "preemptive", "ambush"), required=True)
    check = commands.add_parser("analyze")
    check.add_argument("--capture", type=Path, required=True)
    check.add_argument("--kind", choices=("preemptive", "ambush"), required=True)
    check.add_argument("--out", type=Path, required=True, help="new analysis JSON")
    args = parser.parse_args()
    if args.command == "capture":
        capture(args.out, args.kind)
    else:
        verify_rom(ROM.read_bytes())
        result = analyze_capture(args.capture, args.kind)
        with args.out.open("x") as output:
            output.write(json.dumps(result, indent=2) + "\n")
        print(json.dumps({k: result[k] for k in ("kind", "endpoint", "after_origin", "phases")}, indent=2))


if __name__ == "__main__":
    main()
