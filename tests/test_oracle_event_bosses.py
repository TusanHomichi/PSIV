"""Early event-boss recipes retain the actions their acceptance requires."""
import copy
import json
import unittest

from oracle.sweep import event_bosses
from oracle.sweep.player_capture import patches
from oracle.sweep.forced_cases import FIXTURE_ROOT, observed


class EventBosses(unittest.TestCase):
    def test_committed_captures_show_every_required_enemy_action(self):
        for case in event_bosses.CASES:
            with self.subTest(case=case.name):
                fixture = json.loads((FIXTURE_ROOT / 'event_bosses' /
                                      f'{case.name}.json').read_text())
                self.assertIsNotNone(observed(case, fixture))
                self.assertEqual(len(fixture['rounds']), case.kept)
                self.assertEqual(fixture['formation']['event_battle_index'], case.event)
                self.assertIsNotNone(case.script)
                # A required ability selected only by a party member does not
                # certify the enemy's arm. This is the negative control.
                missing = copy.deepcopy(fixture)
                for round_ in missing['rounds']:
                    for action in round_['actions']:
                        if action.get('ability') == case.ability:
                            action['actor'] = 1
                self.assertIsNone(observed(case, missing))

    def test_inventory_fixture_is_bounded_and_explicit(self):
        raw = bytearray(65536)
        raw[0xF40A:0xF40F] = bytes([255] * 5)
        case = {"rounds": [], "inventory": [0x39]}
        self.assertEqual(patches(case, bytes(raw), 10),
                         ["10:FFFFF410:39" + "00" * 39])
        for inventory in ([1] * 41, [-1], [256], [True], "39"):
            with self.subTest(inventory=inventory), self.assertRaises(ValueError):
                patches({"rounds": [], "inventory": inventory}, bytes(raw), 10)

    def test_zio_runs_the_full_phase_sequence_in_order(self):
        fixture = json.loads((FIXTURE_ROOT / 'event_bosses/zio_phases.json').read_text())
        actions = [a['ability'] for r in fixture['rounds'] for a in r['actions']
                   if a['actor'] > 5]
        self.assertEqual(actions, [0x6B, 0x53, 0x54])
        self.assertEqual(fixture['formation']['priority'], 255)
        self.assertEqual(fixture['rounds'][0]['commands'], [])

    def test_psycho_wand_executes_and_changes_zios_form(self):
        fixture = json.loads((FIXTURE_ROOT / 'event_bosses/zio_psycho.json').read_text())
        items = [a for r in fixture['rounds'] for a in r['actions']
                 if a['actor'] <= 5 and a['kind'] == 'item']
        self.assertEqual([a['ability'] for a in items], [0x39])
        self.assertEqual(fixture['formation']['enemies'][0]['enemy_id'], 139)
        self.assertIn(140, [s.get('enemy_id') for r in fixture['rounds']
                            for s in r['state_after'] if s['id'] > 5])


if __name__ == '__main__':
    unittest.main()
