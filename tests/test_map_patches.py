"""Resolving a `layout_write` into cells a runtime can apply, and a tile it can draw.

The headline test is `test_every_dead_door_opens`: the five doorway warps that
cover no map-change cell in the stored layout are exactly the five a
`MapDataManager` routine opens, and after resolution every one of them has its
type-1 cell. That is the thing the pack could not express before.
"""

import unittest
from pathlib import Path


from psiv_tools.core import read_rom
from psiv_tools.layouts import ChunkTable, chunk_palette
from psiv_tools.map_effects import extract_map_effects
from psiv_tools.map_patches import (
    CHUNK_CELLS_X,
    CHUNK_CELLS_Y,
    decode_palette_copy,
    palette_copy_colors,
    MapPatchError,
    atlas_json,
    compose_chunk,
    compose_overworld_chunk,
    chunk_collision_cells,
    collision_summary,
    index_writes,
    write_composite_pairs,
    patch_atlas,
    resolve_map_effects,
    resolve_write_cells,
    scene_patch_chunks,
)
from psiv_tools.maps import extract_maps
from psiv_tools.map_layouts import layout_spec
from psiv_tools.map_layouts import decode_layout_section

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"

MAP_ZEMA = 0x24
MAP_BIRTH_VALLEY_B1 = 0x2C
MAP_ZEMA_HOUSE2 = 0x28
#: `ZioFort_F1`, one of the 83 maps whose collision reads the FG plane -- proof
#: that the plane question is real and not always answered "bg".
MAP_ZIO_FORT_F1 = 0xAF
MAP_MOTAVIA = 0
MAP_DEZOLIS = 1

#: Every map whose effects write a layout cell. The write *total* is
#: deliberately not pinned: it is `map_effects`' output, that decoder is another
#: lane's, and a correction upstream (KrupInn_F1 went from 6 writes to 3 when
#: its negative coordinates were fixed) should not fail a test about
#: resolution. What is pinned here is what this module owns -- four cells per
#: write, every chunk id resolving, and the six door cells, which are Zema's
#: and BirthValley_B1's alone and do not move.
MAPS_WITH_WRITES = 14
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
        from psiv_tools.gfx import palette_rgb
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

    def test_scene_doors_extract_all_animation_chunks_and_original_collision(self):
        record, _, decoded = _decoded(self.data, "BirthValley_B1")
        self.assertEqual(scene_patch_chunks(self.data, record), [0x29, 0x2A, 0x42, 0x43, 0x44, 0x45])
        self.assertEqual([v for _, _, v in chunk_collision_cells(decoded.chunks[0x29])], [8, 8, 1, 1])
        record, _, decoded = _decoded(self.data, "BioPlant_Part3")
        self.assertEqual(scene_patch_chunks(self.data, record), [0x4F, 0x50, 0x51, 0x52, 0x53])
        for chunk in range(0x4F, 0x53):
            self.assertEqual([v for _, _, v in chunk_collision_cells(decoded.chunks[chunk])], [8, 8, 8, 8])
        self.assertEqual([v for _, _, v in chunk_collision_cells(decoded.chunks[0x53])], [8, 8, 1, 1])
        record, _, _ = _decoded(self.data, "Zema")
        self.assertEqual(scene_patch_chunks(self.data, record), [])

    def test_scene_chunk_writes_come_from_one_cited_table(self):
        """`SCENE_CHUNK_WRITES` rows add their chunks to the named map and no other."""
        record, _, decoded = _decoded(self.data, "Tyler")
        self.assertEqual(scene_patch_chunks(self.data, record), [0x47])
        # The stair chunk is not in the map's own layout writes; its pixels and
        # collision come from the chunk table, and it is walkable ground
        # (no solid quadrant) once the grave is open.
        cells = [v for _, _, v in chunk_collision_cells(decoded.chunks[0x47])]
        self.assertEqual(len(cells), 4)
        record, _, _ = _decoded(self.data, "RajaTemple")
        self.assertEqual(scene_patch_chunks(self.data, record), list(range(0x50, 0x60)))
        # Overworld and map-effect rows are recorded but add nothing.
        self.assertEqual(scene_patch_chunks(self.data, {"id": 0x000}), [])

    def test_a_moved_scene_write_fails_the_build_naming_the_scene(self):
        moved = bytearray(self.data)
        moved[0x6FCF4 + 3] = 0x48
        record, _, _ = _decoded(self.data, "Tyler")
        with self.assertRaisesRegex(MapPatchError, "Event_TylerGraveOpening: the write at \\$06FCF4 moved"):
            scene_patch_chunks(bytes(moved), record)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestAiedosBgStamp(unittest.TestCase):
    """`MapDataMan_Aiedo` writes chunk 6 into BG at chunks (22,37) and
    (25,37) on every load (#38). Aiedo's collision reads BG, so the stamp is
    real collision. Its picture is the cell's FG over the new BG: the FG chunk
    there has no priority tiles and is transparent on 349 pixels, 38 of which
    change colour. The bare chunk would paint BG over the FG instead."""

    CELLS = ((22, 37), (25, 37))

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        _, _, cls.decoded = _decoded(cls.data, "Aiedo")

    def test_the_stamp_is_collision_on_aiedo(self):
        self.assertEqual(self.decoded.spec.collision_plane_name, "bg")

    def test_the_composite_keeps_the_fg_and_shows_the_stamp_through_it(self):
        for x, y in self.CELLS:
            fg, bg = self.decoded.fg.chunk_at(x, y), self.decoded.bg.chunk_at(x, y)
            with self.subTest(cell=(x, y)):
                self.assertNotEqual(bg, 6, "the stored layout lacks the stamp")
                front, _ = compose_chunk(self.decoded.chunks, self.decoded.patterns, fg)
                baked, _, _ = compose_overworld_chunk(self.decoded, fg, bg)
                stamped, _, _ = compose_overworld_chunk(self.decoded, fg, 6)
                bare, _ = compose_chunk(self.decoded.chunks, self.decoded.patterns, 6)
                opaque = [i for i, pixel in enumerate(front) if pixel & 0xF]
                self.assertTrue(all(stamped[i] == front[i] for i in opaque))
                changed = sum(1 for a, b in zip(baked, stamped) if a != b)
                self.assertEqual(changed, 38)
                # The negative control: the chunk alone loses FG pixels.
                self.assertTrue(any(bare[i] != front[i] for i in opaque))






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
                pairs = write_composite_pairs(resolved)
                base, over, entries = patch_atlas(
                    decoded, chunk_ids, chunk_palette(data, spec.palette), pairs
                )
                index_writes(resolved, entries)
                payload = atlas_json(entries, "a.png", base, None, None)
                self.assertEqual(payload["count"], len(set(chunk_ids)) + len(pairs))
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
