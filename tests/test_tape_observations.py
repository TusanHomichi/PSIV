"""Frame addresses are exact and every declared product must exist."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from tools.tape_observations import load_requests, passed


class Observations(unittest.TestCase):
    def test_schema_rejects_ambiguous_addresses_and_artifact_names(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'requests.json'
            good = [{"frame": 1, "label": "state"},
                    {"frame": 1, "label": "shot", "capture": True}]
            path.write_text(json.dumps(good))
            self.assertEqual(load_requests(path), good)
            for bad in ([{"frame": True, "label": "a"}],
                        [{"frame": 1.5, "label": "a"}],
                        [{"frame": 0, "label": "a"}],
                        [{"frame": 2, "label": "a"}, {"frame": 1, "label": "b"}],
                        [{"frame": 1, "label": ".."}],
                        [{"frame": 1, "label": "a", "capture": "yes"}],
                        [good[0], good[0]]):
                path.write_text(json.dumps(bad))
                with self.subTest(bad=bad), self.assertRaises(ValueError):
                    load_requests(path)

    def test_a_pinned_manifest_refuses_another_tape(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "profile.json"
            requests = [{"frame": 1, "label": "first"}]
            path.write_text(json.dumps({"tape_sha256": "a" * 64, "requests": requests}))
            self.assertEqual(load_requests(path, "a" * 64), requests)
            with self.assertRaises(ValueError):
                load_requests(path, "b" * 64)
            path.write_text(json.dumps({"tape_sha256": None, "requests": requests}))
            with self.assertRaises(ValueError):
                load_requests(path)

    def test_negative_controls_missing_state_wrong_frame_and_missing_image_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            requests = [{"frame": 2, "label": "shot", "capture": True}]
            shot = root / "shot.png"
            result = {"label": "shot", "frame": 2, "capture": str(shot),
                      "capture_file_sha256": hashlib.sha256(b"png").hexdigest(),
                      "state": {"map": 0, "cell": [0, 0]}}
            shot.write_bytes(b'png')
            self.assertTrue(passed(requests, {"observations": [result]}, root))
            for bad in (None, {}, {"observations": []},
                        {"observations": [dict(result, frame=3)]},
                        {"observations": [dict(result, frame=2.0)]},
                        {"observations": [dict(result, state=None)]},
                        {"observations": [dict(result, state={})]},
                        {"observations": [dict(result, label="other")]}):
                self.assertFalse(passed(requests, bad, root))
            shot.write_bytes(b"corrupt")
            self.assertFalse(passed(requests, {"observations": [result]}, root))
            shot.unlink()
            self.assertFalse(passed(requests, {"observations": [result]}, root))

    def test_local_bootstrap_refuses_missing_or_stale_binary_before_validation(self):
        from unittest.mock import patch
        from tools import native_verifier
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name in native_verifier.INPUTS:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('current source')
            with patch.object(native_verifier, 'ROOT', root), \
                    patch.dict('os.environ', {}, clear=True), \
                    patch.object(native_verifier, '_exchange', return_value={}) as exchange:
                with self.assertRaisesRegex(ValueError, 'missing or stale'):
                    native_verifier.verify({'operation': 'requests'})
                exchange.assert_not_called()
                binary = root / 'rust/target/debug/psiv-native-verify'
                binary.parent.mkdir(parents=True)
                binary.touch()
                with self.assertRaisesRegex(ValueError, 'missing or stale'):
                    native_verifier.verify({'operation': 'requests'})
                exchange.assert_called_once_with(binary, {'operation': 'identity'})

    def test_prepare_rebuilds_cold_and_stale_artifacts_and_checks_result_identity(self):
        import contextlib
        import itertools
        from types import SimpleNamespace
        from unittest.mock import patch
        from tools import native_verifier
        for initial, configured in itertools.product(('cold', 'stale', 'workspace-stale'), (False, True)):
            with self.subTest(initial=initial, configured=configured), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                expected = {}
                for name in native_verifier.INPUTS:
                    path = root / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text('current source')
                    expected[name] = 'current source'
                target = root / ('configured-target' if configured else 'rust/target')
                environment = {'CARGO_TARGET_DIR': str(target)} if configured else {}
                workspace_test = target / 'debug/deps/workspace-test'
                workspace_test.parent.mkdir(parents=True)
                workspace_test.write_bytes(b'workspace executable')
                workspace_binary = target / 'debug/psiv-native-verify'
                if initial == 'workspace-stale':
                    workspace_binary.touch()
                private_target = target / 'native-verifier'
                binary = private_target / 'debug/psiv-native-verify'
                binary.parent.mkdir(parents=True)
                if initial == 'stale':
                    binary.touch()
                fresh = False
                def build(command, **kwargs):
                    nonlocal fresh
                    # A missing/wrong --target-dir reproduces the gate race:
                    # cargo clean would delete the running lane's executables.
                    self.assertIn('--target-dir', command)
                    self.assertEqual(Path(command[command.index('--target-dir') + 1]), private_target)
                    if command[:2] == ['cargo', 'clean']:
                        self.assertFalse(fresh)
                        binary.unlink(missing_ok=True)
                        return SimpleNamespace(returncode=0, stderr='', stdout='')
                    self.assertEqual(command[:3], ['cargo', 'build', '--quiet'])
                    self.assertIn('psiv-native-verify', command)
                    fresh = True
                    binary.touch()
                    return SimpleNamespace(returncode=0, stderr='', stdout=json.dumps({
                        'reason': 'compiler-artifact', 'target': {'name': 'psiv-native-verify'},
                        'executable': str(binary)}))
                with patch.object(native_verifier, 'ROOT', root), \
                        patch.dict('os.environ', environment, clear=True), \
                        patch.object(native_verifier, '_exchange', side_effect=lambda *args: expected if fresh else {}), \
                        patch.object(native_verifier.subprocess, 'run', side_effect=build) as cargo, \
                        patch('tools.heavy_lock.heavy_lock_path', return_value=root / 'lock'), \
                        patch('tools.heavy_lock.hold_heavy_lock', return_value=contextlib.nullcontext()):
                    self.assertEqual(native_verifier.prepare(), binary)
                    expected_calls = 1 if initial == 'cold' else 2
                    self.assertEqual(cargo.call_count, expected_calls)
                    self.assertEqual(native_verifier.prepare(), binary)
                    self.assertEqual(cargo.call_count, expected_calls)  # no second build
                    self.assertEqual(workspace_test.read_bytes(), b'workspace executable')
                    self.assertEqual(workspace_binary.exists(), initial == 'workspace-stale')

    def test_prepare_packaged_override_needs_no_cargo_or_local_source(self):
        from unittest.mock import patch
        from tools import native_verifier
        with patch.dict('os.environ', {'PSIV_NATIVE_VERIFY_BINARY': '/packaged/verifier'}), \
                patch.object(native_verifier, '_inputs', side_effect=AssertionError('source lookup')), \
                patch.object(native_verifier.subprocess, 'run', side_effect=AssertionError('cargo')):
            self.assertEqual(native_verifier.prepare(), Path('/packaged/verifier'))

    def test_stale_cleanup_failure_aborts_before_build(self):
        import contextlib
        from types import SimpleNamespace
        from unittest.mock import patch
        from tools import native_verifier
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / 'stale'
            binary.touch()
            with patch.object(native_verifier, '_resolve', return_value=None), \
                    patch.object(native_verifier, '_inputs', return_value={}), \
                    patch.object(native_verifier, '_candidates', return_value=[binary]), \
                    patch('tools.heavy_lock.heavy_lock_path', return_value=Path(tmp) / 'lock'), \
                    patch('tools.heavy_lock.hold_heavy_lock', return_value=contextlib.nullcontext()), \
                    patch.object(native_verifier.subprocess, 'run', return_value=SimpleNamespace(returncode=1, stderr='clean failed')) as cargo:
                with self.assertRaisesRegex(ValueError, 'cannot clean stale'):
                    native_verifier.prepare()
                cargo.assert_called_once()
                self.assertEqual(cargo.call_args.args[0][:2], ['cargo', 'clean'])
