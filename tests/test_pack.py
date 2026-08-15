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
from psiv_tools.overworld import DEZOLIS, MOTAVIA, OVERWORLD_MAP_IDS
from psiv_tools.pack import (
    GAME_START_NAME,
    MANIFEST_NAME,
    MAPS_DIRECTORY,
    NPC_COMMANDS_NAME,
    NPC_SPRITES_DIRECTORY,
    NPC_SPRITES_NAME,
    PACK_FORMAT_VERSION,
    PARTY_SPRITES_DIRECTORY,
    PARTY_SPRITES_NAME,
    PackError,
    build_pack,
    decode_map_section,
    layout_spec,
)
from psiv_tools.sprites import PARTY_SYMBOLS

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


class TestImportSurface(unittest.TestCase):
    """`psiv_tools.pack` is one import, whatever the file layout underneath.

    The emitter was split into `warps`, `pack_layouts` and `render` when it
    passed a thousand lines. Nothing that imported from `psiv_tools.pack`
    before should have had to change, and these are the same objects rather
    than copies -- `except PackError` has to catch the one raised in `render`.
    """

    def test_the_names_the_split_moved_are_still_here(self):
        from psiv_tools import pack, pack_layouts, render, warps

        moved = {
            warps: ("PackError", "Rect", "STANDING_CELL_Y_OFFSET", "XY_RANGE_NAMES",
                    "warp_rect", "xy_range_name"),
            pack_layouts: ("UNDEFINED_CHUNK", "decode_layout_section",
                           "decode_map_section", "layout_spec", "unloaded_patterns"),
            render: ("OVERLAY_TRANSPARENT_INDICES", "PLANE_BYTES", "priority_overlay",
                     "priority_tiles"),
        }
        for module, names in moved.items():
            for name in names:
                with self.subTest(module=module.__name__, name=name):
                    self.assertIs(getattr(pack, name), getattr(module, name))

    def test_one_error_class_across_all_four_modules(self):
        from psiv_tools import pack, pack_layouts, render, warps

        self.assertIs(pack.PackError, warps.PackError)
        self.assertIs(pack_layouts.PackError, warps.PackError)
        self.assertIs(render.PackError, warps.PackError)
        self.assertIs(PackError, warps.PackError)


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
                # `png_over` is null for a map with no priority tiles, and then
                # there is deliberately no file to hash.
                if entry["png_over"] is None:
                    self.assertIsNone(entry["png_over_sha256"])
                    self.assertEqual(entry["priority_tiles"], 0)
                else:
                    self.assertEqual(entry["png_over"], f"{MAPS_DIRECTORY}/{stem}_over.png")
                    blob = (self.root / entry["png_over"]).read_bytes()
                    self.assertEqual(entry["png_over_sha256"], hashlib.sha256(blob).hexdigest())
                    self.assertGreater(entry["priority_tiles"], 0)
                self.assertEqual(
                    self.maps[entry["id"]]["png_over"], entry["png_over"]
                )

    def test_every_emitted_json_carries_the_same_format_version(self):
        # The version moved to 1 when field sprites landed. `psiv-data` refuses
        # a pack it does not know, so a file that forgets to stamp it is a file
        # the runtime cannot check.
        self.assertEqual(PACK_FORMAT_VERSION, 1)
        for name in (MANIFEST_NAME, PARTY_SPRITES_NAME, NPC_SPRITES_NAME):
            with self.subTest(file=name):
                payload = json.loads((self.root / name).read_text())
                self.assertEqual(payload["format_version"], PACK_FORMAT_VERSION)
        for payload in self.maps.values():
            self.assertEqual(payload["format_version"], PACK_FORMAT_VERSION)

    def test_filtering_leaves_nothing_skipped(self):
        # `skipped` only ever holds `PtrMap_Null` entries, and none of the three
        # fixtures is one.
        self.assertEqual(self.manifest["skipped"], [])
        # No overworld in this filtered set, so no overworld section content.
        self.assertEqual(self.manifest["overworld"]["maps"], [])

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

    # -------------------------------------------------------- priority overlay
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

    def test_the_manifest_points_at_the_game_start_file(self):
        # Where a new game begins. The manifest carries the headline so a
        # runtime can spawn from it alone; `game_start.json` carries the
        # instruction sites it was read from.
        start = self.manifest["game_start"]
        self.assertEqual(start["file"], GAME_START_NAME)
        blob = (self.root / GAME_START_NAME).read_bytes()
        self.assertEqual(start["sha256"], hashlib.sha256(blob).hexdigest())
        payload = json.loads(blob)
        self.assertEqual(payload["format_version"], PACK_FORMAT_VERSION)
        self.assertEqual(payload["kind"], "game_start")
        # The three sections the extraction is built from, and the one that
        # matters to a runtime with no event engine.
        self.assertEqual(
            sorted(k for k in payload if k not in ("format_version", "kind", "note")),
            ["first_control", "new_game_init", "title_handoff"],
        )
        self.assertEqual(start["party"], ["Chaz"])
        self.assertEqual(start["map"]["symbol"], "PiataAcademy_F1")
        self.assertEqual((start["x_cell"], start["y_cell"]), (48, 19))
        self.assertEqual(start["facing"], {"id": 0, "name": "down"})
        self.assertEqual(start["event_flags_set"], [7])
        # The start map is not one of this fixture's three, and that is fine:
        # `game_start` is pack-wide, not per-map.
        self.assertNotIn(start["map"]["id"], {e["id"] for e in self.manifest["maps"]})

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

    def test_the_event_census_counts_every_reference(self):
        census = self.manifest["census"]["event_ids"]
        total = sum(len(p["events"]) for p in self.maps.values())
        self.assertEqual(sum(census.values()), total)
        self.assertEqual(census["0"], sum(
            1 for p in self.maps.values() for i in p["events"] if i == 0
        ))

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
            # manifest, game_start.json, npc_commands.json, a JSON and a PNG
            # per map, an overlay PNG per map that has priority tiles, the two
            # sprite indexes, the eleven party sheets, and one PNG per
            # deduplicated NPC sheet.
            self.assertEqual(
                len(first_files),
                3 + 2 * len(FIXTURE_MAPS)
                + self.manifest["overlays"]["maps_with_overlay"]
                + 2 + len(PARTY_SYMBOLS)
                + self.manifest["sprites"]["npc_sheet_count"],
            )
            for name in first_files:
                with self.subTest(file=str(name)):
                    self.assertEqual(
                        (self.root / name).read_bytes(), (second / name).read_bytes()
                    )

    def test_map_json_is_sorted_and_newline_terminated(self):
        text = (self.root / MANIFEST_NAME).read_text()
        self.assertTrue(text.endswith("\n"))
        self.assertEqual(text, json.dumps(json.loads(text), indent=2, sort_keys=True) + "\n")

    # -------------------------------------------------------------- sprites
    def test_the_eleven_party_sheets_are_all_there(self):
        index = json.loads((self.root / PARTY_SPRITES_NAME).read_text())
        self.assertEqual(index["format_version"], PACK_FORMAT_VERSION)
        self.assertEqual([sheet["id"] for sheet in index["sheets"]], list(PARTY_SYMBOLS))
        for sheet in index["sheets"]:
            with self.subTest(symbol=sheet["id"]):
                self.assertEqual(sheet["png"], f"{PARTY_SPRITES_DIRECTORY}/{sheet['id']}.png")
                image = (self.root / sheet["png"]).read_bytes()
                self.assertEqual(sheet["png_sha256"], hashlib.sha256(image).hexdigest())
                self.assertEqual(
                    png_size(image),
                    (sheet["frame_width"] * sheet["frame_count"], sheet["frame_height"]),
                )
                # Every party sprite is 16x32 with twelve distinct frames: three
                # per facing, and the right-facing three are the left ones with
                # the pattern word's H-flip bit set.
                self.assertEqual((sheet["frame_width"], sheet["frame_height"]), (16, 32))
                self.assertEqual(sheet["frame_count"], 12)
                self.assertEqual(sheet["palette"]["cram_line"], 2)
                self.assertEqual(sheet["palette"]["source"], "Pal_Init_Line_3")
                self.assertEqual(
                    sorted(sheet["sequences"]),
                    sorted(
                        f"{kind}_{name}"
                        for kind in ("idle", "walk")
                        for name in ("down", "up", "left", "right")
                    ),
                )
                self.assertEqual(sheet["art"]["tile_count"], 72)

    def test_sheet_pngs_are_indexed_with_colour_zero_transparent(self):
        index = json.loads((self.root / NPC_SPRITES_NAME).read_text())
        for sheet in index["sheets"]:
            with self.subTest(sheet=sheet["id"]):
                chunks = dict(parse_png_chunks((self.root / sheet["png"]).read_bytes()))
                self.assertEqual(chunks[b"IHDR"][9], png.COLOR_TYPE_INDEXED)
                self.assertEqual(len(chunks[b"PLTE"]), 16 * 3)
                self.assertEqual(chunks[b"tRNS"][0], 0)
                self.assertEqual(
                    list(chunks[b"PLTE"]),
                    [channel for colour in sheet["palette"]["colors"] for channel in colour],
                )

    def test_every_npc_resolves_to_a_sheet_or_says_why_not(self):
        index = json.loads((self.root / NPC_SPRITES_NAME).read_text())
        by_id = {sheet["id"]: sheet for sheet in index["sheets"]}
        self.assertEqual(index["sheet_count"], len(by_id))
        seen = set()
        for payload in self.maps.values():
            for npc in payload["npcs"]:
                with self.subTest(map=payload["symbol"], npc=npc["index"]):
                    if npc["sprite"] is None:
                        # An object with no sprite has to say which routine
                        # decided that, or the runtime is just guessing.
                        self.assertTrue(npc["sprite_reason"])
                        continue
                    self.assertIsNone(npc["sprite_reason"])
                    sheet = by_id[npc["sprite"]["sheet"]]
                    self.assertEqual(npc["sprite"]["sheets"], NPC_SPRITES_NAME)
                    self.assertIn(npc["sprite"]["idle_sequence"], sheet["sequences"])
                    self.assertIn(npc["sprite"]["walk_sequence"], sheet["sequences"])
                    seen.add(npc["sprite"]["sheet"])
        # Nothing is emitted that nothing points at.
        self.assertEqual(seen, set(by_id))

    def test_sheets_are_shared_across_maps_and_named_for_their_content(self):
        index = json.loads((self.root / NPC_SPRITES_NAME).read_text())
        for sheet in index["sheets"]:
            with self.subTest(sheet=sheet["id"]):
                self.assertEqual(sheet["png"], f"{NPC_SPRITES_DIRECTORY}/{sheet['id']}.png")
                self.assertGreaterEqual(sheet["placements"], 1)
        placed = sum(sheet["placements"] for sheet in index["sheets"])
        self.assertEqual(placed, self.manifest["sprites"]["npc_placements"])
        self.assertEqual(
            placed + self.manifest["sprites"]["artless_objects"],
            sum(len(payload["npcs"]) for payload in self.maps.values()),
        )

    def test_every_npc_says_whether_it_answers_the_talk_probe(self):
        index = json.loads((self.root / NPC_SPRITES_NAME).read_text())
        types = {entry["object_id"]: entry for entry in index["field_objects"]["types"]}
        self.assertEqual(index["field_objects"]["render_flags_bit"], 3)
        self.assertEqual(index["field_objects"]["count"], 222)
        # The complete set of encodings retail uses to write the bit.
        self.assertEqual(
            index["field_objects"]["instruction_forms"],
            ["bset #3, $2(a4)", "bclr #3, $2(a4)"],
        )
        # Both readers are named, because the bit is not dialogue-only: an
        # object with it clear is also invisible to the walker's collision.
        self.assertEqual(
            [reader["routine"] for reader in index["field_objects"]["tested_by"]],
            ["Interaction_ChkObjects", "FieldObj_DoObjCollision"],
        )
        for payload in self.maps.values():
            for npc in payload["npcs"]:
                with self.subTest(map=payload["symbol"], npc=npc["index"]):
                    self.assertIsInstance(npc["interactable"], bool)
                    # The per-map bool and the per-type table cannot disagree.
                    self.assertEqual(
                        npc["interactable"], types[npc["object_id"]]["interactable"]
                    )
                    self.assertTrue(types[npc["object_id"]]["source"])

    def test_the_interaction_census_counts_both_answers(self):
        counted = self.manifest["census"]["npc_interactable"]
        self.assertEqual(
            sum(counted.values()),
            sum(len(payload["npcs"]) for payload in self.maps.values()),
        )
        interaction = self.manifest["sprites"]["interaction"]
        self.assertEqual(interaction["render_flags_bit"], 3)
        self.assertEqual(
            interaction["types_interactable"] + interaction["types_not_interactable"], 222
        )
        self.assertEqual(interaction["types_changing_at_runtime"], ["FellowPenguin"])

    def test_the_sequences_carry_per_frame_durations_in_game_frames(self):
        index = json.loads((self.root / PARTY_SPRITES_NAME).read_text())
        chaz = next(sheet for sheet in index["sheets"] if sheet["id"] == "Chaz")
        walk = chaz["sequences"]["walk_down"]
        self.assertEqual([frame["index"] for frame in walk["frames"]], [0, 1, 0, 2])
        # `SprMapsData_ChazDown` stores $0A, and the counter is spent by a
        # `subq/bpl` pair, so each frame holds for eleven frames.
        self.assertEqual([frame["duration_ticks"] for frame in walk["frames"]], [11] * 4)
        self.assertEqual(chaz["sequences"]["idle_down"]["frames"], [walk["frames"][0]])

    def test_the_manifest_states_where_sprite_colours_come_from(self):
        sprites = self.manifest["sprites"]
        self.assertEqual(sprites["palette"]["cram_lines"], {"0x00": 0, "0x20": 1, "0x40": 2, "0x60": 3})
        self.assertEqual(sprites["palette"]["map_palette_lines"], [0, 1, 3])
        self.assertEqual(sprites["palette"]["fixed_line"], 2)
        self.assertEqual(sprites["palette"]["fixed_line_rom_offset"], "0x296300")
        self.assertEqual(sprites["palette"]["party_line"], 2)
        # Walking is eight frames per collision cell at the normal speed.
        self.assertEqual(sprites["walk"]["frames_per_cell"], 8)
        self.assertEqual(sprites["walk"]["normal_block"], 1)
        self.assertEqual(
            [block["frames_per_cell"] for block in sprites["walk"]["blocks"]], [16, 8, 4]
        )

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
            # `FieldObj_Animate` adds `facing_dir` to `mappings_addr` with no
            # bound, so $10 follows the long *past* this object's four-entry
            # table and lands in the next one. The pack reproduces that rather
            # than rounding the byte down to a real direction, and names the
            # sequence after the byte so nobody reads it as one.
            self.assertEqual(odd[0]["sprite"]["facing"], "facing_0x10")
            self.assertEqual(odd[0]["sprite"]["idle_sequence"], "idle_facing_0x10")
            index = json.loads((root / NPC_SPRITES_NAME).read_text())
            sheet = next(s for s in index["sheets"] if s["id"] == odd[0]["sprite"]["sheet"])
            self.assertEqual(
                sorted(sheet["sequences"]), ["idle_facing_0x10", "walk_facing_0x10"]
            )

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

    def test_the_academy_basement_monsters_do_not_answer_the_talk_probe(self):
        # The live bug: a runtime that probes every nearby object reaches these
        # two, finds dialogue id 0, and falls through its tree to whatever the
        # map's tree 33 chains to. Retail never reaches them at all --
        # `FieldObj_Xanafalgue` and `FieldObj_Igglanova` both `bclr #3`.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "pack"
            manifest = build_pack(
                self.data, root, map_ids=[MAP_ACADEMY_BASEMENT, MAP_ACADEMY_BASEMENT_B2]
            )
            npcs = {}
            for entry in manifest["maps"]:
                payload = json.loads((root / entry["json"]).read_text())
                for npc in payload["npcs"]:
                    npcs.setdefault(npc["symbol"], []).append(npc)
            for symbol in ("Xanafalgue", "Igglanova"):
                with self.subTest(symbol=symbol):
                    self.assertTrue(npcs[symbol])
                    for npc in npcs[symbol]:
                        self.assertFalse(npc["interactable"])
            # The invisible blocks in the same room are the other way round:
            # no art at all, and still interactable.
            for npc in npcs["InvisibleBlock"]:
                self.assertTrue(npc["interactable"])
                self.assertIsNone(npc["sprite"])

    def test_an_unknown_map_id_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(PackError):
                build_pack(self.data, Path(directory) / "pack", map_ids=[999])


if __name__ == "__main__":
    unittest.main()
