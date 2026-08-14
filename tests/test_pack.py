import hashlib
import json
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

from psiv_tools import png
from psiv_tools.core import read_rom
from psiv_tools.layouts import COLLISION_CELL_PIXELS, collision_type_name, is_blocking
from psiv_tools.maps import extract_maps
from psiv_tools.pack import (
    MANIFEST_NAME,
    MAPS_DIRECTORY,
    PACK_FORMAT_VERSION,
    STANDING_CELL_Y_OFFSET,
    XY_RANGE_NAMES,
    PackError,
    Rect,
    build_pack,
    layout_spec,
    warp_rect,
    xy_range_name,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"

# The three maps the pack fixtures use, and the interior Piata's academy door
# leads to. `MapID_PiataAcademy` is $11; `docs/RUNTIME_DESIGN.md` says $13,
# which is `MapID_PiataAcademy_F1`, the floor above.
MAP_PIATA = 0x10
MAP_PIATA_ACADEMY = 0x11
MAP_PIATA_ITEM_SHOP = 0x1B
MAP_ISLAND_CAVE = 0x92
MAP_MOTAVIA = 0x00

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

MAP_CHANGE = 0x1


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
# XYRangeJmpTbl, transcribed. These need no ROM.
# ---------------------------------------------------------------------------
class TestRect(unittest.TestCase):
    def test_cells_are_half_open(self):
        self.assertEqual(
            list(Rect(2, 3, 2, 2).cells()), [(2, 3), (3, 3), (2, 4), (3, 4)]
        )

    def test_an_empty_rect_is_not_a_rect(self):
        with self.assertRaises(PackError):
            Rect(0, 0, 0, 4)
        with self.assertRaises(PackError):
            Rect(0, 0, 4, -1)


class TestWarpRect(unittest.TestCase):
    """`XYRangeJmpTbl`, entry by entry.

    Sizes are the `addi.w` immediates the routines add to the record's
    coordinate before comparing; the accept window is `d0 <= player < d0 + w`,
    so a rectangle is half-open and the immediate is its width in pixels.
    """

    GRID = (64, 64)

    def rect(self, range_id, x=4, y=4):
        return warp_rect(range_id, x, y, *self.GRID)

    def test_every_jump_table_entry_has_a_name(self):
        self.assertEqual(sorted(XY_RANGE_NAMES), list(range(15)))
        self.assertEqual(xy_range_name(0x9), "XPlus20_YPlus10")
        with self.assertRaises(PackError):
            xy_range_name(0xF)

    def test_box_ranges_are_their_addi_immediates(self):
        for range_id, (name, pixels_x, pixels_y) in {
            0x1: ("XYPlus40", 0x40, 0x40),
            0x2: ("XYPlus20", 0x20, 0x20),
            0x9: ("XPlus20_YPlus10", 0x20, 0x10),
            0xA: ("XPlus10_YPlus60", 0x10, 0x60),
            0xB: ("XPlus40_YPlus20", 0x40, 0x20),
            0xC: ("XPlus10_YPlus20", 0x10, 0x20),
            0xD: ("XPlus60_YPlus10", 0x60, 0x10),
            0xE: ("XPlus40_YPlus10", 0x40, 0x10),
        }.items():
            with self.subTest(range=name):
                self.assertEqual(xy_range_name(range_id), name)
                rect = self.rect(range_id)
                self.assertEqual(
                    (rect.width, rect.height),
                    (pixels_x // COLLISION_CELL_PIXELS, pixels_y // COLLISION_CELL_PIXELS),
                )

    def test_a_rect_starts_at_the_record_coordinate_one_row_down(self):
        # GetChunkAndCollision adds $10 to Y before it derives a cell, so the
        # cell a character at curr_y_pos occupies is one row below curr_y_pos/16.
        rect = warp_rect(0x9, 31, 6, *self.GRID)
        self.assertEqual(rect.to_json(), {"x": 31, "y": 7, "width": 2, "height": 1})
        self.assertEqual(STANDING_CELL_Y_OFFSET, 1)

    def test_exact_is_a_single_cell(self):
        self.assertEqual(
            self.rect(0x3, 22, 20).to_json(), {"x": 22, "y": 21, "width": 1, "height": 1}
        )

    def test_null_never_fires(self):
        self.assertIsNone(self.rect(0x0))

    def test_half_plane_ranges_run_to_the_map_edge(self):
        # XLower/YLower accept a player at or before the coordinate, XHigher/
        # YHigher at or after it; the other bound is the grid.
        self.assertEqual(
            self.rect(0x4, 10, 20).to_json(), {"x": 0, "y": 0, "width": 11, "height": 64}
        )
        self.assertEqual(
            self.rect(0x5, 10, 20).to_json(), {"x": 10, "y": 0, "width": 54, "height": 64}
        )
        self.assertEqual(
            self.rect(0x6, 10, 20).to_json(), {"x": 0, "y": 0, "width": 64, "height": 22}
        )
        self.assertEqual(
            self.rect(0x7, 10, 20).to_json(), {"x": 0, "y": 21, "width": 64, "height": 43}
        )

    def test_xy_lower_with_player_y_keeps_its_2a0_floor(self):
        # `cmpi.w #$2A0,d3 / bls` returns before either coordinate is looked at,
        # so the rectangle starts at the first cell a standing player can be in
        # with curr_y_pos above $2A0: ($2A0 + $10) / 16, plus the standing shift.
        rect = warp_rect(0x8, 20, 60, 128, 128)
        self.assertEqual(rect.to_json(), {"x": 0, "y": 44, "width": 21, "height": 18})
        self.assertIsNone(warp_rect(0x8, 20, 10, 128, 128))

    def test_rects_are_clipped_to_the_map(self):
        self.assertEqual(
            warp_rect(0x1, 62, 62, 64, 64).to_json(),
            {"x": 62, "y": 63, "width": 2, "height": 1},
        )
        self.assertIsNone(warp_rect(0x1, 70, 70, 64, 64))

    def test_an_index_past_the_jump_table_is_rejected(self):
        with self.assertRaises(PackError):
            warp_rect(0xF, 0, 0, 64, 64)
        with self.assertRaises(PackError):
            warp_rect(0x9, 0, 0, 0, 64)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestPackFixture(unittest.TestCase):
    """One three-map pack, built once and inspected from every angle."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls._temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls._temp.name) / "pack"
        cls.manifest = build_pack(cls.data, cls.root, map_ids=FIXTURE_MAPS)
        cls.maps = {
            entry["id"]: json.loads((cls.root / entry["json"]).read_text())
            for entry in cls.manifest["maps"]
        }

    @classmethod
    def tearDownClass(cls):
        cls._temp.cleanup()

    # ------------------------------------------------------------- manifest
    def test_manifest_pins_the_rom_it_came_from(self):
        self.assertEqual(
            self.manifest["rom"]["sha256"], hashlib.sha256(self.data).hexdigest()
        )
        self.assertEqual(self.manifest["rom"]["size_bytes"], len(self.data))
        self.assertEqual(self.manifest["format_version"], PACK_FORMAT_VERSION)
        written = json.loads((self.root / MANIFEST_NAME).read_text())
        self.assertEqual(written, self.manifest)

    def test_the_inventory_describes_every_file_on_disk(self):
        self.assertEqual([e["id"] for e in self.manifest["maps"]], list(FIXTURE_MAPS))
        self.assertEqual(self.manifest["map_count"], len(FIXTURE_MAPS))
        for entry in self.manifest["maps"]:
            with self.subTest(map=entry["symbol"]):
                stem = f"{entry['id']:03X}_{entry['symbol']}"
                self.assertEqual(entry["json"], f"{MAPS_DIRECTORY}/{stem}.json")
                self.assertEqual(entry["png"], f"{MAPS_DIRECTORY}/{stem}.png")
                for key, name in (("json_sha256", "json"), ("png_sha256", "png")):
                    blob = (self.root / entry[name]).read_bytes()
                    self.assertEqual(entry[key], hashlib.sha256(blob).hexdigest())

    def test_filtering_leaves_nothing_skipped(self):
        # All three fixtures carry a layout section, so nothing is dropped; the
        # world maps are only reachable through an unfiltered build.
        self.assertEqual(self.manifest["skipped"], [])

    # ------------------------------------------------------------- Piata JSON
    def test_piatas_collision_grid_is_sixty_four_cells_square(self):
        piata = self.maps[MAP_PIATA]
        collision = piata["collision"]
        self.assertEqual((collision["width_cells"], collision["height_cells"]), (64, 64))
        self.assertEqual(len(collision["rows"]), 64)
        self.assertTrue(all(len(row) == 64 for row in collision["rows"]))
        self.assertTrue(all(0 <= v <= 0xF for row in collision["rows"] for v in row))
        # `loc_51AB2`'s first byte is the $FFFFEC24 flag; Piata reads plane B.
        self.assertEqual(collision["plane"], "bg")
        self.assertEqual(collision["plane_byte"], 1)
        self.assertEqual(piata["dimensions"], {
            "cell_pixels": 16,
            "height_cells": 64, "height_chunks": 32, "height_pixels": 1024,
            "width_cells": 64, "width_chunks": 32, "width_pixels": 1024,
        })

    def test_the_academy_doorway_warp(self):
        warp = self._only_warp(MAP_PIATA, target=MAP_PIATA_ACADEMY)
        self.assertEqual(warp["table"], 2)
        self.assertEqual(warp["trigger"], "map_change_tile")
        self.assertEqual(warp["range"], {"id": 9, "name": "XPlus20_YPlus10"})
        self.assertEqual(warp["rect"], {"x": 31, "y": 7, "width": 2, "height": 1})
        self.assertEqual(warp["source"], {"x_byte": 31, "y_byte": 6, "x_cell": 31, "y_cell": 7})
        self.assertEqual(warp["destination"], {"x_cell": 31, "y_cell": 19})
        self.assertEqual(warp["facing"], {"id": 4, "name": "up"})
        self.assertEqual(warp["target"]["symbol"], "PiataAcademy")
        # Both cells of the rectangle are map-change cells, which is the only
        # reason MapTransTile_MapChange ever reaches this record.
        rows = self.maps[MAP_PIATA]["collision"]["rows"]
        self.assertEqual([rows[7][31], rows[7][32]], [MAP_CHANGE, MAP_CHANGE])

    def test_the_motavia_exit_warp(self):
        warp = self._only_warp(MAP_PIATA, target=MAP_MOTAVIA)
        # Table 1 is walked by MapTransTile_Normal, from ordinary ground: the
        # trigger is the whole strip south of the town wall, not a doorway.
        self.assertEqual(warp["table"], 1)
        self.assertEqual(warp["trigger"], "any_walkable_tile")
        self.assertEqual(warp["range"], {"id": 7, "name": "YHigher"})
        self.assertEqual(warp["rect"], {"x": 0, "y": 48, "width": 64, "height": 16})
        self.assertEqual(warp["destination"], {"x_cell": 46, "y_cell": 143})
        self.assertEqual(warp["target"]["symbol"], "Motavia")

    def test_piatas_six_doorways_are_the_six_table_two_warps(self):
        warps = [w for w in self.maps[MAP_PIATA]["warps"] if w["table"] == 2]
        self.assertEqual(
            [w["target"]["id"] for w in warps],
            [0x11, 0x18, 0x19, 0x1A, 0x1B, 0x1C],
        )
        rows = self.maps[MAP_PIATA]["collision"]["rows"]
        covered = set()
        for warp in warps:
            rect = warp["rect"]
            cells = {
                (x, y)
                for y in range(rect["y"], rect["y"] + rect["height"])
                for x in range(rect["x"], rect["x"] + rect["width"])
            }
            hits = {(x, y) for x, y in cells if rows[y][x] == MAP_CHANGE}
            with self.subTest(target=warp["target"]["symbol"]):
                self.assertTrue(hits, f"{warp['range']} covers no map_change cell")
            covered |= hits
        # Seven type-1 cells, all of them reachable: the academy's rectangle is
        # two cells wide and the other five doors are one cell each.
        everywhere = {
            (x, y)
            for y, row in enumerate(rows)
            for x, value in enumerate(row)
            if value == MAP_CHANGE
        }
        self.assertEqual(len(everywhere), 7)
        self.assertEqual(covered, everywhere)

    def test_piatas_npcs_resolve_to_symbols_and_dialogue(self):
        npcs = self.maps[MAP_PIATA]["npcs"]
        self.assertEqual(len(npcs), 10)
        self.assertTrue(all(npc["symbol"] is not None for npc in npcs))
        self.assertEqual(npcs[0]["symbol"], "NPCType6")
        self.assertEqual(npcs[0]["dialogue_id"], 83)
        self.assertEqual(npcs[0]["facing"], {"id": 4, "name": "up"})
        # Objects are placed with `lsl.w #3` and land on the same grid as the
        # collision cells, with the standing-cell shift on Y.
        self.assertEqual((npcs[0]["x_pixels"], npcs[0]["y_pixels"]), (480, 736))
        self.assertEqual((npcs[0]["x_cell"], npcs[0]["y_cell"]), (30, 47))

    def test_island_caves_chest_holds_a_named_item(self):
        chests = self.maps[MAP_ISLAND_CAVE]["treasure_chests"]
        self.assertEqual(len(chests), 1)
        chest = chests[0]
        self.assertEqual(chest["contents_type"], "item")
        self.assertEqual(chest["item_symbol"], "SolDew")
        self.assertIsNone(chest["meseta"])
        self.assertEqual((chest["x_cell"], chest["y_cell"]), (82, 73))
        self.assertEqual((chest["x_pixels"], chest["y_pixels"]), (1312, 1152))

    def test_music_and_flags_come_from_the_record(self):
        record = extract_maps(self.data)["maps"][MAP_ISLAND_CAVE]
        cave = self.maps[MAP_ISLAND_CAVE]
        self.assertEqual(cave["music"]["id"], record["music"]["id"])
        self.assertEqual(cave["music"]["symbol"], record["music"]["symbol"])
        self.assertEqual(cave["flags"], {
            "poison": 1,
            "random_battles": 1,
            "town_teleport": 0,
            "dungeon_teleport_index": 24,
        })
        self.assertEqual(cave["dialogue_tree"], record["dialogue"]["tree"])

    # ------------------------------------------------------------------ PNGs
    def test_every_png_parses_and_matches_its_declared_size(self):
        for entry in self.manifest["maps"]:
            with self.subTest(map=entry["symbol"]):
                image = (self.root / entry["png"]).read_bytes()
                chunks = parse_png_chunks(image)
                self.assertEqual(
                    [kind for kind, _ in chunks], [b"IHDR", b"PLTE", b"IDAT", b"IEND"]
                )
                header = dict(chunks)[b"IHDR"]
                self.assertEqual(header[9], png.COLOR_TYPE_INDEXED)
                size = struct.unpack(">II", header[:8])
                self.assertEqual(size, (entry["width_pixels"], entry["height_pixels"]))
                declared = self.maps[entry["id"]]["dimensions"]
                self.assertEqual(size, (declared["width_pixels"], declared["height_pixels"]))
                self.assertEqual(
                    size,
                    (declared["width_cells"] * COLLISION_CELL_PIXELS,
                     declared["height_cells"] * COLLISION_CELL_PIXELS),
                )

    def test_the_composed_render_uses_both_planes(self):
        # Plane A over plane B with colour 0 transparent: the FG-only pixels are
        # what makes Piata a town rather than a field of ground tiles, so a
        # render that dropped the overlay would lose them.
        from psiv_tools.layouts import chunk_palette, compose_layout, render_layout
        from psiv_tools.pack import decode_layout_section

        record = extract_maps(self.data)["maps"][MAP_PIATA]
        decoded, _ = decode_layout_section(self.data, layout_spec(record))
        _, _, ground, _ = compose_layout(decoded.chunks, decoded.bg, decoded.patterns)
        _, _, composed, _ = compose_layout(
            decoded.chunks, decoded.fg, decoded.patterns, ground
        )
        self.assertNotEqual(bytes(ground), bytes(composed))
        expected = render_layout(
            decoded.chunks,
            decoded.bg,
            decoded.patterns,
            chunk_palette(self.data, decoded.spec.palette),
            overlay=decoded.fg,
        )
        on_disk = (self.root / f"{MAPS_DIRECTORY}/010_Piata.png").read_bytes()
        self.assertEqual(on_disk, expected)

    # ---------------------------------------------------------- cross-checks
    def test_every_warp_target_is_packed_or_declared_unpacked(self):
        packed = {entry["id"] for entry in self.manifest["maps"]}
        unpacked = {entry["id"] for entry in self.manifest["unpacked_warp_targets"]}
        self.assertFalse(packed & unpacked)
        targets = {
            warp["target"]["id"]
            for payload in self.maps.values()
            for warp in payload["warps"]
        }
        self.assertTrue(targets)
        for map_id in targets:
            with self.subTest(target=hex(map_id)):
                self.assertIn(map_id, packed | unpacked)
        # A filtered pack is expected to point outside itself; that is data, not
        # an error, so it has to be present rather than merely tolerated.
        self.assertIn(MAP_MOTAVIA, unpacked)
        self.assertIn(MAP_PIATA_ACADEMY, unpacked)

    def test_every_doorway_in_the_pack_has_its_map_change_cell(self):
        # These three maps' doors are all in the layout as stored, so nothing
        # lands in the table-2 list. The table-1 count is not zero and is not
        # meant to be: MapTransTile_Normal fires on ordinary ground.
        self.assertEqual(self.manifest["warps"]["doors_without_map_change_cell"], [])
        counts = self.manifest["warps"]["without_map_change_cell"]
        self.assertEqual(counts["table_2"], 0)
        self.assertEqual(counts["table_1"], 4)
        self.assertEqual(
            self.manifest["warps"]["count"],
            sum(len(payload["warps"]) for payload in self.maps.values()),
        )
        self.assertEqual(self.manifest["warps"]["rect_units"], "collision cells")

    def test_building_twice_produces_identical_bytes(self):
        with tempfile.TemporaryDirectory() as other:
            second = Path(other) / "pack"
            build_pack(self.data, second, map_ids=FIXTURE_MAPS)
            first_files = sorted(p.relative_to(self.root) for p in self.root.rglob("*") if p.is_file())
            second_files = sorted(p.relative_to(second) for p in second.rglob("*") if p.is_file())
            self.assertEqual(first_files, second_files)
            self.assertEqual(len(first_files), 1 + 2 * len(FIXTURE_MAPS))
            for name in first_files:
                with self.subTest(file=str(name)):
                    self.assertEqual(
                        (self.root / name).read_bytes(), (second / name).read_bytes()
                    )

    def test_map_json_is_sorted_and_newline_terminated(self):
        text = (self.root / MANIFEST_NAME).read_text()
        self.assertTrue(text.endswith("\n"))
        self.assertEqual(text, json.dumps(json.loads(text), indent=2, sort_keys=True) + "\n")

    def _only_warp(self, map_id, target):
        matches = [w for w in self.maps[map_id]["warps"] if w["target"]["id"] == target]
        self.assertEqual(len(matches), 1)
        return matches[0]


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestPackAgainstTheWholeTable(unittest.TestCase):
    """Things that need the map table but not a pack on disk."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.records = extract_maps(cls.data)["maps"]

    def test_layout_spec_reproduces_the_hand_walked_piata_record(self):
        spec = layout_spec(self.records[MAP_PIATA])
        self.assertEqual(spec.chunk_blobs, (0x122A90, 0x123380))
        self.assertEqual((spec.layout_fg, spec.layout_bg), (0x123710, 0x1237B0))
        self.assertEqual(
            (spec.width_chunks_fg, spec.height_chunks_fg, spec.width_chunks_bg, spec.height_chunks_bg),
            (32, 32, 32, 32),
        )
        # `scroll.mode` is loc_51AB2's first byte, the $FFFFEC24 flag.
        self.assertEqual(spec.collision_plane, 1)
        self.assertEqual(spec.collision_plane_name, "bg")
        self.assertEqual(
            spec.tilesets, ((0x010, 0x11C808), (0x111, 0x11DE68), (0x213, 0x11F4A8))
        )
        self.assertEqual(spec.palette, 0x123990)
        self.assertEqual(spec.label, "Piata")

    def test_the_world_maps_are_the_only_records_without_a_layout(self):
        without = [
            record["id"]
            for record in self.records
            if not record["is_null"] and not record["layout"]["present"]
        ]
        self.assertEqual(without, [0x00, 0x01])
        with self.assertRaises(PackError):
            layout_spec(self.records[0x00])

    def test_the_academy_maps_pad_their_layout_blobs(self):
        # Seven records store a 1024-byte layout for a 32x16 grid. `KosDecomp`
        # never checks a length and no reader can reach past row 16, so the
        # surplus is slack; the pack truncates it and says so.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "pack"
            manifest = build_pack(self.data, root, map_ids=[MAP_PIATA_ACADEMY])
            self.assertEqual(manifest["map_count"], 1)
            entry = manifest["maps"][0]
            self.assertEqual((entry["width_cells"], entry["height_cells"]), (64, 32))
            self.assertEqual((entry["width_pixels"], entry["height_pixels"]), (1024, 512))
            self.assertEqual(
                png_size((root / entry["png"]).read_bytes()), (1024, 512)
            )
            mismatches = manifest["layout_anomalies"]
            self.assertEqual([m["plane"] for m in mismatches], ["fg", "bg"])
            for mismatch in mismatches:
                self.assertEqual(mismatch["kind"], "padded")
                self.assertEqual((mismatch["blob_bytes"], mismatch["grid_bytes"]), (1024, 512))

    def test_clim_center_f2_is_short_a_thousand_bg_cells(self):
        # Its BG pointer is ClimCenter_F3's all-zero 32x32 buffer, but the map
        # is 48x48, so the cartridge draws 1280 cells of whatever the previous
        # map left in Map_Layout_BG. Collision reads plane A here, so only the
        # picture is affected.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "pack"
            manifest = build_pack(self.data, root, map_ids=[MAP_CLIM_CENTER_F2])
            mismatches = manifest["layout_anomalies"]
            self.assertEqual(len(mismatches), 1)
            self.assertEqual(mismatches[0]["kind"], "short")
            self.assertEqual(mismatches[0]["plane"], "bg")
            self.assertEqual(
                (mismatches[0]["blob_bytes"], mismatches[0]["grid_bytes"]), (1024, 2304)
            )
            payload = json.loads((root / manifest["maps"][0]["json"]).read_text())
            self.assertEqual(payload["collision"]["plane"], "fg")

    def test_the_five_doors_a_map_data_manager_opens(self):
        # Every table-2 record in the cartridge covers a map-change cell except
        # these five, and all five are doors written into Map_Layout at load
        # time: MapDataMan_ChkZemaNormal swaps in chunk $5A at Zema's four
        # house entrances, MapDataMan_BioPlantDoor swaps in chunk $29 at
        # BirthValley_B1. The layout as stored has plain solid chunks there.
        with tempfile.TemporaryDirectory() as directory:
            manifest = build_pack(
                self.data,
                Path(directory) / "pack",
                map_ids=[MAP_ZEMA, MAP_BIRTH_VALLEY_B1],
            )
            waiting = [
                (a["symbol"], a["target"]["symbol"], a["rect"])
                for a in manifest["warps"]["doors_without_map_change_cell"]
            ]
            self.assertEqual(waiting, [
                ("Zema", "ZemaHouse1", {"x": 44, "y": 23, "width": 1, "height": 1}),
                ("Zema", "ZemaWeaponShop", {"x": 26, "y": 31, "width": 1, "height": 1}),
                ("Zema", "ZemaInn", {"x": 36, "y": 31, "width": 1, "height": 1}),
                ("Zema", "ZemaItemShop", {"x": 42, "y": 43, "width": 1, "height": 1}),
                ("BirthValley_B1", "BioPlant", {"x": 22, "y": 15, "width": 2, "height": 1}),
            ])

    def test_inner_sanctuary_b1_names_a_chunk_it_never_loads(self):
        # One BG cell of one map names chunk $FF while the map loads 128.
        # `SetupChunksBG` shifts the id into Chunk_Table with no bound, so the
        # cartridge draws a slot it never wrote; the pack substitutes an empty
        # definition and says so rather than inventing a tile.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "pack"
            manifest = build_pack(self.data, root, map_ids=[MAP_INNER_SANCTUARY_B1])
            anomalies = manifest["layout_anomalies"]
            self.assertEqual(len(anomalies), 1)
            self.assertEqual(anomalies[0]["kind"], "undefined_chunk")
            self.assertEqual(anomalies[0]["loaded_chunks"], 128)
            self.assertEqual(anomalies[0]["chunk_ids"], ["0xFF"])
            self.assertEqual(
                anomalies[0]["cells"],
                [{"plane": "bg", "x": 26, "y": 28, "chunk_id": "0xFF"}],
            )
            payload = json.loads((root / manifest["maps"][0]["json"]).read_text())
            # The substituted definition carries no collision bits, so the two
            # cells the stray chunk covers read as ordinary ground.
            rows = payload["collision"]["rows"]
            self.assertEqual(
                [rows[y][x] for y in (56, 57) for x in (52, 53)], [0, 0, 0, 0]
            )

    def test_the_census_reports_what_the_jump_tables_only_permit(self):
        # Three places where the reachable set is narrower than the legal one
        # and a consumer that guesses either from the other gets it wrong:
        # collision type $7 is unnamed but real and walkable, dialogue trees are
        # numbered from 1, and exactly one field object faces $10.
        with tempfile.TemporaryDirectory() as directory:
            manifest = build_pack(
                self.data,
                Path(directory) / "pack",
                map_ids=[MAP_AIEDO_PUB, MAP_KADARY_INN_F1, MAP_MILE_DEAD],
            )
            census = manifest["census"]
            self.assertIn("7", census["collision_types"])
            self.assertEqual(census["npc_facing_bytes"]["16"], 1)
            self.assertEqual(sorted(census["dialogue_trees"], key=int)[-1], "43")
            self.assertNotIn("0", census["dialogue_trees"])
            # `COLLISION_TYPE_NAMES` names eight of sixteen codes; the manifest
            # has to say out loud that the other eight are legal, or psiv-data
            # rejects KadaryInn_F1 for holding a type the disassembly never
            # bothered to label.
            self.assertEqual(manifest["collision"]["type_space"], 16)
            self.assertTrue(manifest["collision"]["named_types_are_not_the_valid_set"])
            self.assertNotIn("0x7", manifest["collision"]["type_names"])
            self.assertEqual(collision_type_name(0x7), "unnamed_7")
            self.assertFalse(is_blocking(0x7))

    def test_the_one_field_object_that_faces_outside_the_constant_set(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "pack"
            manifest = build_pack(self.data, root, map_ids=[MAP_AIEDO_PUB])
            payload = json.loads((root / manifest["maps"][0]["json"]).read_text())
            odd = [n for n in payload["npcs"] if n["facing"]["name"] is None]
            self.assertEqual(len(odd), 1)
            self.assertEqual(odd[0]["index"], 2)
            self.assertEqual(odd[0]["symbol"], "NPCType30")
            self.assertEqual(odd[0]["facing"], {"id": 0x10, "name": None})

    def test_an_unknown_map_id_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(PackError):
                build_pack(self.data, Path(directory) / "pack", map_ids=[999])


if __name__ == "__main__":
    unittest.main()
