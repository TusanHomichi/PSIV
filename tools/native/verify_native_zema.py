#!/usr/bin/env python3
"""Check tape-addressed Zema pages, entrance and restored resident palette.

This proves captured presentation state, not whole-scene retail pixel parity.
The palette copy is MapDataMan_ChkZemaNormal at US $051E56.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image


def verify(native: dict, pack: Path) -> dict:
    rows = {row["label"]: row for row in native["observations"]}
    pages = []
    frames = []
    for number in range(1, 51):
        row = rows[f"zema-page-{number:02d}"]
        state = row["state"]
        page = state["dialogue_page"]
        if state["map"] != 0x24 or not page or not page["ready"]:
            raise ValueError(f"Zema page {number} is not a ready page in Zema")
        pages.append(json.dumps([page["tree"], page["lines"]], sort_keys=True))
        frames.append(row["frame"])
    if len(set(pages)) != 50 or any(a >= b for a, b in zip(frames, frames[1:])):
        raise ValueError("Zema must have 50 distinct pages at increasing tape frames")
    for label, y in (("igglanova-halfway", 176), ("igglanova-arrived", 192)):
        state = rows[label]["state"]
        if state["map"] != 0x24 or not any(
                obj["id"] == 0x188 and obj.get("position") == [480, y]
                for obj in state["temporary_objects"]):
            raise ValueError(f"{label}: missing Igglanova at the recorded entrance position")
    restored = rows["zema-restored"]["state"]
    if restored["map"] != 0x24 or 0x37 not in restored["flags"] or len(restored["active_npcs"]) != 7:
        raise ValueError("Zema aftermath did not restore seven residents")
    record = json.loads((pack / "maps/024_Zema.json").read_text())
    copies = [effect for entry in record["map_effects"] for path in entry["paths"]
              for effect in path.get("deferred_effects", []) if effect.get("at") == "0x051E56"]
    if len(copies) != 1:
        raise ValueError("pack must contain the US Zema palette copy at $051E56")
    copy = copies[0]
    words = [int(word, 16) for word in copy["words_hex"]]
    if restored["cram"].get("live") is not True or restored["cram"]["words"][48:64] != words:
        raise ValueError("restored Zema CRAM line 3 differs from the extracted palette copy")
    sheets = {row["index"]: row["sheet"] for row in restored["npc_sheets"]}
    for sheet in copy["affects"]["npc_sheets"]:
        if sheets.get(sheet["npc_index"]) != sheet["to"]:
            raise ValueError(f"resident {sheet['npc_index']} has not taken its restored palette sheet")
    labels = [f"zema-page-{n:02d}" for n in range(1, 51)] + [
        "igglanova-halfway", "igglanova-arrived", "zema-restored"]
    for label in labels:
        with Image.open(rows[label]["capture"]) as picture:
            if picture.size != (1280, 800):
                raise ValueError(f"{label}: expected a 1280x800 rendered capture")
            picture.load()
    return {"pass": True, "pages": 50, "resident_sheets": len(copy["affects"]["npc_sheets"]),
            "palette_copy": copy["at"], "cram_line": 3, "frames": frames}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, help="native-tape output directory")
    parser.add_argument("--pack", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        result = verify(json.loads((args.directory / "native.json").read_text()), args.pack)
    except (OSError, ValueError, KeyError, TypeError) as error:
        result = {"pass": False, "error": str(error)}
    output = args.directory / "zema-presentation.json"
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(f"Zema pages, entrance and resident palette: {'PASS' if result['pass'] else 'FAIL'} ({output})")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
