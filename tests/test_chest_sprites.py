"""Original chest mappings, palette selection and item-number binding."""
import unittest
from pathlib import Path
from psiv_tools.chest_sprites import chest_sprite
from psiv_tools.maps import extract_maps
from psiv_tools.pack import _treasure_chests
from psiv_tools.layouts import decode_map_palette
from psiv_tools.gfx import palette_rgb

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"

@unittest.skipUnless(ROM.is_file(), "private US cartridge required")
class ChestSpritesTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = ROM.read_bytes()
        cls.maps = extract_maps(cls.rom)["maps"]

    def test_both_lids_keep_the_mapping_offsets_and_fixed_palette(self):
        normal = chest_sprite(self.rom, [])
        white = chest_sprite(self.rom, [], white=True)
        self.assertEqual((normal.box.left, normal.box.top, normal.box.width,normal.box.height), (0,8,16,24))
        self.assertEqual((white.box.left,white.box.top,white.box.width,white.box.height),(-4,6,24,24))
        for sheet, closed, opened in ((normal,0x5144A,0x51450),(white,0x51456,0x5145C)):
            self.assertEqual(sheet.palette_line,2)
            self.assertEqual(sheet.sequences['idle_down'].sequence_offset,closed)
            self.assertEqual(sheet.sequences['idle_up'].sequence_offset,opened)
            self.assertEqual(sheet.sequences['idle_down'].durations,(16,))
            self.assertNotEqual(sheet.frames[0].pixels,sheet.frames[1].pixels)

    def test_map_family_32_selects_its_own_art_and_palette(self):
        record = next(r for r in self.maps if not r['is_null'] and r['general_var']==0x32 and r['treasure_chests']['entries'])
        palette=palette_rgb(decode_map_palette(self.rom,int(record['palette']['pointer'],16)))
        special=chest_sprite(self.rom,palette,general_var=0x32)
        self.assertEqual(special.palette_line,1)
        self.assertEqual(list(special.palette),palette[16:32])
        self.assertNotEqual(special.frames[0].pixels,chest_sprite(self.rom,[]).frames[0].pixels)

    def test_item_id_is_one_based_and_story_chest_is_alshline(self):
        chests = _treasure_chests(self.maps[0x4A])
        alshline = next(c for c in chests if c['chest_flag']==8)
        self.assertEqual(alshline['item_id'],141)
        self.assertEqual(alshline['item_symbol'],'Alshline')
        self.assertEqual((alshline['x_cell'],alshline['y_cell']),(40,32))
