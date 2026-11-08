"""ROM-backed assertions for the scene presentation extraction."""

import json
import unittest
from pathlib import Path

from oracle.decode_layout import decode_layout
from psiv_tools.enigma import decompress as enigma_decompress
from psiv_tools.presentation_pack import (
    DIALOGUE_ACTION_PANEL_IDS,
    ENDING_PANEL_IDS,
    PANEL_IDS,
    PANEL_RECORD_SIZE,
    PALETTE_REQUESTS,
    PLATE_ENGINE_LIGHT,
    decode_palette_requests,
    panel_palette_requests,
    panel_record_offset,
)


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
        self.assertEqual(len(records), len(PANEL_IDS))
        self.assertEqual(len(records), 228)
        self.assertTrue(set(DIALOGUE_ACTION_PANEL_IDS) <= set(records))
        self.assertTrue(set(ENDING_PANEL_IDS) <= set(records))
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

            offset = panel_record_offset(panel_id)
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

    def test_plate_engine_light_window_covers_every_read_its_loop_makes(self):
        # `Event_PlateSysEngine`'s loop reads `(a1)+` until a negative word
        # and then `$E(a1)`: the pack's window must reach the deepest second
        # word, the one after the table's last colour, and hold the ROM's bytes.
        data = ROM.read_bytes()
        address, words = PLATE_ENGINE_LIGHT
        records = {r["rom_addr"]: r for r in decode_palette_requests(data)}
        record = records[f"0x{address:06X}"]
        self.assertEqual(record["words"], words)
        raw = bytes.fromhex(record["raw_hex"])
        self.assertEqual(raw, data[address : address + words * 2])
        table = [_word(raw, 2 * i) for i in range(words)]
        terminator = next(i for i, word in enumerate(table) if word & 0x8000)
        self.assertGreater(terminator, 0)
        deepest = terminator + 0xE // 2
        self.assertEqual(deepest, words - 1, "the window ends at the deepest read")
        # That read is past the table: the address of `loc_6E922`'s code.
        self.assertEqual(address + 2 * deepest, 0x06E922)

    def test_scene_palette_requests_are_the_words_the_cartridge_reads(self):
        # Each request beside the instruction bytes that read it, from the US
        # image: the site, the bytes there (a `lea` of the table and its
        # destination, then the count), and the words the site copies.
        data = ROM.read_bytes()
        sites = {
            # InitVRAMAndCRAM: lea Pal_Init / lea Palette_Table_Buffer /
            # moveq #$1F, d0 / move.l (a0)+, (a1)+: 64 words.
            0x09F2BC: (0x05A68A, "41f90009f2bc43f8fb00701f22d8", 64),
            # Pal_Init_Line_3, at its deepest read: Event_GettingOffVehicle's
            # lea Pal_Init_Line_3 / lea Palette_Line_3 / moveq #$10, d7 /
            # trap #1, 17 words. Event_EnterAngerTower's hold ($07383A,
            # move.w #$F, d7) reads the first 16.
            0x296300: (0x06BF2A, "41f90029630043f8fb407e104e41", 17),
            # loc_78296: lea loc_782D2 / lea Palette_Table_Buffer+$2E /
            # moveq #0, d7 / ... / moveq #8, d6 / lea loc_782DA: six frames
            # of nine words (the six delay bytes before $FF at $0782D2).
            0x0782DA: (0x0782AE, "7c0847f9000782da", 54),
            # Event_ZioFortBarrier: lea Palette_Line_4 / lea loc_6EE16 /
            # moveq #6, d7: seven steps reading words n, n+7, n+14.
            0x06EE16: (0x06ED42, "43f8fb6045f90006ee167e06", 21),
            # Cutscene_Ending: lea loc_2EEA04 / lea Palette_Table_Buffer_2 /
            # move.w #$F, d7 / trap #1: 16 words.
            0x2EEA04: (0x0796D4, "41f9002eea0443f8fb803e3c000f4e41", 16),
            # Event_KingRappyDefeated: lea Palette_Line_4 / lea loc_730FC,
            # copied to the $FFFF after its sixteenth word.
            0x0730FC: (0x073054, "41f8fb6043f9000730fc", 16),
        }
        requests = dict(PALETTE_REQUESTS)
        records = {r["rom_addr"]: r for r in decode_palette_requests(data)}
        for address, (site, code, words) in sites.items():
            with self.subTest(address=f"0x{address:06X}"):
                code_bytes = bytes.fromhex(code)
                self.assertEqual(data[site : site + len(code_bytes)], code_bytes)
                self.assertEqual(requests[address], words)
                record = records[f"0x{address:06X}"]
                self.assertEqual(
                    bytes.fromhex(record["raw_hex"]),
                    data[address : address + 2 * words],
                )
        self.assertEqual(data[0x0782D2:0x0782D9], bytes([7, 5, 7, 7, 7, 7, 0xFF]))
        self.assertEqual(_word(data, 0x0730FC + 32), 0xFFFF)

    def test_every_drawn_panel_layer_has_its_palette(self):
        # Panel_Draw copies sixteen words of a layer's palette whenever its
        # art tile is not $FFFF; each such address is a palette record.
        panels = [
            {"art_tile_a": 0x8100, "palette_a": "0x09F37C",
             "art_tile_b": 0xFFFF, "palette_b": None},
            {"art_tile_a": 0x8100, "palette_a": "0x09F37C",
             "art_tile_b": 0xA100, "palette_b": "0x09F39C"},
            {"art_tile_a": 0xFFFF, "palette_a": "0x0A0000",
             "art_tile_b": 0xFFFF, "palette_b": None},
            # Already a named request: not repeated.
            {"art_tile_a": 0x8100, "palette_a": "0x296300",
             "art_tile_b": 0xFFFF, "palette_b": None},
        ]
        self.assertEqual(
            panel_palette_requests(panels), [(0x09F37C, 16), (0x09F39C, 16)]
        )
        data = ROM.read_bytes()
        records = {r["rom_addr"] for r in decode_palette_requests(data, panels)}
        self.assertIn("0x09F39C", records)
        self.assertNotIn("0x0A0000", records)

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
        layout = decode_layout(
            ROOT / "oracle/states/opening/frame_4000.json",
            [],
            "opening-4000",
        )
        self.assertEqual(layout["capture"]["frame"], 4000)
        self.assertEqual(layout["screen"]["visible_width_cells"], 40)
        self.assertEqual(layout["screen"]["visible_height_cells"], 28)
        self.assertIs(layout["self_check"]["passed"], True)
        self.assertTrue(all(layout["self_check"]["checks"].values()))

    def test_scene_owned_assets_and_load_art_are_emitted_additively(self):
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        self.assertEqual(
            manifest["coverage"],
            {
                "panel_records": 228,
                "load_art_records": 7,
                "temporary_object_keys": 9,
                "generic_portraits": 1,
            },
        )
        loads = {
            (record["source_rom_addr"], record["destination_tile"]): record
            for record in manifest["load_art"]
        }
        self.assertEqual(
            set(loads),
            {
                ("0x1D5DF0", "0x02E6"),
                ("0x1D628C", "0x0179"),
                ("0x1D642E", "0x0191"),
                ("0x1D6A2E", "0x01FB"),
                ("0x1D6BC8", "0x020C"),
                ("0x12951A", "0x03A5"),
                ("0x1289FA", "0x04A5"),
            },
        )
        for record in loads.values():
            self.assertTrue((ROOT / "runtime-pack" / record["png"]).is_file())
            self.assertGreater(record["tile_count"], 0)

        temporary = {
            (record["object_id"], record["art_tile"]): record
            for record in manifest["temporary_objects"]
        }
        self.assertEqual(
            set(temporary),
            {
                (0x18, 0x26A),
                (0x18, 0x55C),
                (0x194, 0x4A5),
                (0x214, 0x179),
                (0x218, 0x191),
                (0x21C, 0x1FB),
                (0x220, 0x20C),
                (0x188, 0x3A5),
                (0x1FC, 0x2E6),
            },
        )
        self.assertEqual(temporary[(0x214, 0x179)]["playback_sequence"], "walk_down")
        self.assertEqual(temporary[(0x218, 0x191)]["playback_sequence"], "walk_up")
        self.assertTrue(temporary[(0x218, 0x191)]["playback_once"])
        from psiv_tools import gfx
        expected_flame_palette = gfx.palette_rgb(gfx.decode_palette(ROM.read_bytes()[0x1DE0B8:0x1DE0D8]))
        self.assertEqual(temporary[(0x218, 0x191)]["palette"]["colors"], [list(c) for c in expected_flame_palette])
        igglanova = temporary[(0x188, 0x3A5)]
        expected_igglanova_palette = gfx.palette_rgb(gfx.decode_palette(ROM.read_bytes()[0x52848:0x52864]))
        self.assertEqual(igglanova["palette"]["colors"][:14], [list(c) for c in expected_igglanova_palette])
        self.assertEqual(igglanova["playback_sequence"], "walk_down")
        self.assertEqual(temporary[(0x18, 0x26A)]["render_status"], "exact")
        self.assertEqual(temporary[(0x18, 0x55C)]["render_status"], "exact")
        self.assertEqual(
            temporary[(0x1FC, 0x2E6)]["render_status"],
            "partial_transparent_holes",
        )
        self.assertEqual(
            temporary[(0x1FC, 0x2E6)]["missing_patterns"],
            list(range(0x34B, 0x38D)),
        )
        for record in temporary.values():
            self.assertTrue((ROOT / "runtime-pack" / record["png"]).is_file())
            self.assertGreater(record["frame_count"], 0)

        portrait = manifest["portraits"][0]
        self.assertEqual(portrait["mapping"], "0x2A2B36")
        self.assertEqual(portrait["destination_tile"], "0x055C")
        self.assertEqual(portrait["size_pixels"], [48, 48])
        self.assertTrue((ROOT / "runtime-pack" / portrait["png"]).is_file())
