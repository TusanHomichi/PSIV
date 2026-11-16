"""Add palette-index render inputs without changing accepted pack pictures.

Existing PNGs are indexed already. Replacing their PLTE with (index,0,0)
preserves exact pixel identity even when several CRAM slots share an RGB.
The renderer samples the index, never guesses a CRAM slot from an RGB value.
"""

from __future__ import annotations

import struct

from . import png


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
