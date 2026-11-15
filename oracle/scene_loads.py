"""Measure scene panel/art loads without replacing their work with waits.

    nix develop -c python3 -m oracle.scene_loads --out build/scene-loads

Uses certify's explicit fixtures (46, 43 by default; 38 also available),
instruction-boundary traces and frame-end RAM samples. The report separates
whole-panel work from its Nemesis and Enigma calls. Master cycles include
interrupts and hardware stalls; intrinsic Nemesis CPU cycles come separately
from psiv-pack's existing decoder. Neither quantity is a frame-budget model.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import subprocess
import time
from collections import Counter
from pathlib import Path

from oracle import host_binary

ROOT = Path(__file__).resolve().parents[1]
ROM = ROOT / "Phantasy Star IV (USA).md"
CORE = ROOT / "oracle/core/genesis_plus_gx_libretro.so"
# US NTSC, non-interlaced: system_frame_gen rebases the core's cycle counter
# by 3420 * 262 master cycles each retro_run (core/system.c).
FRAME_CYCLES = 3420 * 262
# Ends are the boundary BEFORE RTS, except load_art's caller continuation.
# Keep call/return overhead explicit when comparing these observations with
# the decoder's intrinsic cpu_cycles (which includes its own RTS).
ROUTINES = {
    "panel": (0x06AF9C, 0x06B008),
    "nemesis": (0x041740, 0x041788),
    "enigma": (0x041A90, 0x041B54),
    "load_art": (0x0702DE, 0x0702E4),  # Eclipse Torch's JSR NemDecomp.
}
BOUNDARY_BYTES = {
    0x06AF9C: "31c0ed006100", 0x06B004: "5238ed024e75",
    0x041740: "48e7ffdc", 0x041784: "4cdf3bff4e75",
    0x041A90: "48e7ff7c", 0x041B50: "4cdf3eff4e75",
    0x0702CE: "41f9001d712c303c03af", 0x0702DE: "4eb900041740",
}
REQUIRED = {46: {"panel:10d", "art:1d712c"},
            43: {"panel:18b", "panel:18f", "panel:190"},
            38: {"panel:143", "panel:144"}}


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open() as handle:
        return list(csv.DictReader(line for line in handle if not line.startswith("#")))


def absolute(row: dict[str, str]) -> int:
    """Preserve instruction overshoot; do not clamp frame-relative cycles."""
    return (int(row["frame"]) - 1) * FRAME_CYCLES + int(row["cycles"])


def spans(trace: list[dict[str, str]]) -> list[dict]:
    """Match nested routine boundaries; ignore portraits outside our loads."""
    starts = {start: name for name, (start, _) in ROUTINES.items()}
    ends = {end: name for name, (_, end) in ROUTINES.items()}
    stack: list[dict] = []
    loads = []
    previous = -1
    for row in trace:
        stamp = absolute(row)
        if stamp < previous:
            raise ValueError("instruction trace is not in execution order")
        previous = stamp
        pc = int(row["pc"], 16)
        if pc in starts:
            name = starts[pc]
            if name in ("panel", "load_art") or stack:
                stack.append({"routine": name, "start": dict(row), "components": []})
        elif pc in ends:
            name = ends[pc]
            if not stack:
                if name in ("panel", "load_art"):
                    raise ValueError(f"{name} returned without an entry")
                continue
            if stack[-1]["routine"] != name:
                raise ValueError(f"unmatched {name} return inside {stack[-1]['routine']}")
            call = stack.pop()
            call["end"] = dict(row)
            call["master_cycles"] = stamp - absolute(call["start"])
            if stack:
                stack[-1]["components"].append(call)
            else:
                loads.append(call)
    if stack:
        raise ValueError("trace ends inside a measured load")
    return loads


def annotate(loads: list[dict], frames: list[dict[str, str]]) -> list[dict]:
    """Count held frame/seed samples crossed by each call, not its duration."""
    by_frame = {int(row["frame"]): row for row in frames}
    panel_returns = Counter(int(call["end"]["frame"]) for call in loads
                            if call["routine"] == "panel")

    def one(call: dict) -> None:
        start, end = (int(call[key]["frame"]) for key in ("start", "end"))
        held = []
        for frame in range(start + 1, end + 1):
            before, after = by_frame[frame - 1], by_frame[frame]
            if before["main_frame_count"] == after["main_frame_count"]:
                if before["rng_seed"] != after["rng_seed"]:
                    raise ValueError(f"seed changed on held frame {frame}")
                held.append(frame)
        call["held_frames"] = held
        call["lag_frames"] = len(held)
        if call["routine"] == "panel":
            if panel_returns[end] != 1:
                raise ValueError(f"multiple panels return on frame {end}: RAM id is ambiguous")
            if any(by_frame[f]["panel_index"] != by_frame[end]["panel_index"]
                   for f in range(start + 1, end + 1)):
                raise ValueError("panel id changed during measured call")
            call["query"] = f"panel:{int(by_frame[end]['panel_index'], 16):x}"
        elif call["routine"] == "load_art":
            call["query"] = "art:1d712c"
        for child in call["components"]:
            one(child)

    for call in loads:
        one(call)
        # Work after the routine returns can hold the same frame count too.
        # Keep those samples separate: they are not decompressor work.
        end = int(call["end"]["frame"])
        following = []
        frame = end + 1
        while frame in by_frame and (by_frame[frame]["main_frame_count"]
                                    == by_frame[end]["main_frame_count"]):
            if by_frame[frame]["rng_seed"] != by_frame[end]["rng_seed"]:
                raise ValueError(f"seed changed on held frame {frame}")
            following.append(frame)
            frame += 1
        call["following_held_frames"] = following
        call["following_count_resumed"] = frame in by_frame
    return loads


def attach_work(call: dict, work: dict) -> None:
    """Require the decoded record to select the observed calls, in order."""
    art = work["art"] if call["routine"] == "panel" else work
    inputs = ([art] if art is not None else [])
    names = (["nemesis"] if art is not None else [])
    if call["routine"] == "panel":
        inputs += work["mappings"]
        names += ["enigma"] * len(work["mappings"])
    if names != [child["routine"] for child in call["components"]]:
        raise ValueError(f"{call['query']}: trace and decoded record select different calls")
    call["input"] = work
    for child, source in zip(call["components"], inputs):
        child["input"] = source
    call["outside_decompressors_master_cycles"] = (
        call["master_cycles"] - sum(child["master_cycles"] for child in call["components"]))


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_coverage(scene: int, calls: list[dict]) -> None:
    missing = REQUIRED[scene] - {call["query"] for call in calls}
    if missing:
        raise ValueError(f"tape {scene} missed required loads: {sorted(missing)}")


def capture(scene: int, recipe: dict, out: Path) -> list[dict]:
    directory = out / f"tape-{scene}"
    directory.mkdir()
    tape = directory / "input.tape"
    tape.write_bytes((ROOT / recipe["tape"]).read_bytes())
    ram_map = directory / "ram_map.tsv"
    ram_map.write_text((ROOT / "oracle/ram_map.tsv").read_text()
                       + "\npanel_index\tFFFFED00\t2\tscene_load\thex\n")
    trace, frames = directory / "pc.csv", directory / "frames.csv"
    command = [str(host_binary.ensure()), "--core", str(CORE), "--rom", str(ROM),
               "--map", str(ram_map), "--tape", str(tape), "--groups", "core,rng,scene_load",
               "--out", str(frames), "--pc-trace", str(trace), "--pc-trace-from", "7200"]
    for pc in sorted({pc for pair in ROUTINES.values() for pc in pair}):
        command += ["--pc-watch", f"{pc:06X}"]
    for patch in recipe["patches"]:
        command += ["--ram-patch", patch]
    inputs = {"rom": ROM, "core": CORE, "host": Path(command[0]),
              "ram_map": ram_map, "tape": tape, "certify_recipes": ROOT / "tools/certify.py"}
    receipt = {"command": command, "cwd": str(ROOT),
               "input_sha256": {key: sha(path) for key, path in inputs.items()},
               "source_revision": subprocess.check_output(
                   ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
               "source_status": subprocess.check_output(
                   ["git", "status", "--porcelain"], cwd=ROOT, text=True),
               "measurement_source_sha256": {
                   name: sha(ROOT / name) for name in (
                       "oracle/scene_loads.py",
                       "rust/psiv-pack/src/presentation/load_work.rs",
                       "rust/psiv-pack/src/decode/nemesis.rs",
                       "rust/psiv-pack/src/decode/enigma.rs",
                       "rust/psiv-pack/examples/scene_load_work.rs")},
               "measurement": "explicit_scene_fixture", "frame_master_cycles": FRAME_CYCLES}
    started = time.monotonic()
    result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    (directory / "stdout.log").write_bytes(result.stdout)
    receipt.update(exit_status=result.returncode, elapsed_seconds=time.monotonic() - started)
    (directory / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    if result.returncode:
        raise RuntimeError(f"oracle exited {result.returncode}; see {directory / 'stdout.log'}")
    calls = annotate(spans(read_csv(trace)), read_csv(frames))
    check_coverage(scene, calls)
    (directory / "loads.json").write_text(json.dumps(calls, indent=2) + "\n")
    return calls


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True, help="new evidence directory")
    parser.add_argument("--scenes", type=int, nargs="+", choices=(46, 43, 38), default=[46, 43])
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    rom = ROM.read_bytes()
    for pc, expected in BOUNDARY_BYTES.items():
        if rom[pc:pc + len(expected) // 2] != bytes.fromhex(expected):
            raise ValueError(f"US routine boundary mismatch at {pc:06X}")
    # Load the owning fixture recipes only in the command, so the parser tests
    # need neither Pillow nor local assets.
    from tools.certify import TAPE_38, TAPE_43, TAPE_46
    recipes = {38: TAPE_38, 43: TAPE_43, 46: TAPE_46}
    measured = {scene: capture(scene, recipes[scene], out) for scene in args.scenes}
    queries = sorted({call["query"] for calls in measured.values() for call in calls})
    command = ["python3", "tools/heavy_lock.py", "run", "--class", "build", "--",
               "cargo", "run", "--quiet", "--manifest-path", "rust/Cargo.toml", "-p", "psiv-pack",
               "--example", "scene_load_work", "--", str(ROM), *queries]
    started = time.monotonic()
    result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    (out / "work.json").write_bytes(result.stdout)
    (out / "work.stderr").write_bytes(result.stderr)
    (out / "work-receipt.json").write_text(json.dumps(
        {"command": command, "exit_status": result.returncode,
         "elapsed_seconds": time.monotonic() - started}, indent=2) + "\n")
    if result.returncode:
        raise RuntimeError(f"work query exited {result.returncode}; see {out / 'work.stderr'}")
    work = json.loads(result.stdout)
    for scene, calls in measured.items():
        for call in calls:
            attach_work(call, work[call["query"]])
            print(f"tape {scene}: {call['query']}: {call['lag_frames']} held frames; "
                  f"{call['master_cycles']} master cycles; "
                  f"{len(call['following_held_frames'])} further held samples after return")
    (out / "summary.json").write_text(json.dumps(measured, indent=2) + "\n")


if __name__ == "__main__":
    main()
