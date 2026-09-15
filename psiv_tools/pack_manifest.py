"""Construction of the runtime pack manifest.

The per-map emitter remains in :mod:`psiv_tools.pack`; this module owns the
large, provenance-heavy summary so the emitter stays below the repository's
one-thousand-line refactor threshold.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

from . import pack as p


def write_manifest(
    directory: Path,
    rom_bytes: bytes,
    state: dict[str, Any],
) -> dict[str, Any]:
    """Write and return the manifest assembled from the packer's state."""
    artless_objects = state["artless_objects"]
    battle = state["battle"]
    census = state["census"]
    dialogue = state["dialogue"]
    effects = state["effects"]
    game_start_sha = state["game_start_sha"]
    inventory = state["inventory"]
    mute_dialogue = state["mute_dialogue"]
    npc_bytes = state["npc_bytes"]
    npc_commands = state["npc_commands"]
    npc_commands_sha = state["npc_commands_sha"]
    npc_entries = state["npc_entries"]
    npc_sha = state["npc_sha"]
    odd_layouts = state["odd_layouts"]
    overworlds = state["overworlds"]
    palette_totals = state["palette_totals"]
    party_bytes = state["party_bytes"]
    party_entries = state["party_entries"]
    party_sha = state["party_sha"]
    vehicle_bytes = state["vehicle_bytes"]
    vehicle_entries = state["vehicle_entries"]
    vehicle_sha = state["vehicle_sha"]
    patch_maps = state["patch_maps"]
    patch_totals = state["patch_totals"]
    placed = state["placed"]
    priority_totals = state["priority_totals"]
    routines = state["routines"]
    shops = state["shops"]
    sound = state["sound"]
    title = state["title"]
    travel = state["travel"]
    presentation = state["presentation"]
    skipped = state["skipped"]
    sprite_census = state["sprite_census"]
    start = state["start"]
    unloaded = state["unloaded"]
    variant_maps = state["variant_maps"]
    warp_anomalies = state["warp_anomalies"]
    warp_count = state["warp_count"]
    warp_targets = state["warp_targets"]
    without_overlay = state["without_overlay"]
    packed = state["packed"]
    manifest = {
        "format_version": p.PACK_FORMAT_VERSION,
        "generator": "psiv_tools.pack",
        "rom": {
            "sha256": hashlib.sha256(rom_bytes).hexdigest(),
            "size_bytes": len(rom_bytes),
        },
        "collision": {
            "cell_pixels": p.COLLISION_CELL_PIXELS,
            # `TileCollNormalPtrs` is a sixteen-entry jump table and everything
            # outside the blocking set routes to `TileColl_Empty`. The names
            # cover only the codes the disassembly annotates, so a consumer that
            # treats them as the valid set will reject legal data -- type $7 is
            # real and walkable. `census.collision_types` is the observed truth.
            "type_space": 1 << 4,
            "type_names": {
                f"0x{value:X}": name for value, name in sorted(p.COLLISION_TYPE_NAMES.items())
            },
            "named_types_are_not_the_valid_set": True,
            "blocking_types": sorted(p.BLOCKING_COLLISION_TYPES),
        },
        "warps": {
            "rect_units": "collision cells",
            "standing_cell_y_offset": p.STANDING_CELL_Y_OFFSET,
            "count": warp_count,
            "xy_ranges": {
                str(value): name for value, name in sorted(p.XY_RANGE_NAMES.items())
            },
            # A table-1 record firing on ordinary ground never wanted a
            # map-change cell, so those are counted rather than listed. A
            # table-2 record without one is a door that is not in the layout as
            # stored, which is worth every consumer's attention.
            "without_map_change_cell": {
                f"table_{table}": sum(1 for a in warp_anomalies if a["table"] == table)
                for _, table, _ in p.TRANSITION_TABLES
            },
            "doors_without_map_change_cell": [
                anomaly for anomaly in warp_anomalies if anomaly["table"] == 2
            ],
        },
        "sprites": {
            "party": p.PARTY_SPRITES_NAME,
            "party_sha256": party_sha,
            "party_sheet_count": len(party_entries),
            "npcs": p.NPC_SPRITES_NAME,
            "npcs_sha256": npc_sha,
            "npc_sheet_count": len(npc_entries),
            "vehicles": p.VEHICLE_SPRITES_NAME,
            "vehicles_sha256": vehicle_sha,
            "vehicle_sheet_count": len(vehicle_entries),
            "vehicle_palette_source": (
                "each selected map palette, CRAM line 3; base is first selected map"
            ),
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
            "bytes": party_bytes + npc_bytes + vehicle_bytes,
            "field_objects": {
                "table": f"0x{p.FIELD_OBJECTS_JMP_TBL:06X}",
                "count": p.FIELD_OBJECT_COUNT,
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
                "map_palette_lines": list(p.MAP_PALETTE_LINE_ORDER),
                "fixed_line": p.PAL_INIT_LINE_3_CRAM_LINE,
                "fixed_line_source": "Pal_Init_Line_3",
                "fixed_line_rom_offset": f"0x{p.PAL_INIT_LINE_3:06X}",
                "party_line": p.PAL_INIT_LINE_3_CRAM_LINE,
            },
            "facings": {str(value): name for value, name in p.FACINGS},
            "walk": p.step_timing_json(rom_bytes, p.COLLISION_CELL_PIXELS),
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
            "file": p.GAME_START_NAME,
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
        "travel": travel,
        "sound": sound,
        "dialogue": dialogue,
        "title": title,
        # The scene-panel census: full provenance lives in
        # presentation/panels.json; the manifest records what was emitted.
        "presentation": {
            "record_table": presentation["record_table"],
            "record_size": presentation["record_size"],
            "panels": len(presentation["panels"]),
            "palettes": len(presentation["palettes"]),
            "load_art": len(presentation["load_art"]),
            "temporary_objects": len(presentation["temporary_objects"]),
            "portraits": len(presentation["portraits"]),
            "coverage": presentation["coverage"],
        },
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
                "atlas_tile_pixels": p.ATLAS_TILE_PIXELS,
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
            "file": p.NPC_COMMANDS_NAME,
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
            "transparent_palette_indices": list(p.OVERLAY_TRANSPARENT_INDICES),
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
            "map_ids": list(p.OVERWORLD_MAP_IDS),
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
            "code": p.overworld_code_sites(rom_bytes),
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
    p._write_json(directory / p.MANIFEST_NAME, manifest)
    return manifest
