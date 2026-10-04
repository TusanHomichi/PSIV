"""Derive a route's regular AND conditional abilities, including scene battles.

Map families and scene documents are scope inputs, never an ability-id list.
Encounter groups, vehicle groups, formations and carriers come from the local
extraction. The ledgers supply classifications read from the cited chains.
Outputs are local evidence; no cartridge records are written into source.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re

from ..force.pack import Pack

ROOT = pathlib.Path(__file__).resolve().parents[2]


def classes(text: str) -> dict[int, str]:
    """Only inventory rows, not the map summary or this tool's receipt."""
    result = {}
    for line in text.splitlines():
        match = re.match(r"\| `\$([0-9A-F]{2})` \(\d+\)", line)
        if not match:
            continue
        cells = line.split("|")
        kind = cells[-3].strip().replace(" †", "")
        if kind == "—" and "enemy_damage::resolve_damage_skill" in cells[-2]:
            kind = "damage"
        result[int(match[1], 16)] = kind
    return result


def scene_records(text: str) -> dict[str, tuple[list[int], list[int]]]:
    """Read only literal map/battle ops of each transcribed Scene record.

    The document's **Data** row selects the records. Splitting at the next
    static prevents a later, unrelated scene's StartBattle leaking into scope.
    Flight maps are selected by the caller's map-family pattern as well.
    """
    records = {}
    for part in re.split(r"pub static ", text)[1:]:
        match = re.match(r"(\w+): Scene = Scene", part)
        if match:
            numbers = lambda pattern: [int(v, 0) for v in re.findall(pattern, part)]
            records[match[1]] = (
                numbers(r"SceneOp::(?:LoadMap|LoadFlightMap)\s*\{\s*map:\s*(0x[\dA-Fa-f]+|\d+)"),
                numbers(r"SceneOp::StartBattle\s*\{\s*index:\s*(0x[\dA-Fa-f]+|\d+)"),
            )
    return records


def derive(pack: Pack, bosses: dict[int, dict], map_ids: set[int],
           battles: set[int], kinds: dict[int, str]) -> dict:
    groups = set()
    maps = []
    for mid in sorted(map_ids):
        m = pack.maps[mid]
        available = set(m.get("groups_available", [])) | set(m.get("vehicle_groups", []))
        if m.get("group") is not None:
            available.add(m["group"])
        groups.update(available)
        maps.append({"id": mid, "symbol": m["map_symbol"], "groups": sorted(available)})
    formations = sorted({fid for g in groups for fid in pack.group_entries(g)})
    random = {entry["enemy"]["id"] for fid in formations
              for entry in pack.formations[fid]["enemies"]}
    event = {entry["enemy"]["id"] for bid in battles
             for entry in bosses[bid]["enemies"]}
    abilities = []
    for ability in sorted({a for eid in random | event for a in pack.ability_ids(eid)}):
        if ability not in kinds:
            raise ValueError(f"ability ${ability:02X} has no inventory classification")
        carriers = sorted(eid for eid in random | event if ability in pack.ability_ids(eid))
        abilities.append({"id": ability, "hex": f"${ability:02X}", "class": kinds[ability],
                          "carriers": carriers,
                          "random_carriers": sorted(set(carriers) & random),
                          "event_carriers": sorted(set(carriers) & event)})
    return {"maps": maps, "groups": sorted(groups), "formations": formations,
            "event_battles": sorted(battles), "random_enemies": sorted(random),
            "event_enemies": sorted(event), "abilities": abilities}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--data-dir", type=pathlib.Path, default=ROOT / "generated")
    parser.add_argument("--map-pattern", required=True)
    parser.add_argument("--scene-doc", action="append", type=pathlib.Path, required=True)
    parser.add_argument("--out", type=pathlib.Path, required=True)
    args = parser.parse_args(argv)
    pack = Pack.load(args.data_dir)
    maps = {mid for mid, m in pack.maps.items() if re.search(args.map_pattern, m["map_symbol"])}
    if not maps:
        parser.error("the map pattern selects no maps")
    inputs = [args.data_dir / f"{name}.json" for name in
              ("enemies", "enemy_skills", "formations", "formation_indexes", "encounters")]
    scene_maps, battles, selected = set(), set(), []
    for path in args.scene_doc:
        text = path.read_text()
        match = re.search(r"\*\*Data:\*\* `([^`]+\.rs)`, `([^`]+)`", text)
        if not match:
            raise ValueError(f"{path}: no literal scene Data row")
        source = ROOT / "rust/psiv-core/src/scenes" / match[1]
        records = scene_records(source.read_text())
        loaded, started = records[match[2]]
        scene_maps.update(loaded)
        battles.update(started)
        selected.append({"document": str(path), "record": match[2],
                         "maps": loaded, "battles": started})
        inputs.extend((path, source))
    inventory = ROOT / "docs/battle/ENEMY_ABILITIES.md"
    bosses = json.loads((args.data_dir / "formations.json").read_text())["boss_formations"]
    result = derive(pack, {b["event_battle_index"]: b for b in bosses},
                    maps | scene_maps, battles, classes(inventory.read_text()))
    result["scenes"] = selected
    result["source_sha256"] = {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                              for p in sorted(set(inputs + [inventory]))}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print(f"{len(result['maps'])} maps; groups {result['groups']}; "
          f"{len(result['formations'])} formations; event battles {result['event_battles']}")
    for ability in result["abilities"]:
        print(f"{ability['hex']} {ability['class']}: carriers {ability['carriers']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
