"""Policy prerequisites are resolved before narrow heavy reservations."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

from tools import memory_policy, verify_native_route, verify_native_tape
from tools.heavy_lock import GODOT, PYTHON, hold_heavy_lock


@unittest.skipIf(os.name == "nt", "fixture executables use POSIX shebangs")
class ExistingArtifactTests(unittest.TestCase):
    def test_fresh_process_reuses_current_binary_under_each_narrow_holder(self):
        self.check_artifact(stale=False)

    def test_fresh_process_refuses_stale_binary_under_each_narrow_holder(self):
        self.check_artifact(stale=True)

    def check_artifact(self, stale):
        for lock_class in (GODOT, PYTHON):
            with self.subTest(lock_class=lock_class), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                target = root / "target"
                binary = target / "debug/psiv-memory-verdict"
                binary.parent.mkdir(parents=True)
                expected = memory_policy._inputs(memory_policy.ROOT)
                if stale:
                    expected["rust/psiv-memory-verdict/src/lib.rs"] = "old source"
                binary.write_text(f"#!{sys.executable}\nimport json, sys\n"
                                  f"identity = {expected!r}\n"
                                  "request = json.load(sys.stdin)\n"
                                  "print(json.dumps(identity if request.get('operation') == "
                                  "'identity' else {'static_available': True}))\n")
                binary.chmod(0o755)
                cargo = root / "cargo"
                cargo.write_text(f"#!{sys.executable}\nimport json, sys\n"
                                 "assert sys.argv[1] == 'metadata', sys.argv\n"
                                 f"print(json.dumps({{'target_directory': {str(target)!r}}}))\n")
                cargo.chmod(0o755)
                env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ["PATH"])
                env.pop("PSIV_MEMORY_VERDICT_BINARY", None)
                lock = root / "heavy.lock"
                env["PSIV_HEAVY_LOCK"] = str(lock)
                env.pop("PSIV_HEAVY_LOCK_HELD", None)
                with hold_heavy_lock(lock, environ=env, lock_class=lock_class):
                    result = subprocess.run(
                        [sys.executable, "-c", "from tools.memory_policy import memory_policy; "
                         "import json; print(json.dumps(memory_policy({'operation': 'prepare'})))"],
                        env=env, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                response = json.loads(result.stdout)
                if stale:
                    self.assertIn("missing or stale", response["error"])
                else:
                    self.assertEqual(response, {"static_available": True})


class StartupTests(unittest.TestCase):
    def test_identity_covers_every_policy_source_file(self):
        source = memory_policy.ROOT / "rust/psiv-memory-verdict/src"
        expected = {str(path.relative_to(memory_policy.ROOT)) for path in source.rglob("*.rs")}
        declared = {name for name in memory_policy.BUILD_INPUTS if name.endswith(".rs")}
        self.assertEqual(declared, expected)

    def test_stale_artifact_cannot_bypass_the_build_reservation(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary = root / "debug/psiv-memory-verdict"
            binary.parent.mkdir()
            binary.touch()
            metadata = subprocess.CompletedProcess([], 0, json.dumps({"target_directory": temp}), "")
            with mock.patch.dict(os.environ, {}, clear=True), \
                    mock.patch.object(memory_policy, "_inputs", return_value={"source": "new"}), \
                    mock.patch.object(memory_policy, "_policy_binary", None), \
                    mock.patch.object(memory_policy, "_current", return_value=False), \
                    mock.patch.object(memory_policy.subprocess, "run", return_value=metadata), \
                    mock.patch("tools.heavy_lock.hold_heavy_lock", side_effect=SystemExit(2)) as hold:
                with self.assertRaisesRegex(ValueError, "missing or stale"):
                    memory_policy._resolve(root)
                self.assertEqual(hold.call_args.kwargs["lock_class"], "build")

    def test_tape_and_route_prepare_before_their_godot_hold(self):
        for module, operation, argv in (
            (verify_native_tape, "replay", ["--tape", "unused", "--out", "unused", "--rss-bound-mb", "8"]),
            (verify_native_route, "verify_route", ["--run-dir", "unused", "--out", "unused", "--rss-bound-mb", "8"]),
        ):
            events = []
            def prepare():
                events.append("prepare")
            def hold(*args, **kwargs):
                events.append("hold")
                return mock.MagicMock()
            with self.subTest(module=module.__name__), \
                    mock.patch.object(verify_native_tape, "prepare_memory_policy", side_effect=prepare), \
                    mock.patch.object(verify_native_tape, "hold_heavy_lock", side_effect=hold), \
                    mock.patch.object(module, operation, return_value=0):
                self.assertEqual(module.main(argv), 0)
            self.assertEqual(events, ["prepare", "hold"])
