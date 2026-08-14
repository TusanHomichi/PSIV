"""Field maps: the pointer table, the record grammar and encounter binding.

Three modules, because the map data is three separable problems:

* `records` -- `FieldMapPtrs` and the per-map record grammar transcribed from
  `GameMode_LoadFieldMap`. Its docstring is the format specification.
* `encounters` -- `Battle_EnemyFormationIndexes` and the two Kosinski position
  grids, transcribed from `Battle_SetupEnemyData`.
* this module -- the public entry points, and the join between the two, which
  is where the ROM's disagreements about its own encounter data surface.

`extract_maps` walks all 417 pointer-table entries; `extract_encounter_binding`
decodes the encounter tables on their own and is re-exported here so callers
never need to reach into a submodule.
"""

from __future__ import annotations

from typing import Any

from ..symbols import MAP_SYMBOLS
from .encounters import extract_encounter_binding
from .records import (
    ERROR_TRAP,
    FIELD_MAP_PTRS,
    MAP_COUNT,
    POINTER_SIZE,
    MapError,
    _check_pointer_table,
    _dialogue_tree_index,
    _hex,
    _map_reference,
    _walk_map,
    be32,
)

__all__ = ["MapError", "extract_encounter_binding", "extract_maps"]


def extract_maps(data: bytes) -> dict[str, Any]:
    """Walk every entry of `FieldMapPtrs`, decoding the full record grammar."""
    table = _check_pointer_table(data)
    dialogue_trees = _dialogue_tree_index(data)

    maps: list[dict[str, Any]] = []
    for map_id in range(MAP_COUNT):
        pointer_offset = FIELD_MAP_PTRS + map_id * POINTER_SIZE
        pointer = be32(data, pointer_offset)
        symbol = MAP_SYMBOLS[map_id]
        if pointer == ERROR_TRAP:
            if not symbol.startswith("Null"):
                raise MapError(
                    f"map 0x{map_id:03X} ({symbol}) points at ErrorTrap but its "
                    "MapID_* symbol is not a PtrMap_Null* placeholder"
                )
            maps.append({
                **_map_reference(map_id),
                "is_null": True,
                "pointer_offset": _hex(pointer_offset),
                "pointer": _hex(pointer),
                "raw_hex": data[pointer_offset:pointer_offset + POINTER_SIZE].hex(),
            })
            continue
        if symbol.startswith("Null"):
            raise MapError(
                f"map 0x{map_id:03X} ({symbol}) is a PtrMap_Null* placeholder but "
                f"points at {_hex(pointer)} instead of ErrorTrap"
            )
        maps.append(_walk_map(data, map_id, pointer, dialogue_trees))

    real = [m for m in maps if not m["is_null"]]
    for entry in real:
        for section in ("transitions", "transitions_2"):
            for record in entry[section]["entries"]:
                target = record["target"]
                if target["symbol"] is None or target["symbol"].startswith("Null"):
                    raise MapError(
                        f"map 0x{entry['id']:03X} ({entry['symbol']}) has a "
                        f"{section} record at {record['rom_offset']} targeting map "
                        f"0x{target['id']:03X}, which is not a real map"
                    )

    encounters = extract_encounter_binding(data)
    by_map = {entry["map_id"]: entry for entry in encounters["maps"]}
    anomalies = []
    for entry in real:
        binding = dict(by_map[entry["id"]])
        rolls_battles = bool(entry["flags"]["random_battles"])
        binding["random_battles_flag"] = entry["flags"]["random_battles"]
        # The two halves of the encounter system are stored in different
        # places and nothing in the game reconciles them. Where they disagree
        # the ROM is either wasting a group or, worse, arming a lookup that
        # would run past Battle_FormationIndexes.
        binding["consistent"] = rolls_battles == (binding["mode"] != "none")
        if binding["mode"] == "outside_table":
            binding["consistent"] = not rolls_battles
        if not binding["consistent"]:
            anomalies.append({
                **_map_reference(entry["id"]),
                "mode": binding["mode"],
                "value": binding["value"],
                "random_battles_flag": entry["flags"]["random_battles"],
                "effect": (
                    "random battles are enabled but the map names no encounter "
                    "group, so Battle_SetupEnemyData would index past the "
                    "formation-index table"
                    if rolls_battles
                    else "an encounter group is assigned but random battles are off"
                ),
            })
        entry["encounters"] = binding

    return {
        "pointer_table": table,
        "map_count": MAP_COUNT,
        "real_map_count": len(real),
        "null_map_count": MAP_COUNT - len(real),
        "totals": {
            "tilesets": sum(m["tilesets"]["entry_count"] for m in real),
            "sprite_art": sum(m["sprites"]["entry_count"] for m in real),
            "chunk_pointers": sum(m["chunks"]["pointer_count"] for m in real),
            "transitions": sum(
                m["transitions"]["count"] + m["transitions_2"]["count"] for m in real
            ),
            "objects": sum(m["objects"]["count"] for m in real),
            "treasure_chests": sum(m["treasure_chests"]["count"] for m in real),
            "tile_animations": sum(m["tile_animations"]["count"] for m in real),
            "interaction_areas": sum(m["interaction_areas"]["count"] for m in real),
            "record_bytes": sum(m["size_bytes"] for m in real),
        },
        "dialogue_trees_bound": sorted({m["dialogue"]["tree"] for m in real}),
        "encounter_anomalies": anomalies,
        "encounters": encounters,
        "maps": maps,
    }
