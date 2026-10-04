"""MapUpdateJmpTbl extraction, including deliberately broken source controls."""

import copy
import struct
import unittest
from pathlib import Path

from psiv_tools.map_updates import MapUpdatesError, dispatch_table, extract_map_updates
from psiv_tools.map_updates_render import index_png
from psiv_tools.maps import extract_maps
from psiv_tools.m68k import DecodeError, w
from psiv_tools.png import encode_indexed

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"


class MapUpdatesExtraction(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = ROM.read_bytes()
        cls.records = extract_maps(cls.rom)["maps"]
        cls.updates = extract_map_updates(cls.rom, cls.records)

    def test_every_table_entry_and_every_map_list_is_accounted_for(self):
        census = self.updates["census"]
        self.assertEqual(census["routine_count"], 64)
        self.assertEqual(census["map_count"], 361)
        self.assertEqual([e["index"] for e in census["routines"]], list(range(64)))
        for record in self.records:
            if not record["is_null"]:
                self.assertEqual([e["index"] for e in self.updates["per_map"][record["id"]]],
                                 record["map_updates"]["ids"])

    def test_aiedo_water_reads_the_cartridge_table_with_eight_frame_clock(self):
        water, = self.updates["per_map"][0x54]
        self.assertEqual(water["index"], 2)
        cycle, = water["program"]["cycles"]
        self.assertEqual(cycle["frame_mask"], 7)
        self.assertEqual(cycle["phase"], {"kind": "frame", "mask": 0x18, "shift": 3})
        # Independent direct reads: MotaTownsWaterCyclingPal $054B0C, four
        # columns at byte displacements 0,8,16,24 (ps4.asm:112992-113016).
        for phase in range(4):
            self.assertEqual(cycle["frames"][phase], [
                {"slot": 26 + color, "word": w(self.rom, 0x54B0C + phase * 2 + color * 8)}
                for color in range(4)])

    def test_canceller_uses_the_extended_event_chest_door(self):
        routine = self.updates["census"]["routines"][0x2D]
        self.assertEqual(routine["address"], "0x055C1C")
        self.assertEqual(routine["program"], {
            "kind": "flags", "chests": [0x0B], "flag": 0x72, "clear_chest": False})
        # The retail call is $05762E, whose bank is $F120, not the fork's
        # temporary door $057638/$F140.
        self.assertEqual(int.from_bytes(self.rom[0x55C22:0x55C26], "big"), 0x5762E)
        self.assertEqual(w(self.rom, 0x57634), 0xF120)

    def test_retail_table_quirks_are_preserved(self):
        routines = self.updates["census"]["routines"]
        air, = routines[0x1F]["program"]["cycles"]
        self.assertEqual(air["phase"]["mask"], 0)
        garuberk, = routines[0x24]["program"]["cycles"]
        self.assertEqual(garuberk["frames"][0][-1]["word"], w(self.rom, 0x55952 + 0x14))
        roof, = routines[0x2C]["program"]["cycles"]
        self.assertEqual(len(roof["frames"]), 14)

    def test_missing_native_buffers_are_explicit(self):
        unsupported = [e["index"] for e in self.updates["census"]["routines"]
                       if e["program"]["kind"] == "unsupported"]
        self.assertEqual(unsupported, [0x11, 0x12, 0x22])

    def test_negative_control_corrupt_table_branch_is_rejected(self):
        broken = bytearray(self.rom)
        table = dispatch_table(self.rom)
        struct.pack_into(">H", broken, table + 2 * 4, 0x4E75)
        with self.assertRaisesRegex(DecodeError, "bra.w"):
            extract_map_updates(bytes(broken), self.records)

    def test_negative_control_out_of_range_map_index_is_rejected(self):
        records = copy.deepcopy(self.records)
        next(r for r in records if r["id"] == 0x54)["map_updates"]["ids"] = [64]
        with self.assertRaisesRegex(MapUpdatesError, "out of bounds"):
            extract_map_updates(self.rom, records)

    def test_negative_control_corrupt_clock_instruction_is_rejected(self):
        broken = bytearray(self.rom)
        struct.pack_into(">H", broken, 0x54AEC, 0x4E71)  # water's lsr.w #2,d0
        with self.assertRaisesRegex(MapUpdatesError, "lsr.w"):
            extract_map_updates(bytes(broken), self.records)


class PaletteIndexImages(unittest.TestCase):
    def test_equal_rgb_slots_keep_distinct_indices_and_alpha(self):
        source = encode_indexed(3, 1, bytes([0, 1, 2]), [(0, 0, 0)] * 3, transparent=[0])
        indexed = index_png(source)
        from PIL import Image
        from io import BytesIO
        image = Image.open(BytesIO(indexed)).convert("RGBA")
        self.assertEqual([image.getpixel((x, 0)) for x in range(3)], [(0, 0, 0, 0), (1, 0, 0, 255), (2, 0, 0, 255)])

    def test_negative_control_rgb_input_is_rejected(self):
        from psiv_tools.png import encode_rgb
        with self.assertRaisesRegex(ValueError, "indexed PNG"):
            index_png(encode_rgb(1, 1, bytes([1, 2, 3])))


class PackComparison(unittest.TestCase):
    def test_additive_comparison_and_original_byte_negative_control(self):
        import json
        import tempfile
        from psiv_tools.map_updates_compare import compare, digest
        with tempfile.TemporaryDirectory() as directory:
            old, new = [Path(directory) / name for name in ("accepted", "candidate")]
            for root in (old, new):
                (root / "maps").mkdir(parents=True)
                (root / "original.png").write_bytes(b"original indexed picture")
            manifest = {"maps": [{"id": 1, "json": "maps/one.json", "json_sha256": "old"}]}
            (old / "manifest.json").write_text(json.dumps(manifest))
            (old / "maps/one.json").write_text(json.dumps({"id": 1, "collision": [0]}))
            manifest["maps"][0]["json_sha256"] = "new"
            manifest["map_updates"] = {"routine_count": 64}
            updated = {"id": 1, "collision": [0], "map_updates": [{"index": 0}],
                       "map_update_images": {"original.png": "new_indices.png"}}
            (new / "maps/one.json").write_text(json.dumps(updated))
            manifest["maps"][0]["json_sha256"] = digest(new / "maps/one.json")
            (new / "manifest.json").write_text(json.dumps(manifest))
            (new / "new_indices.png").write_bytes(b"added")
            self.assertTrue(compare(old, new)["passed"])
            (new / "original.png").write_bytes(b"changed")
            self.assertIn("changed original bytes: original.png", compare(old, new)["errors"])
            (new / "original.png").write_bytes(b"original indexed picture")
            updated["collision"] = [8]
            (new / "maps/one.json").write_text(json.dumps(updated))
            self.assertIn("non-additive map change: maps/one.json", compare(old, new)["errors"])
            self.assertIn("invalid map JSON digest: maps/one.json", compare(old, new)["errors"])
