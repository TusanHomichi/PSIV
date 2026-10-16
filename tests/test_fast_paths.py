"""Table-driven rewrites of pack hot paths agree with the loops they replaced (#130).

Each reference below is the replaced implementation, kept here as the oracle.
"""

import random
import unittest
from pathlib import Path

from psiv_tools import battle_animation_objects as objects
from psiv_tools.core import read_rom
from psiv_tools import chunk_draw, layouts, render
from psiv_tools.gfx import COLORS_PER_LINE, TILE_HEIGHT, TILE_WIDTH, decode_tile
from psiv_tools.maps import extract_maps
from psiv_tools.pack_layouts import decode_map_section
from psiv_tools.planes import PlaneCell, compose

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"


def reference_decode_tile(chunk: bytes) -> bytes:
    out = bytearray(TILE_WIDTH * TILE_HEIGHT)
    for i, byte in enumerate(chunk):
        out[i * 2] = byte >> 4
        out[i * 2 + 1] = byte & 0x0F
    return bytes(out)


def reference_compose(cells, columns, tiles, art_vram_tile=0):
    def oriented(tile, h_flip, v_flip):
        if not h_flip and not v_flip:
            return tile
        rows = [tile[y * TILE_WIDTH:(y + 1) * TILE_WIDTH] for y in range(TILE_HEIGHT)]
        if h_flip:
            rows = [row[::-1] for row in rows]
        if v_flip:
            rows = rows[::-1]
        return b"".join(rows)

    width = columns * TILE_WIDTH
    height = (len(cells) // columns) * TILE_HEIGHT
    pixels = bytearray(width * height)
    for index, cell in enumerate(cells):
        tile = oriented(tiles[cell.tile - art_vram_tile], cell.h_flip, cell.v_flip)
        base = cell.palette_line * COLORS_PER_LINE
        ox = (index % columns) * TILE_WIDTH
        oy = (index // columns) * TILE_HEIGHT
        for y in range(TILE_HEIGHT):
            start = (oy + y) * width + ox
            row = tile[y * TILE_WIDTH:(y + 1) * TILE_WIDTH]
            pixels[start:start + TILE_WIDTH] = bytes(base + p if p else 0 for p in row)
    return width, height, bytes(pixels)


def reference_call_sites(rom, start, stop):
    return [o for o in range(start, stop, 2) if objects._call_target(rom, o) is not None]


def reference_draw(layout, words_at, patterns, pixels, width, overlay):
    """`compose_layout`'s and `_draw_priority_plane`'s per-pixel loop."""
    L = chunk_draw
    for chunk_y in range(layout.height_chunks):
        for chunk_x in range(layout.width_chunks):
            for tile_x, tile_y, raw in words_at(layout.chunk_at(chunk_x, chunk_y)):
                word = L.vdp_word(raw)
                index = L.tile_index(word)
                if index not in patterns.loaded:
                    continue
                tile = patterns.tile(index)
                shift = L.palette_line(word) * 16
                flip_x, flip_y = L.h_flip(word), L.v_flip(word)
                ox = chunk_x * L.CHUNK_PIXELS_X + tile_x * L.TILE_PIXELS
                oy = chunk_y * L.CHUNK_PIXELS_Y + tile_y * L.TILE_PIXELS
                for y in range(L.TILE_PIXELS):
                    source_y = L.TILE_PIXELS - 1 - y if flip_y else y
                    row = tile[source_y * L.TILE_PIXELS:(source_y + 1) * L.TILE_PIXELS]
                    if flip_x:
                        row = row[::-1]
                    start = (oy + y) * width + ox
                    for x in range(L.TILE_PIXELS):
                        if overlay and row[x] == 0:
                            continue
                        pixels[start + x] = row[x] + shift


def cell(tile, line, h_flip, v_flip):
    word = (line << 13) | (v_flip << 12) | (h_flip << 11) | tile
    return PlaneCell(tile=tile, palette_line=line, h_flip=bool(h_flip),
                     v_flip=bool(v_flip), priority=False, word=word)


class TestTables(unittest.TestCase):
    def test_decode_tile_matches_the_nybble_loop_for_every_byte(self):
        rng = random.Random(130)
        chunks = [bytes(range(i, i + 32)) for i in range(0, 256, 32)]
        chunks += [bytes(rng.randrange(256) for _ in range(32)) for _ in range(200)]
        for chunk in chunks:
            self.assertEqual(decode_tile(chunk), reference_decode_tile(chunk))
            self.assertEqual(decode_tile(bytearray(chunk)), reference_decode_tile(chunk))

    def test_compose_matches_the_per_pixel_loop(self):
        rng = random.Random(130)
        tiles = [bytes(rng.randrange(16) for _ in range(64)) for _ in range(12)]
        tiles.append(bytes(64))  # all transparent
        for columns in (1, 3, 8):
            cells = [
                cell(rng.randrange(len(tiles)) + 0x20, rng.randrange(4),
                     rng.randrange(2), rng.randrange(2))
                for _ in range(columns * 5)
            ]
            self.assertEqual(
                compose(cells, columns, tiles, art_vram_tile=0x20),
                reference_compose(cells, columns, tiles, art_vram_tile=0x20),
            )

    def test_compose_still_refuses_an_index_past_the_cram(self):
        # Negative control for the translate table: a pattern value that would
        # leave the 256 indices raised in the loop and must still raise.
        tile = bytes([0xF0] * 64)
        with self.assertRaises(ValueError):
            reference_compose([cell(0, 3, 0, 0)], 1, [tile])
        with self.assertRaises(ValueError):
            compose([cell(0, 3, 0, 0)], 1, [tile])


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestMapDraws(unittest.TestCase):
    """Chunk-at-a-time draws and collision against the per-cell loops, on real maps."""

    # Piata, the Academy and its first floor. The world maps go through the
    # same draw and would only make this test slow.
    MAPS = (0x10, 0x11, 0x13)

    @classmethod
    def setUpClass(cls):
        cls.rom = read_rom(ROM)
        cls.records = {r["id"]: r for r in extract_maps(cls.rom)["maps"]}

    def test_both_planes_and_the_priority_layer_match(self):
        priority_pixels = 0
        for map_id in self.MAPS:
            with self.subTest(map_id=hex(map_id)):
                decoded, _, _ = decode_map_section(self.rom, self.records[map_id])
                chunks, patterns = decoded.chunks, decoded.patterns
                cols = chunk_draw.CHUNK_TILES_X

                def all_words(chunk_id):
                    words = chunks[chunk_id]
                    return [(i % cols, i // cols, w) for i, w in enumerate(words)]

                width, height, base, _ = layouts.compose_layout(chunks, decoded.bg, patterns)
                expected = bytearray(width * height)
                reference_draw(decoded.bg, all_words, patterns, expected, width, False)
                self.assertEqual(base, expected)
                _, _, over, _ = layouts.compose_layout(chunks, decoded.fg, patterns, base)
                reference_draw(decoded.fg, all_words, patterns, expected, width, True)
                self.assertEqual(over, expected)

                by_chunk = render.priority_tiles(chunks)
                drawn = bytearray(width * height)
                expected = bytearray(width * height)
                for layout in (decoded.bg, decoded.fg):
                    render._draw_priority_plane(layout, by_chunk, patterns, drawn, width)
                    reference_draw(layout, lambda c: by_chunk.get(c, ()), patterns,
                                   expected, width, True)
                self.assertEqual(drawn, expected)
                priority_pixels += len(drawn) - drawn.count(0)

                plane = decoded.bg if decoded.spec.collision_plane else decoded.fg
                grid = layouts.decode_collision(chunks, plane)
                self.assertEqual(grid.types, bytes(
                    layouts.collision_at(chunks, plane, x, y)
                    for y in range(grid.height) for x in range(grid.width)
                ))
        self.assertGreater(priority_pixels, 0, "the maps must exercise the priority layer")


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestCallSites(unittest.TestCase):
    def test_the_find_scan_matches_the_word_walk(self):
        rom = read_rom(ROM)
        rng = random.Random(130)
        spans = [(0, 0), (5, 6), (len(rom) - 3, len(rom)), (len(rom) - 10, len(rom) + 20)]
        spans += [(s, s + rng.randrange(4000)) for s in
                  (rng.randrange(len(rom)) for _ in range(300))]
        found = 0
        for start, end in spans:
            stop = max(start, end - 3)
            expected = reference_call_sites(rom, start, stop)
            found += len(expected)
            self.assertEqual(objects._call_sites(rom, start, stop), expected, (start, end))
        self.assertGreater(found, 0, "the spans must include real call sites")


if __name__ == "__main__":
    unittest.main()
