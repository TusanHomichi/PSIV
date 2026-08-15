"""The game-start block: the new-game state the pack hands the runtime.
"""

import hashlib
import json
import unittest

from psiv_tools.pack import (
    GAME_START_NAME,
    PACK_FORMAT_VERSION,
)

# The fixture module is a sibling; `unittest discover -s tests` puts this
# directory on the path, while `-m unittest tests.test_pack_maps` does not.
try:
    from test_pack_fixture import (
        PackFixtureCase,
    )
except ImportError:  # pragma: no cover - invoked as tests.test_pack_*
    from tests.test_pack_fixture import (
        PackFixtureCase,
    )


class TestGameStart(PackFixtureCase):
    def test_the_manifest_points_at_the_game_start_file(self):
        # Where a new game begins. The manifest carries the headline so a
        # runtime can spawn from it alone; `game_start.json` carries the
        # instruction sites it was read from.
        start = self.manifest["game_start"]
        self.assertEqual(start["file"], GAME_START_NAME)
        blob = (self.root / GAME_START_NAME).read_bytes()
        self.assertEqual(start["sha256"], hashlib.sha256(blob).hexdigest())
        payload = json.loads(blob)
        self.assertEqual(payload["format_version"], PACK_FORMAT_VERSION)
        self.assertEqual(payload["kind"], "game_start")
        # The three sections the extraction is built from, and the one that
        # matters to a runtime with no event engine.
        self.assertEqual(
            sorted(k for k in payload if k not in ("format_version", "kind", "note")),
            ["first_control", "new_game_init", "title_handoff"],
        )
        self.assertEqual(start["party"], ["Chaz"])
        self.assertEqual(start["map"]["symbol"], "PiataAcademy_F1")
        self.assertEqual((start["x_cell"], start["y_cell"]), (48, 19))
        self.assertEqual(start["facing"], {"id": 0, "name": "down"})
        # Two scenes run before control: $9F sets $07, then the trigger it
        # leaves satisfied plays $A0, which sets $15. Pinned against
        # oracle/logs/01_newgame.csv (eflags_00 = 01000400).
        self.assertEqual(start["event_flags_set"], [0x07, 0x15])
        self.assertEqual(start["scene_chain"], ["0x9F", "0xA0"])
        self.assertEqual(start["town_flags_set"], [0x00, 0x10, 0x19])
        self.assertEqual(len(start["extended_event_flags_set"]), 11)
        self.assertEqual(start["chest_flags_set"], [])
        # The whole seedable state, as the cartridge holds it.
        self.assertEqual(start["flag_banks"]["event_flags"]["raw_hex"],
                         "01000400" + "00" * 28)
        self.assertEqual(start["flag_banks"]["town_flags"]["raw_hex"],
                         "80008040" + "00" * 28)
        # The start map is not one of this fixture's three, and that is fine:
        # `game_start` is pack-wide, not per-map.
        self.assertNotIn(start["map"]["id"], {e["id"] for e in self.manifest["maps"]})


if __name__ == "__main__":
    unittest.main()
