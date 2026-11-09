"""The Zol slugs' Fusion art (#64): `psiv_tools/battle_fusion_art.py`."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from psiv_tools.battle_fusion_art import (
    FUSION_ART_NAME,
    FusionArtError,
    emit_fusion_art,
    fusion_art_offset,
    fusion_mappings,
    render_record,
)
from psiv_tools.battle_animation_mapping import vdp_sprite_size

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"


def entry(size: int, tile: int, x: int = 0, y: int = 0, h_flip: bool = False) -> dict:
    """An entry as `decode_mapping_record` writes it."""
    columns, rows = vdp_sprite_size(size)
    return {"rom_offset": "0x000000", "y": y, "size": size, "tile_index": tile,
            "h_flip": h_flip, "v_flip": False, "attributes_valid": True, "x": x,
            "width_tiles": columns, "height_tiles": rows}


class Synthetic(unittest.TestCase):
    """No ROM: the size byte and the tile order on tiles that name themselves."""

    def tiles(self, count: int) -> list[list[int]]:
        # Tile n is filled with colour n + 1 (up to 15).
        return [[min(n + 1, 15)] * 64 for n in range(count)]

    def test_the_size_byte_is_columns_in_bits_3_2_and_rows_in_bits_1_0(self):
        self.assertEqual(vdp_sprite_size(0x09), (3, 2))
        self.assertEqual(vdp_sprite_size(0x06), (2, 3))
        self.assertEqual(vdp_sprite_size(0x0F), (4, 4))
        self.assertEqual(vdp_sprite_size(0x00), (1, 1))

    def test_a_sprite_reads_its_tiles_down_each_column_first(self):
        image = render_record(self.tiles(6), {"entries": [entry(0x09, 0)]})
        self.assertEqual(image["size"], [24, 16])
        self.assertEqual(image["origin"], [0, 0])
        pixels = image["pixels"]
        # Column 0 holds tiles 0 and 1, column 1 tiles 2 and 3, column 2 tiles 4 and 5.
        corners = [pixels[y * 24 + x] for y, x in ((0, 0), (8, 0), (0, 8), (8, 8), (0, 16), (8, 16))]
        self.assertEqual(corners, [1, 2, 3, 4, 5, 6])

    def test_a_flipped_entry_mirrors_its_own_columns(self):
        image = render_record(self.tiles(6), {"entries": [entry(0x09, 0, h_flip=True)]})
        self.assertEqual(image["pixels"][0], 5)
        self.assertEqual(image["pixels"][16], 1)

    def test_an_entry_past_the_art_is_refused(self):
        with self.assertRaises(FusionArtError):
            render_record(self.tiles(5), {"entries": [entry(0x09, 0)]})


@unittest.skipUnless(ROM.is_file(), "needs the ROM")
class Retail(unittest.TestCase):
    """The US image: the routine's own mapping writes and the art they draw."""

    @classmethod
    def setUpClass(cls):
        cls.rom = ROM.read_bytes()

    def test_the_routine_selects_five_mappings_and_loads_one_art_stream(self):
        self.assertEqual(
            [f"0x{m:06X}" for m in fusion_mappings(self.rom)],
            ["0x01AA5A", "0x01AA74", "0x01AA82", "0x01AAA8", "0x01AACE"],
        )
        self.assertEqual(fusion_art_offset(self.rom), 0x209766)

    def test_the_frames_are_the_pieces_tape_33_shows(self):
        with tempfile.TemporaryDirectory() as directory:
            payload, total, count = emit_fusion_art(self.rom, directory)
            self.assertEqual(count, 5)
            self.assertTrue((Path(directory) / payload["frames"][0]["png"]).is_file())
            self.assertGreater(total, 0)
        # Standing, flat, then growing: tape 33's fit puts each at y 120.
        self.assertEqual([f["size"] for f in payload["frames"]],
                         [[48, 32], [48, 16], [48, 40], [64, 56], [80, 64]])
        self.assertEqual([f["origin"] for f in payload["frames"]],
                         [[-24, -32], [-24, -16], [-24, -40], [-32, -56], [-40, -64]])
        self.assertEqual(payload["palette_enemy_id"], 34)
        self.assertEqual(FUSION_ART_NAME, "battle/art/fusion.json")
        json.dumps(payload)

    def test_a_routine_that_moved_is_refused(self):
        moved = bytearray(self.rom)
        moved[0x01A090] ^= 0xFF
        with self.assertRaises(FusionArtError):
            fusion_mappings(bytes(moved))


if __name__ == "__main__":
    unittest.main()
