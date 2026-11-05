#!/usr/bin/env python3
"""Verify a fresh native Continue, one harmless Down step, and a new save.

This requires every gameplay byte except the standing Y word to remain
unchanged, allowing the header's slot number and valid checksum table to
change, and the `SY` standing-Y marker to appear when the source is a native
slot written before it (#129; the codec's rule is `legacy_standing_y` in
rust/psiv-core/src/save.rs). Use on checkpoint fixtures where the step has no gameplay
effect (no poison tick, encounter, warp or story trigger).
"""
import argparse
import hashlib
import json
from pathlib import Path


STANDING_Y_MARKER = (0x2E, b"SY")
#: The native header metadata a slot writer adds before the marker: the
#: roster mask (`$F020`), the `TR` dungeon exit (`$F022`) and the `PS`
#: saved-party bridge (`$F026..$F02D`). A cartridge image is zero there.
NATIVE_METADATA = range(0x20, 0x2E)
ROSTER_MASK = range(0x20, 0x22)
DUNGEON_EXIT = range(0x22, 0x26)


def native_metadata(header):
    return any(header[i] for i in NATIVE_METADATA)


def standing_y(header, payload):
    """The standing row times 16 a slot's `$F408` word names: the cartridge
    stores the object Y, one row less; a legacy native slot (native metadata,
    no marker) stored the standing row itself."""
    word = int.from_bytes(payload[0x308:0x30A], "big")
    offset, marker = STANDING_Y_MARKER
    legacy = header[offset:offset + 2] != marker and native_metadata(header)
    return word if legacy else (word + 16) & 0xFFFF


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--source-sha256", required=True)
    parser.add_argument("--resaved", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    source = args.source.read_bytes()
    resaved = args.resaved.read_bytes()
    digest = hashlib.sha256(source).hexdigest()
    assert digest == args.source_sha256, "Continue modified the source save"
    assert len(source) == len(resaved) == 0x1600
    receipt = json.loads(args.receipt.read_text())
    assert receipt["complete"], "native Continue flow did not finish"
    states = {o["label"]: o["state"] for o in receipt["observations"]}
    first, moved, last = (states[k] for k in ["continued", "moved", "resaved"])
    assert moved["cell"] == last["cell"] == [first["cell"][0], first["cell"][1] + 1]
    for key in ["map", "world", "flags", "money", "leader", "inventory", "party_status", "party_resources"]:
        assert first[key] == moved[key] == last[key], f"one-step Continue changed {key}"
    assert source[::2] == resaved[::2], "unused interleaved bytes changed"
    source_header, saved_header = source[1:0x200:2], resaved[1:0x200:2]
    header_changes = [i for i, (a, b) in enumerate(zip(source_header, saved_header)) if a != b]
    marker_offset, marker = STANDING_Y_MARKER
    allowed = set(range(0x12, 0x1A))
    if source_header[marker_offset:marker_offset + 2] != marker:
        allowed |= {marker_offset, marker_offset + 1}
    if not native_metadata(source_header):
        # A cartridge image's first native resave adds the roster mask and the
        # dungeon exit, in the writer's own form: `TR`, the index, its
        # complement.
        allowed |= set(ROSTER_MASK) | set(DUNGEON_EXIT)
        exit_bytes = saved_header[DUNGEON_EXIT.start:DUNGEON_EXIT.stop]
        assert exit_bytes[:2] == b"TR" and exit_bytes[3] == exit_bytes[2] ^ 0xFF, \
            "the resave's dungeon exit is not the writer's"
    assert saved_header[marker_offset:marker_offset + 2] == marker, "the resave lacks the SY marker"
    assert set(header_changes) <= allowed, "non-checksum header state changed"
    a, b = source[0x201::2], resaved[0x201::2]
    changes = [i for i, (old, new) in enumerate(zip(a, b)) if old != new]
    assert set(changes) <= {0x308, 0x309}, f"unexpected gameplay changes: {changes}"
    for header, payload in [(source_header, a), (saved_header, b)]:
        slot = int.from_bytes(header[0x12:0x14], "big")
        assert slot in range(3)
        assert int.from_bytes(header[0x14 + slot * 2:0x16 + slot * 2], "big") == sum(payload) & 0xFFFF
    assert standing_y(saved_header, b) == standing_y(source_header, a) + 16, "the step is not one row down"
    result = {
        "complete": True,
        "source": str(args.source.resolve()),
        "source_sha256": digest,
        "resaved_sha256": hashlib.sha256(resaved).hexdigest(),
        "header_slot_and_checksum_offsets": [f"0x{i:02x}" for i in header_changes],
        "logical_payload_changes": [
            {"offset": f"0x{i:03x}", "before": a[i], "after": b[i]}
            for i in changes
        ],
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
