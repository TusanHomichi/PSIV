"""$076E00's live Raja Temple BG write, decoded and emitted from retail.

The previous census called it a Dezolis write and treated +$40 as X+64.
The actual map load at $076820 and the 64-wide map header forbid both.
"""

import unittest
from pathlib import Path


from psiv_tools.core import read_rom
from psiv_tools.map_patches import MapPatchError, scene_patch_chunks
from psiv_tools.maps import extract_maps

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
