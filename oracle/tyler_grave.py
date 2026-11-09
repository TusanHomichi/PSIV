"""Capture and check Tyler's flagged grave replay, without dialogue dwell.

RAM patches select the map, party position and event; all scene execution is
the unmodified retail cartridge. See docs/scenes/92_TylerGraveOpening.md.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
from pathlib import Path
import subprocess

from oracle import host_binary

ROOT = Path(__file__).resolve().parent.parent
PATCHES = (
    "7000:FFFFEC28:0120", "7000:FFFFEC2A:0000", "7000:FFFFEC4E:02",
    "7000:FFFFEF00:0008", "7000:FFFFF406:0130", "7000:FFFFF408:01B0",
    "7000:FFFFF40A:00010203", "7200:FFFFF110:08", "7200:FFFFECA8:0044",
    "7200:FFFFEC20:000C",
)


def measure(rows: list[dict[str, str]]) -> dict:
    """Fail on changed camera, animation, drift or event-return boundaries."""
    frames = {int(row["frame"]): row for row in rows}

    def expect(frame: int, **values: int | str) -> None:
        row = frames[frame]
        for key, expected in values.items():
            if str(row[key]) != str(expected):
                raise ValueError(f"frame {frame} {key}: {row[key]} != {expected}")

    for frame in range(7199, 7331):
        expect(frame, map_index="0120", c1_x_px=304, c1_y_px=432,
               cam_x_fg_px=156 if frame in (7200, 7268) else 152,
               cam_y_fg_px=344,
               game_mode_routine="0000" if frame in (7199, 7330) else "000C")
    for frame in range(7200, 7204):
        expect(frame, o02_x_px=288, o03_x_px=352, o02_facing=0, o03_facing=4)
    for update in range(1, 65):
        index = 0 if update <= 4 else 1 if update <= 9 else 2 if update <= 14 else 3
        expect(7203 + update, o02_x_px=288 + (-update // 4),
               o03_x_px=352 + update // 2, o02_facing=16, o03_facing=20,
               o02_map_idx=index, o03_map_idx=index)
    for frame in range(7268, 7331):
        expect(frame, o02_x_px=272, o03_x_px=384, o02_facing=8, o03_facing=12,
               o02_map_idx=0, o03_map_idx=0)
    return {"entry_frame": 7200, "first_drift_frame": 7204,
            "last_drift_frame": 7267, "return_frame": 7330,
            "frames_through_return": 131, "drift_frames": 64,
            "animation_index_edges": [7204, 7208, 7213, 7218],
            "camera_lead_frames": [7200, 7268]}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "build/tyler-grave")
    parser.add_argument("--log", type=Path, help="check an existing capture instead")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    log = args.log or args.output / "oracle.csv"
    rom = ROOT / "Phantasy Star IV (USA).md"
    command = None
    if args.log is None:
        command = [str(host_binary.ensure()), "--core",
                   str(ROOT / "oracle/core/genesis_plus_gx_libretro.so"),
                   "--rom", str(rom), "--map", str(ROOT / "oracle/ram_map.tsv"),
                   "--tape", str(ROOT / "oracle/tapes/28_meeting_rika_retail_probe.tape"),
                   "--groups", "core,objects,camera,pos,flagbytes", "--out", str(log),
                   "--dump-frames", "7199,7200,7208,7213,7218,7268,7330",
                   "--dump-frames-dir", str(args.output / "frames")]
        for patch in PATCHES:
            command.extend(["--ram-patch", patch])
        subprocess.run(command, check=True, cwd=ROOT)
    with log.open() as handle:
        rows = list(csv.DictReader(line for line in handle if not line.startswith("#")))
    receipt = {"measurement": measure(rows), "command": command,
               "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()}
    if rom.exists():
        receipt["rom_sha256"] = hashlib.sha256(rom.read_bytes()).hexdigest()
    (args.output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt["measurement"]))


if __name__ == "__main__":
    main()
