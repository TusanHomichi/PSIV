"""`psiv_tools.render`: the priority overlay, from the bit to the pixels.

`priority_tiles` is testable without a ROM -- which pattern words carry bit 15
and where they sit in a chunk. The rest needs the cartridge, so the ROM-backed
half builds a one-map pack for Piata: the town whose palm crowns, dome roofs
and wall top course are the case that found the bug this overlay fixes. How
the *pack* reports an overlay -- the `png_over` field, the null case, the
manifest counts -- stays in `test_pack.py` with the rest of the emission.
"""

import struct
import tempfile
import unittest
import zlib
from pathlib import Path

from psiv_tools import png
from psiv_tools.core import read_rom
from psiv_tools.layouts import (
    CHUNK_PIXELS_X,
    CHUNK_PIXELS_Y,
    CHUNK_WORDS,
    TILE_PIXELS,
    ChunkTable,
)
from psiv_tools.maps import extract_maps
from psiv_tools.pack import MAPS_DIRECTORY, build_pack, decode_layout_section, layout_spec
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


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestPiatasOverlay(unittest.TestCase):
    """One map's overlay, built once and taken apart two ways."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls._temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls._temp.name) / "pack"
        cls.manifest = build_pack(cls.data, cls.root, map_ids=[MAP_PIATA])
        cls.entry = cls.manifest["maps"][0]

    @classmethod
    def tearDownClass(cls):
        cls._temp.cleanup()

    def test_piatas_overlay_is_the_pixels_that_draw_above_sprites(self):
        # Piata is the case that found the bug: its palm crowns, dome roofs and
        # the top course of the town wall all carry bit 15, so a character walks
        # behind them. The digest pins the whole image; the assertions below say
        # what it is made of.
        entry = self.entry
        self.assertEqual(entry["png_over"], f"{MAPS_DIRECTORY}/010_Piata_over.png")
        self.assertEqual(entry["priority_tiles"], 1068)
        self.assertEqual(
            entry["png_over_sha256"],
            "83e57e42c357adbb2e7aed8be55a23b8f46a4ab80c3f89551b6e3632084952f2",
        )

        base = parse_png_chunks((self.root / entry["png"]).read_bytes())
        overlay = parse_png_chunks((self.root / entry["png_over"]).read_bytes())
        # Same palette as the base render, plus a tRNS the base does not have.
        self.assertEqual(dict(base)[b"PLTE"], dict(overlay)[b"PLTE"])
        self.assertNotIn(b"tRNS", dict(base))
        alpha = dict(overlay)[b"tRNS"]
        self.assertEqual(
            [i for i, a in enumerate(alpha) if a == 0], [0, 16]
        )

        width, height, pixels = png_pixels((self.root / entry["png_over"]).read_bytes())
        self.assertEqual((width, height), png_size((self.root / entry["png"]).read_bytes()))
        self.assertEqual((width, height), (1024, 1024))
        # 16 is CRAM line 1 colour 0, which the compositor never writes: it
        # skips colour 0 before adding the line shift, so every non-zero byte is
        # a pixel the cartridge really draws over a sprite.
        self.assertNotIn(16, set(pixels))
        opaque = sum(1 for value in pixels if value)
        self.assertEqual(opaque, 47854)
        # Far less than the map, and not nothing.
        self.assertLess(opaque, width * height // 10)

    def test_the_overlay_only_holds_priority_tiles(self):
        # Re-derived from the chunk definitions rather than from the image: the
        # set of pixels the overlay may touch is exactly the bounding boxes of
        # the tiles whose pattern word has bit 15.
        record = extract_maps(self.data)["maps"][MAP_PIATA]
        decoded, _ = decode_layout_section(self.data, layout_spec(record))
        by_chunk = priority_tiles(decoded.chunks)
        allowed = set()
        for layout in (decoded.bg, decoded.fg):
            for chunk_y in range(layout.height_chunks):
                for chunk_x in range(layout.width_chunks):
                    for tile_x, tile_y, _ in by_chunk.get(
                        layout.chunk_at(chunk_x, chunk_y), ()
                    ):
                        ox = chunk_x * CHUNK_PIXELS_X + tile_x * TILE_PIXELS
                        oy = chunk_y * CHUNK_PIXELS_Y + tile_y * TILE_PIXELS
                        for y in range(TILE_PIXELS):
                            for x in range(TILE_PIXELS):
                                allowed.add((ox + x, oy + y))

        entry = self.entry
        width, _, pixels = png_pixels((self.root / entry["png_over"]).read_bytes())
        drawn = {
            (index % width, index // width)
            for index, value in enumerate(pixels)
            if value
        }
        self.assertTrue(drawn)
        self.assertEqual(drawn - allowed, set())
