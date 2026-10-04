"""Cases for tools/certify.py, the single entry point for the certified pairs.

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

SIX = ["opening-p1", "opening-p2", "meeting-rika", "title", "battle-0x88", "camp-root"]
BATTLE = ["battle-status", "battle-status-2", "battle-fusion", "battle-strip", "battle-tech"]
NAMES = SIX + BATTLE + ["ship-menu"]
ROOT = Path(__file__).resolve().parent.parent


class PairTableCase(unittest.TestCase):
    def test_the_certified_pairs_are_present_once_in_order(self):
        self.assertEqual([pair[0] for pair in certify.PAIRS], NAMES)

    def test_the_six_original_pairs_keep_their_pins(self):
        # The battle pairs were added after these six; their ticks and frames
        # must not move with them.
        pins = {pair[0]: (pair[2], pair[3], pair[4]) for pair in certify.PAIRS}
        self.assertEqual(pins["battle-0x88"][:2], (200, "oracle/frames/frame_25000.png"))
        self.assertEqual(pins["camp-root"][:2], (60, "oracle/frames/frame_7675.png"))
        self.assertEqual(pins["title"][:2], (480, "oracle/frames/title/frame_450.png"))

    def test_battle_pairs_name_a_session_fixture_and_a_regenerable_frame(self):
        for name, env, _, frame, _, recipe in certify.PAIRS:
            if name in BATTLE:
                with self.subTest(pair=name):
                    self.assertIn("PSIV_DEBUG_BATTLE_WINDOW", env)
                    self.assertIsNotNone(recipe)
                    self.assertTrue((ROOT / recipe["tape"]).is_file())
                    # Oracle frames are local and ignored, never committed.
                    self.assertTrue(frame.startswith("build/certify/oracle/"))

    def test_the_ship_menu_pair_names_its_event_fixture_and_a_regenerable_frame(self):
        pair = next(p for p in certify.PAIRS if p[0] == "ship-menu")
        _, env, _, frame, _, recipe = pair
        # The clone starts the real scene (Cutscene_InsideSpaceship) through the
        # session's event fixture, not a hand-built menu.
        self.assertEqual(env, {"PSIV_DEBUG_EVENT": "0x800D"})
        self.assertIs(recipe, certify.TAPE_35)
        self.assertTrue((ROOT / recipe["tape"]).is_file())
        self.assertTrue(frame.startswith("build/certify/oracle/"))
        # The fixture is tape 28's frame-7000 reload and a frame-7200 event
        # entry, with AlysFound (F101 bit 7), all spelled as patches.
        self.assertIn("7200:FFFFECA8:800D", recipe["patches"])
        self.assertIn("7000:FFFFEC28:00BF", recipe["patches"])
        self.assertIn("7000:FFFFF101:80", recipe["patches"])

    def test_battle_pairs_pin_the_enemy_clock_the_oracle_receipt_names(self):
        for name, env, _, _, _, _ in certify.PAIRS:
            if name in ("battle-strip", "battle-tech", "battle-status", "battle-status-2"):
                with self.subTest(pair=name):
                    self.assertIn("PSIV_DEBUG_BATTLE_PHASE", env)

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


class BuildByDefaultTest(unittest.TestCase):
    """A capture must come from the checked-out sources (a stale extension once
    certified the wrong code), so building is the default."""

    def test_default_builds(self):
        self.assertFalse(certify.build_parser().parse_args([]).no_build)

    def test_no_build_is_explicit(self):
        self.assertTrue(certify.build_parser().parse_args(["--no-build"]).no_build)

    def test_legacy_build_flag_still_parses(self):
        self.assertFalse(certify.build_parser().parse_args(["--build"]).no_build)


class CaptureTimeoutTest(unittest.TestCase):
    """The capture budget scales with the tick it must reach."""

    def test_long_pairs_get_longer_budgets(self):
        self.assertGreater(certify.capture_timeout(4550), certify.capture_timeout(60))

    def test_the_longest_pair_outlasts_ten_frames_a_second(self):
        longest = max(tick for _, _, tick, *_ in certify.PAIRS)
        self.assertGreaterEqual(certify.capture_timeout(longest), (longest + certify.QUIT_MARGIN) / 10)

