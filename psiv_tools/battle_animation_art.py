"""PNG emission for the retail enemy attack-object census."""

from __future__ import annotations

import hashlib
import struct
import zlib
from pathlib import Path
from typing import Any

from . import png
from .battle_animation_objects import plc_art as decode_plc_art
from .core import EXPECTED_SHA256
from .nemesis import TILE_SIZE

BASE_OBJECT_Y = 0x00D8
MAPPING_HEADER_BYTES = 1
MAPPING_ENTRY_BYTES = 6
ENEMY_ATTACK_ART_DIRECTORY = "battle/art/enemy_attacks"
ENEMY_ATTACK_ART_NAME = "battle/art/enemy_attacks.json"
ROOT = Path(__file__).resolve().parents[1]


def _hex(value: int) -> str:
    return f"0x{value:06X}"


def _attack_safe(value: str) -> str:
    return "".join(char if char.isalnum() or char in "-_" else "_" for char in value)


def _attack_movement_pixels(movement: dict[str, Any] | None) -> dict[str, Any]:
    """Return the normalized motion contract the Godot layer consumes."""
    runtime = None if movement is None else movement.get("runtime")
    if not runtime:
        return {
            "kind": "static_offset",
            "initial_offset_pixels": [0, 0],
            "step_pixels": [0, 0],
            "limit_offset_pixels": None,
            "branches": [],
        }
    limit_offset = None
    if runtime.get("limit_hardware") is not None:
        limit = runtime["limit_hardware"] - BASE_OBJECT_Y
        limit_offset = [
            limit if runtime.get("axis") == "x" else 0,
            limit if runtime.get("axis") == "y" else 0,
        ]
    return {
        "kind": runtime["kind"],
        "initial_offset_pixels": runtime["initial_offset_pixels"],
        "step_pixels": runtime["step_pixels"],
        "limit_hardware": runtime.get("limit_hardware"),
        "limit_offset_pixels": limit_offset,
        "branches": runtime.get("branches", []),
    }


def _attack_canvas(
    animation: dict[str, Any], art_record: dict[str, Any]
) -> tuple[int, int, int, int]:
    """Find one stable transparent canvas for all of an enemy's frames."""
    body_width = 2 * art_record["half_width_cells"] * 8
    body_top = (15 - art_record["height_cells"]) * 8
    pivot_x = body_width
    pivot_y = BASE_OBJECT_Y - 0x80 - body_top
    positions: list[tuple[int, int, int, int]] = []
    sequence = animation["frame_sequence"]
    for record in sequence["mapping_records"]:
        for entry in record["entries"]:
            positions.append((
                pivot_x + entry["x"],
                pivot_y + entry["y"],
                entry["width_tiles"] * 8,
                entry["height_tiles"] * 8,
            ))
    if not positions:
        return 0, 0, max(body_width, 8), max(art_record["height_cells"] * 8, 8)
    min_x = min(0, *(x for x, _, _, _ in positions))
    min_y = min(0, *(y for _, y, _, _ in positions))
    max_x = max(body_width, *(x + width for x, _, width, _ in positions))
    max_y = max(
        art_record["height_cells"] * 8,
        *(y + height for _, y, _, height in positions),
    )
    return min_x, min_y, max_x - min_x, max_y - min_y


def _render_attack_mapping(
    tiles: list[bytes],
    record: dict[str, Any],
    canvas: tuple[int, int, int, int],
    pivot: tuple[int, int],
    palette: list[tuple[int, int, int]],
) -> bytes:
    min_x, min_y, width, height = canvas
    pixels = bytearray(width * height)
    if record.get("hidden"):
        return png.encode_indexed(width, height, pixels, palette, (0,))
    pivot_x, pivot_y = pivot
    for entry in record["entries"]:
        tile_index = entry["tile_index"]
        width_tiles = entry["width_tiles"]
        height_tiles = entry["height_tiles"]
        sprite_x = pivot_x + entry["x"] - min_x
        sprite_y = pivot_y + entry["y"] - min_y
        for y in range(height_tiles * 8):
            source_y = height_tiles * 8 - 1 - y if entry["v_flip"] else y
            tile_row, pixel_y = divmod(source_y, 8)
            for x in range(width_tiles * 8):
                source_x = width_tiles * 8 - 1 - x if entry["h_flip"] else x
                tile_col, pixel_x = divmod(source_x, 8)
                tile = tiles[tile_index + tile_row * width_tiles + tile_col]
                pixel = tile[pixel_y * 8 + pixel_x]
                if pixel:
                    pixels[(sprite_y + y) * width + sprite_x + x] = pixel
    return png.encode_indexed(width, height, pixels, palette, (0,))


def _indexed_pixels(data: bytes) -> tuple[int, int, bytes]:
    """Read the dependency-free indexed PNGs emitted by the oracle renderer."""
    if data[:8] != png.PNG_SIGNATURE:
        raise ValueError("oracle attack fixture is not a PNG")
    pos = 8
    ihdr = None
    compressed = bytearray()
    while pos < len(data):
        if pos + 12 > len(data):
            raise ValueError("oracle attack fixture has a truncated PNG chunk")
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        payload_start = pos + 8
        payload_end = payload_start + length
        if payload_end + 4 > len(data):
            raise ValueError("oracle attack fixture has an overlong PNG chunk")
        payload = data[payload_start:payload_end]
        if kind == b"IHDR":
            ihdr = payload
        elif kind == b"IDAT":
            compressed.extend(payload)
        pos = payload_end + 4
    if ihdr is None or len(ihdr) != 13:
        raise ValueError("oracle attack fixture has no valid IHDR")
    width, height, depth, color_type, compression, filtering, interlace = struct.unpack(
        ">IIBBBBB", ihdr
    )
    if (depth, color_type, compression, filtering, interlace) != (
        8, png.COLOR_TYPE_INDEXED, 0, 0, 0
    ):
        raise ValueError("oracle attack fixture is not a filter-0 indexed PNG")
    raw = zlib.decompress(bytes(compressed))
    stride = width + 1
    if len(raw) != stride * height:
        raise ValueError("oracle attack fixture has malformed scanlines")
    if any(raw[row * stride] != 0 for row in range(height)):
        raise ValueError("oracle attack fixture uses an unsupported PNG filter")
    pixels = b"".join(
        raw[row * stride + 1:(row + 1) * stride] for row in range(height)
    )
    return width, height, pixels


def _emit_oracle_receipt_art(
    animation: dict[str, Any],
    receipt: dict[str, Any],
    directory: Path,
    entry: dict[str, Any],
    rom: bytes,
) -> tuple[int, int]:
    """Copy VRAM-rendered oracle frames into the normal attack-art surface."""
    from .battle_art_pack import enemy_palette

    prefix = receipt.get("frame_asset_prefix")
    if not prefix:
        raise ValueError(
            f"{receipt.get('routine_offset')}: oracle receipt has no art prefix"
        )
    durations = receipt["durations"]
    origin = receipt.get("origin_pixels")
    if (
        not isinstance(origin, list)
        or len(origin) != 2
        or not all(isinstance(value, int) for value in origin)
    ):
        raise ValueError(
            f"{receipt.get('routine_offset')}: oracle receipt has no local origin"
        )
    total_bytes = 0
    png_count = 0
    for frame_index, duration in enumerate(durations):
        source = ROOT / f"{prefix}_frame{frame_index:02d}.png"
        if not source.is_file():
            raise FileNotFoundError(f"missing oracle attack frame: {source}")
        source_image = source.read_bytes()
        width, height, pixels = _indexed_pixels(source_image)
        # A shared routine body can serve enemies with different CRAM words.
        # Preserve the oracle's structure/pixels, but give each copied PNG its
        # own existing line-relative palette so Godot's indexed recolour pass
        # can resolve it without smuggling a colour decision into the receipt.
        image = png.encode_indexed(
            width,
            height,
            pixels,
            enemy_palette(rom, animation["enemy_id"]),
            transparent=(0,),
        )
        name = (
            f"{animation['enemy_id']:03d}_"
            f"{_attack_safe(animation['symbol'])}_frame{frame_index:02d}.png"
        )
        relative = f"{ENEMY_ATTACK_ART_DIRECTORY}/{name}"
        (directory / relative).write_bytes(image)
        total_bytes += len(image)
        png_count += 1
        entry["frames"].append({
            "index": frame_index,
            "mapping_rom_offset": (
                f"oracle:{receipt['routine_offset']}:frame{frame_index:02d}"
            ),
            "png": relative,
            "png_sha256": hashlib.sha256(image).hexdigest(),
            "duration": duration,
        })
    entry.update({
        "origin_pixels": origin,
        "width_pixels": width,
        "height_pixels": height,
        "frame_duration": durations[0],
        "total_frames": sum(durations),
        "art": [{
            "field": "oracle_vdp_vram",
            "routine_offset": receipt["routine_offset"],
            "representative_enemy_id": receipt["representative_enemy_id"],
            "action_frame": receipt["action_frame"],
            "observed_offsets": receipt["observed_offsets"],
            "capture": receipt["capture"],
            "source": receipt["receipt"],
        }],
    })
    return total_bytes, png_count


def emit_enemy_attack_art(
    rom: bytes,
    out_dir: str | Path,
    animations_payload: dict[str, Any] | None = None,
) -> tuple[dict[str, Any], int, int]:
    """Emit exact frame PNGs, including proven attack PLC pattern sources."""
    from .battle_art import build_tile_bank, enemy_art_bounds, enemy_records
    from .battle_art_pack import enemy_palette
    from .battle_animations import build_enemy_animations

    payload = animations_payload or build_enemy_animations(rom)
    records = enemy_records(rom)
    bounds = enemy_art_bounds(records, len(rom))
    by_id = {record["id"]: record for record in records}
    directory = Path(out_dir)
    (directory / ENEMY_ATTACK_ART_DIRECTORY).mkdir(parents=True, exist_ok=True)
    entries: list[dict[str, Any]] = []
    total_bytes = 0
    png_count = 0

    for animation in payload["animations"]:
        enemy_id = animation["enemy_id"]
        composition = animation["composition"]
        entry: dict[str, Any] = {
            "id": enemy_id,
            "symbol": animation["symbol"],
            "status": composition["status"],
            "reason": composition["reason"],
            "movement": {
                "status": animation["movement"]["status"],
                "reason": animation["movement"]["reason"],
                "runtime": _attack_movement_pixels(animation["movement"]),
                "writes": animation["movement"]["writes"],
            },
            "frames": [],
            "source": {
                "enemy_art_record": _hex(0x27F3AE + enemy_id * 20),
                "mapping_consumer": "Battle_FillSpriteAttributes",
                "grand_cross": 0,
            },
        }
        if composition["status"] != "exact":
            entries.append(entry)
            continue

        receipt = animation.get("oracle_receipt")
        if receipt is not None and receipt.get("frame_asset_prefix"):
            entry["source"]["oracle_receipt"] = {
                "routine_offset": receipt["routine_offset"],
                "representative_enemy_id": receipt["representative_enemy_id"],
                "presentation": receipt["presentation"],
                "action_frame": receipt["action_frame"],
                "receipt": receipt["receipt"],
            }
            receipt_bytes, receipt_pngs = _emit_oracle_receipt_art(
                animation, receipt, directory, entry, rom
            )
            total_bytes += receipt_bytes
            png_count += receipt_pngs
            entries.append(entry)
            continue

        art_record = by_id[enemy_id]
        tiles, art_sources = build_tile_bank(rom, art_record, bounds)
        plc_tiles, _, _ = decode_plc_art(rom, animation["attack_plc"]["loads"])
        if plc_tiles:
            if len(tiles) < len(plc_tiles):
                tiles.extend(bytes(TILE_SIZE) for _ in range(len(plc_tiles) - len(tiles)))
            for source in animation["attack_plc"]["ranges"]:
                for pattern in range(source["first_pattern"], source["last_pattern_exclusive"]):
                    if pattern < len(plc_tiles):
                        tiles[pattern] = plc_tiles[pattern]

        canvas = _attack_canvas(animation, art_record)
        body_width = 2 * art_record["half_width_cells"] * 8
        body_top = (15 - art_record["height_cells"]) * 8
        pivot = (body_width, BASE_OBJECT_Y - 0x80 - body_top)
        palette = enemy_palette(rom, enemy_id)
        durations = animation["frame_sequence"]["frame_durations"]
        for frame_index, record in enumerate(animation["frame_sequence"]["mapping_records"]):
            image = _render_attack_mapping(tiles, record, canvas, pivot, palette)
            name = f"{enemy_id:03d}_{_attack_safe(animation['symbol'])}_frame{frame_index:02d}.png"
            relative = f"{ENEMY_ATTACK_ART_DIRECTORY}/{name}"
            (directory / relative).write_bytes(image)
            total_bytes += len(image)
            png_count += 1
            entry["frames"].append({
                "index": frame_index,
                "mapping_rom_offset": record["rom_offset"],
                "png": relative,
                "png_sha256": hashlib.sha256(image).hexdigest(),
                "duration": durations[frame_index],
            })
        entry.update({
            "origin_pixels": [canvas[0], canvas[1]],
            "width_pixels": canvas[2],
            "height_pixels": canvas[3],
            "frame_duration": animation["frame_sequence"]["frame_duration"],
            "total_frames": animation["frame_sequence"]["total_frames"],
            "art": [
                {
                    "field": source["field"],
                    "rom_offset": source["rom_offset"],
                    "first_pattern": source["first_pattern"],
                    "pattern_count": source["tile_count"],
                }
                for source in art_sources
            ] + [
                {
                    "field": "attack_plc",
                    "plc_id": source.get("plc_id"),
                    "call_rom_offset": source.get("call_rom_offset"),
                    "record_rom_offset": source.get("record_rom_offset"),
                    "stream_rom_offset": source.get("stream_rom_offset"),
                    "first_pattern": source["first_pattern"],
                    "pattern_count": source["pattern_count"],
                }
                for source in animation["attack_plc"]["ranges"]
            ],
        })
        entries.append(entry)

    return {
        "kind": "battle_enemy_attack_art",
        "count": len(entries),
        "source": {
            "enemy_attack_table": "0x00D0A4",
            "enemy_art_table": "0x27F3AE",
            "mapping_consumer": "Battle_FillSpriteAttributes",
            "mapping_header_bytes": MAPPING_HEADER_BYTES,
            "mapping_entry_bytes": MAPPING_ENTRY_BYTES,
            "art_bank_order": [3, 1, 2],
            "rom_sha256": payload["source"].get("rom_sha256", EXPECTED_SHA256),
            "grand_cross": 0,
        },
        "census": {
            "enemies": len(entries),
            "exact": sum(entry["status"] == "exact" for entry in entries),
            "partial": sum(entry["status"] == "partial" for entry in entries),
            "deferred": sum(entry["status"] == "deferred" for entry in entries),
            "frames": png_count,
        },
        "enemies": entries,
    }, total_bytes, png_count
