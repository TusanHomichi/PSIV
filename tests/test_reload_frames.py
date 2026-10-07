"""The cutscene-return reload table (`rust/psiv-core/src/scenes/field_reload.rs`).

    PYTHONPATH=. python3 -m unittest tests.test_reload_frames -v

Two groups. The table cases need only the repository: the Rust table parses,
is sorted, and names exactly the maps the pack has; the maps whose row is a
first-load proxy are listed. The oracle cases re-measure the three anchor maps through
`tests/reload_frames.py` and compare them with the table; they skip with a
message when the ROM, the emulation core or the pack is absent.
"""

from __future__ import annotations

import json
import re
import tempfile
import unittest
from pathlib import Path

from tests import reload_frames

ROOT = Path(__file__).resolve().parent.parent
TABLE = ROOT / "rust/psiv-core/src/scenes/field_reload.rs"
ROM = ROOT / "Phantasy Star IV (USA).md"
CORE = ROOT / "oracle/core/genesis_plus_gx_libretro.so"
PACK = ROOT / "runtime-pack/manifest.json"

#: The two flight landings (tape 35) and the principal's office (tape 07).
ANCHORS = {0x18D: (52, 52), 0xBF: (42, 42), 0x14: (54, 55)}


def table_rows() -> dict[int, tuple[int, int]]:
    text = TABLE.read_text()
    body = text[text.index("static ROWS"):]
    body = body[: body.index("];")]
    return {int(m.group(1), 16): (int(m.group(2)), int(m.group(3)))
            for m in re.finditer(r"\((0x[0-9a-fA-F]+), (\d+), (\d+)\)", body)}


def anchor_mismatches(rows: dict[int, tuple[int, int]]) -> list[int]:
    return [m for m, expected in ANCHORS.items() if rows.get(m) != expected]


def proxy_maps() -> set[int]:
    text = TABLE.read_text()
    body = text[text.index("PROXY_MAPS"):]
    body = body[body.index("// BEGIN GENERATED PROXY"):body.index("// END GENERATED PROXY")]
    return {int(m, 16) for m in re.findall(r"0x[0-9a-fA-F]+", body)}


class TableCase(unittest.TestCase):
    def test_the_table_parses_and_is_sorted_and_includes_the_fade(self):
        rows = table_rows()
        self.assertGreater(len(rows), 300)
        self.assertEqual(list(rows), sorted(rows))
        self.assertTrue(all(16 <= plain <= music < 400 for plain, music in rows.values()))

    def test_the_anchors_are_the_organic_counts(self):
        rows = table_rows()
        self.assertEqual(anchor_mismatches(rows), [])

    def test_negative_control_a_doctored_anchor_is_caught(self):
        rows = dict(table_rows())
        rows[0x14] = (53, 55)
        self.assertEqual(anchor_mismatches(rows), [0x14])
        del rows[0x18D]
        self.assertEqual(anchor_mismatches(rows), [0x18D, 0x14])

    def test_every_pack_map_has_a_row_and_the_proxies_are_named(self):
        if not PACK.is_file():
            self.skipTest("runtime pack absent")
        manifest = json.loads(PACK.read_text())
        packed = {entry["id"] for entry in manifest["maps"]}
        rows = set(table_rows())
        self.assertEqual(packed - rows, set(), "maps with no row")
        self.assertEqual(rows - packed, set(), "rows for maps the pack lacks")
        self.assertTrue(proxy_maps() <= rows)
        self.assertTrue(proxy_maps().isdisjoint(ANCHORS), "an anchor is organic, not a proxy")


class MethodCase(unittest.TestCase):
    @staticmethod
    def log(modes):
        return [{"frame": str(7200 + i), "game_mode": f"{mode:04X}"}
                for i, mode in enumerate(modes)]

    def test_the_reload_is_the_run_of_mode_8_rows_from_the_cutscene_frame(self):
        rows = self.log([8, 8, 8, 12, 12])
        self.assertEqual(reload_frames.reload_rows(rows), 3)
        # A run that began before the cutscene frame is not counted.
        early = [{"frame": "7100", "game_mode": "0008"}] + rows
        self.assertEqual(reload_frames.reload_rows(early), 3)

    def test_negative_control_a_run_that_never_reloads_or_never_returns_is_rejected(self):
        with self.assertRaises(ValueError):
            reload_frames.reload_rows(self.log([12, 12, 12]))
        with self.assertRaises(ValueError):
            reload_frames.reload_rows(self.log([8, 8, 8]))


class OracleCase(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        missing = [p.name for p in (ROM, CORE, PACK) if not p.exists()]
        if missing:
            raise unittest.SkipTest(f"local inputs absent: {', '.join(missing)}")

    def test_the_oracle_reproduces_the_anchor_rows_the_music_frame_and_the_fade_skip(self):
        rows = table_rows()
        with tempfile.TemporaryDirectory() as scratch:
            out = Path(scratch)
            tape = reload_frames.reload_tape(out)
            for map_id in ANCHORS:
                music = reload_frames.raw_music(map_id)
                same = reload_frames.run_one(map_id, music, tape, out)
                other = reload_frames.run_one(map_id, (music + 1) & 0xFF, tape, out)
                # The table pins B to the organic capture, where the fixture's
                # own B can be a frame longer (`ORGANIC_MUSIC_TAKEN`).
                organic = reload_frames.ORGANIC_MUSIC_TAKEN[map_id]
                self.assertEqual(rows[map_id], (same, organic), f"{map_id:#x} table row")
                self.assertIn(other - same, (0, 1), f"{map_id:#x} music frame")
            # Bit 7 of Map_Load_Flags skips Pal_FadeIn's 16 frames.
            map_id = 0x14
            music = reload_frames.raw_music(map_id)
            skipped = reload_frames.run_one(map_id, music, tape, out, bits=0x80)
            self.assertEqual(skipped, rows[map_id][0] - 16)
            # Bit 0 keeps the music branch from running at all.
            bit0 = reload_frames.run_one(map_id, (music + 1) & 0xFF, tape, out, bits=0x01)
            self.assertEqual(bit0, rows[map_id][0])


if __name__ == "__main__":
    unittest.main()
