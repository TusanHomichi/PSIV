"""From a decoded map record to a decoded `MapLayout`.

`psiv_tools.maps` walks the 417-entry pointer table and hands back a record;
`psiv_tools.layouts` decodes chunk definitions, plane layouts and collision
given explicit offsets. This module is the joint between them: it reads a
record's layout section into a `MapLayoutSpec`, picks the right decode path for
the map id, and collects the handful of places where retail data disagrees with
itself so the pack can report them instead of papering over them.

Two decode paths exist and the map id chooses. 359 records store two Kosinski
layout pointers. The two world maps store neither -- `loc_539E2`/`loc_53A04`
branch out on `Field_Map_Index & $FFFE` before reading a pointer and stream
their planes from paged tables instead -- so they go through
`psiv_tools.overworld`. `decode_map_section` is the one entry point that knows
both exist; everything downstream sees a `MapLayout` and cannot tell which path
produced it.
"""

from __future__ import annotations

from typing import Any, Sequence

from .layouts import (
    CHUNK_WORDS,
    MAX_CHUNKS,
    PLANE_BG,
    PLANE_FG,
    ChunkTable,
    Layout,
    MapLayout,
    MapLayoutSpec,
    decode_chunks,
    decode_collision,
    decode_layout,
    decode_tilesets,
    tile_index,
    vdp_word,
)
from .overworld import Overworld, decode_overworld, is_overworld
from .warps import PackError


def _offset(text: str) -> int:
    return int(text, 16)


def layout_spec(record: dict[str, Any]) -> MapLayoutSpec:
    """Assemble the `MapLayoutSpec` a decoded map record describes.

    `scroll.mode` is `loc_51AB2`'s first byte, the `$FFFFEC24` flag that
    `GetChunkAndCollision` reads to pick a plane, so it is the spec's
    `collision_plane` and not a rendering hint.

    This is the pointer-based path only. The two world maps have no layout
    section to read and raise here; `decode_map_section` is what knows to send
    them elsewhere.
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


def decode_map_section(
    rom: bytes, record: dict[str, Any]
) -> tuple[MapLayout, list[dict[str, Any]], Overworld | None]:
    """Decode one map's layout by whichever of the two paths its id selects.

    359 records point at two Kosinski blobs. The two world maps point at
    nothing: `loc_539E2`/`loc_53A04` branch out on `Field_Map_Index & $FFFE`
    before reading a pointer, and their planes stream from paged tables. Both
    paths end in the same `MapLayout`, so everything downstream of here -- the
    per-map JSON, the collision grid, the composed render -- is written once.
    """
    if record["layout"]["present"]:
        decoded, anomalies = decode_layout_section(rom, layout_spec(record))
        return decoded, anomalies, None
    if not is_overworld(record["id"]):
        raise PackError(
            f"map 0x{record['id']:03X} ({record['symbol']}) has no layout section and "
            "is not one of the paged world maps"
        )
    overworld = decode_overworld(rom, record["id"], record, with_tiles=True)
    return overworld.layout, list(overworld.anomalies), overworld


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
