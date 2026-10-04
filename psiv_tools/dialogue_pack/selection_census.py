"""Re-derive every map/area binding from retail and census every valid world.

Run `python3 -m psiv_tools.dialogue_pack.selection_census ROM PACK --json OUT
--markdown TABLE`. Outputs are local evidence; the Markdown table may be
included in the selection ledger. All six worlds are evaluated on every map,
including counterfactual pairs, so transient world changes cannot be omitted.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from .common import DialoguePackError
from .selection import extract_selection
from ..maps import extract_maps


def compare_map(source: dict[str, Any], packed: dict[str, Any]) -> None:
    """Fail when pack metadata disagrees with the independently decoded record."""
    if packed["dialogue_tree"] != source["dialogue"]["tree"]:
        raise DialoguePackError(f"map {source['id']}: per-map tree disagrees with ROM")
    original = source["interaction_areas"]["entries"]
    areas = packed["interaction_areas"]
    if len(areas) != len(original):
        raise DialoguePackError(f"map {source['id']}: area count disagrees with ROM")
    for area, raw in zip(areas, original):
        for key in ("index", "interaction_type", "parameter", "flag", "flag_type"):
            if area[key] != raw[key]:
                raise DialoguePackError(f"map {source['id']} area {raw['index']}: {key} disagrees")


def census(rom: bytes, pack: Path) -> dict[str, Any]:
    """Full selector matrix, with source offsets and per-area differences."""
    manifest_bytes = (pack / "manifest.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    selection = extract_selection(rom)
    source_maps = {row["id"]: row for row in extract_maps(rom)["maps"] if not row["is_null"]}
    if {row["id"] for row in manifest["maps"]} != set(source_maps):
        raise DialoguePackError("census requires all real retail maps")
    rows = []
    for entry in manifest["maps"]:
        packed = json.loads((pack / entry["json"]).read_text())
        source = source_maps[entry["id"]]
        compare_map(source, packed)
        areas = []
        for area in source["interaction_areas"]["entries"]:
            if area["interaction_type"] != 0:
                continue
            trees = list(selection["world_trees"])
            high = selection["first_world_override"]
            if area["parameter"] >= high["entry_from"]:
                trees[0] = high["tree"]
            areas.append({"index": area["index"], "entry": area["parameter"],
                          "record_offset": area["rom_offset"], "world_trees": trees,
                          "differs": [tree != packed["dialogue_tree"] for tree in trees]})
        rows.append({"id": entry["id"], "symbol": source["symbol"],
                     "map_tree": packed["dialogue_tree"],
                     "map_tree_offset": source["dialogue"]["rom_offset"], "areas": areas})
    return {"rom_sha256": hashlib.sha256(rom).hexdigest(),
            "pack_manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
            "selection": selection, "map_count": len(rows),
            "area_map_count": sum(bool(row["areas"]) for row in rows),
            "area_count": sum(len(row["areas"]) for row in rows),
            "differing_area_count": sum(any(a["differs"]) for row in rows for a in row["areas"]),
            "worlds": list(range(len(selection["world_trees"]))), "maps": rows}


def markdown(result: dict[str, Any]) -> str:
    """One row per real map; cells include all selected trees and mismatch counts."""
    lines = ["| Map | NPC/default scene tree | Type-0 areas | W0 | W1 | W2 | W3 | W4 | W5 |",
             "| --- | ---: | ---: | --- | --- | --- | --- | --- | --- |"]
    for row in result["maps"]:
        cells = []
        for world in result["worlds"]:
            trees = sorted({a["world_trees"][world] for a in row["areas"]})
            different = sum(a["differs"][world] for a in row["areas"])
            cells.append("/".join(map(str, trees)) + f" ({different})" if trees else "—")
        lines.append(f"| `${row['id']:03X}` {row['symbol']} | {row['map_tree']} | "
                     f"{len(row['areas'])} | " + " | ".join(cells) + " |")
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rom", type=Path)
    parser.add_argument("pack", type=Path)
    parser.add_argument("--json", type=Path, required=True)
    parser.add_argument("--markdown", type=Path, required=True)
    args = parser.parse_args()
    result = census(args.rom.read_bytes(), args.pack)
    args.json.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    args.markdown.write_text(markdown(result))
    print(f"{result['map_count']} maps, {result['area_count']} type-0 areas on "
          f"{result['area_map_count']} maps, {result['differing_area_count']} differ")


if __name__ == "__main__":
    main()
