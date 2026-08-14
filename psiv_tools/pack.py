"""The runtime pack: the lean bundle `psiv-data` loads instead of `generated/`.

`generated/` is archaeology -- provenance-heavy, metadata-only for anything
pixel-shaped, and shaped for a human reading a diff. The Rust runtime wants the
opposite: one small JSON per map holding exactly the facts field mode needs,
one composed PNG per map, and a manifest that pins the ROM the whole set came
from. `docs/RUNTIME_DESIGN.md` is the decision record; this module is the
emitter, and the JSON it writes is a versioned interface, so field names here
are stable and `format_version` moves when they are not.

Nothing in here re-decodes anything. Map records come from
`psiv_tools.maps.extract_maps`, layouts, collision and the composed render come
from `psiv_tools.layouts`. The one piece of format knowledge that lives here
and nowhere else is the warp trigger rectangle.

Trigger rectangles
------------------

A transition record stores an `XYRangeJmpTbl` index, not a rectangle.
`DoMapTransitionData` loads the record's two coordinate bytes into d0/d1 scaled
by 16, the party leader's `curr_x_pos`/`curr_y_pos` into d2/d3, and jumps into
the table; the routine there answers "is the player inside this transition's
area". The fifteen routines are transcribed below into rectangles.

Two details decide what those rectangles mean, and both are checkable against
the cartridge rather than assumed:

* `DoMapTransitionData` returns immediately when either step counter is
  non-zero, so the comparison only ever runs with the character standing still,
  i.e. exactly on the 16-pixel grid. Every bound the routines test is itself a
  multiple of 16, so a pixel rectangle converts to a cell rectangle with
  nothing left over.
* `GetChunkAndCollision` adds `#$10` to Y before it derives a cell
  (`addi.w #$10,d6`), so the cell a character *occupies* is one row below
  `curr_y_pos // 16`. Record coordinates are `curr_*_pos` values, so every Y a
  map record stores -- transition source and destination, object and chest
  placement -- is one row above the cell it is talking about.

So the rectangles this module emits are in **collision-grid cells**, the same
coordinate space as `collision.rows` and the same space
`docs/RUNTIME_DESIGN.md` puts logical position in: a warp fires when the
player's occupied cell is inside `rect`. The raw record bytes stay in
`source.x_byte` / `source.y_byte` so the shift is re-derivable.

Which table a transition came from is load-bearing and is emitted as `table`:

* table 1 (`Map_Transition_Data_Addr`) is walked by `MapTransTile_Normal`,
  which `RunMapTransitions` selects for every standing collision type *except*
  map-change, solid, ice and shop. These fire on ordinary ground -- map edges,
  cave mouths, doormats.
* table 2 (`Map_Transition_Data_2_Addr`) is walked by `MapTransTile_MapChange`,
  reached only from standing collision type 1, and only when the previously
  occupied cell was not also type 1. These are doorways, and their rectangles
  must overlap a type-1 cell or they can never fire; the manifest reports any
  that do not.

Rectangles are clipped to the map. Five of the routines are open-ended
(`XLower`, `XHigher`, `YLower`, `YHigher`, `XYLowerWithPlayerY` test one bound
and let the other run to infinity), which is only a rectangle at all because
the player cannot leave the grid.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Iterator, Sequence

from .layouts import (
    BLOCKING_COLLISION_TYPES,
    CHUNK_WORDS,
    COLLISION_CELL_PIXELS,
    COLLISION_TYPE_NAMES,
    MAX_CHUNKS,
    PLANE_BG,
    PLANE_FG,
    ChunkTable,
    Layout,
    MapLayout,
    MapLayoutSpec,
    chunk_palette,
    decode_chunks,
    decode_collision,
    decode_layout,
    decode_tilesets,
    render_layout,
    tile_index,
    vdp_word,
)
from .maps import extract_maps
from .sprites import (
    FACINGS,
    FIELD_OBJECTS_JMP_TBL,
    FIELD_OBJECT_COUNT,
    MAP_PALETTE_LINE_ORDER,
    PAL_INIT_LINE_3,
    PAL_INIT_LINE_3_CRAM_LINE,
    SpriteCensus,
    facing_table_extents,
    party_sprites,
    scan_field_objects,
    step_timing_json,
)
# The sprite half of a pack knows only about sprites, so its layout, its sheet
# deduplication and its two index files live in `psiv_tools.sprites.emit`. The
# names are re-exported here because the pack's file layout is one interface.
# The four directory and file names are re-exported rather than used here:
# the pack's file layout is one interface and `psiv_tools.pack` is where a
# consumer looks it up.
from .sprites.emit import (
    NPC_SPRITES_DIRECTORY,
    NPC_SPRITES_NAME,
    PARTY_SPRITES_DIRECTORY,
    PARTY_SPRITES_NAME,
    SheetRegistry,
    emit_party,
    resolve_map_sprites,
)
from .symbols import ITEM_SYMBOLS

#: Bumped whenever a field in the emitted JSON changes meaning or disappears.
#: `psiv-data` refuses a pack whose version it does not know.
#:
#: 1 -- field sprites: `sprites/party.json`, `sprites/npcs.json`, their PNG
#: sheets, and a `sprite` reference on every map record's NPC entries.
PACK_FORMAT_VERSION = 1

MANIFEST_NAME = "manifest.json"
MAPS_DIRECTORY = "maps"

#: `GetChunkAndCollision`: `addi.w #$10,d6` before the shift down to a cell.
STANDING_CELL_Y_OFFSET = 1

#: `Map_Start_Facing_Dir` in the disassembly's constants.
FACING_NAMES: dict[int, str] = {0: "down", 4: "up", 8: "right", 0xC: "left"}

#: The transition tables in record order, and the tile collision that makes
#: `RunMapTransitions` walk each one.
TRANSITION_TABLES = (
    ("transitions", 1, "any_walkable_tile"),
    ("transitions_2", 2, "map_change_tile"),
)

MAP_CHANGE_COLLISION_TYPE = 0x1


class PackError(ValueError):
    pass


# ---------------------------------------------------------------------------
# XYRangeJmpTbl
# ---------------------------------------------------------------------------
@dataclass(frozen=True)
class Rect:
    """A half-open rectangle of collision cells."""

    x: int
    y: int
    width: int
    height: int

    def __post_init__(self) -> None:
        if self.width <= 0 or self.height <= 0:
            raise PackError(
                f"Rect at ({self.x}, {self.y}) is {self.width}x{self.height}; an "
                "empty trigger area is reported as no rectangle at all"
            )

    def cells(self) -> Iterator[tuple[int, int]]:
        for y in range(self.y, self.y + self.height):
            for x in range(self.x, self.x + self.width):
                yield x, y

    def to_json(self) -> dict[str, int]:
        return {"x": self.x, "y": self.y, "width": self.width, "height": self.height}


#: The eight `XYRangeJmpTbl` routines that build a box out of the record's
#: coordinate: `d6 = d0 + w`, `d7 = d1 + h`, then four comparisons that accept
#: `d0 <= player_x < d6` and `d1 <= player_y < d7`. Sizes are in pixels, as the
#: `addi.w` immediates spell them.
_BOX_RANGES: dict[int, tuple[str, int, int]] = {
    0x1: ("XYPlus40", 0x40, 0x40),
    0x2: ("XYPlus20", 0x20, 0x20),
    0x9: ("XPlus20_YPlus10", 0x20, 0x10),
    0xA: ("XPlus10_YPlus60", 0x10, 0x60),
    0xB: ("XPlus40_YPlus20", 0x40, 0x20),
    0xC: ("XPlus10_YPlus20", 0x10, 0x20),
    0xD: ("XPlus60_YPlus10", 0x60, 0x10),
    0xE: ("XPlus40_YPlus10", 0x40, 0x10),
}

#: The seven that do not: a point test, four half-plane tests, the one that
#: also gates on the player being past a fixed Y, and the one that never fires.
_OTHER_RANGES: dict[int, str] = {
    0x0: "Null",
    0x3: "XYExact",
    0x4: "XLower",
    0x5: "XHigher",
    0x6: "YLower",
    0x7: "YHigher",
    0x8: "XYLowerWithPlayerY",
}

XY_RANGE_NAMES: dict[int, str] = {
    **{value: name for value, (name, _, _) in _BOX_RANGES.items()},
    **_OTHER_RANGES,
}

#: `XYRange_XYLowerWithPlayerY` starts `cmpi.w #$2A0,d3 / bls` -- it does
#: nothing at all unless the player's Y is strictly greater than $2A0. On the
#: 16-pixel grid that means `curr_y_pos >= $2B0`, one cell further down again
#: once the standing-cell shift is applied.
_PLAYER_Y_FLOOR_PIXELS = 0x2A0
_PLAYER_Y_FLOOR_CELL = (
    _PLAYER_Y_FLOOR_PIXELS // COLLISION_CELL_PIXELS + 1 + STANDING_CELL_Y_OFFSET
)


def xy_range_name(value: int) -> str:
    if value not in XY_RANGE_NAMES:
        raise PackError(f"XYRange index {value} is outside the 15-entry jump table")
    return XY_RANGE_NAMES[value]


def warp_rect(
    range_id: int, x_byte: int, y_byte: int, width_cells: int, height_cells: int
) -> Rect | None:
    """The cells a transition record covers, clipped to its map.

    `x_byte`/`y_byte` are the record's two coordinate bytes as stored. The
    result is in collision-grid cells, so `y_byte` has already been shifted by
    `STANDING_CELL_Y_OFFSET`. `None` means the transition can never fire, which
    is `XYRange_Null` or a rectangle that lies entirely off the map.
    """
    if width_cells <= 0 or height_cells <= 0:
        raise PackError(f"Map is {width_cells}x{height_cells} cells")
    x = x_byte
    y = y_byte + STANDING_CELL_Y_OFFSET
    #: Half-open bounds before clipping. The open-ended routines are written
    #: with the map's own edge as the missing bound, which is only legitimate
    #: because the player can never stand outside the grid.
    if range_id in _BOX_RANGES:
        _, pixels_x, pixels_y = _BOX_RANGES[range_id]
        bounds = (x, y, x + pixels_x // COLLISION_CELL_PIXELS, y + pixels_y // COLLISION_CELL_PIXELS)
    elif range_id == 0x0:
        # `moveq #0,d7 / rts` -- never inside.
        return None
    elif range_id == 0x3:
        # Both coordinates compared with `bne`: one cell, exactly.
        bounds = (x, y, x + 1, y + 1)
    elif range_id == 0x4:
        # `cmp.w d2,d0 / bcs` fails only when the target X is below the
        # player's, so everything from the left edge up to and including x.
        bounds = (0, 0, x + 1, height_cells)
    elif range_id == 0x5:
        # `bhi` fails when the target X is above the player's: x and rightwards.
        bounds = (x, 0, width_cells, height_cells)
    elif range_id == 0x6:
        bounds = (0, 0, width_cells, y + 1)
    elif range_id == 0x7:
        bounds = (0, y, width_cells, height_cells)
    elif range_id == 0x8:
        # X and Y both "lower", plus the $2A0 floor on the player's own Y.
        bounds = (0, _PLAYER_Y_FLOOR_CELL, x + 1, y + 1)
    else:
        raise PackError(f"XYRange index {range_id} is outside the 15-entry jump table")

    x0, y0 = max(bounds[0], 0), max(bounds[1], 0)
    x1, y1 = min(bounds[2], width_cells), min(bounds[3], height_cells)
    if x1 <= x0 or y1 <= y0:
        return None
    return Rect(x0, y0, x1 - x0, y1 - y0)


# ---------------------------------------------------------------------------
# Record -> layout spec
# ---------------------------------------------------------------------------
def _offset(text: str) -> int:
    return int(text, 16)


def layout_spec(record: dict[str, Any]) -> MapLayoutSpec:
    """Assemble the `MapLayoutSpec` a decoded map record describes.

    `scroll.mode` is `loc_51AB2`'s first byte, the `$FFFFEC24` flag that
    `GetChunkAndCollision` reads to pick a plane, so it is the spec's
    `collision_plane` and not a rendering hint.
    """
    layout = record["layout"]
    if not layout["present"]:
        raise PackError(
            f"map 0x{record['id']:03X} ({record['symbol']}) has no layout section"
        )
    dimensions = record["dimensions"]
    return MapLayoutSpec.from_header(
        chunk_blobs=[_offset(p) for p in record["chunks"]["pointers"]],
        layout_fg=_offset(layout["fg_offset"]),
        layout_bg=_offset(layout["bg_offset"]),
        dimension_bytes=[
            dimensions["fg_row_size"],
            dimensions["fg_column_size"],
            dimensions["bg_row_size"],
            dimensions["bg_column_size"],
        ],
        collision_plane=record["scroll"]["mode"],
        tilesets=[
            (entry["vram_tile"], _offset(entry["source_offset"]))
            for entry in record["tilesets"]["entries"]
        ],
        palette=_offset(record["palette"]["pointer"]),
        label=record["symbol"],
    )


#: A chunk definition of nothing: pattern 0, no flips, no collision bit. This
#: is what `Chunk_Table` holds at a slot no map has decompressed into.
UNDEFINED_CHUNK = (0,) * CHUNK_WORDS


def _fill_undefined_chunks(
    chunks: ChunkTable, planes: Sequence[Layout]
) -> tuple[ChunkTable, dict[str, Any] | None]:
    """Give a layout somewhere to point when it names a chunk the map never loads.

    `SetupChunksFG` turns a layout byte into `Chunk_Table + (id << 5)` with no
    bound of any kind, and `Chunk_Table` is a 1024-definition region, so an
    out-of-range id reads a slot the map did not write -- stale data from the
    previously loaded map, or nothing at all on a cold boot. One cell of one
    retail map does this: `MapID_InnerSanctuary_B1`'s BG plane names chunk $FF
    at chunk (26, 28) while the map loads 128 chunks, surrounded by chunk 0.

    There is no faithful value to emit, because the cartridge's own is
    undefined. The pack substitutes the empty definition, which is what the
    region holds before any map has reached that far, and records the
    substitution so a reader is never told a guess is data. Layout cells are
    single bytes, so this can never grow the table past `MAX_CHUNKS`.
    """
    highest = max(max(layout.cells) for layout in planes)
    if highest < len(chunks):
        return chunks, None
    if highest >= MAX_CHUNKS:
        raise PackError(
            f"Layout names chunk 0x{highest:02X}, past the {MAX_CHUNKS} a byte cell "
            "can reach"
        )
    filled = ChunkTable(
        words=chunks.words
        + tuple(UNDEFINED_CHUNK for _ in range(len(chunks), highest + 1)),
        blobs=chunks.blobs,
    )
    cells = [
        {"plane": layout.plane, "x": index % layout.width_chunks,
         "y": index // layout.width_chunks, "chunk_id": f"0x{value:02X}"}
        for layout in planes
        for index, value in enumerate(layout.cells)
        if value >= len(chunks)
    ]
    return filled, {
        "kind": "undefined_chunk",
        "loaded_chunks": len(chunks),
        "chunk_ids": sorted({cell["chunk_id"] for cell in cells}),
        "cells": cells,
        "effect": (
            "the cartridge reads a Chunk_Table slot this map never wrote; the "
            "pack substitutes an empty chunk definition"
        ),
    }


def decode_layout_section(
    rom: bytes, spec: MapLayoutSpec
) -> tuple[MapLayout, list[dict[str, Any]]]:
    """`layouts.decode_map_layout`, collecting the planes' length anomalies."""
    chunks = decode_chunks(rom, spec.chunk_blobs)
    fg = decode_layout(
        rom, spec.layout_fg, spec.width_chunks_fg, spec.height_chunks_fg, PLANE_FG
    )
    bg = decode_layout(
        rom, spec.layout_bg, spec.width_chunks_bg, spec.height_chunks_bg, PLANE_BG
    )
    fg_anomaly, bg_anomaly = fg.anomaly, bg.anomaly
    chunks, chunk_anomaly = _fill_undefined_chunks(chunks, (fg, bg))
    decoded = MapLayout(
        spec=spec,
        chunks=chunks,
        fg=fg,
        bg=bg,
        collision=decode_collision(chunks, bg if spec.collision_plane else fg),
        patterns=decode_tilesets(rom, spec.tilesets),
    )
    return decoded, [a for a in (fg_anomaly, bg_anomaly, chunk_anomaly) if a is not None]


def unloaded_patterns(decoded) -> list[int]:
    """VRAM tiles a map's chunks name but its `loc_519D2` tilesets never fill.

    `compose_layout` reports these too, but only as a side effect of rendering.
    Walking the distinct chunks instead costs nothing and lets the manifest
    carry the answer for every map.

    Two different things land here and the manifest says so rather than calling
    both a defect. Some of these tiles are filled by the record's *sprite* art
    lists (`loc_51A1A`/`loc_51A5C`), which a `MapLayoutSpec` does not carry, so
    the chunk is drawing perfectly real art this pack has not staged. The rest
    are VRAM nothing in the record writes, which on the cartridge draws
    whatever the previous map left there.
    """
    missing: set[int] = set()
    for layout in (decoded.bg, decoded.fg):
        for chunk_id in layout.distinct_chunks:
            for word in decoded.chunks[chunk_id]:
                index = tile_index(vdp_word(word))
                if index not in decoded.patterns.loaded:
                    missing.add(index)
    return sorted(missing)


# ---------------------------------------------------------------------------
# Per-map JSON
# ---------------------------------------------------------------------------
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


def _npcs(
    record: dict[str, Any],
    sprites: Sequence[tuple[dict[str, Any] | None, str | None]] = (),
) -> list[dict[str, Any]]:
    """`LoadMapObjects` entries.

    Object coordinates are words scaled by 8 (`lsl.w #3,d0`), so 85 of the
    cartridge's 949 objects sit on a half-cell. Pixels are what the record
    says; the cell is the floor of that plus the standing-cell shift, i.e. the
    cell the object's collision would be read from.

    `sprites` is one entry per object, in order: either a reference into
    `sprites/npcs.json` or `None` for an object the cartridge draws nothing
    for, in which case `sprite_reason` says which routine decided that.
    """
    out = []
    for entry in record["objects"]["entries"]:
        sprite, reason = (
            sprites[entry["index"]] if entry["index"] < len(sprites) else (None, None)
        )
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
            "sprite": sprite,
            "sprite_reason": reason,
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
        if item_id is not None and not 0 <= item_id < len(ITEM_SYMBOLS):
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
            "item_symbol": ITEM_SYMBOLS[item_id] if item_id is not None else None,
            "meseta": entry["meseta"],
            "chest_flag": entry["chest_flag"],
        })
    return out


def map_json(
    record: dict[str, Any],
    decoded,
    png_path: str,
    sprites: Sequence[tuple[dict[str, Any] | None, str | None]] = (),
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """The runtime record for one map, and its warp anomalies."""
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
    flags = record["flags"]
    music = record["music"]
    return {
        "format_version": PACK_FORMAT_VERSION,
        "id": record["id"],
        "id_hex": record["id_hex"],
        "symbol": record["symbol"],
        "record_offset": record["rom_offset"],
        "png": png_path,
        "dimensions": {
            "width_cells": grid.width,
            "height_cells": grid.height,
            "width_chunks": layout.width_chunks,
            "height_chunks": layout.height_chunks,
            "width_pixels": layout.width_pixels,
            "height_pixels": layout.height_pixels,
            "cell_pixels": COLLISION_CELL_PIXELS,
        },
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
        "warps": warps,
        "npcs": _npcs(record, sprites),
        "treasure_chests": _treasure_chests(record),
    }, anomalies


# ---------------------------------------------------------------------------
# Emission
# ---------------------------------------------------------------------------
def _write_json(path: Path, payload: dict[str, Any]) -> str:
    """Write a pack JSON file and return its sha256.

    Sorted keys, fixed indent, no timestamps and nothing derived from the
    filesystem, so building the same pack twice produces the same bytes.
    """
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    data = text.encode("utf-8")
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def _map_stem(map_id: int, symbol: str) -> str:
    return f"{map_id:03X}_{symbol}"


def _selected(records: Sequence[dict[str, Any]], map_ids: Iterable[int] | None) -> list[dict[str, Any]]:
    if map_ids is None:
        return list(records)
    wanted = list(map_ids)
    by_id = {record["id"]: record for record in records}
    missing = [i for i in wanted if i not in by_id]
    if missing:
        raise PackError(
            f"map ids {[hex(i) for i in missing]} are outside the "
            f"{len(records)}-entry field map table"
        )
    return [by_id[i] for i in sorted(set(wanted))]


def build_pack(
    rom_bytes: bytes,
    out_dir: str | Path,
    map_ids: Iterable[int] | None = None,
) -> dict[str, Any]:
    """Emit the runtime pack for `rom_bytes` under `out_dir`, return the manifest.

    Every real map whose record carries a layout section is exported. The two
    world maps are not: `loc_539E2`/`loc_53A04` skip the layout pointers for
    `Field_Map_Index & $FFFE == 0` and stream a paged format this project has
    not decoded, so they are listed in `skipped` with that reason rather than
    guessed at. `map_ids` narrows the set for tests; a warp whose target falls
    outside the emitted set is normal and is listed in `unpacked_warp_targets`.

    The output holds Sega-derived pixels and is never committed.
    """
    directory = Path(out_dir)
    maps_directory = directory / MAPS_DIRECTORY
    maps_directory.mkdir(parents=True, exist_ok=True)

    extracted = extract_maps(rom_bytes)
    records = _selected(extracted["maps"], map_ids)

    routines = scan_field_objects(rom_bytes)
    extents = facing_table_extents(routines)
    party = party_sprites(rom_bytes, routines)
    npc_sheets = SheetRegistry(NPC_SPRITES_DIRECTORY)
    sprite_census = SpriteCensus()

    inventory: list[dict[str, Any]] = []
    skipped: list[dict[str, Any]] = []
    warp_anomalies: list[dict[str, Any]] = []
    odd_layouts: list[dict[str, Any]] = []
    unloaded: list[dict[str, Any]] = []
    artless_objects: list[dict[str, Any]] = []
    warp_targets: dict[int, dict[str, Any]] = {}
    warp_count = 0
    census: dict[str, dict[int, int]] = {
        key: {} for key in
        ("collision_types", "npc_facing_bytes", "warp_facing_bytes", "dialogue_trees",
         "sprite_palette_lines")
    }

    def count(key: str, value: int, by: int = 1) -> None:
        census[key][value] = census[key].get(value, 0) + by

    for record in records:
        if record["is_null"]:
            skipped.append({
                **_target(record),
                "reason": "PtrMap_Null placeholder; the table entry points at ErrorTrap",
            })
            continue
        if not record["layout"]["present"]:
            skipped.append({
                **_target(record),
                "reason": record["layout"]["reason"],
            })
            continue

        spec = layout_spec(record)
        decoded, layout_anomalies = decode_layout_section(rom_bytes, spec)
        stem = _map_stem(record["id"], record["symbol"])
        json_name = f"{MAPS_DIRECTORY}/{stem}.json"
        png_name = f"{MAPS_DIRECTORY}/{stem}.png"

        sprites, artless = resolve_map_sprites(
            rom_bytes, record, decoded, routines, extents, npc_sheets, sprite_census
        )
        payload, anomalies = map_json(record, decoded, png_name, sprites)
        json_sha = _write_json(maps_directory / f"{stem}.json", payload)

        image = render_layout(
            decoded.chunks,
            decoded.bg,
            decoded.patterns,
            chunk_palette(rom_bytes, spec.palette),
            overlay=decoded.fg,
        )
        (maps_directory / f"{stem}.png").write_bytes(image)

        for anomaly in anomalies:
            warp_anomalies.append({**_target(record), **anomaly})
        for anomaly in layout_anomalies:
            odd_layouts.append({**_target(record), **anomaly})
        missing = unloaded_patterns(decoded)
        if missing:
            unloaded.append({**_target(record), "patterns": missing})

        warp_count += len(payload["warps"])
        for value, cells in decoded.collision.histogram().items():
            count("collision_types", value, cells)
        count("dialogue_trees", payload["dialogue_tree"])
        for warp in payload["warps"]:
            warp_targets.setdefault(warp["target"]["id"], warp["target"])
            count("warp_facing_bytes", warp["facing"]["id"])
        for npc in payload["npcs"]:
            count("npc_facing_bytes", npc["facing"]["id"])
        for entry in artless:
            artless_objects.append({**_target(record), **entry})

        dimensions = payload["dimensions"]
        inventory.append({
            **_target(record),
            "json": json_name,
            "png": png_name,
            "json_sha256": json_sha,
            "png_sha256": hashlib.sha256(image).hexdigest(),
            "width_cells": dimensions["width_cells"],
            "height_cells": dimensions["height_cells"],
            "width_pixels": dimensions["width_pixels"],
            "height_pixels": dimensions["height_pixels"],
        })

    party_entries, party_bytes = emit_party(directory, party)
    npc_entries, npc_bytes = npc_sheets.emit(directory)
    npc_index = {
        "format_version": PACK_FORMAT_VERSION,
        "kind": "field_npcs",
        "sheet_count": len(npc_entries),
        "sheets": npc_entries,
    }
    npc_sha = _write_json(directory / NPC_SPRITES_NAME, npc_index)
    party_index = {
        "format_version": PACK_FORMAT_VERSION,
        "kind": "field_party",
        "sheet_count": len(party_entries),
        "sheets": party_entries,
    }
    party_sha = _write_json(directory / PARTY_SPRITES_NAME, party_index)

    placed = sum(entry["placements"] for entry in npc_entries)
    for entry in npc_entries:
        count("sprite_palette_lines", entry["palette"]["cram_line"], entry["placements"])

    packed = {entry["id"] for entry in inventory}
    manifest = {
        "format_version": PACK_FORMAT_VERSION,
        "generator": "psiv_tools.pack",
        "rom": {
            "sha256": hashlib.sha256(rom_bytes).hexdigest(),
            "size_bytes": len(rom_bytes),
        },
        "collision": {
            "cell_pixels": COLLISION_CELL_PIXELS,
            # `TileCollNormalPtrs` is a sixteen-entry jump table and everything
            # outside the blocking set routes to `TileColl_Empty`. The names
            # cover only the codes the disassembly annotates, so a consumer that
            # treats them as the valid set will reject legal data -- type $7 is
            # real and walkable. `census.collision_types` is the observed truth.
            "type_space": 1 << 4,
            "type_names": {
                f"0x{value:X}": name for value, name in sorted(COLLISION_TYPE_NAMES.items())
            },
            "named_types_are_not_the_valid_set": True,
            "blocking_types": sorted(BLOCKING_COLLISION_TYPES),
        },
        "warps": {
            "rect_units": "collision cells",
            "standing_cell_y_offset": STANDING_CELL_Y_OFFSET,
            "count": warp_count,
            "xy_ranges": {
                str(value): name for value, name in sorted(XY_RANGE_NAMES.items())
            },
            # A table-1 record firing on ordinary ground never wanted a
            # map-change cell, so those are counted rather than listed. A
            # table-2 record without one is a door that is not in the layout as
            # stored, which is worth every consumer's attention.
            "without_map_change_cell": {
                f"table_{table}": sum(1 for a in warp_anomalies if a["table"] == table)
                for _, table, _ in TRANSITION_TABLES
            },
            "doors_without_map_change_cell": [
                anomaly for anomaly in warp_anomalies if anomaly["table"] == 2
            ],
        },
        "sprites": {
            "party": PARTY_SPRITES_NAME,
            "party_sha256": party_sha,
            "party_sheet_count": len(party_entries),
            "npcs": NPC_SPRITES_NAME,
            "npcs_sha256": npc_sha,
            "npc_sheet_count": len(npc_entries),
            "npc_placements": placed,
            "artless_objects": len(artless_objects),
            # What the composed frames actually contain, as opposed to what the
            # six-byte piece record allows. Every one of these decided a line of
            # the compositor: the low-byte carry is why the pattern word is
            # summed the way Field_FillSpriteAttributes sums it, the V-flip
            # count is why the staged path honours both flip bits, and the
            # per-frame-duration sequences are why both sequence forms are
            # implemented instead of just the common one.
            "census": sprite_census.to_json(),
            "artless": artless_objects,
            "bytes": party_bytes + npc_bytes,
            "field_objects": {
                "table": f"0x{FIELD_OBJECTS_JMP_TBL:06X}",
                "count": FIELD_OBJECT_COUNT,
                "stride": 4,
            },
            # The whole palette answer in one place. A field sprite's colours
            # are decided by the byte its FieldObjectsJmpTbl routine stores at
            # $13, which Field_FillSpriteAttributes ORs into the high half of
            # every pattern word it writes; bits 6-5 of that byte are the CRAM
            # line. Lines 0, 1 and 3 come out of the map record's own palette
            # blob, in that order. Line 2 does not: loc_53F14 copies
            # Pal_Init_Line_3 over it for every map, which is why the party --
            # whose eleven routines all store $40 -- is the same colours
            # everywhere in the game.
            "palette": {
                "selector": "$13(a4), OR-ed into the pattern word's high byte",
                "cram_lines": {"0x00": 0, "0x20": 1, "0x40": 2, "0x60": 3},
                "map_palette_lines": list(MAP_PALETTE_LINE_ORDER),
                "fixed_line": PAL_INIT_LINE_3_CRAM_LINE,
                "fixed_line_source": "Pal_Init_Line_3",
                "fixed_line_rom_offset": f"0x{PAL_INIT_LINE_3:06X}",
                "party_line": PAL_INIT_LINE_3_CRAM_LINE,
            },
            "facings": {str(value): name for value, name in FACINGS},
            "walk": step_timing_json(rom_bytes, COLLISION_CELL_PIXELS),
            "note": (
                "Frame durations are game frames, one per Field_RunObjects call. "
                "idle_<dir> is frame 0 of the direction's sequence, which is where "
                "FieldObj_Move parks a stopped object; walk_<dir> is the whole cycle."
            ),
        },
        "map_count": len(inventory),
        "maps": inventory,
        "skipped": skipped,
        "unpacked_warp_targets": [
            target for map_id, target in sorted(warp_targets.items())
            if map_id not in packed
        ],
        # What the pack actually contains, as opposed to what the jump tables
        # allow. Every one of these is a place where the reachable set is
        # narrower than the legal set, and guessing either from the other has
        # already cost this project time: type $7 is walkable and real, dialogue
        # trees are numbered from 1, and one field object faces $10.
        "census": {
            key: {str(value): counted for value, counted in sorted(values.items())}
            for key, values in census.items()
        },
        "unloaded_patterns": {
            "note": (
                "VRAM tiles a map's chunks name that its loc_519D2 tileset list "
                "does not fill. Some are filled by the record's sprite art lists, "
                "which the pack does not stage; the rest are VRAM no part of the "
                "record writes. Either way the render leaves those pixels showing "
                "whatever is underneath."
            ),
            "maps": unloaded,
        },
        "layout_anomalies": odd_layouts,
    }
    _write_json(directory / MANIFEST_NAME, manifest)
    return manifest
