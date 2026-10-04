"""Measure tape 35's two flight legs using the repository's oracle host.

The return leg changes only the frame-7000 map/world fixture; input and the
certified frame remain unchanged. CPU/stack diagnostics distinguish explicit
VInt waits from map/text setup work. Outputs are ignored local evidence.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
from pathlib import Path
import subprocess

from oracle import host_binary
from tools.certify import ROM, TAPE_35

ROOT = Path(__file__).resolve().parent.parent
CONFIRM_FRAME = 7401


def read_trace(path: Path) -> list[dict[str, str]]:
    with path.open() as handle:
        return list(csv.DictReader(line for line in handle if not line.startswith("#")))


def measure(rows: list[dict[str, str]], landing: int) -> dict:
    """Report map-write edges and the first field-control frame after landing.

    FieldRoutine_Cutscene returns through GameMode_LoadFieldMap; field control
    is GameMode_Field ($0C), routine 0, not merely GameMode_LoadFieldMap ($08)
    (ps4.asm:120738-120768).
    """
    maps = []
    previous = None
    landed = False
    for row in rows:
        frame = int(row["frame"])
        if frame < CONFIRM_FRAME:
            continue
        map_id = int(row["map_index"], 16)
        if map_id != previous:
            maps.append({"frame": frame, "map": map_id,
                         "offset": frame - CONFIRM_FRAME})
            previous = map_id
        landed |= map_id == landing
        if (landed and int(row["game_mode"], 16) == 12
                and int(row["game_mode_routine"], 16) == 0):
            return {"confirm_frame": CONFIRM_FRAME, "map_edges": maps,
                    "control_frame": frame, "frames_to_control": frame - CONFIRM_FRAME}
    raise ValueError("tape ended without field control after landing")


def run(output: Path) -> dict:
    output.mkdir(parents=True, exist_ok=True)
    binary = host_binary.ensure()
    # Extra diagnostic fields; the ordinary map remains the single source of
    # addresses for game mode, world, map, camera and cursor state.
    map_path = output / "ram-map.tsv"
    map_path.write_text((ROOT / "oracle/ram_map.tsv").read_text()
                        + "vdp_reg1\tFFFFEF0A\t2\tflight\thex\n"
                        + "window_mode\tFFFFEC9C\t1\tflight\thex\n"
                        + "text_chars\tFFFFED5A\t1\tflight\t-\n")
    results = {}
    for name, origin, world, landing in [
        ("mota-zelan", 0xBF, 0, 0x18D), ("zelan-mota", 0x18D, 3, 0xBF)
    ]:
        patches = [f"7000:FFFFEC28:{origin:04X}" if "FFFFEC28" in patch else patch
                   for patch in TAPE_35["patches"]]
        patches.append(f"7000:FFFFF400:{world:02X}00")
        trace = output / f"{name}.csv"
        command = [str(binary), "--core", "oracle/core/genesis_plus_gx_libretro.so",
                   "--rom", ROM, "--map", str(map_path), "--tape", TAPE_35["tape"],
                   "--groups", "core,camera,window,rng,flight", "--cpu-trace",
                   "--out", str(trace)]
        for patch in patches:
            command += ["--ram-patch", patch]
        with (output / f"{name}.log").open("w") as log:
            process = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
        if process.returncode:
            raise RuntimeError(f"{name}: oracle exited {process.returncode}")
        result = measure(read_trace(trace), landing)
        result.update(command=command, exit_code=process.returncode,
                      trace_sha256=hashlib.sha256(trace.read_bytes()).hexdigest())
        results[name] = result
    receipt = {"host_build_id": host_binary.reported_build_id(binary),
               "rom_sha256": hashlib.sha256((ROOT / ROM).read_bytes()).hexdigest(),
               "tape_sha256": hashlib.sha256((ROOT / TAPE_35["tape"]).read_bytes()).hexdigest(),
               "legs": results}
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=Path("build/x77-evidence/oracle"))
    args = parser.parse_args()
    receipt = run(args.out)
    for name, leg in receipt["legs"].items():
        print(name, leg["frames_to_control"], leg["map_edges"])


if __name__ == "__main__":
    main()
