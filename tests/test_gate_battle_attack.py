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

    def test_sat_order_wins_before_priority_partition_and_transparency_does_not_claim(self):
        from psiv_tools.battle_animation_art import _render_attack_mapping
        front = bytearray([1] * 64)
        front[0] = 0
        tiles = [bytes(front), bytes([2] * 64)]
        palette = [(index, 0, 0) for index in range(16)]
        palette[1] = (0, 0, 0)  # Opaque index 1 still owns its pixels, even when RGB is black.
        for first_priority in (False, True):
            entries = [{"tile_index": index, "width_tiles": 1, "height_tiles": 1,
                        "h_flip": False, "v_flip": False, "x": 0, "y": 0,
                        "priority": priority}
                       for index, priority in [(0, first_priority), (1, not first_priority)]]
            record = {"entries": entries}
            def pixels(priority):
                args = (tiles, record, (0, 0, 8, 8), (0, 0), palette)
                image = _render_attack_mapping(*args) if priority is None else _render_attack_mapping(*args, priority)
                return png_pixels(image)[2]
            with self.subTest(first_priority=first_priority):
                # The existing canonical-render API must get SAT ownership
                # right, independently of the newly added split surfaces.
                combined = pixels(None)
                self.assertEqual(list(combined), [2] + [1] * 63)
                low, high = pixels(False), pixels(True)
                first_surface, later_surface = (high, low) if first_priority else (low, high)
                self.assertEqual(list(first_surface), [0] + [1] * 63)
                self.assertEqual(list(later_surface), [2] + [0] * 63)
                self.assertTrue(all(not (a and b) for a, b in zip(low, high)))
                self.assertEqual(bytes(a or b for a, b in zip(low, high)), combined)
