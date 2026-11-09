"""The grave oracle checker rejects a changed blocking or animation boundary."""

import unittest

from oracle.tyler_grave import measure


def trace():
    rows = []
    for frame in range(7199, 7331):
        update = min(max(frame - 7203, 0), 64)
        index = 0 if update <= 4 else 1 if update <= 9 else 2 if update <= 14 else 3
        sliding = 7204 <= frame <= 7267
        restored = frame >= 7268
        rows.append(dict(frame=str(frame), map_index="0120", c1_x_px="304",
                         c1_y_px="432", cam_x_fg_px=str(156 if frame in (7200, 7268) else 152),
                         cam_y_fg_px="344",
                         game_mode_routine="0000" if frame in (7199, 7330) else "000C",
                         o02_x_px=str(288 + (-update // 4)), o03_x_px=str(352 + update // 2),
                         o02_facing=str(8 if restored else 16 if sliding else 0),
                         o03_facing=str(12 if restored else 20 if sliding else 4),
                         o02_map_idx=str(index if sliding else 0),
                         o03_map_idx=str(index if sliding else 0)))
    return rows


class TylerGraveOracleTests(unittest.TestCase):
    def test_accepts_recorded_boundaries(self):
        self.assertEqual(measure(trace())["frames_through_return"], 131)

    def test_rejects_early_animation_and_missing_pan_or_wait(self):
        for frame, key, value in [(7207, "o02_map_idx", "1"),
                                  (7200, "cam_x_fg_px", "152"),
                                  (7329, "game_mode_routine", "0000")]:
            with self.subTest(frame=frame, key=key):
                rows = trace()
                rows[frame - 7199][key] = value
                with self.assertRaises(ValueError):
                    measure(rows)
