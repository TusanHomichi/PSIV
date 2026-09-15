"""ROM-backed shop portraits cover every counter and match the shared Baker."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from psiv_tools.shop_portraits import emit_shop_portraits

ROOT = Path(__file__).resolve().parents[1]
ROM = ROOT / 'Phantasy Star IV (USA).md'


@unittest.skipUnless(ROM.is_file(), 'local retail ROM required')
class ShopPortraitTests(unittest.TestCase):
    def test_all_counter_portraits_decode_and_shared_portraits_agree(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            portraits = emit_shop_portraits(ROM.read_bytes(), root)
            by_art = {row['art']: row for row in portraits}
            self.assertEqual(len(portraits), 8)
            counters = json.loads((ROOT / 'runtime-pack/shops.json').read_text())['counters']
            self.assertTrue({row['portrait'] for row in counters} <= by_art.keys())
            for row in portraits:
                self.assertEqual(row['size_pixels'], [48, 48])
                self.assertEqual(row['cram_line'], 2)
                self.assertEqual(hashlib.sha256((root / row['png']).read_bytes()).hexdigest(), row['png_sha256'])
            for art, existing in [
                ('0x29FC66', 'dialogue/portraits/12_Baker.png'),
                ('0x29DE1E', 'presentation/portraits/shopkeeper_2.png'),
            ]:
                self.assertEqual((root / by_art[art]['png']).read_bytes(),
                                 (ROOT / 'runtime-pack' / existing).read_bytes())
