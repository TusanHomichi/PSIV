"""$076E00's live Raja Temple BG write, decoded and emitted from retail.

The previous census called it a Dezolis write and treated +$40 as X+64.
The actual map load at $076820 and the 64-wide map header forbid both.
"""

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image

from psiv_tools.core import read_rom
from psiv_tools.layouts import chunk_palette
from psiv_tools.map_patches import MapPatchError, scene_patch_chunks
from psiv_tools.maps import extract_maps
from psiv_tools.pack import build_pack
from psiv_tools.pack_layouts import decode_map_section
from psiv_tools.render import priority_overlay
from psiv_tools.layouts import render_layout

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
TEMPLE = 0x14C
TABLE = 0x076E58
COORDS = [(47, 9), (48, 9), (47, 10), (48, 10)]
FRAMES = [
    [0x50, 0x51, 0x58, 0x59],
    [0x52, 0x53, 0x58, 0x59],
    [0x54, 0x55, 0x5A, 0x5B],
    [0x56, 0x57, 0x5C, 0x5D],
    [0x56, 0x57, 0x5E, 0x5F],
]


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestCrashLandingChunks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = read_rom(ROM)
        records = extract_maps(cls.rom)["maps"]
        cls.record = next(m for m in records if m["id"] == TEMPLE)
        cls.dezolis = next(m for m in records if m["id"] == 1)

    def test_retail_map_stride_plane_and_five_frames(self):
        # $076820: move.w #MapID_RajaTemple,(Field_Map_Index).w;
        # $076AE6: bsr.w $076E00. Pin operands in the US image, not labels.
        word = lambda at: int.from_bytes(self.rom[at:at + 2], "big")
        self.assertEqual(word(0x076822), TEMPLE)
        self.assertEqual(0x076AE8 + word(0x076AE8), 0x076E00)
        self.assertEqual(word(0x076E02), 47)
        self.assertEqual(word(0x076E06), 9)
        self.assertEqual(self.rom[0x076E09], 1)  # moveq #1,d3: BG
        self.assertEqual(word(0x076E24), 1)
        self.assertEqual(word(0x076E28), 64)
        self.assertEqual(word(0x076E2C), 65)
        self.assertEqual(self.record["dimensions"]["bg_row_size"] + 1, 64)
        self.assertEqual(self.record["scroll"]["mode"], 1)
        self.assertEqual([list(self.rom[TABLE + i:TABLE + i + 4])
                          for i in range(0, 20, 4)], FRAMES)
        self.assertEqual(self.rom[TABLE + 20], 0xFF)
        self.assertEqual(self.rom[0x076E17] + 1, 6)  # d0=5; DBF

    def test_scene_chunks_include_all_frames_not_only_the_load_hook(self):
        self.assertEqual(scene_patch_chunks(self.rom, self.record), list(range(0x50, 0x60)))
        # Negative control: previous census' map must not acquire temple art.
        self.assertEqual(scene_patch_chunks(self.rom, self.dezolis), [])

    def test_missing_terminator_truncated_and_early_terminator_are_rejected(self):
        for changed_at in (TABLE, TABLE + 20):
            broken = bytearray(self.rom)
            broken[changed_at] = 0 if changed_at == TABLE + 20 else 0xFF
            with self.subTest(at=hex(changed_at)), self.assertRaisesRegex(
                MapPatchError, "Cutscene_CrashLaanding: the chunk table at .076E58 is incomplete"
            ):
                scene_patch_chunks(broken, self.record)
        with self.assertRaisesRegex(MapPatchError, "Cutscene_CrashLaanding: the chunk table at .076E58 is incomplete"):
            scene_patch_chunks(self.rom[:TABLE + 20], self.record)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestPackedCrashLanding(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = read_rom(ROM)
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name) / "pack"
        cls.manifest = build_pack(cls.rom, cls.root, map_ids=[TEMPLE])
        cls.payload = json.loads((cls.root / "maps/14C_RajaTemple.json").read_text())
        record = next(m for m in extract_maps(cls.rom)["maps"] if m["id"] == TEMPLE)
        cls.decoded, _, _ = decode_map_section(cls.rom, record)

    def test_manifest_and_plain_atlas_cover_every_live_frame(self):
        census = self.manifest["map_effects"]["scene_patch_chunks"]
        self.assertEqual(len(census), 1)
        self.assertEqual(census[0]["id"], TEMPLE)
        self.assertEqual(census[0]["chunks"], list(range(0x50, 0x60)))
        atlas = self.payload["patch_tiles"]
        self.assertEqual(atlas["count"], 16)
        self.assertEqual([t["chunk_id"] for t in atlas["tiles"]], list(range(0x50, 0x60)))
        for kind in ("png", "png_over"):
            image = (self.root / atlas[kind]).read_bytes()
            self.assertEqual(hashlib.sha256(image).hexdigest(), atlas[kind + "_sha256"])
            with Image.open(self.root / atlas[kind]) as decoded:
                self.assertEqual(decoded.size, (16 * 32, 32))
        # The initial lower-left roof is solid; final both lower chunks solid.
        collision = {t["chunk_id"]: t["collision"] for t in atlas["tiles"]}
        for chunk in range(0x50, 0x60):
            self.assertEqual(collision[chunk], [8] * 4 if chunk in (0x58, 0x5E, 0x5F) else [0] * 4)

    def test_pixels_and_priority_match_the_maps_own_chunk_definitions(self):
        # A one-chunk full render is an independent reference for each atlas
        # tile; do not just hash the pack's own output against itself.
        from dataclasses import replace
        from io import BytesIO

        palette = chunk_palette(self.rom, self.decoded.spec.palette)
        atlas = self.payload["patch_tiles"]
        with Image.open(self.root / atlas["png"]) as base, Image.open(
            self.root / atlas["png_over"]
        ) as over:
            for tile in atlas["tiles"]:
                chunk = tile["chunk_id"]
                bg = replace(self.decoded.bg, width_chunks=1, height_chunks=1, cells=bytes([chunk]))
                fg = replace(self.decoded.fg, width_chunks=1, height_chunks=1, cells=b"\x00")
                reference = render_layout(self.decoded.chunks, bg, self.decoded.patterns, palette)
                decoded = replace(self.decoded, bg=bg, fg=fg)
                overlay, _ = priority_overlay(decoded, palette)
                box = (tile["x"], 0, tile["x"] + 32, 32)
                with Image.open(BytesIO(reference)) as expected:
                    self.assertEqual(base.crop(box).tobytes(), expected.tobytes(), hex(chunk))
                expected_over = (Image.open(BytesIO(overlay)).convert("RGBA")
                                 if overlay else Image.new("RGBA", (32, 32)))
                self.assertEqual(over.convert("RGBA").crop(box).tobytes(),
                                 expected_over.tobytes(), hex(chunk))
                expected_over.close()
