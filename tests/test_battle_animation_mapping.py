"""#364: a battle mapping entry's size byte and tile order, the VDP's way.

The VDP keeps a sprite's horizontal tile count in bits 3-2 of its size byte
and the vertical one in bits 1-0, and its tiles run down each column first.
The decoder read the two counts the other way round and the attack renderer
laid the tiles out row by row, so every non-square entry drew wrong (2,453 of
3,644 in the pack). No ROM needed: the records and tiles are synthetic.
"""

from __future__ import annotations

import io
import unittest

from PIL import Image

from psiv_tools.battle_animation_art import _render_attack_mapping
from psiv_tools.battle_animation_mapping import (
    decode_mapping_record,
    vdp_sprite_size,
    vdp_sprite_tile,
)


def record(size: int, tile: int = 0) -> bytes:
    """One-entry mapping record at offset 0: y 0, `size`, tile word, x 0."""
    return bytes([0, 0, size, tile >> 8, tile & 0xFF, 0, 0])


class SizeByte(unittest.TestCase):
    def test_columns_are_bits_3_2_and_rows_bits_1_0(self):
        self.assertEqual(vdp_sprite_size(0x09), (3, 2))
        self.assertEqual(vdp_sprite_size(0x06), (2, 3))
        self.assertEqual(vdp_sprite_size(0x0C), (4, 1))
        self.assertEqual(vdp_sprite_size(0x00), (1, 1))

    def test_the_decoder_reads_a_non_square_entry_the_vdp_way(self):
        entry = decode_mapping_record(record(0x09, 0x010), 0)["entries"][0]
        self.assertEqual((entry["width_tiles"], entry["height_tiles"]), (3, 2))
        self.assertEqual(entry["tile_span_end"], 0x010 + 6)
        tall = decode_mapping_record(record(0x03), 0)["entries"][0]
        self.assertEqual((tall["width_tiles"], tall["height_tiles"]), (1, 4))

    def test_tiles_run_down_each_column_first(self):
        # A 3x2 sprite: column 1, row 0 is the third pattern, not the second.
        self.assertEqual(vdp_sprite_tile(0x20, 1, 0, 2), 0x22)
        self.assertEqual(vdp_sprite_tile(0x20, 0, 1, 2), 0x21)
        self.assertEqual(vdp_sprite_tile(0x20, 2, 1, 2), 0x25)


class AttackRenderer(unittest.TestCase):
    def test_a_three_by_two_entry_draws_its_tiles_column_major(self):
        # Tile n is filled with colour n + 1, so each 8x8 cell names its tile.
        tiles = [bytes([n + 1] * 64) for n in range(6)]
        entry = decode_mapping_record(record(0x09), 0)["entries"][0]
        image = _render_attack_mapping(
            tiles,
            {"entries": [entry]},
            (0, 0, 24, 16),
            (0, 0),
            [(n * 16, n * 16, n * 16) for n in range(16)],
        )
        decoded = Image.open(io.BytesIO(image))
        self.assertEqual(decoded.size, (24, 16))
        cells = [[decoded.getpixel((column * 8, row * 8)) - 1 for column in range(3)]
                 for row in range(2)]
        self.assertEqual(cells, [[0, 2, 4], [1, 3, 5]])


if __name__ == "__main__":
    unittest.main()
