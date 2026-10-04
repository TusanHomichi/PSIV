"""The runtime pack: the lean bundle `psiv-data` loads instead of `generated/`.

`generated/` is archaeology -- provenance-heavy, metadata-only for anything
pixel-shaped, and shaped for a human reading a diff. The Rust runtime wants the
opposite: one small JSON per map holding exactly the facts field mode needs,
one composed PNG per map, and a manifest that pins the ROM the whole set came
from. `docs/RUNTIME_DESIGN.md` is the decision record; this module is the
emitter, and the JSON it writes is a versioned interface, so field names here
are stable and `format_version` moves when they are not.

Nothing in here re-decodes anything, and the per-map emitter does not compose
the manifest either. Four modules feed it, in dependency order,
and `psiv_tools.pack` re-exports all of them so that
`from psiv_tools.pack import ...` stays the one import a consumer needs:

* `psiv_tools.warps` -- `XYRangeJmpTbl` transcribed into trigger rectangles,
  the standing-cell Y shift, and `PackError`.
* `psiv_tools.pack_layouts` -- a map record's layout section to a decoded
  `MapLayout`, by whichever of the two paths its id selects.
* `psiv_tools.render` -- the priority overlay, the pixels the VDP draws above
  sprites.
* `psiv_tools.overworld` -- the paged layouts of MapID 0 and 1.

What is left here is the shape of the emitted per-map JSON and the act of
writing the pack. The manifest summary lives in `psiv_tools.pack_manifest`,
while the census of what the cartridge's data actually contains remains part
of that same emission workflow. The JSON is a versioned interface, so field
names in this module are stable and `format_version` moves when they are not.

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
    index_overworld_patches,
    patch_atlas,
    resolve_map_effects,
    resolve_overworld_patches,
    scene_patch_chunks,
)
from .maps import extract_maps
from .maps.records import INTERACTION_EVENT_INDEXES
from .battle_art_pack import emit_battle_art
from .battle_pack import emit_battle
from .shop_pack import emit_shops
from .dialogue_pack import emit_dialogue
from .presentation_pack import emit_presentation
from .title_pack import emit_title
from .newgame import extract_new_game
from .travel import emit_travel
from .ship_menu_pack import emit_ship_menu
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
    VEHICLE_SPRITES_NAME,
    VEHICLE_SPRITES_DIRECTORY,
    SheetRegistry,
    emit_party,
    field_objects_json,
    resolve_map_sprites,
)
from .symbols import ITEM_SYMBOLS
from .chest_sprites import bind_chest_sprites
from .map_updates import extract_map_updates
from .map_updates_render import bind_updates
from .pack_format import PACK_FORMAT_VERSION  # noqa: F401
from .pack_map_json import (  # noqa: F401
    FACING_NAMES,
    MAP_CHANGE_COLLISION_TYPE,
    TRANSITION_TABLES,
    _facing,
    _interaction_areas,
    _npcs,
    _target,
    _treasure_chests,
    _warps,
    map_json,
)
from .warps import (  # noqa: F401
    STANDING_CELL_Y_OFFSET,
    XY_RANGE_NAMES,
    PackError,
    Rect,
    interaction_rect,
    warp_rect,
    xy_range_name,
)
from .sound import SoundError, emit_sound  # noqa: F401
from .vehicle_pack import emit_vehicle_index


MANIFEST_NAME = "manifest.json"
MAPS_DIRECTORY = "maps"
GAME_START_NAME = "game_start.json"
#: A layout a MapDataManager routine swaps in, rendered like any other.
VARIANT_SUFFIX = "_variant"
#: A map's patched-chunk atlas, beside its base render.
PATCH_SUFFIX = "_patch"
NPC_COMMANDS_NAME = "npc_commands.json"





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
        "vehicle_battle": {
            "plane": variant.collision_layout.plane,
            "width_chunks": variant.collision_layout.width_chunks,
            "height_chunks": variant.collision_layout.height_chunks,
            "rows": [
                list(
                    variant.collision_layout.cells[
                        y * variant.collision_layout.width_chunks:
                        (y + 1) * variant.collision_layout.width_chunks
                    ]
                )
                for y in range(variant.collision_layout.height_chunks)
            ],
        },
        "differs_from_base_cells": sum(
            1 for a, b in zip(decoded.collision.types, grid.types) if a != b
        ),
    }]


# ---------------------------------------------------------------------------
# Per-map JSON
# ---------------------------------------------------------------------------


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

    # Sound is pack-wide.  Keep the map music ids as usage provenance even for
    # a filtered test pack; the raw sound section always contains the complete
    # driver id space, not only the maps selected for this build.
    sound = emit_sound(
        rom_bytes,
        directory,
        map_music_ids=[
            record["music"]["id"]
            for record in extracted["maps"]
            if record.get("music", {}).get("id") is not None
        ],
    )

    routines = scan_field_objects(rom_bytes)
    extents = facing_table_extents(routines)
    party = party_sprites(rom_bytes, routines)
    vehicle_palette: Sequence[tuple[int, int, int]] | None = None
    vehicle_palette_variants: dict[
        tuple[tuple[int, int, int], ...], list[int]
    ] = {}
    npc_sheets = SheetRegistry(NPC_SPRITES_DIRECTORY)
    sprite_census = SpriteCensus()

    # Every map's MapDataManager list, decoded once and handed out per map.
    effects = extract_map_effects(rom_bytes, extracted["maps"])
    updates = extract_map_updates(rom_bytes, extracted["maps"])
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
    scene_patch_maps: list[dict[str, Any]] = []
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
        vehicle_key = tuple(palette_48[32:48])
        vehicle_palette_variants.setdefault(vehicle_key, []).append(record["id"])
        if vehicle_palette is None:
            # Vehicle routines write $60 to the sprite selector, so their
            # colour source is map CRAM line 3. Keep the first selected map as
            # the backwards-compatible base and emit the other CRAM variants
            # below instead of silently painting every planet with it.
            vehicle_palette = palette_48
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
        overworld_patches, composite_pairs = resolve_overworld_patches(decoded, overworld)
        if composite_pairs and overlay_image is None:
            raise PackError(
                f"map 0x{record['id']:03X}: overworld hook needs a base priority overlay"
            )
        scene_chunks = scene_patch_chunks(rom_bytes, record)
        if scene_chunks:
            scene_patch_maps.append({**_target(record), "chunks": scene_chunks})
        patched_chunks = sorted(set(patched_chunks) | set(scene_chunks))
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
        if patched_chunks or composite_pairs:
            patch_base, patch_over, patch_entries = patch_atlas(
                decoded, patched_chunks, palette, composite_pairs
            )
            index_writes(map_effects, patch_entries)
            index_overworld_patches(overworld_patches, patch_entries)
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
            if patched_chunks:
                patch_maps.append({
                    **_target(record), "writes": patch_counts["layout_writes"],
                    "chunks": len(patched_chunks), "cells": patch_counts["cells"],
                    "has_overlay": patch_over is not None,
                })
            for key, value in patch_counts.items():
                patch_totals[key] = patch_totals.get(key, 0) + value
        payload, anomalies = map_json(
            record, decoded, png_name, sprites, overworld, png_over_name,
            map_effects, variants, patch_tiles, overworld_patches,
        )
        bind_updates(rom_bytes, record, payload, directory, image, overlay_image, updates)
        bind_chest_sprites(rom_bytes, payload["treasure_chests"], palette_48,
                          record["general_var"], npc_sheets)
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
    if vehicle_palette is None:
        raise PackError("cannot extract vehicle sheets from an empty map selection")
    vehicle_index, vehicle_bytes, vehicle_entries = emit_vehicle_index(
        rom_bytes,
        directory,
        vehicle_palette,
        vehicle_palette_variants,
        PACK_FORMAT_VERSION,
    )
    vehicle_sha = _write_json(directory / VEHICLE_SPRITES_NAME, vehicle_index)
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
    travel = emit_travel(rom_bytes, directory)
    ship_menu = emit_ship_menu(rom_bytes, directory)
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

    # The retail front door: Sega logo, title mappings, and the composed
    # 320x224 background. Pixels stay in the ignored runtime pack; the
    # manifest carries the small index and title/layout.json carries the
    # extraction provenance.
    title = emit_title(rom_bytes, directory)

    # The cutscene renderer's Panel_Create records, opening background, and
    # palette requests. Same contract as title: pixels stay in the ignored
    # pack, presentation/panels.json carries the extraction provenance.
    presentation = emit_presentation(rom_bytes, directory)

    # What a scene's MoveActorCommand byte means. Its own file for the same
    # reason: the provenance is bulky and it is read once, not per map.
    npc_commands = {
        "format_version": PACK_FORMAT_VERSION, **extract_npc_commands(rom_bytes)
    }
    npc_commands_sha = _write_json(directory / NPC_COMMANDS_NAME, npc_commands)

    placed = sum(entry["placements"] for entry in npc_entries)
    for entry in npc_entries:
        count("sprite_palette_lines", entry["palette"]["cram_line"], entry["placements"])

    from .pack_manifest import write_manifest

    packed = {entry["id"] for entry in inventory}
    return write_manifest(directory, rom_bytes, locals())
