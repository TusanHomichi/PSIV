"""The manifest, the file inventory, and the pack as a whole.

What the pack says about itself: the ROM it came from, that every file it
lists exists and hashes as declared, that its JSON is canonical, that two
builds of the same ROM are the same bytes, and the census sections that
record what retail data actually contains.
"""

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from psiv_tools import png
from psiv_tools.layouts import collision_type_name, is_blocking
from psiv_tools.pack import (
    MANIFEST_NAME,
    MAPS_DIRECTORY,
    NPC_SPRITES_NAME,
    PACK_FORMAT_VERSION,
    PARTY_SPRITES_NAME,
    PackError,
    build_pack,
)
from psiv_tools.sprites import PARTY_SYMBOLS

# The fixture module is a sibling; `unittest discover -s tests` puts this
# directory on the path, while `-m unittest tests.test_pack_maps` does not.
try:
    from test_pack_fixture import (
        FIXTURE_MAPS,
        MAP_AIEDO_PUB,
        MAP_KADARY_INN_F1,
        MAP_MILE_DEAD,
        MapTableCase,
        PackFixtureCase,
    )
except ImportError:  # pragma: no cover - invoked as tests.test_pack_*
    from tests.test_pack_fixture import (
        FIXTURE_MAPS,
        MAP_AIEDO_PUB,
        MAP_KADARY_INN_F1,
        MAP_MILE_DEAD,
        MapTableCase,
        PackFixtureCase,
    )


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


class TestManifest(PackFixtureCase):
    """What the manifest claims, checked against the files on disk."""

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

    def test_the_manifest_points_at_the_battle_files(self):
        battle = self.manifest["battle"]
        self.assertEqual(battle["directory"], "battle")
        for entry in battle["files"].values():
            with self.subTest(file=entry["file"]):
                blob = (self.root / entry["file"]).read_bytes()
                self.assertEqual(entry["sha256"], hashlib.sha256(blob).hexdigest())
                self.assertEqual(
                    json.loads(blob)["format_version"], PACK_FORMAT_VERSION
                )
        # Battle data is pack-wide, not per map, so a filtered build still
        # carries all of it.
        self.assertEqual(battle["files"]["enemies"]["count"], 153)
        self.assertEqual(battle["files"]["formations"]["count"], 504)
        self.assertEqual(battle["files"]["levels"]["records"], 937)
        self.assertEqual(battle["ability_effects"]["count"], 44)

    def test_the_battle_art_census_shows_the_body_hole_split(self):
        # `battle.art` is a subtree of the battle fragment, not a replacement
        # for it: the data keys are still there beside it (equipment_rules
        # joined with the party/equipment files).
        battle = self.manifest["battle"]
        self.assertEqual(
            sorted(battle),
            ["ability_effects", "art", "census", "directory", "equipment_rules", "files"],
        )
        art = battle["art"]
        self.assertEqual(art["directory"], "battle/art")
        for entry in art["files"].values():
            with self.subTest(file=entry["file"]):
                blob = (self.root / entry["file"]).read_bytes()
                self.assertEqual(entry["sha256"], hashlib.sha256(blob).hexdigest())
                self.assertEqual(json.loads(blob)["format_version"], PACK_FORMAT_VERSION)
        # The split a consumer needs without opening 196 files: the body layer
        # alone completes 84 of the 153 enemies, and the animated sprite-piece
        # overlay that would finish the other 69 is a separate slice.
        census = art["census"]
        self.assertEqual(census["enemies"], 153)
        self.assertEqual(census["body_complete"], 84)
        self.assertEqual(census["body_holes"], 69)
        self.assertEqual(census["body_complete"] + census["body_holes"], census["enemies"])
        self.assertEqual(census["total_body_holes"], 303)
        self.assertEqual(census["characters"], 11)
        self.assertEqual(census["poses"], 43)
        self.assertEqual(census["enemy_cram_lines"], [1, 2])
        # No enemy body pixel reaches the UI colours, which is what lets the
        # bodies ship without a CRAM line baked in.
        self.assertEqual(census["enemy_body_color_indices"], list(range(14)))
        self.assertEqual(art["files"]["enemies"]["png_count"], 153)
        self.assertEqual(art["files"]["characters"]["png_count"], 43)
        # The 32 battle backgrounds over their 20 shared art blobs, and the
        # binding that decides which one a battle uses. Every one is reachable
        # from one of the three selection paths, so none is dead data.
        self.assertEqual(census["backgrounds"], 32)
        self.assertEqual(census["background_art_blobs"], 20)
        self.assertEqual(census["backgrounds_selectable"], 32)
        self.assertEqual(census["backgrounds_never_selectable"], [])
        self.assertEqual(census["background_event_battles"], 27)
        self.assertEqual(
            census["background_maps_bound"] + census["background_maps_none"], 416
        )
        self.assertEqual(art["files"]["backgrounds"]["png_count"], 32)

    def test_the_event_census_counts_every_reference(self):
        census = self.manifest["census"]["event_ids"]
        total = sum(len(p["events"]) for p in self.maps.values())
        self.assertEqual(sum(census.values()), total)
        self.assertEqual(census["0"], sum(
            1 for p in self.maps.values() for i in p["events"] if i == 0
        ))

    def test_building_twice_produces_identical_bytes(self):
        with tempfile.TemporaryDirectory() as other:
            second = Path(other) / "pack"
            build_pack(self.data, second, map_ids=FIXTURE_MAPS)
            first_files = sorted(p.relative_to(self.root) for p in self.root.rglob("*") if p.is_file())
            second_files = sorted(p.relative_to(second) for p in second.rglob("*") if p.is_file())
            self.assertEqual(first_files, second_files)
            # Every file the pack writes belongs to a category, and each
            # category's size comes from the manifest rather than a literal.
            # The top-level JSONs are counted rather than listed on purpose:
            # four lanes add pack files, and a hardcoded total is a tripwire
            # that fires on their work instead of on a defect. What this still
            # catches is a category going missing or growing a file nothing
            # declares.
            def under(prefix):
                return [f for f in first_files if f.parts[0] == prefix]

            self.assertTrue(under("dialogue"), "build_pack emits the dialogue half")
            art = self.manifest["battle"]["art"]["files"]
            self.assertEqual(
                len(under("battle")),
                # The six data files, plus each art index and its PNGs.
                6 + len(art) + sum(entry["png_count"] for entry in art.values()),
            )
            self.assertEqual(
                len(under("sprites")),
                # Two indexes, the eleven party sheets, one PNG per NPC sheet.
                2 + len(PARTY_SYMBOLS) + self.manifest["sprites"]["npc_sheet_count"],
            )
            patches = sum(
                1 + int(entry["has_overlay"])
                for entry in self.manifest["map_effects"]["layout_write_resolution"]["maps"]
            )
            self.assertEqual(
                len(under("maps")),
                # A JSON and a PNG each, an overlay for a map with priority
                # tiles, and a patch atlas for a map whose effects write one.
                2 * len(FIXTURE_MAPS)
                + self.manifest["overlays"]["maps_with_overlay"]
                + patches,
            )
            # Whatever is left is top-level JSON, manifest included.
            categorised = sum(
                len(under(prefix)) for prefix in ("dialogue", "battle", "sprites", "maps")
            )
            top_level = [f for f in first_files if len(f.parts) == 1]
            self.assertEqual(len(first_files), categorised + len(top_level))
            self.assertIn(Path(MANIFEST_NAME), top_level)
            for name in first_files:
                with self.subTest(file=str(name)):
                    self.assertEqual(
                        (self.root / name).read_bytes(), (second / name).read_bytes()
                    )

    def test_map_json_is_sorted_and_newline_terminated(self):
        text = (self.root / MANIFEST_NAME).read_text()
        self.assertTrue(text.endswith("\n"))
        self.assertEqual(text, json.dumps(json.loads(text), indent=2, sort_keys=True) + "\n")


class TestManifestCensus(MapTableCase):
    """The census sections, which need packs of their own."""

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

    def test_an_unknown_map_id_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(PackError):
                build_pack(self.data, Path(directory) / "pack", map_ids=[999])


if __name__ == "__main__":
    unittest.main()


if __name__ == "__main__":
    unittest.main()
