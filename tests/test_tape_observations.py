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
