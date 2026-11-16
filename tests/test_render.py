"""Independent research renderer's priority bits and synthetic PNG reader.

Emitted map, overlay and manifest contracts live in the Rust map suites.
"""

import struct
import unittest
import zlib
from pathlib import Path

from psiv_tools import png
from psiv_tools.chunk_draw import (
    CHUNK_WORDS,
)
from psiv_tools.layouts import ChunkTable
from psiv_tools.render import OVERLAY_TRANSPARENT_INDICES, PLANE_BYTES, priority_tiles

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"

MAP_PIATA = 0x10


def parse_png_chunks(data: bytes) -> list[tuple[bytes, bytes]]:
    """Re-parse a PNG, verifying every chunk CRC."""
    if data[:8] != png.PNG_SIGNATURE:
        raise AssertionError("missing PNG signature")
    chunks = []
    pos = 8
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        payload = data[pos + 8:pos + 8 + length]
        (crc,) = struct.unpack(">I", data[pos + 8 + length:pos + 12 + length])
        expected = zlib.crc32(kind + payload) & 0xFFFFFFFF
        if crc != expected:
            raise AssertionError(f"bad CRC on {kind!r}: {crc:08X} != {expected:08X}")
        chunks.append((kind, payload))
        pos += 12 + length
    return chunks


def png_size(data: bytes) -> tuple[int, int]:
    chunks = dict(parse_png_chunks(data))
    return struct.unpack(">II", chunks[b"IHDR"][:8])


def png_pixels(data: bytes) -> tuple[int, int, bytes]:
    """Width, height and the raw index bytes of an indexed PNG.

    `psiv_tools.png` writes filter type 0 on every scanline and one IDAT, so
    undoing it is stripping one byte per row.
    """
    chunks = parse_png_chunks(data)
    lookup = dict(chunks)
    width, height = struct.unpack(">II", lookup[b"IHDR"][:8])
    raw = zlib.decompress(b"".join(p for kind, p in chunks if kind == b"IDAT"))
    rows = bytearray()
    for y in range(height):
        start = y * (width + 1)
        if raw[start] != 0:
            raise AssertionError(f"row {y} uses filter {raw[start]}, not None")
        rows += raw[start + 1:start + 1 + width]
    return width, height, bytes(rows)


class TestOverlayConstants(unittest.TestCase):
    def test_transparency_is_colour_zero_of_both_reachable_lines(self):
        # Pixel values are `palette_line * 16 + colour_index`, and chunk tiles
        # reach CRAM lines 0 and 1 only, because bit 14 is the collision flag.
        self.assertEqual(OVERLAY_TRANSPARENT_INDICES, (0, 16))

    def test_plane_bytes_match_the_collision_sections_convention(self):
        self.assertEqual(PLANE_BYTES, {"fg": 0, "bg": 1})


class TestPriorityTiles(unittest.TestCase):
    """Bit 15 of a pattern-name word, on chunk definitions this test writes."""

    def chunks(self, *definitions):
        return ChunkTable(
            words=tuple(
                tuple(words) + (0,) * (CHUNK_WORDS - len(words))
                for words in definitions
            ),
            blobs=(),
        )

    def test_a_chunk_without_the_bit_is_absent_rather_than_empty(self):
        # Chunk 0 has none; chunk 1 has bit 15 on its second and sixth words,
        # i.e. tile (1, 0) and tile (1, 1) of the 4x4.
        found = priority_tiles(self.chunks([0x0001, 0x4002], [0, 0x8003, 0, 0, 0, 0x8004]))
        self.assertEqual(list(found), [1])
        self.assertEqual(found[1], ((1, 0, 0x8003), (1, 1, 0x8004)))

    def test_the_collision_bit_is_not_the_priority_bit(self):
        # $4000 is collision, $8000 is priority; only the second is video data.
        self.assertEqual(priority_tiles(self.chunks([0x4000] * 16)), {})
        self.assertEqual(len(priority_tiles(self.chunks([0x8000] * 16))[0]), 16)
