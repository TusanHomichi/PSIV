"""ROM-backed assertions for the scene presentation extraction."""

import json
import runpy
import unittest
from pathlib import Path

from psiv_tools.enigma import decompress as enigma_decompress
from psiv_tools.presentation_pack import PANEL_RECORD_SIZE, ROM_PANEL_TABLE


ROOT = Path(__file__).resolve().parents[1]
ROM = ROOT / "Phantasy Star IV (USA).md"
MANIFEST = ROOT / "runtime-pack/presentation/panels.json"


def _word(data: bytes, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 2], "big")


class PresentationPackTests(unittest.TestCase):
    def test_panel_manifest_preserves_retail_layout_and_decodes_both_planes(self):
        data = ROM.read_bytes()
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        expected = {
            0x1A: (9, 13, 29, 5),
            0x1B: (6, 10, 24, 2),
            0x1F: (26, 11, 11, 2),
            0x20: (18, 6, 3, 14),
            0x33: (13, 17, 23, 2),
            0x34: (20, 7, 4, 3),
            0x3B: (13, 15, 23, 3),
            0x3C: (15, 8, 10, 11),
        }
        records = {record["id"]: record for record in manifest["panels"]}
        self.assertEqual(len(records), 15)
        self.assertEqual(manifest["record_table"], "0x07B000")
        self.assertEqual(manifest["record_size"], PANEL_RECORD_SIZE)

        for panel_id, (columns, rows, x_cells, y_cells) in expected.items():
            record = records[panel_id]
            self.assertEqual(
                (
                    record["columns"],
                    record["rows"],
                    record["x_cells"],
                    record["y_cells"],
                ),
                (columns, rows, x_cells, y_cells),
            )
            self.assertEqual(record["decoded_size"], [columns * 8, rows * 8])
            png = ROOT / "runtime-pack" / record["png"]
            self.assertTrue(png.is_file())

            offset = ROM_PANEL_TABLE + panel_id * PANEL_RECORD_SIZE
            raw = data[offset : offset + PANEL_RECORD_SIZE]
            art_a = _word(raw, 10)
            mapping_a = int.from_bytes(raw[16:20], "big")
            words_a, _ = enigma_decompress(
                data, mapping_a, base_tile=art_a, max_words=columns * rows
            )
            self.assertEqual(len(words_a), columns * rows)

            art_b = _word(raw, 20)
            mapping_b = int.from_bytes(raw[26:30], "big")
            if art_b != 0xFFFF:
                words_b, _ = enigma_decompress(
                    data, mapping_b, base_tile=art_b, max_words=columns * rows
                )
                self.assertEqual(len(words_b), columns * rows)

    def test_opening_background_manifest_matches_game_start_addresses(self):
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        opening = manifest["opening_background"]
        self.assertEqual(
            opening,
            {
                "art": "0x1CF1F2",
                "art_consumed": 5681,
                "mapping": "0x1D25BE",
                "mapping_consumed": 338,
                "palette": "0x1D2A3C",
                # Oracle-measured: the narration image band is rows 40..167
                # of the 320x224 frame (oracle/frames/opening/*.png).
                "screen_y": 40,
                "size": [320, 128],
            },
        )

    def test_opening_oracle_state_passes_decoded_layout_contract(self):
        decoder = runpy.run_path(str(ROOT / "oracle/decode_layout.py"))
        layout = decoder["decode_layout"](
            ROOT / "oracle/states/opening/frame_4000.json",
            [],
            "opening-4000",
        )
        self.assertEqual(layout["capture"]["frame"], 4000)
        self.assertEqual(layout["screen"]["visible_width_cells"], 40)
        self.assertEqual(layout["screen"]["visible_height_cells"], 28)
        self.assertIs(layout["self_check"]["passed"], True)
        self.assertTrue(all(layout["self_check"]["checks"].values()))
