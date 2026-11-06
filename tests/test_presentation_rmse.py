"""The presentation comparison's crop (#128), without Godot.

    PYTHONPATH=. python3 -m unittest tests.test_presentation_rmse -v
"""

import unittest

from psiv_tools.presentation_rmse import PngError, crop, rmse


def frame(fill=(0, 0, 0)):
    return bytes(fill) * (320 * 224)


def paint(rgb, x, y, colour):
    out = bytearray(rgb)
    start = (y * 320 + x) * 3
    out[start:start + 3] = bytes(colour)
    return bytes(out)


class CropTest(unittest.TestCase):
    REGION = (144, 104, 112, 96)

    def test_a_difference_outside_the_region_does_not_count(self):
        oracle = frame()
        godot = paint(oracle, 10, 10, (255, 0, 0))
        self.assertGreater(rmse(godot, oracle), 0)
        self.assertEqual(rmse(crop(godot, self.REGION), crop(oracle, self.REGION)), 0)

    def test_a_difference_inside_the_region_counts(self):
        oracle = frame()
        for x, y in [(144, 104), (255, 199), (200, 150)]:
            with self.subTest(pixel=(x, y)):
                godot = paint(oracle, x, y, (255, 0, 0))
                self.assertGreater(rmse(crop(godot, self.REGION), crop(oracle, self.REGION)), 0)

    def test_the_crop_is_the_rectangle(self):
        cropped = crop(paint(frame(), 144, 104, (1, 2, 3)), self.REGION)
        self.assertEqual(len(cropped), 112 * 96 * 3)
        self.assertEqual(cropped[:3], bytes((1, 2, 3)))

    def test_a_region_outside_the_frame_is_refused(self):
        for region in [(300, 0, 40, 10), (0, 0, 0, 10), (-1, 0, 10, 10), (0, 220, 10, 10)]:
            with self.subTest(region=region):
                with self.assertRaises(PngError):
                    crop(frame(), region)


if __name__ == "__main__":
    unittest.main()
