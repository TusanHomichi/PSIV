"""Field sprite sheets, their sequences, and the interaction bit.

`sprites/party.json`, `sprites/npcs.json`, the PNG sheets both index, and the
`interactable` answer every NPC carries.
"""

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from psiv_tools import png
from psiv_tools.pack import (
    NPC_SPRITES_DIRECTORY,
    NPC_SPRITES_NAME,
    PACK_FORMAT_VERSION,
    PARTY_SPRITES_DIRECTORY,
    PARTY_SPRITES_NAME,
    VEHICLE_SPRITES_DIRECTORY,
    VEHICLE_SPRITES_NAME,
    build_pack,
)
from psiv_tools.sprites import PARTY_SYMBOLS

# The fixture module is a sibling; `unittest discover -s tests` puts this
# directory on the path, while `-m unittest tests.test_pack_maps` does not.
try:
    from test_pack_fixture import (
        MAP_ACADEMY_BASEMENT,
        MAP_ACADEMY_BASEMENT_B2,
        MAP_AIEDO_PUB,
        MapTableCase,
        PackFixtureCase,
        parse_png_chunks,
        png_size,
    )
except ImportError:  # pragma: no cover - invoked as tests.test_pack_*
    from tests.test_pack_fixture import (
        MAP_ACADEMY_BASEMENT,
        MAP_ACADEMY_BASEMENT_B2,
        MAP_AIEDO_PUB,
        MapTableCase,
        PackFixtureCase,
        parse_png_chunks,
        png_size,
    )


class TestSpriteSheets(PackFixtureCase):
    """The party and NPC sheets the pack emits."""

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

    def test_vehicle_sheets_are_the_three_retail_selectors(self):
        index = json.loads((self.root / VEHICLE_SPRITES_NAME).read_text())
        self.assertEqual(index["format_version"], PACK_FORMAT_VERSION)
        self.assertEqual([sheet["vehicle_index"] for sheet in index["sheets"]], [1, 2, 3])
        for sheet in index["sheets"]:
            with self.subTest(selector=sheet["vehicle_index"]):
                self.assertEqual(
                    sheet["png"], f"{VEHICLE_SPRITES_DIRECTORY}/{sheet['id']}.png"
                )
                self.assertEqual((sheet["frame_width"], sheet["frame_height"]), (40, 32))
                self.assertEqual(sheet["palette"]["cram_line"], 3)
                self.assertEqual(sheet["art"]["compression"], "nemesis")

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


class TestInteraction(PackFixtureCase):
    """`render_flags` bit 3: the talk probe and object collision."""

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


class TestInteractionAgainstTheWholeTable(MapTableCase):
    """Objects whose interaction answer needs a pack of its own."""

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


class TestCampReceiptAssets(MapTableCase):
    """The frame-7675 SAT line and the sheet selected by the camp fixture."""

    def test_top_npc_uses_the_live_line_three_sheet_and_status_word(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "pack"
            manifest = build_pack(self.data, root, map_ids=[0x13])
            payload = json.loads((root / manifest["maps"][0]["json"]).read_text())
            top = next(npc for npc in payload["npcs"] if npc["index"] == 0)
            self.assertEqual(top["sprite"]["sheet"], "NPCType2_cb8a59c5")

            sheets = json.loads((root / NPC_SPRITES_NAME).read_text())["sheets"]
            sheet = next(entry for entry in sheets if entry["id"] == "NPCType2_cb8a59c5")
            self.assertEqual(sheet["palette"]["cram_line"], 3)
            self.assertEqual(sheet["palette"]["colors"][13], [98, 68, 172])

        state_path = Path(__file__).resolve().parents[1] / (
            "oracle/states/camp_root_idle_vdp_7675.json"
        )
        state = json.loads(state_path.read_text())
        plane_a = bytes.fromhex(state["regions"]["plane_a"]["bytes_hex"])
        screen_x, screen_y = 28, 4
        buffer_x, buffer_y = (screen_x + 11) % 64, (screen_y + 29) % 32
        offset = (buffer_y * 64 + buffer_x) * 2
        self.assertEqual(int.from_bytes(plane_a[offset:offset + 2], "big"), 0xC6F9)


if __name__ == "__main__":
    unittest.main()
