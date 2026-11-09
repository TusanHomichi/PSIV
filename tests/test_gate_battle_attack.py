"""ROM-free VDP attack regressions, discovered by the base flake's gate-tests.

The public mapping decoder must preserve non-square hardware sprite shapes;
rendered patterns run down columns, including when both axes are flipped.
Synthetic records/patterns only: these checks run without local game inputs.
"""
import unittest

from tests.test_battle_art_pack import png_pixels


class TestAttackMapping(unittest.TestCase):
    def test_vdp_size_bits_keep_width_and_height_in_cartridge_order(self):
        from psiv_tools.battle_animation_mapping import decode_mapping_record
        for size, expected in [(1, (1, 2)), (4, (2, 1)), (6, (2, 3))]:
            record = decode_mapping_record(bytes([0, 0, size, 0, 0, 0, 0]), 0)
            entry = record["entries"][0]
            self.assertEqual((entry["width_tiles"], entry["height_tiles"]), expected)
            self.assertNotEqual(expected, tuple(reversed(expected)), "non-square control")

    def test_vdp_patterns_run_down_columns_before_the_next_column(self):
        from psiv_tools.battle_animation_art import _render_attack_mapping
        entry = {"tile_index": 0, "width_tiles": 2, "height_tiles": 3,
                 "h_flip": False, "v_flip": False, "x": 0, "y": 0}
        tiles = [bytes([index]) * 64 for index in range(1, 7)]
        palette = [(index, 0, 0) for index in range(16)]
        image = _render_attack_mapping(tiles, {"entries": [entry]}, (0, 0, 16, 24), (0, 0), palette)
        width, height, pixels = png_pixels(image)
        self.assertEqual((width, height), (16, 24))
        self.assertEqual([pixels[y * width + x] for y in [0, 8, 16] for x in [0, 8]],
                         [1, 4, 2, 5, 3, 6])
        entry.update(h_flip=True, v_flip=True)
        image = _render_attack_mapping(tiles, {"entries": [entry]}, (0, 0, 16, 24), (0, 0), palette)
        _, _, flipped = png_pixels(image)
        self.assertEqual([flipped[y * width + x] for y in [0, 8, 16] for x in [0, 8]],
                         [6, 3, 5, 2, 4, 1])
