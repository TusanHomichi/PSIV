"""Resolving a `layout_write` into cells a runtime can apply, and a tile it can draw.

The headline test is `test_every_dead_door_opens`: the five doorway warps that
cover no map-change cell in the stored layout are exactly the five a
`MapDataManager` routine opens, and after resolution every one of them has its
type-1 cell. That is the thing the pack could not express before.
"""

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from psiv_tools.core import read_rom
from psiv_tools.layouts import ChunkTable, chunk_palette
from psiv_tools.map_effects import extract_map_effects
from psiv_tools.map_patches import (
    ATLAS_TILE_PIXELS,
    CHUNK_CELLS_X,
    CHUNK_CELLS_Y,
    decode_palette_copy,
    palette_copy_colors,
    resolve_palette_effects,
    MapPatchError,
    atlas_json,
    chunk_collision_cells,
    collision_summary,
    index_writes,
    patch_atlas,
    resolve_map_effects,
    resolve_write_cells,
)
from psiv_tools.maps import extract_maps
from psiv_tools.pack import build_pack, layout_spec
from psiv_tools.pack_layouts import decode_layout_section

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"

MAP_ZEMA = 0x24
MAP_BIRTH_VALLEY_B1 = 0x2C
MAP_ZEMA_HOUSE2 = 0x28
#: `ZioFort_F1`, one of the 83 maps whose collision reads the FG plane -- proof
#: that the plane question is real and not always answered "bg".
MAP_ZIO_FORT_F1 = 0xAF

#: Every map whose effects write a layout cell. The write *total* is
#: deliberately not pinned: it is `map_effects`' output, that decoder is another
#: lane's, and a correction upstream (KrupInn_F1 went from 6 writes to 3 when
#: its negative coordinates were fixed) should not fail a test about
#: resolution. What is pinned here is what this module owns -- four cells per
#: write, every chunk id resolving, and the six door cells, which are Zema's
#: and BirthValley_B1's alone and do not move.
MAPS_WITH_WRITES = 13
MINIMUM_WRITES = 100
TOTAL_MAP_CHANGE_CELLS = 6

#: `MapDataMan_ChkZemaNormal`'s palette copy, byte for byte:
#: `lea (ZemaNormalPalettes).l,a0 / lea (Palette_Table_Buffer+$60).w,a1 /
#:  move.w #7,d7 / move.l (a0)+,(a1)+ / dbf d7`.
ZEMA_COPY_SITE = 0x051E56
ZEMA_COPY_BYTES = bytes.fromhex("41f900133e2043f8fb603e3c000722d851cffffc")

#: The three `MapDataManager` routines that copy a CRAM line.
ZEMA_ROUTINE = 0x051E4C
ZEMA_HOUSE2_ROUTINE = 0x05203E
BIRTH_VALLEY_ROUTINE = 0x0520A4

#: The chunk the four Zema door writes stamp in beside chunk 89, and the one
#: that carries the doorway.
ZEMA_DOOR_CHUNK = 0x5A


def _decoded(rom, symbol):
    record = next(
        m for m in extract_maps(rom)["maps"]
        if not m["is_null"] and m["symbol"] == symbol
    )
    spec = layout_spec(record)
    decoded, _ = decode_layout_section(rom, spec)
    return record, spec, decoded


class TestChunkCollision(unittest.TestCase):
    """The 2x2 a chunk definition imposes. No ROM needed."""

    def _chunk(self, flagged):
        # 16 words; bit 14 is the collision flag `GetChunkAndCollision` reads.
        return tuple(0x4000 if index in flagged else 0 for index in range(16))

    def test_a_chunk_covers_two_by_two_cells(self):
        self.assertEqual((CHUNK_CELLS_X, CHUNK_CELLS_Y), (2, 2))
        cells = chunk_collision_cells(self._chunk(()))
        self.assertEqual([(dx, dy) for dx, dy, _ in cells], [(0, 0), (1, 0), (0, 1), (1, 1)])

    def test_the_four_tiles_of_a_cell_are_bits_one_two_four_and_eight(self):
        # `GetChunkAndCollision` reads +0, +2, +8 and +$A from the quadrant
        # base and adds 1, 2, 4, 8 in that order.
        for word, expected in ((0, 1), (1, 2), (4, 4), (5, 8)):
            with self.subTest(word=word):
                cells = dict(((dx, dy), v) for dx, dy, v in
                             chunk_collision_cells(self._chunk({word})))
                self.assertEqual(cells[(0, 0)], expected)

    def test_each_quadrant_reads_its_own_corner(self):
        # word 2 is the top-right cell's first tile; word 8 the bottom-left's.
        cells = dict(((dx, dy), v) for dx, dy, v in chunk_collision_cells(self._chunk({2})))
        self.assertEqual(cells[(1, 0)], 1)
        self.assertEqual(cells[(0, 0)], 0)
        cells = dict(((dx, dy), v) for dx, dy, v in chunk_collision_cells(self._chunk({8})))
        self.assertEqual(cells[(0, 1)], 1)

    def test_a_solid_chunk_is_solid_in_all_four_cells(self):
        cells = chunk_collision_cells(self._chunk(range(16)))
        self.assertEqual([v for _, _, v in cells], [0xF] * 4)

    def test_a_definition_of_the_wrong_length_is_rejected(self):
        with self.assertRaises(MapPatchError):
            chunk_collision_cells((0,) * 15)

    def test_a_write_lands_its_cells_at_the_chunks_corner(self):
        chunks = ChunkTable(words=(self._chunk({0}),), blobs=())
        self.assertEqual(
            resolve_write_cells(chunks, 0, 44, 22),
            [
                {"x": 44, "y": 22, "collision": 1},
                {"x": 45, "y": 22, "collision": 0},
                {"x": 44, "y": 23, "collision": 0},
                {"x": 45, "y": 23, "collision": 0},
            ],
        )

    def test_a_chunk_the_map_never_loaded_is_a_finding_not_a_cell(self):
        chunks = ChunkTable(words=(self._chunk(()),), blobs=())
        with self.assertRaises(MapPatchError):
            resolve_write_cells(chunks, 5, 0, 0)

    def test_collision_summary_names_the_types(self):
        cells = [{"x": 0, "y": 0, "collision": 1}, {"x": 1, "y": 0, "collision": 8},
                 {"x": 0, "y": 1, "collision": 8}]
        self.assertEqual(collision_summary(cells), {"map_change": 1, "solid": 2})


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestTheZemaDeferredCopy(unittest.TestCase):
    """The trap: is the bulk copy a chunk-table write in disguise?

    It is not. It is a sixteen-word CRAM line into palette line 3 -- Zema
    recolours once Igglanova is dead -- so chunk ids 89 and 90 resolve against
    the map's own chunk table, which is what the resolver does.
    """

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)

    def test_the_copy_writes_a_palette_line_not_the_chunk_table(self):
        self.assertEqual(
            self.data[ZEMA_COPY_SITE:ZEMA_COPY_SITE + len(ZEMA_COPY_BYTES)],
            ZEMA_COPY_BYTES,
        )
        copy = decode_palette_copy(self.data, ZEMA_ROUTINE)
        self.assertIsNotNone(copy)
        self.assertFalse(copy["installs_chunk_definitions"])
        self.assertEqual(copy["resolved_as"], "palette_write")
        # 8 longs = 16 words = one CRAM line, at Palette_Table_Buffer + $60,
        # which is line 3.
        self.assertEqual(copy["words"], 16)
        self.assertEqual(copy["cram_line"], 3)
        self.assertEqual(copy["source"], "0x133E20")

    def test_the_chunks_the_doors_write_are_in_zemas_own_table(self):
        # The whole point of resolving the trap: if the copy had installed
        # these definitions, the base table would be the wrong bytes to read.
        _, _, decoded = _decoded(self.data, "Zema")
        self.assertEqual(len(decoded.chunks.words), 182)
        for chunk_id in (0x59, ZEMA_DOOR_CHUNK):
            self.assertLess(chunk_id, len(decoded.chunks.words))

    def test_the_door_chunk_carries_exactly_one_map_change_cell(self):
        _, _, decoded = _decoded(self.data, "Zema")
        cells = chunk_collision_cells(decoded.chunks[ZEMA_DOOR_CHUNK])
        self.assertEqual([v for _, _, v in cells], [8, 8, 1, 8])
        # And the chunk it replaces is solid all through, which is the closed
        # door the stored layout holds.
        self.assertEqual(
            [v for _, _, v in chunk_collision_cells(decoded.chunks[0x56])], [8] * 4
        )

    def test_all_three_copies_are_one_cram_line_three(self):
        # Not a Zema special case: three maps do this, all writing line 3.
        for routine, source in (
            (ZEMA_ROUTINE, "0x133E20"),
            (ZEMA_HOUSE2_ROUTINE, "0x12F570"),
            (BIRTH_VALLEY_ROUTINE, "0x12F570"),
        ):
            with self.subTest(routine=hex(routine)):
                copy = decode_palette_copy(self.data, routine)
                self.assertEqual(copy["cram_line"], 3)
                self.assertEqual(copy["words"], 16)
                self.assertEqual(copy["source"], source)
                self.assertEqual(len(palette_copy_colors(self.data, copy)), 16)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestThePaletteCopy(unittest.TestCase):
    """What a CRAM-line copy actually repaints, which is not the map."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)

    def test_a_whole_map_alternate_render_would_be_a_duplicate(self):
        # The load-bearing measurement. A chunk word carries one palette bit --
        # bit 14 is the collision flag, masked off before the VDP sees it -- so
        # map tiles reach CRAM lines 0 and 1 only. All three copies write line
        # 3, so re-rendering the map under the post-copy palette produces the
        # same bytes, and shipping one would ship a duplicate.
        from psiv_tools.gfx import decode_palette, palette_rgb
        from psiv_tools.layouts import decode_map_palette, render_layout

        record, spec, decoded = _decoded(self.data, "Zema")
        stored = palette_rgb(decode_map_palette(self.data, spec.palette))
        copy = decode_palette_copy(self.data, ZEMA_ROUTINE)
        after = list(stored)
        after[32:48] = palette_rgb(palette_copy_colors(self.data, copy))
        self.assertNotEqual(stored[32:48], after[32:48])
        base = render_layout(
            decoded.chunks, decoded.bg, decoded.patterns, stored[:32], overlay=decoded.fg
        )
        alternate = render_layout(
            decoded.chunks, decoded.bg, decoded.patterns, after[:32], overlay=decoded.fg
        )
        self.assertEqual(base, alternate)

    def test_no_chunk_word_can_select_the_line_the_copies_write(self):
        _, _, decoded = _decoded(self.data, "Zema")
        lines = {(word >> 13) & 1 for chunk in decoded.chunks.words for word in chunk}
        # One bit, so lines 0 and 1 and nothing else.
        self.assertLessEqual(lines, {0, 1})

    def test_the_copy_is_stored_beside_the_maps_own_palette(self):
        # ZemaNormalPalettes sits immediately after Zema's 96-byte palette
        # blob, which is what an alternate line 3 would look like on disk.
        _, spec, _ = _decoded(self.data, "Zema")
        copy = decode_palette_copy(self.data, ZEMA_ROUTINE)
        self.assertEqual(int(copy["source"], 16), spec.palette + 96)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestTheCollisionPlane(unittest.TestCase):
    """Which plane collision reads is per map, and it is not always FG."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)

    def test_the_plane_comes_from_the_records_own_scroll_byte(self):
        # `GetChunkAndCollision` reads Map_Layout_BG when $FFFFEC24 is non-zero
        # and Map_Layout_FG when it is zero; that byte is loc_51AB2's first.
        for symbol, mode, plane in (("Zema", 1, "bg"), ("ZioFort_F1", 0, "fg")):
            with self.subTest(symbol=symbol):
                record, _, decoded = _decoded(self.data, symbol)
                self.assertEqual(record["scroll"]["mode"], mode)
                self.assertEqual(decoded.spec.collision_plane_name, plane)

    def test_zemas_doors_write_the_plane_collision_reads(self):
        # `MapDataMan_ChkZemaNormal` does `moveq #1,d3` before
        # GetMapLayoutOffset, which selects BG -- the same plane Zema's scroll
        # mode makes collision-authoritative. The doors are real, not painted.
        record, _, decoded = _decoded(self.data, "Zema")
        effects = extract_map_effects(self.data, extract_maps(self.data)["maps"])
        resolved, _, counts = resolve_map_effects(decoded, effects["per_map"][record["id"]])
        doors = [
            w for e in resolved for p in e["paths"] for w in p["writes"]
            if w["kind"] == "layout_write" and w["chunk_id"] == ZEMA_DOOR_CHUNK
        ]
        self.assertEqual(len(doors), 4)
        for write in doors:
            self.assertEqual(write["plane"], "bg")
            self.assertEqual(write["collision_plane"], "bg")
            self.assertTrue(write["collision_authoritative"])
        # One Zema write lands on the other plane and changes the picture only.
        self.assertEqual(counts["picture_only"], 1)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestThePackedResolution(unittest.TestCase):
    """The emitted pack, built once."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls._temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls._temp.name) / "pack"
        cls.manifest = build_pack(
            cls.data, cls.root,
            map_ids=[MAP_ZEMA, MAP_ZEMA_HOUSE2, MAP_BIRTH_VALLEY_B1, MAP_ZIO_FORT_F1]
        )
        cls.maps = {
            entry["id"]: json.loads((cls.root / entry["json"]).read_text())
            for entry in cls.manifest["maps"]
        }

    @classmethod
    def tearDownClass(cls):
        cls._temp.cleanup()

    def test_every_dead_door_opens(self):
        # The five doorway warps the manifest lists as covering no map-change
        # cell are exactly the five a MapDataManager routine opens. Applying
        # the resolved cells gives every one of them its type-1 cell -- which
        # is the whole point of this slice.
        opened = 0
        for payload in self.maps.values():
            patched = {
                (cell["x"], cell["y"]): cell["collision"]
                for effect in payload["map_effects"]
                for path in effect["paths"]
                for write in path["writes"]
                if write["kind"] == "layout_write"
                for cell in write["cells"]
            }
            rows = payload["collision"]["rows"]
            for warp in payload["warps"]:
                if warp["table"] != 2 or warp["rect"] is None:
                    continue
                rect = warp["rect"]
                cells = [
                    (x, y)
                    for y in range(rect["y"], rect["y"] + rect["height"])
                    for x in range(rect["x"], rect["x"] + rect["width"])
                ]
                stored = sum(1 for x, y in cells if rows[y][x] == 1)
                after = sum(1 for x, y in cells if patched.get((x, y), rows[y][x]) == 1)
                with self.subTest(map=payload["symbol"], target=warp["target"]["symbol"]):
                    # No door loses its cell, and the dead ones gain one.
                    self.assertGreaterEqual(after, max(stored, 1))
                if stored == 0:
                    opened += 1
        self.assertEqual(opened, 5)
        self.assertEqual(
            len(self.manifest["warps"]["doors_without_map_change_cell"]), opened
        )

    def test_every_layout_write_carries_four_resolved_cells(self):
        for payload in self.maps.values():
            for effect in payload["map_effects"]:
                for path in effect["paths"]:
                    for write in path["writes"]:
                        if write["kind"] != "layout_write":
                            self.assertNotIn("cells", write)
                            continue
                        with self.subTest(map=payload["symbol"], at=write["at"]):
                            self.assertEqual(len(write["cells"]), 4)
                            self.assertEqual(write["cells"][0]["x"], write["cell_x"])
                            self.assertEqual(write["cells"][0]["y"], write["cell_y"])
                            self.assertEqual(write["cell_x"], write["chunk_x"] * 2)
                            self.assertIn("collision_authoritative", write)
                            self.assertIn("patch_tile", write)

    def test_the_atlas_holds_one_tile_per_distinct_written_chunk(self):
        for payload in self.maps.values():
            atlas = payload["patch_tiles"]
            written = {
                write["chunk_id"]
                for effect in payload["map_effects"]
                for path in effect["paths"]
                for write in path["writes"]
                if write["kind"] == "layout_write"
            }
            if not written:
                self.assertIsNone(atlas)
                continue
            with self.subTest(map=payload["symbol"]):
                self.assertEqual({t["chunk_id"] for t in atlas["tiles"]}, written)
                self.assertEqual(atlas["count"], len(written))
                self.assertEqual(atlas["tile_pixels"], ATLAS_TILE_PIXELS)
                image = (self.root / atlas["png"]).read_bytes()
                self.assertEqual(atlas["png_sha256"], hashlib.sha256(image).hexdigest())
                for index, tile in enumerate(atlas["tiles"]):
                    self.assertEqual(tile["index"], index)
                    self.assertEqual(tile["x"], index * ATLAS_TILE_PIXELS)

    def test_a_map_with_no_layout_writes_has_no_atlas(self):
        # ZioFort_F1 is in the fixture precisely to be the negative case.
        self.assertIsNone(self.maps[MAP_ZIO_FORT_F1]["patch_tiles"])

    def test_the_palette_copies_repaint_npcs_and_nothing_else(self):
        # Attached to the path, not to a write: two of the three copies sit on
        # paths whose only writes are object_despawn, so anything keyed to a
        # layout_write would drop them.
        found = {}
        for map_id, payload in self.maps.items():
            for effect in payload["map_effects"]:
                for path in effect["paths"]:
                    for copy in path.get("deferred_effects", ()):
                        found[map_id] = copy
                        with self.subTest(map=payload["symbol"]):
                            self.assertEqual(copy["resolved_as"], "palette_write")
                            self.assertEqual(copy["cram_line"], 3)
                            self.assertEqual(copy["words"], 16)
                            self.assertEqual(len(copy["colors"]), 16)
                            self.assertEqual(copy["affects"]["map_pixels"], 0)
                            self.assertTrue(copy["affects"]["map_render_identical"])
        self.assertEqual(sorted(found), [MAP_ZEMA, MAP_ZEMA_HOUSE2, MAP_BIRTH_VALLEY_B1])
        # ZemaHouse2's path performs no layout write at all, which is why the
        # effect cannot live on one.
        house2 = self.maps[MAP_ZEMA_HOUSE2]
        kinds = {
            write["kind"]
            for effect in house2["map_effects"] for path in effect["paths"]
            for write in path["writes"] if "deferred_effects" in path
        }
        self.assertEqual(kinds, {"object_despawn"})

    def test_every_repainted_npc_names_a_sheet_that_exists(self):
        index = json.loads((self.root / "sprites/npcs.json").read_text())
        by_id = {sheet["id"]: sheet for sheet in index["sheets"]}
        repainted = 0
        for payload in self.maps.values():
            npcs = payload["npcs"]
            for effect in payload["map_effects"]:
                for path in effect["paths"]:
                    for copy in path.get("deferred_effects", ()):
                        for swap in copy["affects"]["npc_sheets"]:
                            repainted += 1
                            with self.subTest(map=payload["symbol"], npc=swap["npc_index"]):
                                self.assertIn(swap["from"], by_id)
                                self.assertIn(swap["to"], by_id)
                                self.assertNotEqual(swap["from"], swap["to"])
                                # The NPC it names really does draw on line 3.
                                self.assertEqual(
                                    npcs[swap["npc_index"]]["sprite"]["sheet"], swap["from"]
                                )
                                # An alternate is the same picture in other
                                # colours: same geometry, same frames.
                                before, after = by_id[swap["from"]], by_id[swap["to"]]
                                self.assertEqual(
                                    (before["frame_width"], before["frame_height"],
                                     before["frame_count"]),
                                    (after["frame_width"], after["frame_height"],
                                     after["frame_count"]),
                                )
                                self.assertEqual(after["palette"]["cram_line"], 3)
                                self.assertNotEqual(
                                    before["palette"]["colors"], after["palette"]["colors"]
                                )
        self.assertTrue(repainted)

    def test_an_alternate_sheet_is_not_a_placement(self):
        # `placements` means "how many placed NPCs draw this sheet". An
        # alternate is a file the runtime may switch to, not something standing
        # anywhere, so it registers without incrementing the count -- otherwise
        # the census stops adding up against the maps.
        index = json.loads((self.root / "sprites/npcs.json").read_text())
        by_id = {sheet["id"]: sheet for sheet in index["sheets"]}
        alternates = {
            swap["to"]
            for payload in self.maps.values()
            for effect in payload["map_effects"] for path in effect["paths"]
            for copy in path.get("deferred_effects", ())
            for swap in copy["affects"]["npc_sheets"]
        }
        self.assertTrue(alternates)
        for sheet_id in alternates:
            with self.subTest(sheet=sheet_id):
                # An alternate reached only by the swap stands nowhere; one
                # that some other map also places legitimately counts there.
                self.assertGreaterEqual(by_id[sheet_id]["placements"], 0)
        placed = sum(sheet["placements"] for sheet in index["sheets"])
        npcs = sum(len(payload["npcs"]) for payload in self.maps.values())
        self.assertEqual(placed, self.manifest["sprites"]["npc_placements"])
        self.assertEqual(placed + self.manifest["sprites"]["artless_objects"], npcs)

    def test_the_palette_census_counts_the_copies(self):
        census = self.manifest["map_effects"]["palette_copies"]
        self.assertEqual(census["cram_line"], 3)
        self.assertEqual(census["map_tiles_affected"], 0)
        self.assertEqual(census["palette_copies"], 3)
        self.assertGreater(census["alternate_sheets"], 0)
        self.assertGreater(census["repainted_npcs"], 0)

    def test_the_census_counts_the_resolution(self):
        census = self.manifest["map_effects"]["layout_write_resolution"]
        self.assertEqual(census["cells_per_chunk"], 4)
        self.assertEqual(census["atlas_tile_pixels"], ATLAS_TILE_PIXELS)
        self.assertEqual(census["map_count"], len(census["maps"]))
        self.assertEqual(census["cells"], census["layout_writes"] * 4)
        self.assertEqual(
            census["collision_authoritative"] + census["picture_only"],
            census["layout_writes"],
        )
        # The two fixture maps that patch, and the six door cells between them.
        self.assertEqual(census["map_count"], 2)
        self.assertEqual(census["map_change_cells"], TOTAL_MAP_CHANGE_CELLS)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestEveryMapThatPatches(unittest.TestCase):
    """The totals over the whole table, without building a full pack."""

    def test_thirteen_maps_patch_and_every_chunk_resolves(self):
        data = read_rom(ROM)
        records = extract_maps(data)["maps"]
        effects = extract_map_effects(data, records)
        totals = {"layout_writes": 0, "cells": 0, "map_change_cells": 0}
        maps = 0
        for record in records:
            if record["is_null"] or not record["layout"]["present"]:
                continue
            per_map = effects["per_map"].get(record["id"], [])
            if not any(
                write.get("kind") == "layout_write"
                for effect in per_map
                for path in effect.get("paths", ())
                for write in path.get("writes", ())
            ):
                continue
            maps += 1
            spec = layout_spec(record)
            decoded, _ = decode_layout_section(data, spec)
            with self.subTest(map=record["symbol"]):
                # Raises if any write names a chunk the map never loaded.
                resolved, chunk_ids, counts = resolve_map_effects(decoded, per_map)
                base, over, entries = patch_atlas(
                    decoded, chunk_ids, chunk_palette(data, spec.palette)
                )
                index_writes(resolved, entries)
                payload = atlas_json(entries, "a.png", base, None, None)
                self.assertEqual(payload["count"], len(set(chunk_ids)))
            for key in totals:
                totals[key] += counts[key]
        self.assertEqual(maps, MAPS_WITH_WRITES)
        self.assertGreater(totals["layout_writes"], MINIMUM_WRITES)
        # The invariant this module owns: a chunk is 2x2 cells, always.
        self.assertEqual(totals["cells"], totals["layout_writes"] * 4)
        # Only Zema and BirthValley_B1 patch a doorway into existence.
        self.assertEqual(totals["map_change_cells"], TOTAL_MAP_CHANGE_CELLS)


if __name__ == "__main__":
    unittest.main()
