#!/usr/bin/env python3
"""Verify the ordinary-input BioPlant ORDER receipt against save and oracle bytes.

Static menu comparisons cover the two ORDER windows while choosing, then
the chosen-list window after commit. Oracle frame ranges let the independently
clocked cursor reach the same visible/hidden phase; no pixels are masked.
The surrounding camp summary and field camera are outside this comparison.
"""
import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--ordered", type=Path, required=True)
    parser.add_argument("--route", type=Path, required=True)
    parser.add_argument("--oracle-ram", type=Path, required=True)
    parser.add_argument("--oracle-frames", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    source, ordered = args.source.read_bytes(), args.ordered.read_bytes()
    assert len(source) == len(ordered) == 0x1600
    original = [1, 0, 2, 4, 255]
    wanted = [4, 1, 0, 2, 255]
    a, b = source[0x201::2], ordered[0x201::2]
    assert list(a[0x30A:0x30F]) == original
    assert list(b[0x30A:0x30F]) == wanted
    raw_changes = [i for i, (x, y) in enumerate(zip(source, ordered)) if x != y]
    assert raw_changes == [0x201 + 2 * i for i in range(0x30A, 0x30E)]
    # A permutation also preserves the cartridge's additive checksum.
    assert source[:0x200] == ordered[:0x200]
    header = ordered[1:0x200:2]
    slot = int.from_bytes(header[0x12:0x14], "big")
    assert slot == 0
    assert int.from_bytes(header[0x14:0x16], "big") == sum(b) & 0xFFFF
    assert list(args.oracle_ram.read_bytes()[0xF40A:0xF40F]) == wanted
    receipt = json.loads(args.route.read_text())
    assert receipt["complete"] and receipt["battles"] == 0
    states = {entry["name"]: entry["state"] for entry in receipt["checkpoints"]}
    initial = states["CONTINUE-BioPlant-order"]
    unchanged = ["map", "cell", "money", "inventory", "flags", "leader",
                 "party_status", "party_resources"]
    for label in ["order-open", "order-picked", "order-undone", "order-cancelled"]:
        for key in unchanged:
            assert initial[key] == states[label][key], (label, key)
    for label in ["order-committed", "BioPlant-Gryz-leading", "Saved"]:
        state = states[label]
        assert [int(p["id"]) for p in state["party_status"]] == wanted[:-1]
        assert state["leader"] == 4
        for key in ["map", "cell", "money", "inventory", "flags"]:
            assert initial[key] == state[key], (label, key)
        for key in ["party_status", "party_resources"]:
            assert sorted(initial[key], key=lambda p: p["id"]) == sorted(state[key], key=lambda p: p["id"])
    comparisons = []
    for label, frames, rect in [
        ("order-open", range(1810, 1859), (16, 104, 144, 184)),
        ("order-picked", range(1875, 1896), (16, 104, 144, 184)),
        ("order-undone", range(1903, 1927), (16, 104, 144, 184)),
        ("order-committed", [1985], (88, 104, 144, 184)),
    ]:
        path = args.route.parent / (label + ".png")
        native = Image.open(path).convert("RGB")
        assert native.size == (1280, 800)
        native = native.crop((160, 64, 1120, 736)).resize((320, 224), Image.Resampling.NEAREST)
        candidate = np.asarray(native.crop(rect))
        matches = []
        for frame in frames:
            oracle = args.oracle_frames / f"frame_{frame}.png"
            reference = np.asarray(Image.open(oracle).convert("RGB").crop(rect))
            if np.array_equal(candidate, reference):
                matches.append(frame)
        assert matches, f"no exact ORDER region match for {label}"
        comparisons.append({"label": label, "region": rect, "oracle_frame": matches[0],
                            "different_pixels": 0, "rmse": 0.0,
                            "native_sha256": digest(path),
                            "oracle_sha256": digest(args.oracle_frames / f"frame_{matches[0]}.png")})
    result = {"complete": True, "source_sha256": digest(args.source),
              "ordered_sha256": digest(args.ordered), "party_before": original,
              "party_after": wanted, "changed_payload_offsets": [hex(i) for i in range(0x30A, 0x30E)],
              "oracle_ram_sha256": digest(args.oracle_ram), "menu_regions": comparisons}
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
