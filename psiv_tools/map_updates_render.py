"""Add palette-index render inputs without changing accepted pack pictures.

Existing PNGs are indexed already. Replacing their PLTE with (index,0,0)
preserves exact pixel identity even when several CRAM slots share an RGB.
The renderer samples the index, never guesses a CRAM slot from an RGB value.
"""

from __future__ import annotations

import struct
from pathlib import Path

from . import png
from .m68k import w
from .sprites.field import PAL_INIT_LINE_3


def index_png(data: bytes) -> bytes:
    """Preserve IDAT, dimensions and transparency; encode the CRAM index in R."""
    if not data.startswith(png.PNG_SIGNATURE):
        raise ValueError("palette-index input is not a PNG")
    output = bytearray(data[:8])
    at, found = 8, False
    while at < len(data):
        size = struct.unpack_from(">I", data, at)[0]
        kind = data[at + 4:at + 8]
        if kind == b"IHDR" and data[at + 17] != 3:
            raise ValueError("palette-index input must be an indexed PNG")
        if kind == b"PLTE":
            output += png._chunk(kind, bytes(v for i in range(size // 3) for v in (i, 0, 0)))
            found = True
        else:
            output += data[at:at + size + 12]
        at += size + 12
    if not found:
        raise ValueError("palette-index input has no palette")
    return bytes(output)


def bind_updates(rom: bytes, record: dict, payload: dict, directory: Path,
                 base: bytes, overlay: bytes | None, updates: dict) -> None:
    """Attach ordered programs and additive index images to one packed map."""
    entries = updates["per_map"][record["id"]]
    payload["map_updates"] = entries
    pointer = int(record["palette"]["pointer"], 16)
    words = [w(rom, pointer + i * 2) for i in range(48)]
    words[32:32] = [w(rom, PAL_INIT_LINE_3 + i * 2) for i in range(16)]
    payload["map_update_palette"] = words
    if not any(e["program"]["kind"] in ("palette", "crystals", "edge", "edge_line")
               for e in entries):
        return
    sources = [(payload["png"], base)]
    if overlay is not None:
        sources.append((payload["png_over"], overlay))
    for variant in payload["layout_variants"]:
        for key in ("png", "png_over"):
            if variant.get(key):
                name = variant[key]
                sources.append((name, (directory / name).read_bytes()))
    if payload.get("patch_tiles"):
        for key in ("png", "png_over"):
            if payload["patch_tiles"].get(key):
                name = payload["patch_tiles"][key]
                sources.append((name, (directory / name).read_bytes()))
    images = {}
    for name, data in sources:
        indexed = name.removesuffix(".png") + "_indices.png"
        (directory / indexed).write_bytes(index_png(data))
        images[name] = indexed
    payload["map_update_images"] = images
