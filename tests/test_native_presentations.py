"""The restored presentation comparisons reject changed pixels and state."""
import contextlib
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest

import numpy as np
from PIL import Image

from tools.native import verify_native_alarm as alarm
from tools.native import verify_native_zema as zema


class Alarm(unittest.TestCase):
    def fixture(self, root):
        shots = root / 'observations'
        shots.mkdir()
        rows = []
        for phase, direction in enumerate(('out', 'in')):
            for stage in range(1, 9):
                label = f'alarm-{direction}-{stage}'
                level = min(7, stage) if phase else max(0, 7 - stage)
                picture = np.array([[[238, alarm.GREEN[level], alarm.BLUE[level]]]], dtype=np.uint8)
                Image.fromarray(picture).save(shots / f'{label}.png')
                rows.append({'label': label, 'frame': 100 + (phase * 8 + stage - 1) * 4, 'state': {}})
        (root / 'native.json').write_text(json.dumps({'observations': rows}))
        return rows

    def run_check(self, root):
        with self.assertRaises(SystemExit) as status, contextlib.redirect_stdout(io.StringIO()):
            alarm.main([str(root)])
        return status.exception.code

    def test_component_ramps_and_four_frame_cadence_with_negative_controls(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            rows = self.fixture(root)
            self.assertEqual(self.run_check(root), 0)
            rows[1]['frame'] += 1
            (root / 'native.json').write_text(json.dumps({'observations': rows}))
            self.assertEqual(self.run_check(root), 1)
            rows[1]['frame'] -= 1
            (root / 'native.json').write_text(json.dumps({'observations': rows}))
            Image.new('RGB', (1, 1), (0, 0, 0)).save(root / 'observations/alarm-out-1.png')
            self.assertEqual(self.run_check(root), 1)

    def test_negative_control_blank_captures_cannot_prove_a_palette_ramp(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.fixture(root)
            for path in (root / "observations").glob("*.png"):
                Image.new("RGB", (1, 1), (0, 0, 0)).save(path)
            with self.assertRaisesRegex(ValueError, "no green or blue"):
                alarm.main([str(root)])


class Zema(unittest.TestCase):
    def fixture(self, root):
        (root / 'maps').mkdir()
        words = list(range(16))
        effect = {'at': '0x051E56', 'words_hex': [hex(n) for n in words],
                  'affects': {'npc_sheets': [{'npc_index': n, 'to': f'normal-{n}'} for n in range(7)]}}
        (root / 'maps/024_Zema.json').write_text(json.dumps(
            {'map_effects': [{'paths': [{'deferred_effects': [effect]}]}]}))
        shot = root / 'shot.png'
        Image.new('RGB', (1280, 800), (238, 0, 0)).save(shot)
        rows = [{'label': f'zema-page-{n:02d}', 'frame': n,
                 'capture': str(shot), 'state': {'map': 0x24, 'dialogue_page':
                 {'ready': True, 'tree': 105, 'lines': [str(n)]}}} for n in range(1, 51)]
        for label, y in [('igglanova-halfway', 176), ('igglanova-arrived', 192)]:
            rows.append({'label': label, 'capture': str(shot), 'state': {'map': 0x24,
                         'temporary_objects': [{'id': 0x188, 'position': [480, y]}]}})
        rows.append({'label': 'zema-restored', 'capture': str(shot), 'state': {
            'map': 0x24, 'flags': [0x37], 'active_npcs': list(range(7)),
            'cram': {'live': True, 'words': [0]*48 + words},
            'npc_sheets': [{'index': n, 'sheet': f'normal-{n}'} for n in range(7)]}})
        return {'observations': rows}

    def test_pages_entrance_and_resident_palette_with_negative_controls(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            good = self.fixture(root)
            self.assertTrue(zema.verify(good, root)['pass'])
            mutations = [
                lambda rows: rows.pop(0),
                lambda rows: rows[0]['state']['dialogue_page'].update(ready=False),
                lambda rows: rows[1].update(frame=1),
                lambda rows: rows[50]['state']['temporary_objects'][0].update(position=[480, 175]),
                lambda rows: rows[-1]['state']['cram']['words'].__setitem__(49, 0),
                lambda rows: rows[-1]['state']['cram'].update(live=False),
                lambda rows: rows[-1]['state']['npc_sheets'][0].update(sheet='petrified'),
            ]
            for mutate in mutations:
                bad = copy.deepcopy(good)
                mutate(bad['observations'])
                with self.assertRaises((ValueError, KeyError)):
                    zema.verify(bad, root)
