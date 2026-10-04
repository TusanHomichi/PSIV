"""One map record's pack JSON: warps, interaction areas, NPCs, chests, facings.

Split out of `psiv_tools.pack` (which re-exports every name here) when that
module passed the 1,000-line limit. The builders read a decoded map record and
its collision grid and return plain JSON; writing files stays in `pack`.
"""

from __future__ import annotations

from typing import Any, Sequence

from .layouts import COLLISION_CELL_PIXELS
from .maps.records import INTERACTION_EVENT_INDEXES
from .overworld import Overworld
from .pack_format import PACK_FORMAT_VERSION
from .sprites.emit import NpcMetadata
from .symbols import ITEM_SYMBOLS
from .warps import (
    STANDING_CELL_Y_OFFSET,
    PackError,
    interaction_rect,
    warp_rect,
    xy_range_name,
)

#: `Map_Start_Facing_Dir` in the disassembly's constants.
FACING_NAMES: dict[int, str] = {0: "down", 4: "up", 8: "right", 0xC: "left"}

#: The transition tables in record order, and the tile collision that makes
#: `RunMapTransitions` walk each one.
TRANSITION_TABLES = (
    ("transitions", 1, "any_walkable_tile"),
    ("transitions_2", 2, "map_change_tile"),
)

MAP_CHANGE_COLLISION_TYPE = 0x1

def _facing(value: int) -> dict[str, Any]:
    return {"id": value, "name": FACING_NAMES.get(value)}


def _target(reference: dict[str, Any]) -> dict[str, Any]:
    return {
        "id": reference["id"],
        "id_hex": reference["id_hex"],
        "symbol": reference["symbol"],
    }


def _warps(record: dict[str, Any], grid) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Every transition of both tables, with its trigger rectangle.

    The second return value collects transitions whose rectangle covers no
    type-1 cell. For table 1 that is the normal case, because
    `MapTransTile_Normal` runs on ordinary ground. For table 2 it means the
    door cannot fire from the layout as stored, since `MapTransTile_MapChange`
    is only ever reached from a type-1 cell.

    Five retail records are in the second class, and none of them is a defect:
    they are doors a `MapDataManager` routine opens at load time by writing a
    different chunk id into `Map_Layout` through `GetMapLayoutOffset`.
    `MapDataMan_ChkZemaNormal` swaps chunks $55/$56 for $59/$5A at Zema's four
    house entrances once `EventFlag_IgglanovaZema` is set, and chunk $5A is the
    one carrying a map-change cell; `MapDataMan_BioPlantDoor` does the same with
    chunk $29 at `MapID_BirthValley_B1`. (The disassembly labels Zema's table
    `Zema_LockedDoorsOffs`, which is backwards -- those offsets are where the
    doors open.) The pack emits the layout as stored and reports the five, so a
    runtime that has not implemented `MapDataManager` yet knows which doors it
    is missing rather than finding out by walking into a wall.
    """
    warps: list[dict[str, Any]] = []
    anomalies: list[dict[str, Any]] = []
    for section, table, trigger in TRANSITION_TABLES:
        for entry in record[section]["entries"]:
            source = entry["source"]
            range_id = entry["range"]["id"]
            rect = warp_rect(range_id, source["x_tile"], source["y_tile"], grid.width, grid.height)
            covered = (
                sum(
                    1
                    for x, y in rect.cells()
                    if grid.type_at(x, y) == MAP_CHANGE_COLLISION_TYPE
                )
                if rect is not None
                else 0
            )
            destination = entry["destination"]
            warp = {
                "index": len(warps),
                "table": table,
                "trigger": trigger,
                "record_offset": entry["rom_offset"],
                "range": {"id": range_id, "name": xy_range_name(range_id)},
                "source": {
                    "x_byte": source["x_tile"],
                    "y_byte": source["y_tile"],
                    "x_cell": source["x_tile"],
                    "y_cell": source["y_tile"] + STANDING_CELL_Y_OFFSET,
                },
                "rect": rect.to_json() if rect is not None else None,
                "target": _target(entry["target"]),
                "destination": {
                    "x_cell": destination["x_tile"],
                    "y_cell": destination["y_tile"] + STANDING_CELL_Y_OFFSET,
                },
                "facing": _facing(entry["facing_dir"]),
                "character_alignment": entry["character_alignment"],
            }
            warps.append(warp)
            if covered == 0:
                anomalies.append({
                    "warp_index": warp["index"],
                    "table": table,
                    "range": warp["range"],
                    "record_offset": warp["record_offset"],
                    "rect": warp["rect"],
                    "target": warp["target"],
                })
    return warps, anomalies


def _interaction_areas(record: dict[str, Any], grid) -> list[dict[str, Any]]:
    """Emit interaction areas with their resolved collision rectangles.

    The extractor keeps the ROM's raw 8-pixel coordinates.  The runtime walks
    a 16-pixel collision grid, so the conversion is explicit here rather than
    asking every consumer to rediscover it.  Type 2 (`Interaction_GetEvent`)
    records also carry the indexed Event_Index word; other handlers retain
    their raw parameter without pretending it has event semantics.
    """
    areas = []
    for entry in record["interaction_areas"]["entries"]:
        source = entry
        range_id = entry["range"]["id"]
        rect = interaction_rect(
            range_id,
            source["x_tile"],
            source["y_tile"],
            grid.width,
            grid.height,
        )
        event_index = None
        if entry["interaction_type"] == 2:
            parameter = entry["parameter"]
            if parameter >= len(INTERACTION_EVENT_INDEXES):
                raise PackError(
                    f"interaction area at {entry['rom_offset']} indexes "
                    f"Interaction_EventIndexes with ${parameter:02X}, outside "
                    f"the {len(INTERACTION_EVENT_INDEXES)}-entry table"
                )
            event_index = INTERACTION_EVENT_INDEXES[parameter]
        areas.append({
            "index": entry["index"],
            "record_offset": entry["rom_offset"],
            "range": {"id": range_id, "name": xy_range_name(range_id)},
            "source": {
                "x_byte": source["x_tile"],
                "y_byte": source["y_tile"],
                "x_cell": source["x_tile"] // 2,
                "y_cell": source["y_tile"] // 2 + STANDING_CELL_Y_OFFSET,
            },
            "rect": rect.to_json() if rect is not None else None,
            "flag_type": entry["flag_type"],
            "flag": entry["flag"],
            "interaction_type": entry["interaction_type"],
            "parameter": entry["parameter"],
            "event_index": event_index,
        })
    return areas


def _npcs(
    record: dict[str, Any], sprites: Sequence[NpcMetadata] = ()
) -> list[dict[str, Any]]:
    """`LoadMapObjects` entries.

    Object coordinates are words scaled by 8 (`lsl.w #3,d0`), so 85 of the
    cartridge's 949 objects sit on a half-cell. Pixels are what the record
    says; the cell is the floor of that plus the standing-cell shift, i.e. the
    cell the object's collision would be read from.

    `sprites` is one `NpcMetadata` per object, in order: the sheet reference or
    the reason there is none, plus `interactable`. That last one is independent
    of art -- an invisible block draws nothing and still answers the talk probe
    and still blocks the walker -- so it sits beside `sprite`, not inside it.
    """
    out = []
    blank = NpcMetadata(None, None, False, False)
    for entry in record["objects"]["entries"]:
        meta = sprites[entry["index"]] if entry["index"] < len(sprites) else blank
        out.append({
            "index": entry["index"],
            "record_offset": entry["rom_offset"],
            "object_id": entry["object_id"],
            "symbol": entry["symbol"],
            "x_pixels": entry["x"],
            "y_pixels": entry["y"],
            "x_cell": entry["x"] // COLLISION_CELL_PIXELS,
            "y_cell": entry["y"] // COLLISION_CELL_PIXELS + STANDING_CELL_Y_OFFSET,
            "facing": _facing(entry["facing_dir"]),
            "dialogue_id": entry["dialogue_id"],
            "art_tile": entry["art_tile"],
            **meta.to_json(),
        })
    return out


def _treasure_chests(record: dict[str, Any]) -> list[dict[str, Any]]:
    """`LoadTreasureChests` entries: coordinates are bytes scaled by 16.

    Byte 1 selects how byte 3 reads. Zero makes it an item id; anything else
    makes it a count of *hundreds* of meseta, which is `loc_66BEE` printing the
    stored number in front of a string that already begins "00".
    """
    out = []
    for entry in record["treasure_chests"]["entries"]:
        item_id = entry["item_id"]
        if item_id is not None and not 1 <= item_id <= len(ITEM_SYMBOLS):
            raise PackError(
                f"treasure chest at {entry['rom_offset']} names item {item_id}, "
                f"outside the {len(ITEM_SYMBOLS)}-entry item table"
            )
        out.append({
            "index": entry["index"],
            "record_offset": entry["rom_offset"],
            "x_cell": entry["x_tile"],
            "y_cell": entry["y_tile"] + STANDING_CELL_Y_OFFSET,
            "x_pixels": entry["x"],
            "y_pixels": entry["y"],
            "white_chest": entry["white_chest"],
            "object_symbol": entry["object_symbol"],
            "contents_type": entry["contents_type"],
            "item_id": item_id,
            "item_symbol": ITEM_SYMBOLS[item_id - 1] if item_id is not None else None,
            "meseta": entry["meseta"],
            "chest_flag": entry["chest_flag"],
        })
    return out


def map_json(
    record: dict[str, Any],
    decoded,
    png_path: str,
    sprites: Sequence[tuple[dict[str, Any] | None, str | None]] = (),
    overworld: Overworld | None = None,
    png_over_path: str | None = None,
    effects: Sequence[dict[str, Any]] = (),
    variants: Sequence[dict[str, Any]] = (),
    patch_tiles: dict[str, Any] | None = None,
    overworld_patches: Sequence[dict[str, Any]] = (),
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """The runtime record for one map, and its warp anomalies.

    `png_over_path` names the priority overlay when the map has one, and is
    `None` when every tile it draws is below sprites. The key is always
    present, so a consumer tests its value rather than its existence.

    `overworld` is set for the two paged maps and adds raw `layout_patches`
    plus `overworld_patches`, their resolved flag-selected collision/composite
    tiles. These are page-loader hooks, distinct from `MapDataManager` entries.
    """
    grid = decoded.collision
    layout = decoded.collision_layout
    if (grid.width, grid.height) != (
        layout.width_chunks * 2,
        layout.height_chunks * 2,
    ):
        raise PackError(
            f"map 0x{record['id']:03X}: collision grid is {grid.width}x{grid.height} "
            f"cells but its plane is {layout.width_chunks}x{layout.height_chunks} chunks"
        )

    warps, anomalies = _warps(record, grid)
    interaction_areas = _interaction_areas(record, grid)
    flags = record["flags"]
    music = record["music"]
    payload = {
        "format_version": PACK_FORMAT_VERSION,
        "id": record["id"],
        "id_hex": record["id_hex"],
        "symbol": record["symbol"],
        "record_offset": record["rom_offset"],
        "png": png_path,
        "png_over": png_over_path,
        "dimensions": {
            "width_cells": grid.width,
            "height_cells": grid.height,
            "width_chunks": layout.width_chunks,
            "height_chunks": layout.height_chunks,
            "width_pixels": layout.width_pixels,
            "height_pixels": layout.height_pixels,
            "cell_pixels": COLLISION_CELL_PIXELS,
        },
        # `loc_51AB2` consumes this record section before object loading and
        # writes EC24/25/26; it is camera control, not object metadata.
        "scroll": record["scroll"],
        "collision": {
            "plane": decoded.spec.collision_plane_name,
            "plane_byte": decoded.spec.collision_plane,
            "width_cells": grid.width,
            "height_cells": grid.height,
            "rows": [
                list(grid.types[y * grid.width:(y + 1) * grid.width])
                for y in range(grid.height)
            ],
        },
        # Vehicle battle selects its background from the raw chunk id at the
        # current 32px cell, before collision decoding. Preserve that plane so
        # the runtime does not try to infer a chunk from a four-bit nibble.
        "vehicle_battle": {
            "plane": layout.plane,
            "width_chunks": layout.width_chunks,
            "height_chunks": layout.height_chunks,
            "rows": [
                list(layout.cells[y * layout.width_chunks:(y + 1) * layout.width_chunks])
                for y in range(layout.height_chunks)
            ],
        },
        "music": {
            "id": music["id"],
            "symbol": music["symbol"],
            "changes_music": music["changes_music"],
        },
        "flags": {
            "poison": flags["poison"],
            "random_battles": flags["random_battles"],
            "town_teleport": flags["town_teleport"],
            "dungeon_teleport_index": flags["dungeon_teleport_index"],
        },
        "dialogue_tree": record["dialogue"]["tree"],
        # `RunEvents` walks this list every frame the party is standing still,
        # calls each id's `RunEventsJmpTbl` entry in order, and stops at the
        # first one whose condition is met. So the order is evaluation order and
        # the list is a priority list, not a set.
        "events": record["events"]["ids"],
        # The map's MapDataManager list, decoded. Applied when the map is
        # built, from the flag state at that moment, and never re-evaluated
        # while it is loaded -- see `psiv_tools.map_effects`.
        "map_effects": list(effects),
        # Alternate layouts a `layout_replace` swaps in, already decoded and
        # rendered so no consumer needs a decompressor.
        "layout_variants": list(variants),
        # The chunks a `layout_write` stamps in, drawn: each write names a tile
        # in this atlas, and the resolved collision travels on the write itself
        # as `cells`. `None` for a map whose effects write no layout cell --
        # which is every map but thirteen.
        "patch_tiles": patch_tiles,
        "warps": warps,
        "interaction_areas": interaction_areas,
        "npcs": _npcs(record, sprites),
        "treasure_chests": _treasure_chests(record),
    }
    if overworld is not None:
        payload["layout_patches"] = [patch.to_json() for patch in overworld.patches]
        payload["overworld_patches"] = list(overworld_patches)
    return payload, anomalies
