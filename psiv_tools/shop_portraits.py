"""Decode the eight portraits selected by the retail shop pointer tables."""
from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

from . import gfx, png
from .nemesis import decompress
from .sprites.field import pal_init_line_3

# Win_ShopMeseta: two groups of pointers, then the shared 6x6 Plane A map.
PORTRAIT_GROUPS = ((0x68006, 18), (0x6804E, 49))
PORTRAIT_MAPPING = 0x2A2B36


def emit_shop_portraits(rom: bytes, root: Path) -> list[dict[str, Any]]:
    pointers = sorted({
        int.from_bytes(rom[at:at + 4], "big")
        for base, count in PORTRAIT_GROUPS
        for at in range(base, base + count * 4, 4)
    })
    words = [int.from_bytes(rom[at:at + 2], "big")
             for at in range(PORTRAIT_MAPPING, PORTRAIT_MAPPING + 72, 2)]
    if words != list(range(0xC55C, 0xC580)):
        raise ValueError("shop portrait mapping differs from the retail 6x6 tile grid")
    directory = root / "shops" / "portraits"
    directory.mkdir(parents=True, exist_ok=True)
    palette = pal_init_line_3(rom)
    records = []
    for address in pointers:
        raw, consumed = decompress(rom, address)
        tiles = gfx.decode_tiles(raw)
        if len(tiles) != 36:
            raise ValueError(f"shop portrait {address:#x} has {len(tiles)} tiles")
        width, height, pixels = gfx.compose_sheet(tiles, 6)
        image = png.encode_indexed(width, height, pixels, palette, transparent=(0,))
        relative = f"shops/portraits/{address:06x}.png"
        (root / relative).write_bytes(image)
        records.append({
            "art": f"0x{address:06X}", "png": relative,
            "png_sha256": hashlib.sha256(image).hexdigest(),
            "source_consumed": consumed, "mapping": "0x2A2B36",
            "size_pixels": [width, height], "cram_line": 2,
        })
    return records
