"""Making a `layout_write` something a runtime can actually apply.

`psiv_tools.map_effects` decodes what a `MapDataManager` routine writes into
`Map_Layout`, and it stops where a decoder honestly can: at a chunk id and a
chunk coordinate. That is the cartridge's own vocabulary, and it is useless to
a consumer, because the pack ships neither per-chunk collision definitions nor
any way to draw a chunk that is not already in the baked map PNG. A runtime
handed `{plane: "bg", chunk_x: 22, chunk_y: 11, chunk_id: 90}` can do nothing
with it.

This module resolves both halves of that at extraction time.

## Collision

A chunk covers exactly 2x2 collision cells, which is why the emission's
`cell_x` is `chunk_x * 2`. `collision_at` derives one cell from a layout and a
chunk table; the same arithmetic applied to a chunk definition on its own gives
the four cells that definition *imposes* wherever it is written, and that is
what a patch needs.

## Which plane collision reads

Not an assumption, and not always FG. `GetChunkAndCollision` selects
`Map_Layout_BG` when `$FFFFEC24` is non-zero and `Map_Layout_FG` when it is
zero, and that byte is `loc_51AB2`'s first byte -- the map record's own scroll
mode, already carried as `MapLayoutSpec.collision_plane`. A write to the plane
collision does not read changes the picture and nothing else, so each resolved
write says whether it is `collision_authoritative` rather than leaving a
consumer to infer it.

Zema is the case that matters and it lands the right way round: its scroll mode
is 1, so collision reads BG, and `MapDataMan_ChkZemaNormal` writes BG
(`moveq #1,d3` before `GetMapLayoutOffset`, which is the routine's plane
selector). The doors it opens are therefore real collision changes.

## The Zema "postincrement bulk copy"

`map_effects` marks Zema's path `deferred: ["postincrement bulk copy"]`, and
the question that blocks everything else is whether that copy installs chunk
89/90's *definitions* -- in which case resolving those ids against the map's
base chunk table would be reading the wrong bytes.

It does not. The copy is:

    lea     (ZemaNormalPalettes).l, a0     ; 0x133E20
    lea     (Palette_Table_Buffer+$60).w, a1
    move.w  #7, d7
    -   move.l  (a0)+, (a1)+
        dbf     d7, -

Eight longs -- sixteen words, one CRAM line -- into palette line 3. It is a
**palette swap**, not a chunk-table write: Zema recolours once Igglanova is
dead. `Chunk_Table` is untouched, chunks 89 and 90 are in the map's own chunk
blobs (Zema loads 182), and resolving against the base table is correct.

That swap is a real visual effect this module does not emit: the baked map PNG
uses the map's stored palette, so a Zema after Igglanova renders in the wrong
colours until somebody emits the alternate line. It is recorded per write as
`deferred_effects` so the gap is visible rather than silently wrong.

## Visual

Each written chunk is composed into a 32x32 tile using the map's own tileset
and palette -- `compose_layout`'s inner loop, restricted to one chunk -- and
the distinct chunks of a map are packed into one atlas row. A write then names
an atlas index, and a renderer blits that tile over the baked PNG at
`chunk_x * 32, chunk_y * 32`.

Priority gets the same treatment as the map render: a raw chunk's
above-sprites tiles go into a second atlas with the same indices, emitted
when some raw patched chunk has priority pixels.

Paged overworld hooks are a separate source path. Their atlas entries draw
the final FG-over-BG pair, because either plane may change and the other must
remain visible through transparent tiles. A fully transparent priority tile
is still emitted so blitting can clear stale above-sprites pixels.
"""

from __future__ import annotations

import hashlib
import re
from typing import Any, Sequence

from . import png
from .gfx import decode_palette, palette_rgb
from .layouts import (
    CHUNK_PIXELS_X,
    CHUNK_PIXELS_Y,
    CHUNK_TILES_X,
    COLLISION_CELL_TILES,
    TILE_PIXELS,
    ChunkTable,
    RGB,
    _COLLISION_WORD_OFFSETS,
    collision_flag,
    collision_type_name,
    h_flip,
    palette_line,
    priority,
    tile_index,
    v_flip,
    vdp_word,
)
from .render import OVERLAY_TRANSPARENT_INDICES

#: The map palette blob holds CRAM lines 0, 1 and 3, in that order.
MAP_PALETTE_LINE_ORDER = (0, 1, 3)

#: A chunk is 2x2 collision cells, which is why the emission's `cell_x` is
#: `chunk_x * 2`.
CHUNK_CELLS_X = CHUNK_TILES_X // COLLISION_CELL_TILES
CHUNK_CELLS_Y = CHUNK_CELLS_X

#: The atlas is one row of 32x32 tiles.
ATLAS_TILE_PIXELS = CHUNK_PIXELS_X

class MapPatchError(ValueError):
    pass


def scene_patch_chunks(rom: bytes, record: dict[str, Any]) -> list[int]:
    """Original door-animation chunks needed by this map's interaction events.

    These are live scene writes, independent of MapDataManager's load-time
    paths. Keep them in the same atlas so each write carries pixels and its
    four collision values. No animation or doorway is inferred from art.
    """
    events = {
        area["parameter"]
        for area in record.get("interaction_areas", {}).get("entries", ())
        if area["interaction_type"] == 2
    }
    chunks: set[int] = set()
    if 8 in events:
        # Event_BioPlantDoorOpening: delay, upper chunk, lower chunk; FF.
        table = rom[0x6B9A4:0x6B9AE]
        if len(table) != 10 or table[-1] != 0xFF:
            raise MapPatchError("BioPlant door animation table is incomplete")
        chunks.update(table[i] for i in (1, 2, 4, 5, 7, 8))
    if 0x13 in events:
        # Opening and closing tables: CPU delay, chunk; FF. $4F is the
        # original closed chunk checked by Event_ElevatorDoorOpening.
        chunks.add(0x4F)
        for offset in (0x6C334, 0x6C46E):
            table = rom[offset:offset + 9]
            if len(table) != 9 or table[-1] != 0xFF:
                raise MapPatchError("elevator door animation table is incomplete")
            chunks.update(table[1:8:2])
    return sorted(chunks)


# ---------------------------------------------------------------------------
# Collision
# ---------------------------------------------------------------------------
def chunk_collision_cells(words: Sequence[int]) -> tuple[tuple[int, int, int], ...]:
    """The 2x2 `(dx, dy, collision)` one chunk definition imposes.

    `collision_at` asks a layout which chunk sits at a cell and then does this;
    a patch already knows the chunk, so only the second half applies. The
    quadrant arithmetic is the asm's: `andi.w #2,d0` steps two words across and
    `andi.w #2,d1` steps eight words down.
    """
    if len(words) != CHUNK_TILES_X * CHUNK_TILES_X:
        raise MapPatchError(
            f"a chunk definition is {CHUNK_TILES_X * CHUNK_TILES_X} words, got {len(words)}"
        )
    cells = []
    for dy in range(CHUNK_CELLS_Y):
        for dx in range(CHUNK_CELLS_X):
            base = dy * (CHUNK_TILES_X * COLLISION_CELL_TILES) + dx * COLLISION_CELL_TILES
            value = 0
            for bit, step in enumerate(_COLLISION_WORD_OFFSETS):
                if collision_flag(words[base + step]):
                    value |= 1 << bit
            cells.append((dx, dy, value))
    return tuple(cells)


def resolve_write_cells(
    chunks: ChunkTable, chunk_id: int, cell_x: int, cell_y: int
) -> list[dict[str, Any]]:
    """One `layout_write` as the collision cells it changes.

    Raises rather than guessing when the id is outside the map's chunk table:
    a patch that writes a chunk the map never loaded is the cartridge reading
    `Chunk_Table` past what it filled, and that is a finding, not a cell.
    """
    if not 0 <= chunk_id < len(chunks.words):
        raise MapPatchError(
            f"layout_write names chunk 0x{chunk_id:02X}, outside the "
            f"{len(chunks.words)} this map loads"
        )
    return [
        {"x": cell_x + dx, "y": cell_y + dy, "collision": value}
        for dx, dy, value in chunk_collision_cells(chunks[chunk_id])
    ]


# ---------------------------------------------------------------------------
# Visual
# ---------------------------------------------------------------------------
def compose_chunk(
    chunks: ChunkTable, patterns, chunk_id: int, priority_only: bool = False
) -> tuple[bytearray, int]:
    """One chunk definition drawn into a 32x32 index buffer.

    `compose_layout`'s inner loop with the layout walk removed. `priority_only`
    keeps just the tiles the VDP draws above sprites, which is what the overlay
    atlas holds; it returns how many it placed so an all-zero overlay can be
    skipped.
    """
    pixels = bytearray(CHUNK_PIXELS_X * CHUNK_PIXELS_Y)
    placed = 0
    for index, raw in enumerate(chunks[chunk_id]):
        word = vdp_word(raw)
        if priority_only and not priority(raw):
            continue
        pattern = tile_index(word)
        if pattern not in patterns.loaded:
            # The base render draws nothing here either; `unloaded_patterns`
            # already reports it for the map as a whole.
            continue
        placed += 1
        tile = patterns.tile(pattern)
        shift = palette_line(word) * 16
        flip_x, flip_y = h_flip(word), v_flip(word)
        ox = (index % CHUNK_TILES_X) * TILE_PIXELS
        oy = (index // CHUNK_TILES_X) * TILE_PIXELS
        for y in range(TILE_PIXELS):
            source_y = TILE_PIXELS - 1 - y if flip_y else y
            row = tile[source_y * TILE_PIXELS:(source_y + 1) * TILE_PIXELS]
            if flip_x:
                row = row[::-1]
            start = (oy + y) * CHUNK_PIXELS_X + ox
            for x in range(TILE_PIXELS):
                value = row[x]
                if value or not priority_only:
                    pixels[start + x] = value + shift if value else 0
    return pixels, placed


def compose_overworld_chunk(decoded, fg_id: int, bg_id: int) -> tuple[bytearray, bytearray, int]:
    """Draw a page-hook result, BG then transparent FG, in both VDP layers."""
    bg, _ = compose_chunk(decoded.chunks, decoded.patterns, bg_id)
    fg, _ = compose_chunk(decoded.chunks, decoded.patterns, fg_id)
    base = bytearray(fg_pixel if fg_pixel & 0xF else bg_pixel
                     for fg_pixel, bg_pixel in zip(fg, bg))
    bg_over, bg_count = compose_chunk(
        decoded.chunks, decoded.patterns, bg_id, priority_only=True
    )
    fg_over, fg_count = compose_chunk(
        decoded.chunks, decoded.patterns, fg_id, priority_only=True
    )
    over = bytearray(fg_pixel if fg_pixel & 0xF else bg_pixel
                     for fg_pixel, bg_pixel in zip(fg_over, bg_over))
    return base, over, bg_count + fg_count


def resolve_overworld_patches(decoded, overworld) -> tuple[list[dict[str, Any]], list[tuple[int, int]]]:
    """Resolve paged-loader hooks separately from MapDataManager effects.

    Retail replays these hooks when pages stream. The current native map is a
    full baked layout, so this emits the same flag-selected result at map
    construction/refresh. No mid-map flag change is implied by this record.
    Distinct flags must not touch one chunk: otherwise a single precomposed
    tile would conceal an unmodelled combination of active hooks.
    """
    if overworld is None:
        return [], []
    width, height = decoded.bg.width_chunks, decoded.bg.height_chunks
    if (decoded.fg.width_chunks, decoded.fg.height_chunks) != (width, height):
        raise MapPatchError("overworld planes have different dimensions")
    collision_plane = decoded.spec.collision_plane_name
    if collision_plane not in ("fg", "bg"):
        raise MapPatchError(f"unknown overworld collision plane {collision_plane!r}")
    by_flag: dict[int, dict[tuple[int, int], dict[str, int]]] = {}
    owner: dict[tuple[int, int], int] = {}
    for patch in overworld.patches:
        flag = patch.event_flag
        if not 0 <= flag < 512:
            raise MapPatchError(f"overworld event flag {flag} is outside the event bank")
        chunks = by_flag.setdefault(flag, {})
        for write in patch.writes:
            if write.plane not in ("fg", "bg"):
                raise MapPatchError(f"unknown overworld write plane {write.plane!r}")
            if not write.chunk_ids:
                raise MapPatchError("overworld hook contains an empty chunk write")
            for step, chunk_id in enumerate(write.chunk_ids):
                x, y = write.chunk_x + step, write.chunk_y
                if not (0 <= x < width and 0 <= y < height):
                    raise MapPatchError(f"overworld patch chunk ({x},{y}) is outside {width}x{height}")
                if not 0 <= chunk_id < len(decoded.chunks.words):
                    raise MapPatchError(f"overworld patch chunk {chunk_id:#04x} is not loaded")
                previous = owner.setdefault((x, y), flag)
                if previous != flag:
                    raise MapPatchError(
                        f"overworld chunk ({x},{y}) has overlapping flags {previous:#x}/{flag:#x}"
                    )
                pair = chunks.setdefault((x, y), {
                    "fg": decoded.fg.chunk_at(x, y),
                    "bg": decoded.bg.chunk_at(x, y),
                })
                pair[write.plane] = chunk_id
    groups: list[dict[str, Any]] = []
    pairs: set[tuple[int, int]] = set()
    for flag, chunks in by_flag.items():
        tiles = []
        for (x, y), pair in sorted(chunks.items()):
            fg_id, bg_id = pair["fg"], pair["bg"]
            collision_id = pair[collision_plane]
            cells = [value for _, _, value in chunk_collision_cells(decoded.chunks[collision_id])]
            tiles.append({
                "chunk_x": x, "chunk_y": y,
                "fg_chunk_id": fg_id, "bg_chunk_id": bg_id,
                "collision_chunk_id": collision_id,
                "collision": cells,
            })
            pairs.add((fg_id, bg_id))
        groups.append({"event_flag": flag, "tiles": tiles})
    return groups, sorted(pairs)


def _atlas(tiles: Sequence[bytearray], palette: Sequence[RGB], transparent) -> bytes:
    width = ATLAS_TILE_PIXELS * len(tiles)
    pixels = bytearray(width * ATLAS_TILE_PIXELS)
    for index, tile in enumerate(tiles):
        left = index * ATLAS_TILE_PIXELS
        for y in range(ATLAS_TILE_PIXELS):
            start = y * width + left
            pixels[start:start + ATLAS_TILE_PIXELS] = tile[
                y * ATLAS_TILE_PIXELS:(y + 1) * ATLAS_TILE_PIXELS
            ]
    return png.encode_indexed(width, ATLAS_TILE_PIXELS, bytes(pixels), palette, transparent)


def patch_atlas(
    decoded, chunk_ids: Sequence[int], palette: Sequence[RGB],
    composite_pairs: Sequence[tuple[int, int]] = (),
) -> tuple[bytes, bytes | None, list[dict[str, Any]]]:
    """Raw chunk tiles, then composed overworld FG/BG pairs, in one atlas row.

    Returns `(base png, overlay png or None, entries)`. The overlay is `None`
    when no raw tile has priority and there are no overworld composites.
    Composites always get an overlay atlas, even when fully transparent, so a
    changed chunk can clear the baked map's old above-sprites pixels.
    """
    ordered = sorted(set(chunk_ids))
    composite_pairs = sorted(set(composite_pairs))
    if not ordered and not composite_pairs:
        raise MapPatchError("cannot build an atlas from no chunks")
    base_tiles, over_tiles, entries = [], [], []
    priority_pixels = 0
    for index, chunk_id in enumerate(ordered):
        base, _ = compose_chunk(decoded.chunks, decoded.patterns, chunk_id)
        over, placed = compose_chunk(
            decoded.chunks, decoded.patterns, chunk_id, priority_only=True
        )
        opaque = sum(1 for value in over if value)
        priority_pixels += opaque
        base_tiles.append(base)
        over_tiles.append(over)
        entries.append({
            "index": index,
            "chunk_id": chunk_id,
            "x": index * ATLAS_TILE_PIXELS,
            "priority_tiles": placed,
            "priority_pixels": opaque,
            "collision": [value for _, _, value in chunk_collision_cells(decoded.chunks[chunk_id])],
        })
    for fg_id, bg_id in composite_pairs:
        index = len(entries)
        base, over, placed = compose_overworld_chunk(decoded, fg_id, bg_id)
        opaque = sum(1 for value in over if value & 0xF)
        priority_pixels += opaque
        base_tiles.append(base)
        over_tiles.append(over)
        collision_id = fg_id if decoded.spec.collision_plane_name == "fg" else bg_id
        entries.append({
            "index": index,
            "chunk_id": None,
            "fg_chunk_id": fg_id,
            "bg_chunk_id": bg_id,
            "x": index * ATLAS_TILE_PIXELS,
            "priority_tiles": placed,
            "priority_pixels": opaque,
            "collision": [value for _, _, value in chunk_collision_cells(decoded.chunks[collision_id])],
        })
    base_png = _atlas(base_tiles, palette, ())
    over_png = (
        _atlas(over_tiles, palette, OVERLAY_TRANSPARENT_INDICES)
        if priority_pixels or composite_pairs else None
    )
    return base_png, over_png, entries


# ---------------------------------------------------------------------------
# Resolution over a map's effects
# ---------------------------------------------------------------------------
def resolve_map_effects(
    decoded, effects: Sequence[dict[str, Any]]
) -> tuple[list[dict[str, Any]], list[int], dict[str, int]]:
    """Fold resolved cells into a map's effect list; collect its patched chunks.

    Returns `(effects, chunk ids, census counts)`. The effects are rebuilt
    rather than mutated so the caller's copy stays the decoder's own output.
    """
    plane = decoded.spec.collision_plane_name
    chunk_ids: list[int] = []
    counts = {
        "layout_writes": 0,
        "collision_authoritative": 0,
        "picture_only": 0,
        "cells": 0,
        "map_change_cells": 0,
    }

    resolved_effects = []
    for effect in effects:
        paths = []
        for path in effect.get("paths", ()):
            writes = []
            for write in path.get("writes", ()):
                if write.get("kind") != "layout_write":
                    writes.append(write)
                    continue
                counts["layout_writes"] += 1
                cells = resolve_write_cells(
                    decoded.chunks, write["chunk_id"], write["cell_x"], write["cell_y"]
                )
                authoritative = write["plane"] == plane
                counts["collision_authoritative" if authoritative else "picture_only"] += 1
                counts["cells"] += len(cells)
                counts["map_change_cells"] += sum(1 for c in cells if c["collision"] == 1)
                chunk_ids.append(write["chunk_id"])
                resolved = {
                    **write,
                    "cells": cells,
                    # A write to the plane collision does not read changes the
                    # picture and nothing else.
                    "collision_authoritative": authoritative,
                    "collision_plane": plane,
                }
                writes.append(resolved)
            paths.append({**path, "writes": writes})
        resolved_effects.append({**effect, "paths": paths})
    return resolved_effects, chunk_ids, counts


def atlas_json(
    entries: Sequence[dict[str, Any]], base_png: str, base_image: bytes,
    over_png: str | None, over_image: bytes | None,
) -> dict[str, Any]:
    """The atlas as a map record carries it."""
    return {
        "png": base_png,
        "png_sha256": hashlib.sha256(base_image).hexdigest(),
        "png_over": over_png,
        "png_over_sha256": (
            hashlib.sha256(over_image).hexdigest() if over_image is not None else None
        ),
        "tile_pixels": ATLAS_TILE_PIXELS,
        "count": len(entries),
        "note": (
            "Blit tile `index` at (chunk_x * 32, chunk_y * 32) over the baked "
            "map PNG; the overlay tile of the same index goes above sprites."
        ),
        "tiles": list(entries),
    }


def index_writes(effects: Sequence[dict[str, Any]], entries: Sequence[dict[str, Any]]) -> None:
    """Point each resolved write at its atlas tile, in place."""
    by_chunk = {entry["chunk_id"]: entry["index"] for entry in entries
                if entry["chunk_id"] is not None}
    for effect in effects:
        for path in effect.get("paths", ()):
            for write in path.get("writes", ()):
                if write.get("kind") == "layout_write":
                    write["patch_tile"] = by_chunk[write["chunk_id"]]


def index_overworld_patches(
    patches: Sequence[dict[str, Any]], entries: Sequence[dict[str, Any]]
) -> None:
    """Bind composed page-hook tiles without pretending they are raw chunks."""
    by_pair = {
        (entry["fg_chunk_id"], entry["bg_chunk_id"]): entry["index"]
        for entry in entries if entry["chunk_id"] is None
    }
    for patch in patches:
        for tile in patch["tiles"]:
            tile["patch_tile"] = by_pair[(tile["fg_chunk_id"], tile["bg_chunk_id"])]


def collision_summary(cells: Sequence[dict[str, Any]]) -> dict[str, int]:
    """How many of each collision type a set of resolved cells carries."""
    counts: dict[str, int] = {}
    for cell in cells:
        name = collision_type_name(cell["collision"])
        counts[name] = counts.get(name, 0) + 1
    return counts


# ---------------------------------------------------------------------------
# Palette copies
# ---------------------------------------------------------------------------
# Three `MapDataManager` routines end their gated path with the same five
# instructions -- `lea (src).l,a0 / lea (dst).w,a1 / move.w #n,d7 /
# move.l (a0)+,(a1)+ / dbf` -- which `map_effects` steps over as a
# "postincrement bulk copy". All three copy eight longs into
# `Palette_Table_Buffer + $60`, which is CRAM line 3.
#
# What that line reaches is the whole point. A field map's chunk tiles cannot
# select it: `ChunkTilesToBuffer` masks bit 14 off, so a chunk word carries one
# palette bit and reaches CRAM lines 0 and 1 only. Line 3 belongs to field
# *sprites* -- the objects whose `FieldObjectsJmpTbl` routine stores `$60` in
# `$13(a4)`. So one of these copies repaints NPCs and cannot change a single
# map pixel, which is checked rather than asserted: rendering Zema under the
# post-copy palette produces a byte-identical PNG.
_PALETTE_COPY = re.compile(
    rb"\x41\xf9(....)"      # lea (imm32).l, a0
    rb"\x43\xf8(..)"        # lea (imm16).w, a1
    rb"\x3e\x3c(..)"        # move.w #count-1, d7
    rb"\x22\xd8\x51\xcf\xff\xfc",  # move.l (a0)+,(a1)+ / dbf d7
    re.S,
)
PALETTE_COPY_INSTRUCTION = "postincrement bulk copy"
PALETTE_TABLE_BUFFER = 0xFB00
CRAM_LINE_BYTES = 32
CRAM_LINE_COLORS = 16
#: How far past a routine's start the copy is looked for. Every one of the
#: three sits inside the first flag-gated block.
PALETTE_COPY_SEARCH = 0x80


def decode_palette_copy(rom: bytes, routine: int) -> dict[str, Any] | None:
    """The bulk copy at `routine`, resolved, or `None` if it is not one.

    Fails closed on a copy that is not a whole CRAM line landing on a line
    boundary: this module's whole claim about what a copy affects rests on it
    being a palette line, and a copy that is not one has to be looked at rather
    than guessed at.
    """
    match = _PALETTE_COPY.search(rom, routine, routine + PALETTE_COPY_SEARCH)
    if match is None:
        return None
    source = int.from_bytes(match.group(1), "big")
    destination = int.from_bytes(match.group(2), "big")
    longs = int.from_bytes(match.group(3), "big") + 1
    words = longs * 2
    offset = destination - PALETTE_TABLE_BUFFER
    if offset < 0 or offset % CRAM_LINE_BYTES or words != CRAM_LINE_COLORS:
        raise MapPatchError(
            f"the bulk copy at 0x{routine:06X} writes {words} words to "
            f"0xFFFF{destination:04X}, which is not one whole CRAM line of "
            f"{PALETTE_TABLE_BUFFER:#06X}"
        )
    return {
        "instruction": PALETTE_COPY_INSTRUCTION,
        "resolved_as": "palette_write",
        "at": f"0x{match.start():06X}",
        "source": f"0x{source:06X}",
        "destination": f"Palette_Table_Buffer+0x{offset:02X}",
        "cram_line": offset // CRAM_LINE_BYTES,
        "words": words,
        "installs_chunk_definitions": False,
    }


def palette_copy_colors(rom: bytes, effect: dict[str, Any]) -> list[dict[str, Any]]:
    """The sixteen CRAM words a decoded copy installs."""
    source = int(effect["source"], 16)
    return decode_palette(rom[source:source + CRAM_LINE_BYTES])


def resolve_palette_effects(
    rom: bytes,
    record: dict[str, Any],
    decoded,
    effects: list[dict[str, Any]],
    npcs: Sequence[Any],
    routines: Sequence[Any],
    extents: dict[int, int],
    registry: Any,
    census: Any,
    map_palette: Sequence[RGB],
    dispatch: int,
) -> dict[str, int]:
    """Attach each palette copy to the sprite sheets it repaints.

    The effect is hung on the *path*, not on a write: two of the three copies
    sit on paths whose only writes are `object_despawn`, so anything keyed to a
    `layout_write` would drop them.

    An affected NPC is re-resolved under the post-copy palette and the result
    registered in the same sheet registry, which keys on content -- palette
    included -- so an alternate is a real sheet with the same pixels and
    different colours, deduplicated against every other map exactly like a base
    sheet. The path then names `from` and `to` per NPC and the runtime picks by
    the gate already on that path.
    """
    from .map_effects import routine_address
    from .sprites import object_sprite, stage_map_art, tile_source_from_patterns

    counts = {"palette_copies": 0, "alternate_sheets": 0, "repainted_npcs": 0}
    by_id = {routine.object_id: routine for routine in routines}
    art = None
    for effect in effects:
        for path in effect.get("paths", ()):
            if PALETTE_COPY_INSTRUCTION not in path.get("deferred", ()):
                continue
            resolved = decode_palette_copy(
                rom, routine_address(rom, dispatch, effect["entry"])
            )
            if resolved is None:
                continue
            counts["palette_copies"] += 1
            colors = palette_copy_colors(rom, resolved)
            line = resolved["cram_line"]
            # The map's 48-colour blob with the copied line substituted; only
            # lines 0, 1 and 3 are in it, in that order.
            position = MAP_PALETTE_LINE_ORDER.index(line)
            after = list(map_palette)
            after[position * CRAM_LINE_COLORS:(position + 1) * CRAM_LINE_COLORS] = (
                palette_rgb(colors)
            )

            repainted = []
            for entry, meta in zip(record["objects"]["entries"], npcs):
                routine = by_id.get(entry["object_id"])
                if routine is None or routine.palette_line != line or meta.sprite is None:
                    continue
                if art is None:
                    art = stage_map_art(
                        rom, record, tile_source_from_patterns(decoded.patterns)
                    )
                alternate = object_sprite(
                    rom, routine, extents, art, after,
                    facing=entry["facing_dir"], art_tile=entry["art_tile"], census=census,
                )
                if alternate.sheet is None:
                    continue
                sheet_id = registry.register(
                    alternate.sheet, entry["symbol"] or "FieldObj", placed=False
                )
                if sheet_id == meta.sprite["sheet"]:
                    continue
                repainted.append({
                    "npc_index": entry["index"],
                    "symbol": entry["symbol"],
                    "from": meta.sprite["sheet"],
                    "to": sheet_id,
                })
            counts["repainted_npcs"] += len(repainted)
            counts["alternate_sheets"] += len({r["to"] for r in repainted})
            path["deferred_effects"] = [{
                **resolved,
                "colors": [list(color) for color in palette_rgb(colors)],
                "words_hex": [color["raw"] for color in colors],
                # A chunk word carries one palette bit, so map tiles reach CRAM
                # lines 0 and 1 only. A write to any other line cannot change
                # the baked render, and re-rendering under it is byte-identical.
                "affects": {
                    "map_pixels": 0,
                    "map_render_identical": True,
                    "reason": (
                        "chunk tiles reach CRAM lines 0 and 1 only; bit 14 is "
                        "the collision flag and never reaches the VDP"
                    ),
                    "npc_sheets": repainted,
                },
            }]
    return counts
