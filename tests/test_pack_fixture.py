"""The shared pack fixture, and everything the `test_pack_*` modules build on.

The pack tests were one 1,100-line file until the shop's own thousand-line rule
caught up with them. They are split by what they interrogate -- the manifest,
the map records, the sprite sheets, the game-start block -- and everything they
have in common lives here.

The expensive part is the fixture: building the three-map pack costs a few
seconds, and naively giving each module its own `setUpClass` would pay that
cost once per file. `fixture_pack` and `map_records` are module-level caches, so
the ROM is read once, the pack is built once, and the map table is walked once,
no matter how many `TestCase` classes across how many files ask for them.

Nothing here is a test. Two base classes are exported:

* `PackFixtureCase` -- for anything that reads the built pack (`root`,
  `manifest`, `maps`).
* `MapTableCase` -- for anything that needs the decoded map records but not a
  pack on disk, including the tests that build their own narrow packs.
"""

import atexit
import json
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

from psiv_tools import png
from psiv_tools.core import read_rom
from psiv_tools.maps import extract_maps
from psiv_tools.overworld import DEZOLIS, MOTAVIA
from psiv_tools.pack import build_pack

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"

# The three maps the pack fixtures use, and the interior Piata's academy door
# leads to. `MapID_PiataAcademy` is $11; `docs/RUNTIME_DESIGN.md` says $13,
# which is `MapID_PiataAcademy_F1`, the floor above.
MAP_PIATA = 0x10
MAP_PIATA_ACADEMY = 0x11
MAP_PIATA_ITEM_SHOP = 0x1B
MAP_ISLAND_CAVE = 0x92
MAP_MOTAVIA = MOTAVIA

FIXTURE_MAPS = (MAP_PIATA, MAP_PIATA_ITEM_SHOP, MAP_ISLAND_CAVE)

# `MapID_ClimCenter_F2`: a 48x48 map pointed at a 32x32 BG blob.
MAP_CLIM_CENTER_F2 = 0xC2
# `MapID_InnerSanctuary_B1`: one BG cell names a chunk the map never loads.
MAP_INNER_SANCTUARY_B1 = 0x16F
# The two maps whose doorway collision is written at load time, not stored.
MAP_ZEMA = 0x24
MAP_BIRTH_VALLEY_B1 = 0x2C
# `MapID_KadaryInn_F1` carries collision type $7, which the disassembly never
# names; `MapID_AiedoPub` holds the one object facing $10; `MapID_MileDead`
# binds dialogue tree 43, the highest there is.
MAP_AIEDO_PUB = 0x64
MAP_KADARY_INN_F1 = 0x72
MAP_MILE_DEAD = 0x1E
# The two academy-basement rooms whose bosses must not answer the talk probe.
MAP_ACADEMY_BASEMENT = 0x15
MAP_ACADEMY_BASEMENT_B2 = 0x17
MAP_DEZOLIS = DEZOLIS
#: `MapID_TheEdge`: one of the 22 maps that draw nothing above sprites.
MAP_THE_EDGE = 0x100

MAP_CHANGE = 0x1

#: Real records, and `PtrMap_Null` entries, in the 417-entry table. Every real
#: one is packed now that the two paged world maps decode.
REAL_MAPS = 361
NULL_MAPS = 56


def parse_png_chunks(data: bytes) -> list[tuple[bytes, bytes]]:
    """Re-parse a PNG, verifying every chunk CRC."""
    if data[:8] != png.PNG_SIGNATURE:
        raise AssertionError("missing PNG signature")
    chunks = []
    pos = 8
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        payload = data[pos + 8:pos + 8 + length]
        (crc,) = struct.unpack(">I", data[pos + 8 + length:pos + 12 + length])
        expected = zlib.crc32(kind + payload) & 0xFFFFFFFF
        if crc != expected:
            raise AssertionError(f"bad CRC on {kind!r}: {crc:08X} != {expected:08X}")
        chunks.append((kind, payload))
        pos += 12 + length
    return chunks


def png_size(data: bytes) -> tuple[int, int]:
    chunks = dict(parse_png_chunks(data))
    return struct.unpack(">II", chunks[b"IHDR"][:8])


# ---------------------------------------------------------------------------
# Cached fixtures
# ---------------------------------------------------------------------------
_rom: bytes | None = None
_pack: tuple[Path, dict, dict] | None = None
_records: list | None = None
_temp: tempfile.TemporaryDirectory | None = None


def rom_bytes() -> bytes:
    """The verified retail image, read once per process."""
    global _rom
    if _rom is None:
        _rom = read_rom(ROM)
    return _rom


def fixture_pack() -> tuple[Path, dict, dict]:
    """Build the three-map pack once; return `(root, manifest, maps)`.

    The temporary directory outlives every `TestCase` that reads it, because
    there is no last one to tear it down -- classes in four modules share it.
    `atexit` is the only teardown that cannot fire while somebody is still
    looking.
    """
    global _pack, _temp
    if _pack is None:
        _temp = tempfile.TemporaryDirectory()
        atexit.register(_temp.cleanup)
        root = Path(_temp.name) / "pack"
        manifest = build_pack(rom_bytes(), root, map_ids=FIXTURE_MAPS)
        maps = {
            entry["id"]: json.loads((root / entry["json"]).read_text())
            for entry in manifest["maps"]
        }
        _pack = (root, manifest, maps)
    return _pack


def map_records() -> list:
    """Every decoded map record, walked once per process."""
    global _records
    if _records is None:
        _records = extract_maps(rom_bytes())["maps"]
    return _records


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class PackFixtureCase(unittest.TestCase):
    """One three-map pack, built once and inspected from every angle."""

    @classmethod
    def setUpClass(cls):
        cls.data = rom_bytes()
        cls.root, cls.manifest, cls.maps = fixture_pack()

    def _only_warp(self, map_id, target):
        matches = [w for w in self.maps[map_id]["warps"] if w["target"]["id"] == target]
        self.assertEqual(len(matches), 1)
        return matches[0]


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class MapTableCase(unittest.TestCase):
    """Things that need the map table but not a pack on disk."""

    @classmethod
    def setUpClass(cls):
        cls.data = rom_bytes()
        cls.records = map_records()
