"""The eleven STATUS portraits: Nemesis art and the original 10x10 maps."""
from __future__ import annotations

import hashlib
from pathlib import Path

from . import gfx, planes, png
from .nemesis import decompress
from .sprites.field import pal_init_line_3

ART_TABLE = 0x5D548
MAP_TABLE = 0x5D574
AGE_TABLE = 0x2A8E12


def character_age(rom: bytes, character: int) -> int:
    """Zero is the original unknown-age marker, not an inferred biography."""
    offset = AGE_TABLE + character * 2
    return int.from_bytes(rom[offset:offset + 2], "big")


def emit_status_portraits(rom: bytes, root: Path) -> list[dict]:
    directory = root / "battle" / "status_portraits"
    directory.mkdir(parents=True, exist_ok=True)
    palette = [(0, 0, 0)] * 32 + pal_init_line_3(rom) + [(0, 0, 0)] * 16
    records = []
    for character in range(11):
        art = gfx.read_long(rom, ART_TABLE + character * 4)
        mapping = gfx.read_long(rom, MAP_TABLE + character * 4)
        raw, consumed = decompress(rom, art)
        cells = [planes.decode_cell(int.from_bytes(rom[p:p + 2], "big"))
                 for p in range(mapping, mapping + 200, 2)]
        if any(cell.palette_line != 2 for cell in cells):
            raise ValueError(f"STATUS portrait {character} uses an unexpected palette")
        width, height, pixels = planes.compose(cells, 10, gfx.decode_tiles(raw), 0x580)
        image = png.encode_indexed(width, height, pixels, palette, transparent=(0,))
        relative = f"battle/status_portraits/{character:02x}.png"
        (root / relative).write_bytes(image)
        records.append({
            "png": relative, "png_sha256": hashlib.sha256(image).hexdigest(),
            "art": f"0x{art:06X}", "mapping": f"0x{mapping:06X}",
            "source_consumed": consumed, "size_pixels": [width, height],
            "screen_pixels": [24, 16], "cram_line": 2,
        })
    return records
