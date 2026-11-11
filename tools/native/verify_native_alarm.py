#!/usr/bin/env python3
"""Check captured BioPlant palette stages against the original component ramps.

The reference is the restored native frame, not an emulator image. This
checks the fade's pixels and cadence, not whole-scene retail parity.
"""
import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image

GREEN = np.array([0, 32, 68, 101, 137, 170, 206, 238], dtype=np.int16)
BLUE = np.array([0, 32, 65, 98, 139, 172, 205, 238], dtype=np.int16)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args(argv)
    directory = args.directory
    receipt = directory / "native.json"
    observations = json.loads(receipt.read_text())["observations"]
    frames = {row["label"]: row["frame"] for row in observations}
    directory = directory / "observations"
    reference = np.array(Image.open(directory / "alarm-in-8.png").convert("RGB"))
    if not np.any(reference[:, :, 1:]):
        raise ValueError("restored alarm reference has no green or blue pixels")
    levels = [
        np.abs(reference[:, :, channel, None].astype(np.int16) - ramp).argmin(axis=2)
        for channel, ramp in [(1, GREEN), (2, BLUE)]
    ]
    result = {"reference": "alarm-in-8.png", "frames": [], "stage_ticks": {}}
    first_tick = int(frames["alarm-out-1"])
    for phase, direction in enumerate(["out", "in"]):
        for stage in range(1, 9):
            label = f"alarm-{direction}-{stage}"
            actual = np.array(Image.open(directory / f"{label}.png").convert("RGB"))
            if actual.shape != reference.shape:
                raise ValueError(f"{label}: capture size changed")
            expected = reference.copy()
            for channel, ramp, level in zip([1, 2], [GREEN, BLUE], levels):
                index = np.minimum(level, stage) if phase else np.maximum(0, level - stage)
                expected[:, :, channel] = ramp[index]
            tick = int(frames[label])
            result["stage_ticks"][label] = tick
            result["frames"].append({
                "direction": direction,
                "stage": stage,
                "pixels_different": int(np.any(actual != expected, axis=2).sum()),
                "red_pixels_different": int((actual[:, :, 0] != reference[:, :, 0]).sum()),
                "tick_error": tick - (first_tick + (phase * 8 + stage - 1) * 4),
            })
    result["complete"] = all(
        row["pixels_different"] == row["red_pixels_different"] == row["tick_error"] == 0
        for row in result["frames"]
    )
    output = receipt.parent / "alarm-pixels.json"
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(f"Alarm pixels and cadence: {'PASS' if result['complete'] else 'FAIL'} ({output})")
    raise SystemExit(0 if result["complete"] else 1)


if __name__ == "__main__":
    main()
