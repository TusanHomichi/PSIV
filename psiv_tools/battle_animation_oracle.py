"""Render checked-in enemy-animation art from retail oracle state receipts.

The oracle state dump contains the battle SAT buffer and the live VDP VRAM.
This renderer follows the Genesis sprite link chain, decodes the hardware
size/tile attributes, and writes the resulting line-relative indexed image.
It deliberately does not synthesize a mapping record or a colour ramp.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any, Iterable

from . import png
from .battle_animation_receipts import receipt_file
from .battle_art_pack import enemy_palette
from .gfx import decode_tile

SCREEN_WIDTH = 320
SCREEN_HEIGHT = 224
SPRITE_TABLE_BYTES = 0x0280
VRAM_BYTES = 0x10000
BASE_COORDINATE = 0x80


class BattleOracleRenderError(ValueError):
    """An oracle state receipt cannot be rendered or verified."""


def _region_bytes(state: dict[str, Any], name: str, size: int) -> bytes:
    try:
        region = state["regions"][name]
        data = bytes.fromhex(region["bytes_hex"])
    except (KeyError, TypeError, ValueError) as exc:
        raise BattleOracleRenderError(f"state is missing region {name}") from exc
    if len(data) != size:
        raise BattleOracleRenderError(
            f"state region {name} has {len(data)} bytes, expected {size}"
        )
    return data


def _linked_sprites(state: dict[str, Any]) -> Iterable[tuple[int, int, int, int, int]]:
    """Yield `(y, size, tile_word, x, link)` entries in VDP display order."""
    table = _region_bytes(state, "sprite_table", SPRITE_TABLE_BYTES)
    index = 0
    seen: set[int] = set()
    while 0 <= index < SPRITE_TABLE_BYTES // 8 and index not in seen:
        seen.add(index)
        entry = table[index * 8:(index + 1) * 8]
        if not any(entry):
            break
        y = int.from_bytes(entry[0:2], "big")
        size = entry[2]
        link = entry[3] & 0x7F
        tile_word = int.from_bytes(entry[4:6], "big")
        x = int.from_bytes(entry[6:8], "big")
        yield y, size, tile_word, x, link
        if link == 0:
            break
        index = link


def render_state(state: dict[str, Any], rom: bytes, enemy_id: int) -> bytes:
    """Render one live SAT/VRAM state as a transparent indexed PNG."""
    vram = _region_bytes(state, "vdp_vram", VRAM_BYTES)
    pixels = bytearray(SCREEN_WIDTH * SCREEN_HEIGHT)
    for y, size, tile_word, x, _ in _linked_sprites(state):
        width_tiles = (size & 0x03) + 1
        height_tiles = ((size >> 2) & 0x03) + 1
        tile_index = tile_word & 0x07FF
        h_flip = bool(tile_word & 0x0800)
        v_flip = bool(tile_word & 0x1000)
        origin_x = x - BASE_COORDINATE
        origin_y = y - BASE_COORDINATE
        for output_y in range(height_tiles * 8):
            source_y = height_tiles * 8 - 1 - output_y if v_flip else output_y
            tile_row, pixel_y = divmod(source_y, 8)
            for output_x in range(width_tiles * 8):
                source_x = width_tiles * 8 - 1 - output_x if h_flip else output_x
                tile_column, pixel_x = divmod(source_x, 8)
                pattern = tile_index + tile_row * width_tiles + tile_column
                start = pattern * 32
                if start + 32 > len(vram):
                    raise BattleOracleRenderError(
                        f"sprite tile {pattern:#x} runs past VDP VRAM"
                    )
                pixel = decode_tile(vram[start:start + 32])[pixel_y * 8 + pixel_x]
                target_x = origin_x + output_x
                target_y = origin_y + output_y
                if pixel and 0 <= target_x < SCREEN_WIDTH and 0 <= target_y < SCREEN_HEIGHT:
                    pixels[target_y * SCREEN_WIDTH + target_x] = pixel
    return png.encode_indexed(
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        pixels,
        enemy_palette(rom, enemy_id),
        transparent=(0,),
    )


def _capture_directory(roots: Iterable[Path], enemy_id: int) -> Path:
    name = f"psiv-fulltrace-{enemy_id}"
    for root in roots:
        candidate = root / name
        if candidate.is_dir():
            return candidate
    raise BattleOracleRenderError(f"missing capture directory {name}")


def _verify_capture(receipt: dict[str, Any], directory: Path) -> None:
    capture = receipt.get("capture")
    if not isinstance(capture, dict):
        raise BattleOracleRenderError(
            f"{receipt['routine_offset']}: capture hash receipt is missing"
        )
    sprite_hash = hashlib.sha256()
    vram_hash = hashlib.sha256()
    first = capture["frame_first"]
    count = capture["frame_count"]
    for frame in range(first, first + count):
        path = directory / f"frame_{frame}.json"
        if not path.is_file():
            raise BattleOracleRenderError(f"missing oracle state: {path}")
        state = json.loads(path.read_text())
        sprite_hash.update(_region_bytes(state, "sprite_table", SPRITE_TABLE_BYTES))
        vram_hash.update(_region_bytes(state, "vdp_vram", VRAM_BYTES))
    if sprite_hash.hexdigest() != capture["sprite_table_sha256"]:
        raise BattleOracleRenderError(
            f"{receipt['routine_offset']}: sprite-table capture hash disagrees"
        )
    if vram_hash.hexdigest() != capture["vdp_vram_sha256"]:
        raise BattleOracleRenderError(
            f"{receipt['routine_offset']}: VDP-VRAM capture hash disagrees"
        )


def render_receipt_art(
    rom: bytes, output: Path, capture_roots: Iterable[Path]
) -> int:
    """Verify all dense captures and emit one PNG for every observed state."""
    payload = receipt_file()
    output.mkdir(parents=True, exist_ok=True)
    count = 0
    for receipt in payload["receipts"]:
        representative = receipt["representative_enemy_id"]
        directory = _capture_directory(capture_roots, representative)
        _verify_capture(receipt, directory)
        prefix = receipt["routine_offset"].removeprefix("0x")
        for index, offset in enumerate(receipt["observed_offsets"]):
            frame = receipt["action_frame"] + offset
            state = json.loads((directory / f"frame_{frame}.json").read_text())
            image = render_state(state, rom, representative)
            (output / f"{prefix}_frame{index:02d}.png").write_bytes(image)
            count += 1
    return count


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rom", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument(
        "--capture-root",
        action="append",
        type=Path,
        required=True,
        help="directory containing psiv-fulltrace-<enemy-id> state directories",
    )
    args = parser.parse_args()
    count = render_receipt_art(args.rom.read_bytes(), args.output, args.capture_root)
    print(f"rendered {count} oracle receipt frames")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
