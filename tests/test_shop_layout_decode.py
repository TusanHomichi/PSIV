"""Oracle-gated evidence for the Piata shop/inn implementation."""

import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
STATE_FRAMES = {
    "shop_piata_open": 9461,
    "shop_piata_greeting": 9581,
    "shop_piata_buy_list": 10309,
    "shop_piata_buy_confirm": 10433,
    "shop_piata_sell_list": 10929,
    "shop_piata_sell_confirm": 11053,
    "shop_piata_sell_yesno": 11173,
    "shop_piata_inn_greeting": 9429,
    "shop_piata_inn_confirm": 10152,
    "shop_piata_inn_night": 10576,
}


class TestPiataShopOracle(unittest.TestCase):
    """Keep the implementation tied to captured retail states."""

    def test_reach_tapes_keep_the_required_marks(self):
        retail = (ROOT / "oracle/tapes/23_piata_shop_reach.tape").read_text()
        inn = (ROOT / "oracle/tapes/26_piata_inn_reach.tape").read_text()
        for mark in (
            "trigger_item_shop",
            "greeting_drawn",
            "buy_list",
            "quantity_screen",
            "sell_list",
            "sell_yes",
        ):
            self.assertIn(mark, retail)
        for mark in ("trigger_piata_inn", "inn_greeting_drawn", "rest_confirm", "night_wake"):
            self.assertIn(mark, inn)

    def test_every_captured_state_has_the_recorded_frame_and_screen(self):
        for name, frame in STATE_FRAMES.items():
            with self.subTest(state=name):
                state = json.loads((ROOT / f"oracle/states/{name}.json").read_text())
                self.assertEqual(state["kind"], "psiv_oracle_state")
                self.assertEqual(state["frame"], frame)
                self.assertEqual(
                    state["visible_screen"],
                    {
                        "width_pixels": 320,
                        "height_pixels": 224,
                        "width_cells": 40,
                        "height_cells": 28,
                    },
                )

    def test_unmodified_generic_decoder_outputs_are_present_and_self_checked(self):
        for name in STATE_FRAMES:
            with self.subTest(layout=name):
                layout = json.loads((ROOT / f"oracle/layouts/{name}.json").read_text())
                self.assertEqual(layout["kind"], "psiv_battle_layout")
                self.assertTrue(layout["self_check"]["passed"])

    def test_decode_record_pins_camera_remap_and_windows(self):
        document = (ROOT / "docs/camp/SHOP_LAYOUT_DECODED.md").read_text()
        for text in (
            "`(47,51)`",
            "`(63,51)`",
            "`MONEY`",
            "`BUY_QUANTITY`",
            "`SELL_PANE`",
            "`INN_CONFIRM`",
            "oracle/decode_layout.py",
            "PROVISIONAL",
        ):
            self.assertIn(text, document)


if __name__ == "__main__":
    unittest.main()
