"""Ship caption decoding and the tape-35 field-control measurement boundary."""

from pathlib import Path
import unittest

from oracle.ship_flight import measure

ROOT = Path(__file__).resolve().parent.parent




class ControlBoundaryCase(unittest.TestCase):
    @staticmethod
    def row(frame, map_id, mode, routine):
        return dict(frame=str(frame), map_index=f"{map_id:04X}",
                    game_mode=f"{mode:04X}", game_mode_routine=f"{routine:04X}")

    def test_landing_write_reload_and_place_name_do_not_count_as_control(self):
        rows = [self.row(7401, 0xBF, 12, 12), self.row(9053, 0x18D, 12, 12),
                self.row(9086, 0x18D, 8, 0), self.row(9138, 0x18D, 12, 28),
                self.row(9262, 0x18D, 12, 0)]
        self.assertEqual(measure(rows, 0x18D)["frames_to_control"], 1861)
        # Negative control: the tape must actually reach field control.
        with self.assertRaisesRegex(ValueError, "without field control"):
            measure(rows[:-1], 0x18D)


if __name__ == "__main__":
    unittest.main()
