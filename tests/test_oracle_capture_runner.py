"""One capture pipeline, with constructed evidence and rejecting controls."""
import ast
import dataclasses
import hashlib
import json
import pathlib
import tempfile
import unittest
from unittest import mock

from oracle.sweep import forced_cases as runner
from oracle.sweep import arc, zelan, air_castle, endgame, event_bosses, jobs


def command_owners(paths):
    """A copied force/fixture command constructor must remain in its owner."""
    return {path.name for path in paths
            if any(isinstance(node, ast.List)
                   and any(isinstance(item, ast.Constant)
                           and item.value in ('oracle.force', 'oracle.fixture')
                           for item in node.elts)
                   for node in ast.walk(ast.parse(path.read_text())))}


class Runner(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = pathlib.Path(self.directory.name)
        self.work, self.fixtures = self.root / 'work', self.root / 'fixtures'
        self.case = runner.Case('sample', 'test', 12, 2, formation=1)
        self.calls = []

    def execute(self, argv, log):
        self.calls.append(argv)
        log.parent.mkdir(parents=True, exist_ok=True)
        log.write_text('constructed test output')
        out = pathlib.Path(argv[argv.index('--out') + 1])
        if 'oracle.force' in argv:
            report = {'tape': 'test.tape', 'battle_first': 10, 'battle_last': 20}
            for key in ('trace', 'log'):
                path = log.parent / f'{key}.csv'
                path.write_text(key)
                report[key] = str(path)
                report[key + '_sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
            (out / 'report.json').write_text(json.dumps(report))
        else:
            out.write_text(json.dumps({'rounds': [{'round': 1, 'actions': [
                {'actor': 6, 'kind': 'ability', 'ability': 12, 'targets': [{'damage': 10}]}]}]}))
        return {'command': argv, 'exit': 0, 'log': str(log)}

    def run_case(self, **kwargs):
        return runner.run_case(self.case, self.work, self.fixtures,
                               execute=self.execute, **kwargs)

    def test_capture_extract_then_hash_verified_extract_only(self):
        self.assertEqual(self.run_case()['status'], 'captured')
        self.assertEqual([a[2] for a in self.calls], ['oracle.force', 'oracle.fixture'])
        self.calls.clear()
        self.assertEqual(self.run_case(capture=False)['status'], 'captured')
        self.assertEqual([a[2] for a in self.calls], ['oracle.fixture'])
        (self.work / 'sample' / 'trace.csv').write_text('tampered')
        self.calls.clear()
        result = self.run_case(capture=False)
        self.assertEqual(result['status'], 'failed')
        self.assertIn('hash changed', result['attempts'][0]['error'])
        self.assertEqual(self.calls, [])

    def test_changed_case_cannot_reuse_capture(self):
        self.run_case()
        self.case = dataclasses.replace(self.case, rounds=3)
        self.calls.clear()
        result = self.run_case(capture=False)
        self.assertEqual(result['status'], 'failed')
        self.assertIn('case changed', result['attempts'][0]['error'])
        self.assertEqual(self.calls, [])

    def test_failed_recapture_cannot_relabel_old_evidence(self):
        self.run_case()
        self.case = dataclasses.replace(self.case, rounds=3)
        self.execute = lambda argv, log: {'exit': 2, 'command': argv}
        self.assertEqual(self.run_case()['status'], 'failed')
        self.assertFalse((self.work / 'sample' / 'case.sha256').exists())
        self.assertEqual(self.run_case(capture=False)['status'], 'failed')

    def test_rejected_extraction_preserves_committed_fixture(self):
        self.fixtures.mkdir()
        target = self.fixtures / 'sample.json'
        target.write_text('previous fixture')
        self.case = dataclasses.replace(self.case, ability=13)
        result = self.run_case()
        self.assertEqual(result['status'], 'failed')
        self.assertIn('absent', result['attempts'][0]['error'])
        self.assertEqual(target.read_text(), 'previous fixture')

    def test_delay_retries_share_scout_and_publish_only_valid_attempt(self):
        self.case = dataclasses.replace(self.case, max_delay=2)
        execute = self.execute
        def first_absent(argv, log):
            if 'oracle.force' in argv and argv[argv.index('--delay') + 1] == '0':
                self.calls.append(argv)
                return {'exit': 1, 'command': argv}
            return execute(argv, log)
        self.execute = first_absent
        result = self.run_case()
        self.assertEqual(result['status'], 'captured')
        self.assertEqual([a['delay'] for a in result['attempts']], [0, 1])
        for argv in self.calls:
            if 'oracle.force' in argv:
                self.assertEqual(argv[argv.index('--scout') + 1], str(self.work / 'scout.json'))
        self.assertIn('delay-1', result['report'])
        self.calls.clear()
        self.assertEqual(self.run_case(capture=False)['status'], 'captured')
        self.assertEqual([a[2] for a in self.calls], ['oracle.fixture'])

    def test_infrastructure_failure_stops_retry(self):
        self.case = dataclasses.replace(self.case, max_delay=2)
        self.execute = lambda argv, log: {'exit': 2, 'command': argv}
        result = self.run_case()
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(len(result['attempts']), 1)

    def test_kept_damage_and_lane_guards(self):
        fixture = {'rounds': [{'round': 1, 'actions': [
            {'actor': 6, 'ability': 12, 'kind': 'ability', 'targets': []}]}]}
        case = dataclasses.replace(self.case, damage=True)
        with self.assertRaisesRegex(ValueError, 'damage ability absent'):
            runner.validate(case, fixture)
        fixture['rounds'][0]['actions'][0]['targets'] = [{'damage': 1}]
        runner.validate(case, fixture)
        with self.assertRaisesRegex(ValueError, 'outside'):
            runner.validate(dataclasses.replace(case, allowed=(13,)), fixture)

    def test_flat_player_cases_use_same_schema(self):
        path = self.root / 'cases.json'
        path.write_text(json.dumps([{'name': 'party', 'event': 1,
                                     'rounds': [{'1': {'command': 'defend'}}]}]))
        case, = runner.load_cases(path)
        self.assertEqual(case.rounds, 1)
        self.assertFalse(case.durable)
        self.assertFalse(case.script['repeat_last'])
        path.write_text(json.dumps([{'name': '../escape', 'formation': 1, 'rounds': 1}]))
        with self.assertRaisesRegex(ValueError, 'basenames'):
            runner.load_cases(path)
        path.write_text(json.dumps([{'name': 'same', 'formation': 1, 'rounds': 1}] * 2))
        with self.assertRaisesRegex(ValueError, 'distinct'):
            runner.load_cases(path)

    def test_invalid_keep_and_scripted_event_delay_search_are_refused(self):
        with self.assertRaises(ValueError):
            dataclasses.replace(self.case, keep=0)
        event = dataclasses.replace(self.case, formation=None, event=1, max_delay=2)
        # Fixed-policy event tapes can shift; a menu-driven script absorbs seam delays.
        with self.assertRaisesRegex(ValueError, 'scripted event'):
            dataclasses.replace(event, script={'rounds': []})

    def test_every_named_recipe_uses_the_same_runner(self):
        for module in (arc, zelan, air_castle, endgame, event_bosses):
            with self.subTest(module=module.__name__), mock.patch.object(module, 'run_cases', return_value=0) as run:
                self.assertEqual(module.main(['--list']), 0)
                self.assertIsInstance(run.call_args.args[0][0], runner.Case)
        with mock.patch.object(runner, 'run_case', return_value={
                'status': 'failed', 'attempts': [{'stage': 'capture', 'commands': {}, 'error': 'test'}]}) as run:
            entry = mock.Mock(formation=1, vehicle=None, as_json=lambda: {'formation': 1})
            jobs.run_formation(entry, jobs.Options(self.work, self.fixtures))
            run.assert_called_once()

    def test_only_owner_constructs_force_and_fixture_commands(self):
        directory = runner.ROOT / 'oracle/sweep'
        self.assertEqual(command_owners(directory.glob('*.py')), {'forced_cases.py'})
        duplicate = self.root / 'fifth_runner.py'
        duplicate.write_text('argv = ["python3", "-m", "oracle.force"]\n')
        self.assertEqual(command_owners([duplicate]), {'fifth_runner.py'})
        self.assertFalse((directory / 'player_capture.py').exists())
        self.assertFalse((directory / 'capture_route.py').exists())
        self.assertFalse(hasattr(arc, 'run_recipe'))


if __name__ == '__main__':
    unittest.main()
