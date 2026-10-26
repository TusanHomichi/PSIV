"""A minimal PNG encoder, standard library only.

Only what this project needs: 8-bit indexed-colour and 8-bit truecolour
images, one IDAT, filter type 0 on every scanline. That is enough to dump
decoded Mega Drive tiles for inspection and small enough to stay auditable.

Everything here follows the PNG specification (RFC 2083): an 8-byte signature
followed by length/type/data/CRC-32 chunks, IHDR first and IEND last, PLTE
before IDAT for colour type 3.

Deflate output is not specified byte for byte: zlib and zlib-ng compress the
same rows to different streams. So a digest that names an image is
`image_sha256`, taken over what the file decodes to, never `sha256` of the
file's bytes.

Encoded pixel data is never committed by this project. These helpers exist so
that extracted artwork can be written under `generated/`, which is ignored.
"""

from __future__ import annotations

import hashlib
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
    # Deleting every valid index leaves exactly the pixels that have none.
    unpaletted = data.translate(None, bytes(range(len(palette))))
    if unpaletted:
        raise PngError(
            f"Pixel index {max(unpaletted)} has no palette entry ({len(palette)} colours)"
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


# ---------------------------------------------------------------------------
# Image identity
# ---------------------------------------------------------------------------
#: Bytes per pixel at bit depth 8, by colour type (grey, RGB, indexed,
#: grey+alpha, RGBA).
_BYTES_PER_PIXEL = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}
#: The chunks that say what the image is; the rest (gAMA, tEXt, ...) annotate it.
_IDENTITY_CHUNKS = (b"PLTE", b"tRNS")
_IMAGE_DIGEST_DOMAIN = b"psiv-image-sha256-v1\0"


def _paeth(left: int, up: int, corner: int) -> int:
    estimate = left + up - corner
    to_left, to_up, to_corner = abs(estimate - left), abs(estimate - up), abs(estimate - corner)
    if to_left <= to_up and to_left <= to_corner:
        return left
    return up if to_up <= to_corner else corner


def _unfilter(raw: bytes, stride: int, height: int, bpp: int) -> bytes:
    """Undo the per-scanline filters (RFC 2083 section 6)."""
    if len(raw) != (stride + 1) * height:
        raise PngError(
            f"IDAT inflates to {len(raw)} bytes, expected {(stride + 1) * height} "
            f"({height} rows of 1 + {stride})"
        )
    out = bytearray()
    previous = bytes(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind = raw[start]
        row = bytearray(raw[start + 1:start + 1 + stride])
        if kind == 1:
            for x in range(bpp, stride):
                row[x] = (row[x] + row[x - bpp]) & 0xFF
        elif kind == 2:
            for x in range(stride):
                row[x] = (row[x] + previous[x]) & 0xFF
        elif kind == 3:
            for x in range(stride):
                left = row[x - bpp] if x >= bpp else 0
                row[x] = (row[x] + ((left + previous[x]) >> 1)) & 0xFF
        elif kind == 4:
            for x in range(stride):
                left = row[x - bpp] if x >= bpp else 0
                corner = previous[x - bpp] if x >= bpp else 0
                row[x] = (row[x] + _paeth(left, previous[x], corner)) & 0xFF
        elif kind != 0:
            raise PngError(f"Scanline {y} has filter type {kind}, not 0-4")
        out += row
        previous = row
    return bytes(out)


def image_sha256(data: bytes) -> str:
    """The identity of a PNG's image: sha256 of what it decodes to.

    Covers the IHDR fields, the PLTE and tRNS payloads and the unfiltered
    pixel rows, so the same pixels in the same palette give the same digest
    whichever deflate implementation, level or filter wrote the file, and any
    changed pixel, colour, transparency or dimension gives a different one.
    Ancillary chunks other than tRNS are not part of the image. Takes 8-bit,
    non-interlaced images, which is every image this project writes.
    """
    if data[:8] != PNG_SIGNATURE:
        raise PngError("missing PNG signature")
    pos = 8
    header = None
    parts: dict[bytes, bytes] = {}
    idat = bytearray()
    while pos < len(data):
        if pos + 12 > len(data):
            raise PngError(f"truncated chunk at byte {pos}")
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        payload = data[pos + 8:pos + 8 + length]
        if len(payload) != length or pos + 12 + length > len(data):
            raise PngError(f"truncated {kind!r} chunk at byte {pos}")
        (crc,) = struct.unpack(">I", data[pos + 8 + length:pos + 12 + length])
        if crc != zlib.crc32(kind + payload) & 0xFFFFFFFF:
            raise PngError(f"{kind!r} chunk at byte {pos} fails its CRC")
        pos += 12 + length
        if kind == b"IHDR":
            header = payload
        elif kind == b"IDAT":
            idat += payload
        elif kind in _IDENTITY_CHUNKS:
            parts[kind] = payload
        elif kind == b"IEND":
            break
    if header is None or len(header) != 13:
        raise PngError("missing or malformed IHDR")
    width, height, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", header)
    if depth != 8 or colour not in _BYTES_PER_PIXEL or interlace != 0:
        raise PngError(
            f"image_sha256 takes 8-bit non-interlaced images, not depth {depth} "
            f"colour type {colour} interlace {interlace}"
        )
    bpp = _BYTES_PER_PIXEL[colour]
    pixels = _unfilter(zlib.decompress(bytes(idat)), width * bpp, height, bpp)
    digest = hashlib.sha256(_IMAGE_DIGEST_DOMAIN + header)
    for kind in _IDENTITY_CHUNKS:
        payload = parts.get(kind, b"")
        digest.update(kind + struct.pack(">I", len(payload)) + payload)
    digest.update(pixels)
    return digest.hexdigest()
