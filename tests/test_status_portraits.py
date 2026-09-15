"""All original STATUS maps resolve, with age words read from the cartridge."""
from pathlib import Path
import tempfile
import unittest

from psiv_tools.status_portraits import character_age, emit_status_portraits

ROOT = Path(__file__).resolve().parents[1]
ROM = ROOT / "Phantasy Star IV (USA).md"


@unittest.skipUnless(ROM.is_file(), "local retail ROM required")
class StatusPortraitTests(unittest.TestCase):
    def test_all_original_portraits_and_age_words(self):
        rom = ROM.read_bytes()
        self.assertEqual([character_age(rom, i) for i in range(11)],
                         [16, 0, 24, 0, 19, 1, 324, 998, 85, 18, 39])
        with tempfile.TemporaryDirectory() as temp:
            records = emit_status_portraits(rom, Path(temp))
            self.assertEqual(len(records), 11)
            self.assertEqual(len({r["png_sha256"] for r in records}), 11)
            for row in records:
                self.assertEqual(row["size_pixels"], [80, 80])
                self.assertEqual(row["screen_pixels"], [24, 16])
                self.assertGreater((Path(temp) / row["png"]).stat().st_size, 500)
