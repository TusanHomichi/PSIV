"""Guard: the Garuberk Tower's door scenes write only chunks the tower maps carry.

    PYTHONPATH=. python3 -m unittest tests.test_garuberk_door_atlas -v

The door scenes (`rust/psiv-core/src/scenes/garuberk_events.rs`,
`docs/scenes/103_GaruberkTowerDoors.md`) write chunk pairs at the leader's live
position (`WriteActorMapChunks`), so `tests.test_scene_chunk_atlas`, which reads
literal `WriteMapChunks` coordinates, cannot see them. Like the elevator and the
BioPlant doors, their chunks enter a map's atlas by the map's interaction areas
(`psiv_tools.map_patches.scene_patch_chunks`): every chunk the four ROM door tables
write (`garuberk_door_chunks`) goes into every tower map with a door opening.

This test holds both ends: every chunk a door scene writes is a chunk of the ROM
tables, and every tower map in the pack carries all of them. It reads
`PSIV_RUNTIME_PACK`, else the repo's `runtime-pack`, and skips with a message when
there is none; a pack built before the door rule landed fails here by design
(rebuild it, `docs/DEVELOPMENT.md`). The negative controls move a guard byte, and
take a chunk out of a pack map's atlas.
"""
from __future__ import annotations

import json
import os
import re
import unittest
from pathlib import Path

from psiv_tools import map_patches

ROOT = Path(__file__).resolve().parents[1]
ROM = ROOT / "Phantasy Star IV (USA).md"
PACK = Path(os.environ.get("PSIV_RUNTIME_PACK", ROOT / "runtime-pack"))
SCENES = ROOT / "rust" / "psiv-core" / "src" / "scenes" / "garuberk_events.rs"

ROW_RE = re.compile(r"row\(&\[(.*?)\]\)", re.S)
TUPLE_RE = re.compile(r"\(\s*-?\d+\s*,\s*-?\d+\s*,\s*(0x[0-9A-Fa-f]+)\s*\)")
DOOR_EVENTS = (map_patches.GARUBERK_DOOR_OPENING_1, map_patches.GARUBERK_DOOR_OPENING_2)


def scene_chunks(text: str) -> set[int]:
    """Every chunk id a `row(&[...])` of the door scenes writes."""
    return {int(chunk, 16) for row in ROW_RE.findall(text) for chunk in TUPLE_RE.findall(row)}


def missing_in_pack(pack: Path, chunks: set[int], maps: dict[int, dict] | None = None) -> list[str]:
    """`"<map>: chunk $NN"` for each door chunk a tower map's atlas lacks."""
    problems = []
    for path in sorted(pack.glob("maps/19[9A-F]_GaruberkTower*.json")):
        record = json.loads(path.read_text(encoding="utf-8"))
        record = (maps or {}).get(record["id"], record)
        if not any(
            area.get("event_index") in (0x35, 0x36) for area in record.get("interaction_areas", ())
        ):
            continue
        atlas = {
            tile.get("chunk_id") for tile in (record.get("patch_tiles") or {}).get("tiles", ())
        }
        problems += [f"{record['symbol']}: chunk ${c:02X}" for c in sorted(chunks - atlas)]
    return problems


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class GaruberkDoorAtlas(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ROM.read_bytes()
        cls.table = map_patches.garuberk_door_chunks(cls.rom)

    def test_the_tables_write_the_fifteen_door_chunks(self) -> None:
        self.assertEqual(self.table, set(range(0x30, 0x3F)))

    def test_every_door_scene_chunk_is_a_table_chunk(self) -> None:
        written = scene_chunks(SCENES.read_text(encoding="utf-8"))
        self.assertEqual(written - self.table, set())
        self.assertEqual(written, self.table, "the four scenes walk every row")

    @unittest.skipUnless(
        (PACK / "manifest.json").is_file(),
        f"no runtime pack at {PACK} (set PSIV_RUNTIME_PACK)",
    )
    def test_every_tower_map_carries_the_door_chunks(self) -> None:
        problems = missing_in_pack(PACK, self.table)
        self.assertEqual(problems, [], "\n".join(problems))


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class NegativeControl(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ROM.read_bytes()

    def test_a_moved_guard_fails_the_build(self) -> None:
        rom = bytearray(self.rom)
        rom[0x6F478 + 3] = 0x39  # cmpi.b #$39 instead of the closed door $38
        with self.assertRaises(map_patches.MapPatchError):
            map_patches.garuberk_door_chunks(bytes(rom))

    def test_a_short_table_fails_the_build(self) -> None:
        rom = bytearray(self.rom)
        rom[0x6F714 + 15] = 0x03  # the closing table loses its $FF
        with self.assertRaises(map_patches.MapPatchError):
            map_patches.garuberk_door_chunks(bytes(rom))

    @unittest.skipUnless(
        (PACK / "manifest.json").is_file(),
        f"no runtime pack at {PACK} (set PSIV_RUNTIME_PACK)",
    )
    def test_a_map_without_a_door_chunk_is_named(self) -> None:
        path = next(PACK.glob("maps/19A_GaruberkTower_Part2.json"))
        record = json.loads(path.read_text(encoding="utf-8"))
        tiles = (record.get("patch_tiles") or {}).get("tiles", [])
        if not any(tile.get("chunk_id") == 0x3C for tile in tiles):
            self.skipTest("this pack predates the door atlas")
        record["patch_tiles"]["tiles"] = [t for t in tiles if t.get("chunk_id") != 0x3C]
        problems = missing_in_pack(
            PACK, map_patches.garuberk_door_chunks(self.rom), maps={record["id"]: record}
        )
        self.assertEqual(problems, ["GaruberkTower_Part2: chunk $3C"])


if __name__ == "__main__":
    unittest.main()
