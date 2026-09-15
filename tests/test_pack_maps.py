"""Per-map records: collision, warps, objects, chests, and the renders.

Everything that is about one map's contents rather than about the pack. The
whole-table checks live here too, because what they check is the map records.
"""

import json
import struct
import tempfile
import unittest
from pathlib import Path

from psiv_tools import png
from psiv_tools.layouts import COLLISION_CELL_PIXELS, is_blocking
from psiv_tools.maps import extract_maps
from psiv_tools.overworld import OVERWORLD_MAP_IDS
from psiv_tools.pack import (
    MAPS_DIRECTORY,
    PACK_FORMAT_VERSION,
    PackError,
    build_pack,
    decode_map_section,
    layout_spec,
)

# The fixture module is a sibling; `unittest discover -s tests` puts this
# directory on the path, while `-m unittest tests.test_pack_maps` does not.
try:
    from test_pack_fixture import (
        MAP_BIRTH_VALLEY_B1,
        MAP_CHANGE,
        MAP_CLIM_CENTER_F2,
        MAP_DEZOLIS,
        MAP_INNER_SANCTUARY_B1,
        MAP_ISLAND_CAVE,
        MAP_MOTAVIA,
        MAP_PIATA,
        MAP_PIATA_ACADEMY,
        MAP_THE_EDGE,
        MAP_ZEMA,
        NULL_MAPS,
        REAL_MAPS,
        MapTableCase,
        PackFixtureCase,
        parse_png_chunks,
        png_size,
    )
except ImportError:  # pragma: no cover - invoked as tests.test_pack_*
    from tests.test_pack_fixture import (
        MAP_BIRTH_VALLEY_B1,
        MAP_CHANGE,
        MAP_CLIM_CENTER_F2,
        MAP_DEZOLIS,
        MAP_INNER_SANCTUARY_B1,
        MAP_ISLAND_CAVE,
        MAP_MOTAVIA,
        MAP_PIATA,
        MAP_PIATA_ACADEMY,
        MAP_THE_EDGE,
        MAP_ZEMA,
        NULL_MAPS,
        REAL_MAPS,
        MapTableCase,
        PackFixtureCase,
        parse_png_chunks,
        png_size,
    )


class TestMapRecords(PackFixtureCase):
    """The three fixture maps, read out of the built pack."""

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
        self.assertEqual(chest["item_id"], 133)
        self.assertEqual(chest["item_symbol"], "Escapipe")
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

    def test_every_map_carries_its_event_list_in_evaluation_order(self):
        # `RunEvents` walks the record's byte list in order and stops at the
        # first id whose condition holds, so the list is a priority list. It is
        # emitted exactly as the record stores it.
        records = {r["id"]: r for r in extract_maps(self.data)["maps"]}
        for map_id, payload in self.maps.items():
            with self.subTest(map=payload["symbol"]):
                self.assertEqual(payload["events"], records[map_id]["events"]["ids"])
                self.assertTrue(all(isinstance(i, int) for i in payload["events"]))
                # `RunEventsJmpTbl` is 128 entries; the record walker already
                # refuses anything past it, so this is the pack restating the
                # bound its consumer needs.
                self.assertTrue(all(0 <= i < 0x80 for i in payload["events"]))
        # Piata's is the null event, which is what most maps carry.
        self.assertEqual(self.maps[MAP_PIATA]["events"], [0])

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


class TestMapRenders(PackFixtureCase):
    """The composed PNGs and the priority overlay beside them."""

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

    def test_a_map_with_no_priority_tiles_gets_no_overlay_file(self):
        # `MapID_TheEdge` draws nothing above sprites, so the pack says null
        # rather than writing a 1024x1024 file of pure transparency.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "pack"
            manifest = build_pack(self.data, root, map_ids=[MAP_THE_EDGE])
            entry = manifest["maps"][0]
            self.assertIsNone(entry["png_over"])
            self.assertIsNone(entry["png_over_sha256"])
            self.assertEqual(entry["priority_tiles"], 0)
            self.assertEqual(
                sorted(p.name for p in (root / MAPS_DIRECTORY).iterdir()),
                ["100_TheEdge.json", "100_TheEdge.png"],
            )
            payload = json.loads((root / entry["json"]).read_text())
            # The key is always present; only its value moves.
            self.assertIn("png_over", payload)
            self.assertIsNone(payload["png_over"])
            overlays = manifest["overlays"]
            self.assertEqual(overlays["maps_with_overlay"], 0)
            self.assertEqual(overlays["maps_without_overlay"], 1)
            self.assertEqual(
                overlays["without_overlay"][0]["symbol"], "TheEdge"
            )

    def test_the_manifest_says_where_the_overlay_sits_in_the_draw_order(self):
        overlays = self.manifest["overlays"]
        self.assertEqual(overlays["priority_bit"], 15)
        self.assertEqual(overlays["file_suffix"], "_over.png")
        self.assertEqual(overlays["transparent_palette_indices"], [0, 16])
        self.assertEqual(
            overlays["maps_with_overlay"] + overlays["maps_without_overlay"],
            self.manifest["map_count"],
        )
        self.assertEqual(
            overlays["priority_tiles"],
            sum(entry["priority_tiles"] for entry in self.manifest["maps"]),
        )
        # `census.priority_tiles` is keyed by the same plane byte the collision
        # section uses: 0 is plane A, 1 is plane B.
        self.assertEqual(
            sum(self.manifest["census"]["priority_tiles"].values()),
            overlays["priority_tiles"],
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


class TestTheWholeMapTable(MapTableCase):
    """All 417 records, and the maps whose data is odd."""

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
        self.assertEqual(without, list(OVERWORLD_MAP_IDS))
        # `layout_spec` reads the record's two layout pointers, which these two
        # records do not have; `decode_map_section` is the one that knows there
        # is a second path and takes it.
        with self.assertRaises(PackError):
            layout_spec(self.records[MAP_MOTAVIA])
        decoded, anomalies, overworld = decode_map_section(
            self.data, self.records[MAP_MOTAVIA]
        )
        self.assertIsNotNone(overworld)
        self.assertEqual(anomalies, [])
        self.assertEqual(
            (decoded.collision.width, decoded.collision.height), (256, 256)
        )
        # And an interior still comes back with no overworld attached.
        _, _, none = decode_map_section(self.data, self.records[MAP_PIATA])
        self.assertIsNone(none)

    def test_the_table_holds_361_real_maps_and_56_nulls(self):
        real = [record for record in self.records if not record["is_null"]]
        self.assertEqual(len(real), REAL_MAPS)
        self.assertEqual(len(self.records) - len(real), NULL_MAPS)
        # Every one of them has a layout by one path or the other, so an
        # unfiltered build packs all 361 and skips only the nulls.
        self.assertEqual(
            sum(
                1
                for record in real
                if record["layout"]["present"] or record["id"] in OVERWORLD_MAP_IDS
            ),
            REAL_MAPS,
        )

    def test_the_map_graph_closes(self):
        # Every transition in the cartridge targets a real record, so an
        # unfiltered build leaves `unpacked_warp_targets` empty. Checking it on
        # the records rather than on a full pack keeps the claim cheap.
        real = {record["id"] for record in self.records if not record["is_null"]}
        targets = {
            entry["target"]["id"]
            for record in self.records
            if not record["is_null"]
            for section in ("transitions", "transitions_2")
            for entry in record[section]["entries"]
        }
        self.assertTrue(targets)
        self.assertEqual(targets - real, set())
        # The two that used to be missing are in there, and they are the only
        # maps every town's exit leads to.
        self.assertTrue(set(OVERWORLD_MAP_IDS) <= targets)

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


class TestTheOverworlds(MapTableCase):
    """The two paged world maps, which pack like any other map."""

    def test_the_overworlds_pack_like_any_other_map(self):
        # Three maps: the two paged world maps and the town whose exit leads to
        # one of them, so both directions of one warp are inside the same pack.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "pack"
            manifest = build_pack(
                self.data, root, map_ids=[MAP_MOTAVIA, MAP_DEZOLIS, MAP_PIATA]
            )
            self.assertEqual(manifest["map_count"], 3)
            self.assertEqual(manifest["skipped"], [])
            maps = {
                entry["id"]: json.loads((root / entry["json"]).read_text())
                for entry in manifest["maps"]
            }

            for map_id, symbol in ((MAP_MOTAVIA, "Motavia"), (MAP_DEZOLIS, "Dezolis")):
                with self.subTest(map=symbol):
                    payload = maps[map_id]
                    self.assertEqual(payload["format_version"], PACK_FORMAT_VERSION)
                    self.assertEqual(payload["symbol"], symbol)
                    self.assertEqual(payload["dimensions"], {
                        "cell_pixels": 16,
                        "height_cells": 256, "height_chunks": 128, "height_pixels": 4096,
                        "width_cells": 256, "width_chunks": 128, "width_pixels": 4096,
                    })
                    self.assertEqual(payload["collision"]["plane"], "bg")
                    self.assertEqual(len(payload["collision"]["rows"]), 256)
                    self.assertTrue(all(len(r) == 256 for r in payload["collision"]["rows"]))
                    # No objects and no chests on either world map, and every
                    # transition is a doorway.
                    self.assertEqual(payload["npcs"], [])
                    self.assertEqual(payload["treasure_chests"], [])
                    self.assertTrue(all(w["table"] == 2 for w in payload["warps"]))
                    entry = next(e for e in manifest["maps"] if e["id"] == map_id)
                    self.assertEqual(
                        png_size((root / entry["png"]).read_bytes()), (4096, 4096)
                    )

            self.assertEqual(len(maps[MAP_MOTAVIA]["warps"]), 29)
            self.assertEqual(len(maps[MAP_DEZOLIS]["warps"]), 14)
            # Piata's exit and Motavia's entrance are now both in the pack, and
            # the destination the town stores is inside the world's trigger.
            exit_warp = next(
                w for w in maps[MAP_PIATA]["warps"] if w["target"]["id"] == MAP_MOTAVIA
            )
            entrance = next(
                w for w in maps[MAP_MOTAVIA]["warps"] if w["target"]["id"] == MAP_PIATA
            )
            self.assertEqual(exit_warp["destination"], {"x_cell": 46, "y_cell": 143})
            self.assertEqual(
                entrance["rect"], {"x": 44, "y": 140, "width": 4, "height": 4}
            )
            rows = maps[MAP_MOTAVIA]["collision"]["rows"]
            self.assertEqual(rows[143][46], MAP_CHANGE)

    def test_only_the_overworlds_carry_layout_patches(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "pack"
            manifest = build_pack(
                self.data, root, map_ids=[MAP_MOTAVIA, MAP_DEZOLIS, MAP_PIATA]
            )
            maps = {
                entry["id"]: json.loads((root / entry["json"]).read_text())
                for entry in manifest["maps"]
            }
            self.assertNotIn("layout_patches", maps[MAP_PIATA])
            self.assertEqual(len(maps[MAP_MOTAVIA]["layout_patches"]), 9)
            self.assertEqual(len(maps[MAP_DEZOLIS]["layout_patches"]), 3)
            patch = next(
                p for p in maps[MAP_MOTAVIA]["layout_patches"]
                if p["event_flag"]["symbol"] == "EventFlag_MotaSpaceport"
            )
            self.assertEqual(patch["writes"], [{
                "plane": "bg", "chunk_x": 26, "chunk_y": 45,
                "cell_x": 52, "cell_y": 90,
                "chunk_ids": ["0x3F"], "displacement": 0,
            }])

            overworld = manifest["overworld"]
            self.assertEqual(overworld["map_ids"], list(OVERWORLD_MAP_IDS))
            self.assertEqual([m["map_id"] for m in overworld["maps"]], [0, 1])
            self.assertEqual(overworld["maps"][0]["compression"], None)
            self.assertEqual(
                overworld["code"]["page_tables"]["fg"]["motavia"], "0x107DC2"
            )
            self.assertEqual(
                overworld["code"]["page_tables"]["bg"]["dezolis"], "0x1175C4"
            )

    def test_the_six_overworld_doors_an_event_opens(self):
        # These are doorways whose trigger covers no map-change cell in the
        # stored layout, exactly like Zema's four -- except that here the pack
        # can name the flag, because the write is in the page loader and the
        # pack decodes it.
        with tempfile.TemporaryDirectory() as directory:
            manifest = build_pack(
                self.data, Path(directory) / "pack", map_ids=[MAP_MOTAVIA, MAP_DEZOLIS]
            )
            waiting = [
                (a["symbol"], a["target"]["symbol"],
                 [o["symbol"] for o in a["opened_by"]])
                for a in manifest["warps"]["doors_without_map_change_cell"]
            ]
            self.assertEqual(waiting, [
                ("Motavia", "MachineCenter", ["EventFlag_MachineCenter"]),
                ("Motavia", "MotaSpaceport", ["EventFlag_MotaSpaceport"]),
                ("Motavia", "TheEdge", ["EventFlag_Reunion"]),
                ("Motavia", "TheEdge", ["EventFlag_Reunion"]),
                ("Motavia", "TheEdge", ["EventFlag_Reunion"]),
                ("Dezolis", "DezoSpaceport", ["EventFlag_DezoSpaceport"]),
            ])
            self.assertEqual(manifest["warps"]["without_map_change_cell"], {
                "table_1": 0, "table_2": 6,
            })

    def test_the_overworlds_bring_the_two_missing_collision_types(self):
        # Sand and ice route to `TileColl_Solid` and appeared nowhere in the
        # cartridge until these two maps were decoded, so the census is the
        # only place a consumer can learn they are real.
        with tempfile.TemporaryDirectory() as directory:
            manifest = build_pack(
                self.data, Path(directory) / "pack", map_ids=[MAP_MOTAVIA, MAP_DEZOLIS]
            )
            census = manifest["census"]["collision_types"]
            self.assertEqual(census["10"], 632)
            self.assertEqual(census["11"], 1704)
            for value in (0xA, 0xB):
                self.assertTrue(is_blocking(value))
                self.assertIn(f"0x{value:X}", manifest["collision"]["type_names"])

    def test_dezolis_repeats_its_last_page(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = build_pack(
                self.data, Path(directory) / "pack", map_ids=[MAP_DEZOLIS]
            )
            anomalies = manifest["layout_anomalies"]
            self.assertEqual([a["kind"] for a in anomalies], ["aliased_pages"] * 2)
            self.assertEqual([a["plane"] for a in anomalies], ["fg", "bg"])
            for anomaly in anomalies:
                self.assertEqual(anomaly["symbol"], "Dezolis")
                self.assertEqual(anomaly["distinct_pages"], 8)
                self.assertEqual(len(anomaly["aliased_pages"]), 8)


if __name__ == "__main__":
    unittest.main()
