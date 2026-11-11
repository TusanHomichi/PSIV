"""Both default gates provision native verification before Python workers start."""
import io
import os
from pathlib import Path
import unittest
from unittest.mock import patch

from tools.py_suite import SLOW_PYTHON_MODULES


class NativeVerifierPrerequisite(unittest.TestCase):
    def test_both_default_gate_suites_prepare_before_workers_and_restore_environment(self):
        from tools import py_suite
        from tools.gate import FAST_COMMANDS, GATE_COMMANDS
        for commands in (FAST_COMMANDS, GATE_COMMANDS):
            command = next(c for c in commands if 'tools/py_suite.py' in c)
            argv = ['--fast'] if command.endswith('--fast') else []
            for initial in ('cold', 'stale'):
                with self.subTest(command=command, initial=initial):
                    events = []
                    def prepare():
                        events.append(initial)
                        return Path('/prepared/native-verifier')
                    def suite(*args, **kwargs):
                        self.assertEqual(events, [initial])
                        self.assertEqual(os.environ['PSIV_NATIVE_VERIFY_BINARY'], str(Path('/prepared/native-verifier')))
                        self.assertEqual(args[1], SLOW_PYTHON_MODULES if argv else ())
                        events.append('workers')
                        return 0
                    with patch.dict(os.environ, {'PSIV_NATIVE_VERIFY_BINARY': 'caller'}), \
                            patch('tools.native_verifier.prepare', side_effect=prepare), \
                            patch.object(py_suite, 'run_suite', side_effect=suite):
                        self.assertEqual(py_suite.main(argv), 0)
                        self.assertEqual(os.environ['PSIV_NATIVE_VERIFY_BINARY'], 'caller')
                    self.assertEqual(events, [initial, 'workers'])

    def test_failed_prerequisite_fails_before_creating_workers(self):
        from tools import py_suite
        for argv in ([], ['--fast']):
            with self.subTest(argv=argv), patch('tools.native_verifier.prepare', side_effect=ValueError('build failed')), \
                    patch.object(py_suite, 'run_suite') as suite, patch('sys.stderr', new_callable=io.StringIO) as output:
                self.assertEqual(py_suite.main(argv), 1)
                suite.assert_not_called()
                self.assertIn('prerequisite failed: build failed', output.getvalue())
