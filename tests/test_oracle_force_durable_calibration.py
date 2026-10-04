"""A forced formation's observed load frame outranks the probe estimate."""
import unittest

from oracle.force.durable import Durable, align_to_capture
from oracle.force.errors import ForceError


class DurableCalibrationTests(unittest.TestCase):
    def test_alignment_moves_only_hp_cells_and_updates_the_report_frame(self):
        layout = {"hp": {"addr": "FFFF1000", "size": 2},
                  "maxhp": {"addr": "FFFF1002", "size": 2}}
        durable = Durable(999, 22, ["hp", "maxhp"], [])
        original = ["19:FFFFEF0C:001C", "21:FFFFEC28:0015"] + durable.specs(layout)
        corrected = align_to_capture(durable, original, layout, 21)
        self.assertEqual(corrected[:2], original[:2])
        self.assertEqual(corrected[2:], ["21:FFFF1000:03E7", "21:FFFF1002:03E7"])
        self.assertEqual(durable.report()["frame"], 21)
        self.assertNotEqual(corrected, original, "negative control: late patches differ")

    def test_an_incomplete_patch_list_cannot_be_silently_calibrated(self):
        durable = Durable(999, 22, ["hp"], [])
        with self.assertRaisesRegex(ForceError, "every planned HP cell"):
            align_to_capture(durable, [], {"hp": {"addr": "FFFF1000", "size": 2}}, 21)
