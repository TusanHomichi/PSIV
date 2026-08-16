"""Retail enemy sprite-piece lists and their dynamic tile renders.

The enemy body is a plane mapping, but the cartridge builds part of the same
visual by copying animated Art #2 tiles into a temporary Art #3 bank.  The
piece list is therefore a *tile replacement* stream, not an alpha sprite
layer.  Keeping that distinction here matters: an old body pixel must be
cleared when a replacement tile contains colour zero.

The decoder is deliberately strict.  The table, its ZoranBult entry, and the
animation routine are byte-pinned to the USA cartridge.  Descriptor pointers,
sequence lengths, source ranges, and destination ranges are checked before a
piece is exposed to either extraction or emission.  The 18 descriptors with
the high bit set in their pointer are retained as decoded metadata but are
not rendered: the retail init routine uses that bit to disable their RAM
sprite slot.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Sequence

from . import planes, png
from .battle_art import (
    BLANK_PATTERN,
    ENEMY_COUNT,
    ENEMY_CRAM_LINES,
    ENEMY_PALETTE_TABLE,
    BattleArtError,
    build_tile_bank,
    enemy_art_bounds,
    enemy_cram_line,
    enemy_mapping,
    enemy_records,
)
from .gfx import decode_palette, palette_rgb


ENEMY_SPRITE_MAPPING_TABLE: dict[str, Any] = {
    "label": "EnemySpriteMappingsOffs",
    "rom_offset": 0x010FCE,
    "entry_count": ENEMY_COUNT,
    "entry_size": 2,
    "data_start": 0x011100,
}

# `EnemySprites_Animate` starts here in the retail disassembly.  The first
# bytes cover the ordinary timer/frame path; the compact high-bit sequence
# branch is checked by the parser as well.
ENEMY_SPRITE_ANIMATE_OFFSET = 0x040F66
ENEMY_SPRITE_ANIMATE_SIGNATURE = bytes.fromhex(
    "08ec00050002302c0004226c"
)

ZORAN_MAPPING_OFFSET = 0x01190C
ZORAN_MAPPING_SIGNATURE = bytes.fromhex(
    "00021102000119200d040001192a040900011934"
)

OVERLAY_ART_DIRECTORY = "battle/art/enemy_overlays"
OVERLAY_ART_NAME = "battle/art/enemy_overlays.json"


def _slice(data: bytes, offset: int, size: int, what: str) -> bytes:
    if offset < 0 or size < 0 or offset + size > len(data):
        raise BattleArtError(
            f"{what} at 0x{offset:06X} (+{size}) runs past the end of the ROM"
        )
    return data[offset:offset + size]


def _word(data: bytes, offset: int, what: str = "word") -> int:
    return int.from_bytes(_slice(data, offset, 2, what), "big")


def _long(data: bytes, offset: int, what: str = "pointer") -> int:
    return int.from_bytes(_slice(data, offset, 4, what), "big")


def verify_enemy_overlay_sources(data: bytes) -> dict[str, Any]:
    """Pin the retail table/routine bytes before decoding any descriptors."""
    table = ENEMY_SPRITE_MAPPING_TABLE
    base = table["rom_offset"]
    first = _slice(data, base, 4, table["label"])
    if first != bytes.fromhex("09960996"):
        raise BattleArtError(
            f"{table['label']} at 0x{base:06X}: retail signature changed "
            f"({first.hex()}); refusing to decode the reference clone"
        )
    zoran_entry = _word(data, base + 10 * table["entry_size"], "ZoranBult table entry")
    zoran_target = base + zoran_entry
    if zoran_target != ZORAN_MAPPING_OFFSET:
        raise BattleArtError(
            f"{table['label']} ZoranBult entry points to 0x{zoran_target:06X}, "
            f"expected 0x{ZORAN_MAPPING_OFFSET:06X}"
        )
    zoran = _slice(data, ZORAN_MAPPING_OFFSET, len(ZORAN_MAPPING_SIGNATURE), "Mappings_ZoranBult")
    if zoran != ZORAN_MAPPING_SIGNATURE:
        raise BattleArtError(
            "Mappings_ZoranBult: retail descriptor signature changed "
            f"({zoran.hex()})"
        )
    animate = _slice(
        data,
        ENEMY_SPRITE_ANIMATE_OFFSET,
        len(ENEMY_SPRITE_ANIMATE_SIGNATURE),
        "EnemySprites_Animate",
    )
    if animate != ENEMY_SPRITE_ANIMATE_SIGNATURE:
        raise BattleArtError(
            "EnemySprites_Animate: retail opcode signature changed "
            f"({animate.hex()})"
        )
    return {
        "rom_sha256": hashlib.sha256(data).hexdigest(),
        "table_signature": first.hex(),
        "table_signature_sha256": hashlib.sha256(data[base:base + 8]).hexdigest(),
        "zoran_mapping_signature": ZORAN_MAPPING_SIGNATURE.hex(),
        "animation_routine": {
            "label": "EnemySprites_Animate",
            "rom_offset": f"0x{ENEMY_SPRITE_ANIMATE_OFFSET:06X}",
            "signature": ENEMY_SPRITE_ANIMATE_SIGNATURE.hex(),
            "signature_sha256": hashlib.sha256(animate).hexdigest(),
        },
    }


@dataclass
class OverlayPiece:
    """One `EnemySpriteMappingsOffs` descriptor and its decoded sequence."""

    index: int
    descriptor_offset: int
    destination_tile: int
    tile_count: int
    pointer_raw: int
    pointer: int
    sequence_data: int
    enabled: bool
    sequence_format: str
    cadence: int | None
    source_patterns: tuple[int, ...]
    durations: tuple[int, ...]
    initial_source_pattern: int
    placements: list[dict[str, Any]]
    source_range_valid: bool

    @property
    def runtime_order(self) -> tuple[int, ...]:
        if len(self.source_patterns) <= 1:
            return (0,)
        return tuple(range(1, len(self.source_patterns))) + (0,)

    @property
    def runtime_durations(self) -> tuple[int, ...]:
        return tuple(self.durations[index] for index in self.runtime_order)

    @property
    def body_cell_indices(self) -> list[int]:
        return [index for placement in self.placements for index in placement["cell_indices"]]

    def metadata(self, source_art: dict[str, Any]) -> dict[str, Any]:
        first = self.placements[0] if self.placements else None
        return {
            "piece": self.index,
            "descriptor_rom_offset": f"0x{self.descriptor_offset:06X}",
            "enabled": self.enabled,
            "pointer_flag_bit31": not self.enabled,
            "pointer_rom_value": f"0x{self.pointer_raw:08X}",
            "pointer": f"0x{self.pointer:06X}",
            "sequence_data": f"0x{self.sequence_data:06X}",
            "destination_tile": self.destination_tile,
            "tile_count": self.tile_count,
            "destination_tile_end": self.destination_tile + self.tile_count - 1,
            "tile_source": source_art,
            "tile_source_addressing": "Art #2 local pattern offsets",
            "palette": {
                "source": "enemy CRAM line",
                "cram_lines": list(ENEMY_CRAM_LINES),
                "pixel_indices": "line-relative 0-15",
            },
            "sequence_format": self.sequence_format,
            "frame_count": len(self.source_patterns),
            "cadence": self.cadence,
            "cadence_ticks": None if self.cadence is None else self.cadence + 1,
            "durations": list(self.durations),
            "duration_ticks": [duration + 1 for duration in self.durations],
            "source_patterns": list(self.source_patterns),
            "initial_source_pattern": self.initial_source_pattern,
            "runtime_order": list(self.runtime_order),
            "runtime_source_patterns": [self.source_patterns[i] for i in self.runtime_order],
            "runtime_durations": list(self.runtime_durations),
            "source_range_valid": self.source_range_valid,
            "placement_status": (
                "body_mapping_rectangle" if self.placements else "not_in_body_mapping"
            ),
            # Convenience fields for consumers that only need the canonical
            # placement. `placements` is authoritative when a mirrored copy
            # exists.
            "offset_pixels": None if first is None else first["offset_pixels"],
            "size_pixels": None if first is None else first["size_pixels"],
            "flip_h": None if first is None else first["flip_h"],
            "flip_v": None if first is None else first["flip_v"],
            "placements": self.placements,
            "covered_body_cells": self.body_cell_indices,
        }


@dataclass
class DecodedEnemyOverlay:
    enemy_id: int
    symbol: str | None
    columns: int
    rows: int
    words: list[int]
    tiles: list[bytes]
    art2_tiles: list[bytes]
    art2_source: dict[str, Any]
    body_hole_cells: list[int]
    pieces: list[OverlayPiece]
    mapping_offset: int
    block_offset: int


def _decode_sequence(data: bytes, pointer_raw: int, label: str) -> dict[str, Any]:
    pointer = pointer_raw & 0x7FFFFFFF
    if pointer & 1:
        raise BattleArtError(f"{label}: sequence wrapper 0x{pointer:06X} is not even")
    sequence_data = _long(data, pointer, f"{label} sequence wrapper")
    first = _slice(data, sequence_data, 1, f"{label} sequence")[0]
    if first & 0x80:
        count = first & 0x7F
        if count == 0:
            raise BattleArtError(f"{label}: compact sequence has zero frames")
        durations = list(_slice(data, sequence_data + 1, count, f"{label} durations"))
        sources = list(_slice(data, sequence_data + 1 + count, count, f"{label} sources"))
        sequence_format = "compact_per_frame_duration"
        cadence = None
    else:
        count = first
        if count == 0:
            raise BattleArtError(f"{label}: ordinary sequence has zero frames")
        cadence = _slice(data, sequence_data + 1, 1, f"{label} cadence")[0]
        sources = list(_slice(data, sequence_data + 2, count, f"{label} sources"))
        durations = [cadence] * count
        sequence_format = "constant_cadence"
    # `loc_7C56` consumes the count byte, skips the cadence/duration byte,
    # then reads the first source byte.  The cadence is a timer only; it is
    # not the startup tile offset.
    initial_source = sources[0]
    return {
        "pointer": pointer,
        "sequence_data": sequence_data,
        "format": sequence_format,
        "cadence": cadence,
        "sources": tuple(sources),
        "durations": tuple(durations),
        "initial_source": initial_source,
    }


def _placements(
    words: Sequence[int], columns: int, rows: int, destination: int, count: int
) -> list[dict[str, Any]]:
    """Find body rectangles whose tile IDs are a descriptor's destination run."""
    cells = planes.decode_cells(words)
    found: list[dict[str, Any]] = []
    for height in range(1, rows + 1):
        if count % height:
            continue
        width = count // height
        if width > columns:
            continue
        for y in range(rows - height + 1):
            for x in range(columns - width + 1):
                for flip_v in (False, True):
                    for flip_h in (False, True):
                        indices: list[int] = []
                        matched = True
                        for row in range(height):
                            for column in range(width):
                                source_x = width - 1 - column if flip_h else column
                                source_y = height - 1 - row if flip_v else row
                                index = (y + row) * columns + x + column
                                cell = cells[index]
                                expected = destination + source_y * width + source_x
                                if (
                                    cell.tile != expected
                                    or cell.h_flip != flip_h
                                    or cell.v_flip != flip_v
                                ):
                                    matched = False
                                    break
                                indices.append(index)
                            if not matched:
                                break
                        if matched:
                            found.append({
                                "offset_cells": [x, y],
                                "offset_pixels": [x * 8, y * 8],
                                "size_cells": [width, height],
                                "size_pixels": [width * 8, height * 8],
                                "flip_h": flip_h,
                                "flip_v": flip_v,
                                "cell_indices": indices,
                            })
    return found


def _block_for_enemy(data: bytes, enemy_id: int) -> tuple[int, list[tuple[int, int, int, int]]]:
    base = ENEMY_SPRITE_MAPPING_TABLE["rom_offset"]
    entry = _word(data, base + enemy_id * ENEMY_SPRITE_MAPPING_TABLE["entry_size"], "enemy sprite table entry")
    block = base + entry
    if block < ENEMY_SPRITE_MAPPING_TABLE["data_start"] or block & 1:
        raise BattleArtError(
            f"enemy 0x{enemy_id:02X}: EnemySpriteMappingsOffs entry reaches "
            f"invalid block 0x{block:06X}"
        )
    descriptor_count = _word(data, block, "enemy sprite descriptor count") + 1
    cursor = block + 2
    descriptors: list[tuple[int, int, int, int]] = []
    for index in range(descriptor_count):
        raw = _slice(data, cursor, 6, f"enemy 0x{enemy_id:02X} descriptor {index}")
        descriptors.append((cursor, raw[0], raw[1], int.from_bytes(raw[2:], "big")))
        cursor += 6
    return block, descriptors


def decode_enemy_overlay_records(
    data: bytes,
    records: Sequence[dict[str, Any]] | None = None,
    bounds: dict[int, int] | None = None,
) -> tuple[dict[str, Any], list[DecodedEnemyOverlay]]:
    """Decode all 153 entries and return provenance plus renderable records."""
    provenance = verify_enemy_overlay_sources(data)
    if records is None:
        records = enemy_records(data)
    if bounds is None:
        bounds = enemy_art_bounds(records, len(data))
    decoded: list[DecodedEnemyOverlay] = []
    max_end = ENEMY_SPRITE_MAPPING_TABLE["data_start"]
    for record in records:
        enemy_id = record["id"]
        tiles, sources = build_tile_bank(data, record, bounds)
        words, mapping = enemy_mapping(data, record)
        source = sources[1]
        art2_start = source["first_pattern"]
        art2_count = source["tile_count"]
        body_holes = [
            index
            for index, word in enumerate(words)
            if (word & 0x07FF)
            and tiles[word & 0x07FF] == BLANK_PATTERN
        ]
        block, descriptors = _block_for_enemy(data, enemy_id)
        max_end = max(max_end, block + 2 + 6 * len(descriptors))
        pieces: list[OverlayPiece] = []
        for index, (descriptor_offset, destination, count, pointer_raw) in enumerate(descriptors):
            sequence = _decode_sequence(
                data, pointer_raw, f"enemy 0x{enemy_id:02X} piece {index}"
            )
            max_end = max(
                max_end,
                sequence["sequence_data"]
                + 1
                + (len(sequence["sources"]) * 2 if sequence["cadence"] is None else 1 + len(sequence["sources"])),
            )
            source_patterns = sequence["sources"]
            source_range_valid = all(
                source_pattern + count <= art2_count
                for source_pattern in (*source_patterns, sequence["initial_source"])
            )
            if not (pointer_raw & 0x80000000) and not source_range_valid:
                raise BattleArtError(
                    f"enemy 0x{enemy_id:02X} piece {index}: active Art #2 source "
                    f"range exceeds its {art2_count}-tile blob"
                )
            if destination + count > len(tiles):
                raise BattleArtError(
                    f"enemy 0x{enemy_id:02X} piece {index}: destination "
                    f"{destination}+{count} exceeds its {len(tiles)}-tile bank"
                )
            pieces.append(OverlayPiece(
                index=index,
                descriptor_offset=descriptor_offset,
                destination_tile=destination,
                tile_count=count,
                pointer_raw=pointer_raw,
                pointer=sequence["pointer"],
                sequence_data=sequence["sequence_data"],
                enabled=not bool(pointer_raw & 0x80000000),
                sequence_format=sequence["format"],
                cadence=sequence["cadence"],
                source_patterns=source_patterns,
                durations=sequence["durations"],
                initial_source_pattern=sequence["initial_source"],
                placements=_placements(
                    words, mapping["columns"], mapping["rows"], destination, count
                ),
                source_range_valid=source_range_valid,
            ))
        decoded.append(DecodedEnemyOverlay(
            enemy_id=enemy_id,
            symbol=record["symbol"],
            columns=mapping["columns"],
            rows=mapping["rows"],
            words=words,
            tiles=tiles,
            art2_tiles=tiles[art2_start:art2_start + art2_count],
            art2_source={
                "field": source["field"],
                "rom_offset": source["rom_offset"],
                "first_pattern": source["first_pattern"],
                "tile_count": source["tile_count"],
            },
            body_hole_cells=body_holes,
            pieces=pieces,
            mapping_offset=record["mapping_offset"],
            block_offset=block,
        ))
    provenance["table"] = {
        **ENEMY_SPRITE_MAPPING_TABLE,
        "rom_offset": f"0x{ENEMY_SPRITE_MAPPING_TABLE['rom_offset']:06X}",
        "data_start": f"0x{ENEMY_SPRITE_MAPPING_TABLE['data_start']:06X}",
        "decoded_data_end_exclusive": f"0x{max_end:06X}",
    }
    provenance["descriptor_format"] = (
        "dc.w descriptor_count_minus_1; "
        "dc.b destination_tile, tile_count; dc.l sequence_pointer"
    )
    provenance["pointer_flag"] = "bit31 disables the Enemy_Sprites RAM slot"
    provenance["source_art"] = "ArtNem enemy field +0x06 (Art #2), 32-byte tiles"
    provenance["destination_art"] = "Art #3 staging bank, 32-byte tile offsets"
    provenance["animation_decode"] = {
        "ordinary": "count, cadence, source_pattern[count]",
        "compact": "0x80|count, duration[count], source_pattern[count]",
        "initial_frame": "loc_7C56 startup source; EnemySprites_Animate advances frame then wraps",
        "timer_ticks": "stored_duration + 1 update calls",
    }
    return provenance, decoded


def _coverage(decoded: Sequence[DecodedEnemyOverlay]) -> dict[str, Any]:
    holed = [enemy for enemy in decoded if enemy.body_hole_cells]
    covered_ids: list[int] = []
    all_frame_ids: list[int] = []
    total_covered = 0
    total_all_frame = 0
    for enemy in decoded:
        covered = 0
        all_frame = 0
        for cell_index in enemy.body_hole_cells:
            tile = enemy.words[cell_index] & 0x07FF
            pieces = [
                piece
                for piece in enemy.pieces
                if piece.enabled
                and piece.destination_tile <= tile < piece.destination_tile + piece.tile_count
            ]
            if not pieces:
                continue
            covered += 1
            tile_all_frames = False
            for piece in pieces:
                offset = tile - piece.destination_tile
                values = [
                    enemy.art2_tiles[source + offset]
                    for source in (*piece.source_patterns, piece.initial_source_pattern)
                    if source + offset < len(enemy.art2_tiles)
                ]
                if values and all(value != BLANK_PATTERN for value in values):
                    tile_all_frames = True
            if tile_all_frames:
                all_frame += 1
        total_covered += covered
        total_all_frame += all_frame
        if enemy.body_hole_cells and covered == len(enemy.body_hole_cells):
            covered_ids.append(enemy.enemy_id)
        if enemy.body_hole_cells and all_frame == len(enemy.body_hole_cells):
            all_frame_ids.append(enemy.enemy_id)
    return {
        "holed_enemy_count": len(holed),
        "holed_enemy_ids": [enemy.enemy_id for enemy in holed],
        "holed_enemies_fully_covered_by_destination": covered_ids,
        "holed_enemies_nonblank_in_every_decoded_frame": all_frame_ids,
        "total_body_holes": sum(len(enemy.body_hole_cells) for enemy in decoded),
        "total_holes_covered_by_destination": total_covered,
        "total_holes_nonblank_in_every_decoded_frame": total_all_frame,
        "remaining_holes_by_destination": sum(
            len(enemy.body_hole_cells)
            - sum(
                1
                for index in enemy.body_hole_cells
                if any(
                    piece.enabled
                    and piece.destination_tile <= (enemy.words[index] & 0x07FF)
                    < piece.destination_tile + piece.tile_count
                    for piece in enemy.pieces
                )
            )
            for enemy in decoded
        ),
    }


def overlay_payload(provenance: dict[str, Any], decoded: Sequence[DecodedEnemyOverlay]) -> dict[str, Any]:
    """JSON-safe extraction payload without emitted PNG paths."""
    enemies: list[dict[str, Any]] = []
    for enemy in decoded:
        pieces = [piece.metadata(enemy.art2_source) for piece in enemy.pieces]
        enemies.append({
            "id": enemy.enemy_id,
            "symbol": enemy.symbol,
            "mapping_rom_offset": f"0x{enemy.mapping_offset:06X}",
            "mapping_block_rom_offset": f"0x{enemy.block_offset:06X}",
            "piece_count": len(pieces),
            "enabled_piece_count": sum(piece["enabled"] for piece in pieces),
            "body_holes": len(enemy.body_hole_cells),
            "body_hole_cells": enemy.body_hole_cells,
            "pieces": pieces,
        })
    active = [piece for enemy in decoded for piece in enemy.pieces if piece.enabled]
    return {
        "kind": "battle_enemy_overlay_art",
        "table": provenance["table"],
        "provenance": provenance,
        "piece_count": sum(len(enemy.pieces) for enemy in decoded),
        "enabled_piece_count": len(active),
        "distinct_mapping_blocks": len({enemy.block_offset for enemy in decoded}),
        "coverage": _coverage(decoded),
        "enemies": enemies,
    }


def extract_enemy_overlays(data: bytes) -> dict[str, Any]:
    """Return strict, metadata-only overlay extraction for all enemies."""
    provenance, decoded = decode_enemy_overlay_records(data)
    return overlay_payload(provenance, decoded)


def _line_palette(data: bytes, enemy_id: int) -> list[tuple[int, int, int]]:
    words = enemy_cram_line(data, enemy_id, 1)
    colors = palette_rgb(decode_palette(b"".join(word.to_bytes(2, "big") for word in words)))
    # Pack consumers replace these with the battle backdrop/line-specific UI
    # values.  Keeping the same indexed convention as enemy_body_png makes
    # piece frames directly interchangeable with body PNGs.
    colors[0] = (0, 0, 0)
    colors[15] = (0, 0, 0)
    return colors


def render_overlay_piece_frame(
    data: bytes, enemy: DecodedEnemyOverlay, piece: OverlayPiece, source_pattern: int
) -> bytes:
    """Render one descriptor state as a full body-sized replacement frame."""
    if not piece.enabled:
        raise BattleArtError(f"enemy 0x{enemy.enemy_id:02X} piece {piece.index} is disabled")
    if source_pattern + piece.tile_count > len(enemy.art2_tiles):
        raise BattleArtError(
            f"enemy 0x{enemy.enemy_id:02X} piece {piece.index}: source frame "
            f"{source_pattern}+{piece.tile_count} is outside Art #2"
        )
    tiles = list(enemy.tiles)
    for index in range(piece.tile_count):
        tiles[piece.destination_tile + index] = enemy.art2_tiles[source_pattern + index]
    cells = planes.decode_cells(enemy.words)
    width, height, pixels = planes.compose(cells, enemy.columns, tiles)
    return png.encode_indexed(
        width,
        height,
        pixels,
        _line_palette(data, enemy.enemy_id),
        (0,),
    )


def _safe(label: str | None) -> str:
    return "".join(c if c.isalnum() or c in "-_" else "_" for c in label or "unknown")


def emit_enemy_overlays(
    data: bytes, out_dir: str | Path
) -> tuple[dict[str, Any], int, int]:
    """Emit deterministic full-body replacement frames for every active piece."""
    provenance, decoded = decode_enemy_overlay_records(data)
    directory = Path(out_dir)
    root = directory / OVERLAY_ART_DIRECTORY
    root.mkdir(parents=True, exist_ok=True)
    payload = overlay_payload(provenance, decoded)
    by_id = {enemy["id"]: enemy for enemy in payload["enemies"]}
    total_bytes = 0
    png_count = 0
    for enemy in decoded:
        entry = by_id[enemy.enemy_id]
        for piece, piece_entry in zip(enemy.pieces, entry["pieces"]):
            if not piece.enabled:
                piece_entry["initial_png"] = None
                piece_entry["frames"] = []
                continue
            prefix = f"{enemy.enemy_id:03d}_{_safe(enemy.symbol)}_piece{piece.index:02d}"
            initial = render_overlay_piece_frame(
                data, enemy, piece, piece.initial_source_pattern
            )
            initial_name = f"{prefix}_initial.png"
            (root / initial_name).write_bytes(initial)
            total_bytes += len(initial)
            png_count += 1
            piece_entry["initial_png"] = f"{OVERLAY_ART_DIRECTORY}/{initial_name}"
            piece_entry["initial_png_sha256"] = hashlib.sha256(initial).hexdigest()
            frames: list[dict[str, Any]] = []
            for frame, source_pattern in enumerate(piece.source_patterns):
                image = render_overlay_piece_frame(data, enemy, piece, source_pattern)
                name = f"{prefix}_frame{frame:02d}.png"
                (root / name).write_bytes(image)
                total_bytes += len(image)
                png_count += 1
                frames.append({
                    "frame": frame,
                    "source_pattern": source_pattern,
                    "duration": piece.durations[frame],
                    "duration_ticks": piece.durations[frame] + 1,
                    "png": f"{OVERLAY_ART_DIRECTORY}/{name}",
                    "png_sha256": hashlib.sha256(image).hexdigest(),
                })
            piece_entry["frames"] = frames
    return payload, total_bytes, png_count
