"""The runtime pack: the lean bundle `psiv-data` loads instead of `generated/`.

`generated/` is archaeology -- provenance-heavy, metadata-only for anything
pixel-shaped, and shaped for a human reading a diff. The Rust runtime wants the
opposite: one small JSON per map holding exactly the facts field mode needs,
one composed PNG per map, and a manifest that pins the ROM the whole set came
from. `docs/RUNTIME_DESIGN.md` is the decision record; this module is the
emitter, and the JSON it writes is a versioned interface, so field names here
are stable and `format_version` moves when they are not.

Nothing in here re-decodes anything, and since the file passed a thousand lines
it does not compose anything either. Four modules feed it, in dependency order,
and `psiv_tools.pack` re-exports all of them so that
`from psiv_tools.pack import ...` stays the one import a consumer needs:

* `psiv_tools.warps` -- `XYRangeJmpTbl` transcribed into trigger rectangles,
  the standing-cell Y shift, and `PackError`.
* `psiv_tools.pack_layouts` -- a map record's layout section to a decoded
  `MapLayout`, by whichever of the two paths its id selects.
* `psiv_tools.render` -- the priority overlay, the pixels the VDP draws above
  sprites.
* `psiv_tools.overworld` -- the paged layouts of MapID 0 and 1.

What is left here is the shape of the emitted JSON and the act of writing it:
per-map records, the manifest, and the census of what the cartridge's data
actually contains. The JSON is a versioned interface, so field names in this
module are stable and `format_version` moves when they are not.

Which transition table a warp came from is load-bearing and is emitted as
`table`:

* table 1 (`Map_Transition_Data_Addr`) is walked by `MapTransTile_Normal`,
  which `RunMapTransitions` selects for every standing collision type *except*
  map-change, solid, ice and shop. These fire on ordinary ground -- map edges,
  cave mouths, doormats.
* table 2 (`Map_Transition_Data_2_Addr`) is walked by `MapTransTile_MapChange`,
  reached only from standing collision type 1, and only when the previously
  occupied cell was not also type 1. These are doorways, and their rectangles
  must overlap a type-1 cell or they can never fire; the manifest reports any
  that do not.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Iterable, Sequence

from .gfx import palette_rgb
from .layouts import (
    BLOCKING_COLLISION_TYPES,
    COLLISION_CELL_PIXELS,
    COLLISION_TYPE_NAMES,
    chunk_palette,
    decode_map_palette,
    render_layout,
)
from .map_effects import extract_map_effects
from .map_patches import (
    ATLAS_TILE_PIXELS,
    atlas_json,
    resolve_palette_effects,
    index_writes,
    patch_atlas,
    resolve_map_effects,
)
from .maps import extract_maps
from .battle_art_pack import emit_battle_art
from .battle_pack import emit_battle
from .shop_pack import emit_shops
from .dialogue_pack import emit_dialogue
from .newgame import extract_new_game
from .npc_commands import extract_npc_commands
# The two world maps' layouts are not in their records at all -- they stream
# from paged tables -- so their decode lives in `psiv_tools.overworld`.
from .overworld import (
    OVERWORLD_MAP_IDS,
    Overworld,
    check_code_sites as overworld_code_sites,
    opening_patches,
)
# Some of these are re-exports rather than uses: `psiv_tools.pack` is the one
# import a consumer needs, so every name the pack's interface ever had is still
# reachable from here after the split.
from .pack_layouts import (  # noqa: F401
    UNDEFINED_CHUNK,
    decode_layout_section,
    decode_map_section,
    layout_spec,
    unloaded_patterns,
)
from .render import (  # noqa: F401
    OVERLAY_TRANSPARENT_INDICES,
    PLANE_BYTES,
    priority_overlay,
    priority_tiles,
)
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
    NpcMetadata,
    NPC_SPRITES_NAME,
    PARTY_SPRITES_DIRECTORY,
    PARTY_SPRITES_NAME,
    SheetRegistry,
    emit_party,
    field_objects_json,
    resolve_map_sprites,
)
from .symbols import ITEM_SYMBOLS
from .warps import (  # noqa: F401
    STANDING_CELL_Y_OFFSET,
    XY_RANGE_NAMES,
    PackError,
    Rect,
    warp_rect,
    xy_range_name,
)

#: Bumped whenever a field in the emitted JSON changes meaning or disappears.
#: `psiv-data` refuses a pack whose version it does not know.
#:
#: 1 -- field sprites: `sprites/party.json`, `sprites/npcs.json`, their PNG
#: sheets, and a `sprite` reference on every map record's NPC entries. The two
#: overworlds joined at the same version: their records use every field the
#: others do, and the one thing they add -- `layout_patches`, the event-gated
#: chunk writes their page loader performs -- is a key no other map carries and
#: no existing key changed meaning for. `interactable` on every NPC entry and
#: the `field_objects` table in `sprites/npcs.json` join on the same reasoning:
#: both are new keys, and nothing that was already emitted reads differently.
PACK_FORMAT_VERSION = 1

MANIFEST_NAME = "manifest.json"
MAPS_DIRECTORY = "maps"
GAME_START_NAME = "game_start.json"
#: A layout a MapDataManager routine swaps in, rendered like any other.
VARIANT_SUFFIX = "_variant"
#: A map's patched-chunk atlas, beside its base render.
PATCH_SUFFIX = "_patch"
NPC_COMMANDS_NAME = "npc_commands.json"

#: `Map_Start_Facing_Dir` in the disassembly's constants.
FACING_NAMES: dict[int, str] = {0: "down", 4: "up", 8: "right", 0xC: "left"}

#: The transition tables in record order, and the tile collision that makes
#: `RunMapTransitions` walk each one.
TRANSITION_TABLES = (
    ("transitions", 1, "any_walkable_tile"),
    ("transitions_2", 2, "map_change_tile"),
)

MAP_CHANGE_COLLISION_TYPE = 0x1


def layout_variants(rom: bytes, record: dict[str, Any], decoded, replacements,
                    directory: Path, stem: str) -> list[dict[str, Any]]:
    """Render the alternate layouts a `layout_replace` swaps in.

    `MapDataMan_GaruberkTowerPart2` and `...Part6` do not patch cells: they
    `KosDecomp` a whole different layout over a plane, which changes the
    collision grid as much as the picture. Emitting the blob pointer would make
    every consumer a Kosinski decoder, so the pack ships each alternate as a
    first-class layout -- decoded grid, collision rows, composed PNG and
    priority overlay -- and the runtime swaps whole variants at build time.

    One plane of each pair turns out to be the map's own base blob, so only the
    other actually changes; that is recorded rather than hidden.
    """
    from .layouts import Layout, MapLayout, decode_collision
    from .kosinski import decompress as kos_decompress

    if not replacements:
        return []
    spec = decoded.spec
    base = {"fg": spec.layout_fg, "bg": spec.layout_bg}
    sizes = {
        "fg": (spec.width_chunks_fg, spec.height_chunks_fg),
        "bg": (spec.width_chunks_bg, spec.height_chunks_bg),
    }
    planes = {"fg": decoded.fg, "bg": decoded.bg}
    changed = []
    for replacement in replacements:
        plane = replacement["plane"]
        source = int(replacement["source"], 16)
        width, height = sizes[plane]
        cells, _ = kos_decompress(rom, source)
        expected = width * height
        if len(cells) != expected:
            raise PackError(
                f"map 0x{record['id']:03X}: the {plane.upper()} replacement at "
                f"0x{source:06X} decompresses to {len(cells)} bytes, not the "
                f"{expected} a {width}x{height} grid needs"
            )
        planes[plane] = Layout(plane=plane, width_chunks=width, height_chunks=height,
                               cells=cells, blob=planes[plane].blob)
        changed.append({
            "plane": plane,
            "source": replacement["source"],
            "identical_to_base": source == base[plane],
        })

    variant = MapLayout(
        spec=spec, chunks=decoded.chunks, fg=planes["fg"], bg=planes["bg"],
        collision=decode_collision(decoded.chunks,
                                   planes["bg"] if spec.collision_plane else planes["fg"]),
        patterns=decoded.patterns,
    )
    palette = chunk_palette(rom, spec.palette)
    image = render_layout(variant.chunks, variant.bg, variant.patterns, palette,
                          overlay=variant.fg)
    overlay_image, priority_counts = priority_overlay(variant, palette)
    png_name = f"{MAPS_DIRECTORY}/{stem}{VARIANT_SUFFIX}.png"
    (directory / png_name).write_bytes(image)
    over_name = None
    if overlay_image is not None:
        over_name = f"{MAPS_DIRECTORY}/{stem}{VARIANT_SUFFIX}_over.png"
        (directory / over_name).write_bytes(overlay_image)

    grid = variant.collision
    return [{
        "id": 0,
        "planes": changed,
        "png": png_name,
        "png_over": over_name,
        "png_sha256": hashlib.sha256(image).hexdigest(),
        "png_over_sha256": (hashlib.sha256(overlay_image).hexdigest()
                            if overlay_image is not None else None),
        "priority_tiles": sum(priority_counts["tiles"].values()),
        "unloaded_patterns": unloaded_patterns(variant),
        "collision": {
            "plane": spec.collision_plane_name,
            "width_cells": grid.width,
            "height_cells": grid.height,
            "rows": [list(grid.types[y * grid.width:(y + 1) * grid.width])
                     for y in range(grid.height)],
            "sha256": hashlib.sha256(grid.types).hexdigest(),
        },
        "differs_from_base_cells": sum(
            1 for a, b in zip(decoded.collision.types, grid.types) if a != b
        ),
    }]


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
    blank = NpcMetadata(None, None, False)
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
    overworld: Overworld | None = None,
    png_over_path: str | None = None,
    effects: Sequence[dict[str, Any]] = (),
    variants: Sequence[dict[str, Any]] = (),
    patch_tiles: dict[str, Any] | None = None,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """The runtime record for one map, and its warp anomalies.

    `png_over_path` names the priority overlay when the map has one, and is
    `None` when every tile it draws is below sprites. The key is always
    present, so a consumer tests its value rather than its existence.

    `overworld` is set for the two paged maps and adds `layout_patches`: the
    event-gated chunk writes their page loader performs after copying a page,
    which is where five of their six doors come from. No other map carries the
    key, because no other map has a loader step that writes `Map_Layout`. The
    equivalent for interiors is `MapDataManager`, which this pack does not
    decode for anyone.
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
        "npcs": _npcs(record, sprites),
        "treasure_chests": _treasure_chests(record),
    }
    if overworld is not None:
        payload["layout_patches"] = [patch.to_json() for patch in overworld.patches]
    return payload, anomalies


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

    Every real map is exported, the two world maps included: their layouts come
    from the paged tables `psiv_tools.overworld` decodes rather than from their
    records, and everything else about them is an ordinary map record. Only the
    56 `PtrMap_Null` entries are skipped. `map_ids` narrows the set for tests; a
    warp whose target falls outside the emitted set is normal and is listed in
    `unpacked_warp_targets`, which for an unfiltered build is empty -- the map
    graph closes.

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

    # Every map's MapDataManager list, decoded once and handed out per map.
    effects = extract_map_effects(rom_bytes, extracted["maps"])
    replacements_by_map: dict[int, list[dict[str, Any]]] = {}
    for replacement in effects["replacements"]:
        replacements_by_map.setdefault(replacement["map"], []).append(replacement)

    inventory: list[dict[str, Any]] = []
    # Kept for the shop join: a counter names a shopkeeper by position.
    map_payloads: dict[int, dict[str, Any]] = {}
    overworlds: list[dict[str, Any]] = []
    variant_maps: list[dict[str, Any]] = []
    skipped: list[dict[str, Any]] = []
    warp_anomalies: list[dict[str, Any]] = []
    odd_layouts: list[dict[str, Any]] = []
    unloaded: list[dict[str, Any]] = []
    artless_objects: list[dict[str, Any]] = []
    patch_maps: list[dict[str, Any]] = []
    patch_totals: dict[str, int] = {}
    palette_totals: dict[str, int] = {}
    object_placements: dict[int, int] = {}
    mute_dialogue: list[dict[str, Any]] = []
    without_overlay: list[dict[str, Any]] = []
    priority_totals = {"tiles": 0, "opaque_pixels": 0}
    warp_targets: dict[int, dict[str, Any]] = {}
    warp_count = 0
    census: dict[str, dict[int, int]] = {
        key: {} for key in
        ("collision_types", "npc_facing_bytes", "warp_facing_bytes", "dialogue_trees",
         "sprite_palette_lines", "priority_tiles", "event_ids", "npc_interactable")
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
        decoded, layout_anomalies, overworld = decode_map_section(rom_bytes, record)
        spec = decoded.spec
        stem = _map_stem(record["id"], record["symbol"])
        json_name = f"{MAPS_DIRECTORY}/{stem}.json"
        png_name = f"{MAPS_DIRECTORY}/{stem}.png"

        sprites, artless = resolve_map_sprites(
            rom_bytes, record, decoded, routines, extents, npc_sheets, sprite_census
        )
        palette_48 = palette_rgb(decode_map_palette(rom_bytes, spec.palette))
        palette = palette_48[:32]
        image = render_layout(
            decoded.chunks, decoded.bg, decoded.patterns, palette, overlay=decoded.fg
        )
        overlay_image, priority_counts = priority_overlay(decoded, palette)
        png_over_name = (
            f"{MAPS_DIRECTORY}/{stem}_over.png" if overlay_image is not None else None
        )

        variants = layout_variants(
            rom_bytes, record, decoded, replacements_by_map.get(record["id"], []),
            directory, stem,
        )
        # `layout_write` entries arrive as chunk ids; a runtime needs the cells
        # they change and a picture of the chunk. Both are resolved here, where
        # the map's own chunk table and tileset are already decoded.
        map_effects, patched_chunks, patch_counts = resolve_map_effects(
            decoded, effects["per_map"].get(record["id"], [])
        )
        # A path that copies a CRAM line repaints the NPCs drawn on it, and
        # nothing else -- map tiles cannot select the line these three copies
        # write. The alternates register as ordinary sheets.
        for key, value in resolve_palette_effects(
            rom_bytes, record, decoded, map_effects, sprites, routines, extents,
            npc_sheets, sprite_census, palette_48,
            int(effects["census"]["jump_table"], 16),
        ).items():
            palette_totals[key] = palette_totals.get(key, 0) + value
        patch_tiles = None
        if patched_chunks:
            patch_base, patch_over, patch_entries = patch_atlas(
                decoded, patched_chunks, palette
            )
            index_writes(map_effects, patch_entries)
            patch_name = f"{MAPS_DIRECTORY}/{stem}{PATCH_SUFFIX}.png"
            patch_over_name = (
                f"{MAPS_DIRECTORY}/{stem}{PATCH_SUFFIX}_over.png"
                if patch_over is not None else None
            )
            (directory / patch_name).write_bytes(patch_base)
            if patch_over is not None:
                (directory / patch_over_name).write_bytes(patch_over)
            patch_tiles = atlas_json(
                patch_entries, patch_name, patch_base, patch_over_name, patch_over
            )
            patch_maps.append({
                **_target(record), "writes": patch_counts["layout_writes"],
                "chunks": len(patch_entries), "cells": patch_counts["cells"],
                "has_overlay": patch_over is not None,
            })
            for key, value in patch_counts.items():
                patch_totals[key] = patch_totals.get(key, 0) + value
        payload, anomalies = map_json(
            record, decoded, png_name, sprites, overworld, png_over_name,
            map_effects, variants, patch_tiles,
        )
        if variants:
            variant_maps.append({**_target(record), "variants": len(variants)})
        map_payloads[record["id"]] = payload
        json_sha = _write_json(maps_directory / f"{stem}.json", payload)
        (maps_directory / f"{stem}.png").write_bytes(image)
        if overlay_image is not None:
            (maps_directory / f"{stem}_over.png").write_bytes(overlay_image)
        for plane, placed in priority_counts["tiles"].items():
            if placed:
                count("priority_tiles", PLANE_BYTES[plane], placed)
        priority_totals["tiles"] += sum(priority_counts["tiles"].values())
        priority_totals["opaque_pixels"] += priority_counts["opaque_pixels"]
        if overlay_image is None:
            without_overlay.append({
                **_target(record),
                "priority_tiles": sum(priority_counts["tiles"].values()),
            })

        for anomaly in anomalies:
            entry = {**_target(record), **anomaly}
            if overworld is not None and anomaly["rect"] is not None:
                # An overworld door with no map-change cell is a door an event
                # opens, and the pack proves which event by applying each patch
                # to the collision plane rather than by quoting a comment.
                rect = Rect(**anomaly["rect"])
                entry["opened_by"] = [
                    {
                        "id": patch.event_flag,
                        "id_hex": f"0x{patch.event_flag:02X}",
                        "symbol": patch.event_symbol,
                        "routine": f"0x{patch.routine:06X}",
                    }
                    for patch in opening_patches(overworld, list(rect.cells()))
                ]
            warp_anomalies.append(entry)
        for anomaly in layout_anomalies:
            odd_layouts.append({**_target(record), **anomaly})
        if overworld is not None:
            overworlds.append(overworld.to_json())
        missing = unloaded_patterns(decoded)
        if missing:
            unloaded.append({**_target(record), "patterns": missing})

        warp_count += len(payload["warps"])
        for value, cells in decoded.collision.histogram().items():
            count("collision_types", value, cells)
        count("dialogue_trees", payload["dialogue_tree"])
        for event_id in payload["events"]:
            count("event_ids", event_id)
        for warp in payload["warps"]:
            warp_targets.setdefault(warp["target"]["id"], warp["target"])
            count("warp_facing_bytes", warp["facing"]["id"])
        for npc in payload["npcs"]:
            count("npc_facing_bytes", npc["facing"]["id"])
        for npc in payload["npcs"]:
            count("npc_interactable", int(npc["interactable"]))
            object_placements[npc["object_id"]] = (
                object_placements.get(npc["object_id"], 0) + 1
            )
            if npc["dialogue_id"] and not npc["interactable"]:
                mute_dialogue.append({
                    **_target(record), "npc_index": npc["index"],
                    "symbol": npc["symbol"], "dialogue_id": npc["dialogue_id"],
                })
        for entry in artless:
            artless_objects.append({**_target(record), **entry})

        dimensions = payload["dimensions"]
        inventory.append({
            **_target(record),
            "json": json_name,
            "png": png_name,
            "png_over": png_over_name,
            "json_sha256": json_sha,
            "png_sha256": hashlib.sha256(image).hexdigest(),
            "png_over_sha256": (
                hashlib.sha256(overlay_image).hexdigest()
                if overlay_image is not None else None
            ),
            "priority_tiles": sum(priority_counts["tiles"].values()),
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
        "field_objects": field_objects_json(routines, object_placements),
    }
    npc_sha = _write_json(directory / NPC_SPRITES_NAME, npc_index)
    party_index = {
        "format_version": PACK_FORMAT_VERSION,
        "kind": "field_party",
        "sheet_count": len(party_entries),
        "sheets": party_entries,
    }
    party_sha = _write_json(directory / PARTY_SPRITES_NAME, party_index)

    # Where a fresh playthrough begins. Its own file rather than a manifest
    # section: it carries the provenance of nine instruction sites, which is
    # more than a manifest entry should hold, and a runtime reads it once at
    # new-game and never again.
    game_start = {
        "format_version": PACK_FORMAT_VERSION,
        **extract_new_game(rom_bytes, extracted["maps"]),
    }
    game_start_sha = _write_json(directory / GAME_START_NAME, game_start)
    start = game_start["first_control"]

    # Enemies, formations, level progression and the ability records the
    # damage pipeline consumes. Its own directory because it is a different
    # half of the game from field mode, and `psiv_tools.battle_pack` is the
    # only thing that knows its shape.
    battle = emit_battle(rom_bytes, directory, PACK_FORMAT_VERSION)
    # Where the party spends its money. One file rather than a per-map
    # section: a counter is looked up by (map, position) once, on talking
    # to a shopkeeper, and the price and inn rules are global.
    shops = emit_shops(
        rom_bytes, map_payloads, directory, PACK_FORMAT_VERSION,
        complete=map_ids is None,
    )
    # The pictures that go with those records: 153 enemy bodies and 43
    # character poses, under battle/art/. A subtree of the battle fragment
    # rather than a sibling of it, because it is the same half of the game and
    # `psiv_tools.battle_art_pack` owns its shape.
    battle["art"] = emit_battle_art(rom_bytes, directory, PACK_FORMAT_VERSION)

    # The dialogue half: trees, font, portraits, window chrome. Emitted here
    # rather than by the CLI so that a programmatic build_pack() produces a
    # complete pack — its absence once shipped a game that couldn't talk.
    dialogue = emit_dialogue(rom_bytes, directory)

    # What a scene's MoveActorCommand byte means. Its own file for the same
    # reason: the provenance is bulky and it is read once, not per map.
    npc_commands = {
        "format_version": PACK_FORMAT_VERSION, **extract_npc_commands(rom_bytes)
    }
    npc_commands_sha = _write_json(directory / NPC_COMMANDS_NAME, npc_commands)

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
            # `render_flags` bit 3 decides both the talk probe and object
            # collision. Objects that carry a dialogue id and still have it
            # clear are the interesting class: the cartridge never lets the
            # player reach that dialogue, so a runtime that probes them anyway
            # answers with whatever its tree lookup falls back to.
            "interaction": {
                "render_flags_bit": 3,
                "types_interactable": sum(1 for r in routines if r.interactable),
                "types_not_interactable": sum(1 for r in routines if not r.interactable),
                "types_changing_at_runtime": [
                    r.symbol for r in routines if r.interactable_changes_at_runtime
                ],
                "dialogue_id_but_not_interactable": mute_dialogue,
            },
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
        # The headline of `game_start.json`, so the manifest alone answers
        # "where does a new game begin" without a second file read. The full
        # extraction, including every instruction site it was read from, is in
        # the file.
        "game_start": {
            "file": GAME_START_NAME,
            "sha256": game_start_sha,
            "map": start["map"],
            "x_cell": start["position"]["x_cell"],
            "y_cell": start["position"]["y_cell"],
            "facing": start["facing"],
            "party": [slot["symbol"] for slot in start["party"] if not slot["empty"]],
            "music": start["music"],
            # The whole seedable flag state, not just the base bank. Two scenes
            # run before control and only the base bank changes, but a runtime
            # has to seed all four, and the extended bank is preloaded from a
            # table rather than starting clear.
            "event_flags_set": [flag["id"] for flag in start["event_flags_set"]],
            "extended_event_flags_set": start["extended_event_flags_set"],
            "town_flags_set": start["town_flags_set"],
            "chest_flags_set": start["chest_flags_set"],
            "flag_banks": {
                bank: {"address": value["address"], "raw_hex": value["raw_hex"]}
                for bank, value in start["flag_banks"].items()
            },
            "scene_chain": [scene["event_hex"] for scene in start["scene_chain"]],
            "note": (
                "the first controllable moment, after the two scenes the title "
                "screen's event 0x9F chains through. The party the new-game "
                "initialiser writes (Chaz and Alys) does not survive them, and "
                "neither scene ends the flag state -- Event_PiataChazAlone sets "
                "the flag its own trigger tests, which is what stops the chain."
            ),
        },
        # Enemies, formations, levels and abilities, under battle/.
        "battle": battle,
        # Counters, inventories and inn rates, in shops.json.
        "shops": shops,
        "dialogue": dialogue,
        # The flag-gated patches a map's MapDataManager list applies when the
        # map is built. Per-map lists live on the map records; this is the
        # census over all of them.
        "map_effects": {
            **effects["census"],
            # A `layout_write` resolved: the collision cells it changes, and a
            # drawn tile of the chunk it stamps in. `collision_authoritative`
            # counts the writes that land on the plane GetChunkAndCollision
            # actually reads for that map; the rest change the picture only.
            "layout_write_resolution": {
                **patch_totals,
                "maps": patch_maps,
                "map_count": len(patch_maps),
                "atlas_tile_pixels": ATLAS_TILE_PIXELS,
                "cells_per_chunk": 4,
                "note": (
                    "Each layout_write carries `cells` (the 2x2 collision cells "
                    "the written chunk imposes) and `patch_tile` (an index into "
                    "that map's patch_tiles atlas)."
                ),
            },
            # A `MapDataManager` path that copies a CRAM line. All three retail
            # copies write line 3, which map chunks cannot select -- so they
            # repaint NPC sprites and leave the baked render untouched.
            "palette_copies": {
                **palette_totals,
                "cram_line": 3,
                "map_tiles_affected": 0,
                "note": (
                    "Chunk words carry one palette bit, so map tiles reach CRAM "
                    "lines 0 and 1. A copy to line 3 repaints the field objects "
                    "whose routine stores $60; the alternates are ordinary "
                    "sheets named per NPC on the path's deferred_effects."
                ),
            },
            "maps_with_layout_variants": variant_maps,
            "slice": 1,
            "kinds_emitted": [
                "object_despawn", "object_rewrite", "object_dialogue",
                "layout_write", "layout_replace",
            ],
            "kinds_deferred": (
                "palette, scroll, chunk-table rewrites, alternate colours and "
                "flag writes are later slices; a path that performs one carries "
                "it in `deferred` rather than dropping it"
            ),
        },
        # The NPC movement-command table, indexed by a scene op's command byte.
        "npc_commands": {
            "file": NPC_COMMANDS_NAME,
            "sha256": npc_commands_sha,
            "command_count": npc_commands["command_count"],
            "speed_count": npc_commands["dispatch"]["speed_count"],
            "frames_per_cell": [
                speed["frames_per_cell"][0] for speed in npc_commands["speeds"]
            ],
        },
        # The above-sprites layer. `png_over` is null for a map whose tiles all
        # draw below sprites, and those maps get no file at all rather than an
        # entirely transparent one.
        "overlays": {
            "file_suffix": "_over.png",
            "priority_bit": 15,
            "draw_order": [
                "high-priority sprites",
                "<id>_<symbol>_over.png",
                "field sprites",
                "<id>_<symbol>.png",
            ],
            "transparent_palette_indices": list(OVERLAY_TRANSPARENT_INDICES),
            "composited": "plane B first, plane A over it, colour 0 transparent",
            "maps_with_overlay": sum(1 for e in inventory if e["png_over"]),
            "maps_without_overlay": len(without_overlay),
            "priority_tiles": priority_totals["tiles"],
            "opaque_pixels": priority_totals["opaque_pixels"],
            "without_overlay": without_overlay,
            "note": (
                "Bit 15 survives ChunkTilesToBuffer's #$BFFF mask, so it is real "
                "VDP data: a map tile carrying it draws above an ordinary sprite. "
                "A field object whose flag byte has bit 4 set is a high-priority "
                "sprite (Field_FillSpriteAttributes' bset #7) and belongs above "
                "the overlay; the party never is."
            ),
        },
        "maps": inventory,
        # Everything a consumer needs to know that maps 0 and 1 were built by a
        # different loader path, and that their runtime records are otherwise
        # the same shape as everyone else's.
        "overworld": {
            "map_ids": list(OVERWORLD_MAP_IDS),
            "selector": "Field_Map_Index & $FFFE == 0",
            "note": (
                "loc_539E2/loc_53A04 read no layout pointer for these two; both "
                "planes stream 1KB pages of raw chunk ids from fixed tables into a "
                "four-page rolling window. Nothing on this path is compressed. "
                "layout_patches on the map record are the event-gated writes the "
                "page loader performs; MapDataManager effects are not decoded for "
                "any map, including MapDataMan_Dezolis, which rewrites Dezolis' "
                "chunk definitions once EventFlag_DarkForce2 is set."
            ),
            "code": overworld_code_sites(rom_bytes),
            "maps": overworlds,
        },
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
