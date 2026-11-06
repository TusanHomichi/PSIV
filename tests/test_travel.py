"""Travel data follows ROM entry direction and world-specific town flag banks."""
from pathlib import Path
import unittest
from psiv_tools.travel import MESSAGES, decode_message, extract_travel

ROM = Path(__file__).resolve().parents[1] / 'Phantasy Star IV (USA).md'

@unittest.skipUnless(ROM.is_file(), 'local retail ROM required')
class TravelTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = ROM.read_bytes()
        cls.travel = extract_travel(cls.rom)

    def test_destinations_and_aliases_keep_original_eight_pixel_units(self):
        towns = self.travel['towns']
        self.assertEqual(len(towns), 26)
        piata = towns[0]
        self.assertEqual((piata['name'], piata['previous_map'], piata['x'], piata['y']),
                         ('PIATA', 0x10, 0x5C, 0x11E))
        self.assertEqual(towns[13]['rom_offset'], towns[16]['rom_offset'])
        self.assertEqual(towns[-1]['town_flag'], 25)
        self.assertEqual(towns[-1]['world'], 2)
        self.assertEqual(len(self.travel['dungeons']), 34)
        self.assertEqual(self.travel['dungeons'][5]['map'], 0x42)
        self.assertEqual(self.travel['dungeons'][11]['map'], 0x12)

    def test_maze_entrances_choose_opposite_hinas_returns(self):
        entries = self.travel['entries']
        self.assertEqual(len(entries), 62)
        entrances = [e for e in entries if e['place_id'] == 7]
        self.assertEqual([(e['map'], e['previous_map'], e['selector']) for e in entrances],
                         [(0x9B, 0xD8, 0x83), (0xA1, 0xD9, 0x84)])
        first, second = [self.travel['dungeons'][i] for i in (3, 4)]
        self.assertEqual((first['x'], first['y']), (0x100, 0xF2))
        self.assertEqual((second['x'], second['y']), (0x128, 0xBA))

    def test_wrong_table_cannot_produce_a_plausible_pack(self):
        rom = bytearray(self.rom)
        rom[0x6115A] = 99
        with self.assertRaisesRegex(ValueError, 'travel table guard'):
            extract_travel(bytes(rom))

    def test_the_teleport_strings_come_from_the_window_charset(self):
        messages = self.travel['messages']
        self.assertEqual(sorted(messages), sorted(key for key, _ in MESSAGES))
        # The town list's label is one three-cell word; the ready message is
        # two lines split at its `$FC`; the refusals are one line each.
        self.assertEqual(len(messages['towns_label']), 3)
        self.assertEqual(len(messages['teleported'].split('\n')), 2)
        self.assertTrue(all(line for line in messages['teleported'].split('\n')))
        for key in ('cannot_be_used', 'not_enough_tp'):
            self.assertNotIn('\n', messages[key])
        # The `$7A $79` tile pair reads as the two letters it draws.
        self.assertTrue(messages['not_enough_tp'].endswith('TP!'))

    def test_a_window_string_without_its_terminator_is_refused(self):
        offset = MESSAGES[0][1]
        rom = bytearray(self.rom)
        rom[offset:offset + 64] = bytes([1]) * 64
        with self.assertRaisesRegex(ValueError, 'no window string terminator'):
            decode_message(bytes(rom), offset)
        rom[offset] = 0x7B
        rom[offset + 1] = 0xFE
        with self.assertRaises(ValueError):
            decode_message(bytes(rom), offset)
