"""Cases for tools/certify.py, the single entry point for the six certified pairs.

    PYTHONPATH=. python3 -m unittest tests.test_certify -v

Hermetic: no Godot, no oracle host, no local game assets. They pin the shape of
the pair table (so a pair cannot silently lose its hash or its recipe) and the
hash check that stops a regenerated or edited oracle frame from re-baselining a
certified pair.
"""
import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools import certify

NAMES = ["opening-p1", "opening-p2", "meeting-rika", "title", "battle-0x88", "camp-root"]


class PairTableCase(unittest.TestCase):
    def test_the_six_certified_pairs_are_present_once_in_order(self):
        self.assertEqual([pair[0] for pair in certify.PAIRS], NAMES)

    def test_every_pair_pins_a_sha256_and_a_positive_tick(self):
        for name, env, tick, frame, digest, _ in certify.PAIRS:
            with self.subTest(pair=name):
                self.assertRegex(digest, r"^[0-9a-f]{64}$")
                self.assertGreater(tick, 0)
                self.assertTrue(frame.endswith(".png"))
                self.assertIsInstance(env, dict)

    def test_opening_goes_through_the_real_title_start(self):
        # #44: a hand-built PSIV_DEBUG_EVENT=0x9f fixture rotted unnoticed.
        for name, env, _, _, _, recipe in certify.PAIRS:
            if name.startswith("opening"):
                with self.subTest(pair=name):
                    self.assertEqual(env.get("PSIV_DEBUG_TITLE_AUTOSTART"), "1")
                    self.assertEqual(env.get("PSIV_DEBUG_TITLE_SHOT"), "1")
                    self.assertNotIn("PSIV_DEBUG_EVENT", env)
                    self.assertIs(recipe, certify.TAPE_27)

    def test_opening_pairs_share_one_regeneration_run(self):
        frames = {Path(p[3]).name: p[5]["frames"] for p in certify.PAIRS
                  if p[0].startswith("opening")}
        self.assertEqual(sorted(frames), ["frame_4000.png", "frame_5200.png"])
        self.assertEqual(certify.TAPE_27["frames"], [4000, 5200])

    def test_recipes_name_their_own_frame(self):
        for name, _, _, frame, _, recipe in certify.PAIRS:
            if recipe is not None:
                with self.subTest(pair=name):
                    number = int(Path(frame).stem.split("_")[1])
                    self.assertIn(number, recipe["frames"])
                    self.assertTrue(recipe["tape"].startswith("oracle/tapes/"))


class OracleFrameHashCase(unittest.TestCase):
    def setUp(self):
        self._dir = tempfile.TemporaryDirectory()
        self.addCleanup(self._dir.cleanup)
        self.root = Path(self._dir.name)

    def write_frame(self, content):
        path = self.root / "frame.png"
        path.write_bytes(content)
        return path, hashlib.sha256(content).hexdigest()

    def test_a_frame_matching_its_pin_is_accepted(self):
        path, digest = self.write_frame(b"certified frame")
        found, error = certify.oracle_frame("p", str(path), digest, None, self.root)
        self.assertEqual((found, error), (path, None))

    def test_a_frame_differing_from_its_pin_is_refused(self):
        # Negative control: an edited or regenerated frame must not re-baseline.
        path, digest = self.write_frame(b"certified frame")
        path.write_bytes(b"edited frame")
        found, error = certify.oracle_frame("p", str(path), digest, None, self.root)
        self.assertIsNone(found)
        self.assertIn("!= certified", error)

    def test_a_missing_frame_without_a_recipe_is_refused(self):
        found, error = certify.oracle_frame("p", str(self.root / "gone.png"), "0" * 64,
                                            None, self.root)
        self.assertIsNone(found)
        self.assertIn("missing oracle frame", error)

    def test_a_missing_frame_is_regenerated_then_hash_checked(self):
        target = self.root / "out" / "frame_1.png"
        recipe = {"tape": "t.tape", "patches": [], "frames": [1]}

        def fake_regenerate(_recipe, frame_path, _log):
            frame_path.parent.mkdir(parents=True, exist_ok=True)
            frame_path.write_bytes(b"regenerated")
            return 0

        good = hashlib.sha256(b"regenerated").hexdigest()
        with mock.patch.object(certify, "regenerate", fake_regenerate):
            found, error = certify.oracle_frame("p", str(target), good, recipe, self.root)
            self.assertEqual((found, error), (target, None))
            target.unlink()
            found, error = certify.oracle_frame("p", str(target), "1" * 64, recipe, self.root)
            self.assertIsNone(found)
            self.assertIn("!= certified", error)

    def test_a_failing_regeneration_is_reported(self):
        recipe = {"tape": "t.tape", "patches": [], "frames": [1]}
        with mock.patch.object(certify, "regenerate", lambda *args: 7):
            found, error = certify.oracle_frame("p", str(self.root / "f.png"), "0" * 64,
                                                recipe, self.root)
        self.assertIsNone(found)
        self.assertIn("exited 7", error)


if __name__ == "__main__":
    unittest.main()
