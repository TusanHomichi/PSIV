"""A minimal PNG encoder, standard library only.

Only what this project needs: 8-bit indexed-colour and 8-bit truecolour
images, one IDAT, filter type 0 on every scanline. That is enough to dump
decoded Mega Drive tiles for inspection and small enough to stay auditable.

Everything here follows the PNG specification (RFC 2083): an 8-byte signature
followed by length/type/data/CRC-32 chunks, IHDR first and IEND last, PLTE
before IDAT for colour type 3.

Encoded pixel data is never committed by this project. These helpers exist so
that extracted artwork can be written under `generated/`, which is ignored.
"""

from __future__ import annotations

import struct
import zlib
from typing import Iterable, Sequence

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"

COLOR_TYPE_RGB = 2
COLOR_TYPE_INDEXED = 3

RGB = tuple[int, int, int]


class PngError(ValueError):
    pass


def _chunk(kind: bytes, payload: bytes) -> bytes:
    """One length/type/data/CRC record. The CRC covers the type and the data."""
    if len(kind) != 4:
        raise PngError(f"Chunk type {kind!r} is not four bytes")
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
    )


def _ihdr(width: int, height: int, color_type: int) -> bytes:
    if width <= 0 or height <= 0:
        raise PngError(f"Image dimensions must be positive, got {width}x{height}")
    # bit depth 8, deflate, adaptive filtering, no interlace
    return _chunk("IHDR".encode("ascii"), struct.pack(">IIBBBBB", width, height, 8, color_type, 0, 0, 0))


def _idat(raw: bytes) -> bytes:
    return _chunk("IDAT".encode("ascii"), zlib.compress(raw, 9))


def _scanlines(pixels: bytes, stride: int, height: int) -> bytes:
    """Prefix every scanline with filter type 0 (None)."""
    if len(pixels) != stride * height:
        raise PngError(
            f"Pixel buffer is {len(pixels)} bytes, expected {stride * height} "
            f"({height} rows of {stride})"
        )
    out = bytearray()
    for y in range(height):
        out.append(0)
        out += pixels[y * stride:(y + 1) * stride]
    return bytes(out)


def encode_indexed(
    width: int,
    height: int,
    pixels: bytes | Sequence[int],
    palette: Sequence[RGB],
    transparent: Iterable[int] = (),
) -> bytes:
    """Encode an 8-bit indexed image.

    `pixels` is row-major, one byte per pixel. `palette` is up to 256 RGB
    triples. `transparent` lists palette indices that become fully
    transparent, emitted as a tRNS chunk; Mega Drive colour 0 is the
    transparent one for sprites, so this is worth having.
    """
    data = bytes(pixels)
    if not 1 <= len(palette) <= 256:
        raise PngError(f"Palette must hold 1..256 colours, got {len(palette)}")
    highest = max(data) if data else 0
    if highest >= len(palette):
        raise PngError(
            f"Pixel index {highest} has no palette entry ({len(palette)} colours)"
        )

    plte = bytearray()
    for index, colour in enumerate(palette):
        if len(colour) != 3 or any(not 0 <= c <= 255 for c in colour):
            raise PngError(f"Palette entry {index} is not an 8-bit RGB triple: {colour!r}")
        plte += bytes(colour)

    out = bytearray(PNG_SIGNATURE)
    out += _ihdr(width, height, COLOR_TYPE_INDEXED)
    out += _chunk("PLTE".encode("ascii"), bytes(plte))

    alpha = bytearray(255 for _ in palette)
    marked = False
    for index in transparent:
        if not 0 <= index < len(palette):
            raise PngError(f"Transparent index {index} is outside the palette")
        alpha[index] = 0
        marked = True
    if marked:
        # tRNS may be truncated after the last non-opaque entry.
        last = max(i for i, a in enumerate(alpha) if a != 255)
        out += _chunk("tRNS".encode("ascii"), bytes(alpha[:last + 1]))

    out += _idat(_scanlines(data, width, height))
    out += _chunk("IEND".encode("ascii"), b"")
    return bytes(out)


def encode_rgb(width: int, height: int, pixels: bytes | Sequence[int]) -> bytes:
    """Encode an 8-bit-per-channel truecolour image; `pixels` is RGB triples."""
    data = bytes(pixels)
    out = bytearray(PNG_SIGNATURE)
    out += _ihdr(width, height, COLOR_TYPE_RGB)
    out += _idat(_scanlines(data, width * 3, height))
    out += _chunk("IEND".encode("ascii"), b"")
    return bytes(out)
