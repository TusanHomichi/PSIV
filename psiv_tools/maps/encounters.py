"""Per-map encounter binding: which formation group a map rolls from.

`Battle_SetupEnemyData` needs three tables to answer that, and they disagree
about their own shape often enough that all three are decoded together here.
The grammar of the map records themselves lives in `records.py`; this module
only borrows that module's primitives and its map-space constants.
"""

from __future__ import annotations

from typing import Any

from ..kosinski import decompress
from ..symbols import MAP_SYMBOLS
from .records import MAP_COUNT, MapError, _hex, _map_reference

# Battle_EnemyFormationIndexes: one byte per MapID, read as
# `move.b (a0,d1.w), d0` with d1 = Field_Map_Index. There is no bounds check
# and the table is 416 bytes, one short of the 417-entry map space.
ENEMY_FORMATION_INDEXES = 0x008050
ENEMY_FORMATION_INDEXES_SIZE = 416
ENEMY_FORMATION_INDEXES_SIGNATURE = bytes.fromhex(
    "00013cffffffffffffffffffffffffffffffffffff0e0f0fffffffffffffffff"
    "ffffffffffffffffffffff1011ffffffffffffffffffffffffffffffffffffff"
)
# Values 0 and 1 fall through to the world-map path instead of naming a group.
WORLD_MAP_SENTINEL_MAX = 1
NO_ENCOUNTERS = 0xFF

# Kosinski-compressed position grids, end-exclusive. Both are indexed as
# `(y >> 6) << 6 | (x >> 6)`, i.e. 64 columns of 64-pixel-square cells.
POSITION_GRID_COLUMNS = 0x40
POSITION_GRIDS: list[dict[str, Any]] = [
    {
        "name": "motavia",
        "label": "Battle_MotaFormationGroupIndexes",
        "map_id": 0,
        "start": 0x28501C,
        "end": 0x2853C8,
    },
    {
        "name": "dezolis",
        "label": "Battle_DezoFormationGroupIndexes",
        "map_id": 1,
        "start": 0x2853CC,
        "end": 0x28541F,
    },
]

# Battle_SetupEnemyData's vehicle branch bypasses the position grids entirely
# and hard-codes a group per (world, Mota_Battle_BG_Index).
VEHICLE_GROUPS: list[dict[str, Any]] = [
    {"world": "dezolis", "condition": "any vehicle on Dezolis", "group": 0x0D},
    {"world": "motavia", "condition": "Mota_Battle_BG_Index == 1", "group": 0x09},
    {"world": "motavia", "condition": "Mota_Battle_BG_Index == 3", "group": 0x0A},
    {"world": "motavia", "condition": "any other Mota_Battle_BG_Index", "group": 0x08},
]


def _check_enemy_formation_indexes(data: bytes) -> bytes:
    start = ENEMY_FORMATION_INDEXES
    actual = data[start:start + len(ENEMY_FORMATION_INDEXES_SIGNATURE)]
    if actual != ENEMY_FORMATION_INDEXES_SIGNATURE:
        raise MapError(
            f"Battle_EnemyFormationIndexes signature mismatch at {_hex(start)}: "
            f"expected {ENEMY_FORMATION_INDEXES_SIGNATURE.hex()}, got {actual.hex()}"
        )
    if data.count(ENEMY_FORMATION_INDEXES_SIGNATURE) != 1:
        raise MapError(
            "Battle_EnemyFormationIndexes signature is not unique in this ROM; "
            "the offset would not be self-verifying"
        )
    return data[start:start + ENEMY_FORMATION_INDEXES_SIZE]


def _decode_position_grid(data: bytes, spec: dict[str, Any], group_count: int) -> dict[str, Any]:
    decompressed, consumed = decompress(data, spec["start"])
    expected = spec["end"] - spec["start"]
    if consumed != expected:
        raise MapError(
            f"{spec['label']} at {_hex(spec['start'])}: decompressor consumed "
            f"{consumed} bytes, but the documented range is {expected} bytes"
        )
    if len(decompressed) % POSITION_GRID_COLUMNS:
        raise MapError(
            f"{spec['label']} decompresses to {len(decompressed)} bytes, which is "
            f"not a whole number of {POSITION_GRID_COLUMNS}-column rows"
        )
    rows = len(decompressed) // POSITION_GRID_COLUMNS
    groups: set[int] = set()
    fallback_cells = 0
    for value in decompressed:
        if value & 0x80:
            # `bpl` fails, so Battle_SetupEnemyData substitutes group 0.
            fallback_cells += 1
            continue
        if value >= group_count:
            raise MapError(
                f"{spec['label']} names encounter group {value}, but only "
                f"{group_count} groups exist in Battle_FormationIndexes"
            )
        groups.add(value)
    return {
        "name": spec["name"],
        "label": spec["label"],
        "map": _map_reference(spec["map_id"]),
        "rom_offset": _hex(spec["start"]),
        "rom_end_exclusive": _hex(spec["end"]),
        "compression": "kosinski",
        "compressed_size": expected,
        "decompressed_size": len(decompressed),
        "columns": POSITION_GRID_COLUMNS,
        "rows": rows,
        "cell_size_pixels": 0x40,
        "indexing": "(character_y >> 6) * 64 + (character_x >> 6)",
        "fallback_cells": fallback_cells,
        "fallback_group": 0,
        "groups_used": sorted(groups),
        "cells": [
            list(decompressed[row * POSITION_GRID_COLUMNS:(row + 1) * POSITION_GRID_COLUMNS])
            for row in range(rows)
        ],
    }


def extract_encounter_binding(data: bytes, group_count: int | None = None) -> dict[str, Any]:
    """Decode how a map turns into a `Battle_FormationIndexes` group.

    `Battle_SetupEnemyData` reads one byte of `Battle_EnemyFormationIndexes`
    indexed straight by `Field_Map_Index`. Values above 1 are the encounter
    group. Values 0 and 1 mean "this is a world map": the group then comes
    either from a hard-coded vehicle table or, on foot, from a Kosinski
    position grid indexed by the party leader's coordinates.

    $FF is how the data says "never roll an encounter here". It is not a
    terminator and the routine does not special-case it -- 255 would be used
    as a group index and read far past the 68 groups -- so it is only safe
    because those maps also clear `Random_Battles_Flag`.
    """
    if group_count is None:
        from ..formations import extract_formation_indexes

        group_count = extract_formation_indexes(data)["group_count"]

    table = _check_enemy_formation_indexes(data)
    grids = [_decode_position_grid(data, spec, group_count) for spec in POSITION_GRIDS]
    grid_by_map = {grid["map"]["id"]: grid for grid in grids}

    for entry in VEHICLE_GROUPS:
        if entry["group"] >= group_count:
            raise MapError(
                f"the vehicle encounter group {entry['group']} for "
                f"{entry['condition']} is outside the {group_count} known groups"
            )

    entries = []
    for map_id in range(MAP_COUNT):
        record: dict[str, Any] = {
            "map_id": map_id,
            **{f"map_{k}": v for k, v in _map_reference(map_id).items() if k != "id"},
        }
        if map_id >= len(table):
            # The table is one byte short of the map space; see module notes.
            record.update({
                "in_table": False,
                "rom_offset": None,
                "value": None,
                "mode": "outside_table",
                "group": None,
            })
            entries.append(record)
            continue
        value = table[map_id]
        record.update({
            "in_table": True,
            "rom_offset": _hex(ENEMY_FORMATION_INDEXES + map_id),
            "value": value,
        })
        if value == NO_ENCOUNTERS:
            record.update({"mode": "none", "group": None})
        elif value <= WORLD_MAP_SENTINEL_MAX:
            grid = grid_by_map.get(map_id)
            if grid is None:
                raise MapError(
                    f"map 0x{map_id:03X} ({MAP_SYMBOLS[map_id]}) has world-map "
                    f"encounter sentinel {value} but is not one of the two maps "
                    "with a position grid"
                )
            record.update({
                "mode": "position_grid",
                "group": None,
                "position_grid": grid["name"],
                "groups_available": grid["groups_used"],
                "vehicle_groups": [
                    e["group"] for e in VEHICLE_GROUPS if e["world"] == grid["name"]
                ],
            })
        else:
            if value >= group_count:
                raise MapError(
                    f"map 0x{map_id:03X} ({MAP_SYMBOLS[map_id]}) names encounter "
                    f"group {value}, but only {group_count} groups exist"
                )
            record.update({"mode": "group", "group": value})
        entries.append(record)

    return {
        "table": {
            "label": "Battle_EnemyFormationIndexes",
            "rom_offset": _hex(ENEMY_FORMATION_INDEXES),
            "rom_end_exclusive": _hex(ENEMY_FORMATION_INDEXES + len(table)),
            "entry_size": 1,
            "count": len(table),
            "map_count": MAP_COUNT,
            "signature_hex": ENEMY_FORMATION_INDEXES_SIGNATURE.hex(),
            "indexing": "Field_Map_Index, one byte, no bounds check",
            "no_encounter_value": f"0x{NO_ENCOUNTERS:02X}",
            "world_map_sentinel_max": WORLD_MAP_SENTINEL_MAX,
            "maps_outside_table": MAP_COUNT - len(table),
        },
        "formation_group_count": group_count,
        "vehicle_groups": VEHICLE_GROUPS,
        "position_grids": grids,
        "maps": entries,
    }
