"""Battle art as the runtime pack emits it.

`test_battle_art.py` proves the decode. This proves the emission: that the
files land where the manifest says, that an enemy body carries no CRAM line,
that a pose is addressed by its index, and that two builds are the same bytes.
"""

import atexit
import hashlib
import json
import re
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

from psiv_tools import png
from psiv_tools.battle_art import (
    CHARACTER_COUNT,
    CHARACTER_PALETTE_TABLE,
    ENEMY_COUNT,
    ENEMY_CRAM_LINES,
    RAW_MAPPING_ENEMY_IDS,
    UI_COLOR_SITES,
)
from psiv_tools.battle_art_pack import (
    ART_DIRECTORY,
    BACKGROUND_ART_NAME,
    BACKGROUND_PNG_DIRECTORY,
    BATTLE_SETUP_BACKGROUND,
    BATTLE_SETUP_SIGNATURE,
    DARK_FORCE_2_SWAP,
    EVENT_BG_COUNT,
    EVENT_BG_INDEXES,
    FIELD_MAP_BG_COUNT,
    FIELD_MAP_BG_INDEXES,
    MOTA_BG_COUNT,
    MOTA_BG_INDEXES,
    MOTA_STORED_BIAS,
    NO_BACKGROUND,
    CHARACTER_ART_NAME,
    CHARACTER_PNG_DIRECTORY,
    ENEMY_ART_NAME,
    ENEMY_OVERLAY_ART_NAME,
    ENEMY_PNG_DIRECTORY,
    LINE_DEPENDENT_INDEX,
    PLACEHOLDER_COLOR,
    POSE_LABEL_SOURCE,
    POSE_LABELS,
    TRANSPARENT_INDEX,
    VEHICLE_VARIANTS,
    emit_battle_art,
)
from psiv_tools.core import read_rom

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
DISASM = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm" / "ps4.asm"

PACK_VERSION = 1

#: The body layer alone leaves 69 of the 153 enemies with cells the animated
#: sprite-piece overlay covers. Pinned so the split cannot drift unnoticed.
BODIES_COMPLETE = 84
BODIES_HOLED = 69
TOTAL_BODY_HOLES = 303
TOTAL_POSES = 43
#: `BattleBGArtPtrs` is 32 entries over 20 distinct art blobs.
BACKGROUNDS = 32
BACKGROUND_ART_BLOBS = 20
#: `MapID_AirCastleSpace`, the 417th map id, which the 416-byte table cannot
#: reach -- the same off-by-one `Battle_EnemyFormationIndexes` has.
MAP_AIR_CASTLE_SPACE = 0x1A0

_emitted: tuple[Path, dict] | None = None
_temp: tempfile.TemporaryDirectory | None = None


def emitted() -> tuple[Path, dict]:
    """Emit the battle art once for the whole module."""
    global _emitted, _temp
    if _emitted is None:
        _temp = tempfile.TemporaryDirectory()
        atexit.register(_temp.cleanup)
        root = Path(_temp.name)
        fragment = emit_battle_art(read_rom(ROM), root, PACK_VERSION)
        _emitted = (root, fragment)
    return _emitted


def png_chunks(data: bytes) -> dict[bytes, bytes]:
    if data[:8] != png.PNG_SIGNATURE:
        raise AssertionError("missing PNG signature")
    chunks: dict[bytes, bytes] = {}
    idat = b""
    pos = 8
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        payload = data[pos + 8:pos + 8 + length]
        if kind == b"IDAT":
            idat += payload
        else:
            chunks[kind] = payload
        pos += 12 + length
    chunks[b"IDAT"] = idat
    return chunks


def png_pixels(data: bytes) -> tuple[int, int, bytes]:
    chunks = png_chunks(data)
    width, height = struct.unpack(">II", chunks[b"IHDR"][:8])
    raw = zlib.decompress(chunks[b"IDAT"])
    rows = b"".join(raw[y * (width + 1) + 1:(y + 1) * (width + 1)] for y in range(height))
    return width, height, rows


class TestPoseLabels(unittest.TestCase):
    """The label table itself, which needs no ROM."""

    def test_eleven_characters_and_forty_three_poses(self):
        self.assertEqual(len(POSE_LABELS), CHARACTER_COUNT)
        self.assertEqual(sum(len(v) for v in POSE_LABELS.values()), TOTAL_POSES)

    def test_pose_zero_is_idle_for_everybody(self):
        # `loc_8244` indexes a pose with `d0 = 4 * pose`, and every block's
        # first long after the art pointer is the idle mapping.
        for symbol, labels in POSE_LABELS.items():
            with self.subTest(symbol=symbol):
                self.assertEqual(labels[0], "Idle")


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestEmittedFiles(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root, cls.fragment = emitted()
        cls.enemies = json.loads((cls.root / ENEMY_ART_NAME).read_text())
        cls.characters = json.loads((cls.root / CHARACTER_ART_NAME).read_text())

    def test_the_fragment_describes_every_file_on_disk(self):
        files = self.fragment["files"]
        self.assertEqual(self.fragment["directory"], ART_DIRECTORY)
        for key, name in (
            ("enemies", ENEMY_ART_NAME),
            ("enemy_overlays", ENEMY_OVERLAY_ART_NAME),
            ("characters", CHARACTER_ART_NAME),
            ("backgrounds", BACKGROUND_ART_NAME),
        ):
            with self.subTest(key=key):
                blob = (self.root / name).read_bytes()
                self.assertEqual(files[key]["file"], name)
                self.assertEqual(files[key]["sha256"], hashlib.sha256(blob).hexdigest())
        on_disk = sorted(p for p in (self.root / ART_DIRECTORY).rglob("*") if p.is_file())
        self.assertEqual(
            len(on_disk),
            len(files) + sum(entry["png_count"] for entry in files.values()),
        )
        self.assertEqual(files["enemies"]["png_count"], ENEMY_COUNT)
        self.assertEqual(files["characters"]["png_count"], TOTAL_POSES)
        self.assertEqual(files["backgrounds"]["png_count"], BACKGROUNDS)
        self.assertGreater(files["enemy_overlays"]["png_count"], files["enemy_overlays"]["count"])

    def test_every_json_carries_the_pack_format_version(self):
        backgrounds = json.loads((self.root / BACKGROUND_ART_NAME).read_text())
        overlays = json.loads((self.root / ENEMY_OVERLAY_ART_NAME).read_text())
        for payload in (self.enemies, overlays, self.characters, backgrounds):
            self.assertEqual(payload["format_version"], PACK_VERSION)

    def test_json_is_canonical_and_newline_terminated(self):
        for name in (
            ENEMY_ART_NAME,
            ENEMY_OVERLAY_ART_NAME,
            CHARACTER_ART_NAME,
            BACKGROUND_ART_NAME,
        ):
            with self.subTest(file=name):
                text = (self.root / name).read_text()
                self.assertTrue(text.endswith("\n"))
                self.assertEqual(
                    text, json.dumps(json.loads(text), indent=2, sort_keys=True) + "\n"
                )

    def test_building_twice_produces_identical_bytes(self):
        with tempfile.TemporaryDirectory() as other:
            second = Path(other)
            emit_battle_art(read_rom(ROM), second, PACK_VERSION)
            first_files = sorted(
                p.relative_to(self.root) for p in (self.root / ART_DIRECTORY).rglob("*")
                if p.is_file()
            )
            second_files = sorted(
                p.relative_to(second) for p in (second / ART_DIRECTORY).rglob("*")
                if p.is_file()
            )
            self.assertEqual(first_files, second_files)
            for name in first_files:
                with self.subTest(file=str(name)):
                    self.assertEqual(
                        (self.root / name).read_bytes(), (second / name).read_bytes()
                    )


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestEnemyBodies(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root, cls.fragment = emitted()
        cls.payload = json.loads((cls.root / ENEMY_ART_NAME).read_text())
        cls.entries = cls.payload["enemies"]

    def test_one_hundred_and_fifty_three_in_id_order(self):
        self.assertEqual(len(self.entries), ENEMY_COUNT)
        self.assertEqual([e["id"] for e in self.entries], list(range(ENEMY_COUNT)))

    def test_the_stored_width_is_a_half_width(self):
        # `loc_10ED4` does `add.w d1,d1` before `PlaneMapToRAM`, so the drawn
        # grid is twice the byte the record stores. Both are emitted, and the
        # PNG is the doubled one.
        for entry in self.entries:
            with self.subTest(symbol=entry["symbol"]):
                self.assertEqual(entry["width_cells"], 2 * entry["half_width_cells"])
                self.assertEqual(entry["width_pixels"], entry["width_cells"] * 8)
                self.assertEqual(entry["height_pixels"], entry["height_cells"] * 8)
                image = (self.root / entry["png"]).read_bytes()
                width, height, _ = png_pixels(image)
                self.assertEqual((width, height), (entry["width_pixels"], entry["height_pixels"]))
                self.assertEqual(entry["png_sha256"], hashlib.sha256(image).hexdigest())

    def test_no_body_pixel_reaches_the_line_dependent_colour(self):
        # The whole line-agnostic claim in one assertion: index 15 is the only
        # CRAM entry that differs between an enemy's two possible lines, and
        # index 14 is the other UI colour. No body pixel is either, across all
        # 153, so the emitted palette carries no slot choice at all.
        used: set[int] = set()
        for entry in self.entries:
            _, _, pixels = png_pixels((self.root / entry["png"]).read_bytes())
            used.update(pixels)
        self.assertEqual(sorted(used), list(range(14)))
        self.assertNotIn(14, used)
        self.assertNotIn(LINE_DEPENDENT_INDEX, used)
        self.assertEqual(
            self.payload["palette_layout"]["body_color_indices_used"], sorted(used)
        )

    def test_the_palette_layout_carries_both_line_variants(self):
        layout = self.payload["palette_layout"]
        self.assertEqual(layout["cram_lines"], list(ENEMY_CRAM_LINES))
        by_line = layout["index_15_by_cram_line"]
        for _, line, index, value in UI_COLOR_SITES:
            if index == LINE_DEPENDENT_INDEX:
                with self.subTest(line=line):
                    self.assertEqual(by_line[str(line)], f"0x{value:04X}")
        # Both of the enemy's own lines are present, not just one.
        for line in ENEMY_CRAM_LINES:
            self.assertIn(str(line), by_line)

    def test_the_png_palette_bakes_no_line(self):
        entry = self.entries[0]
        chunks = png_chunks((self.root / entry["png"]).read_bytes())
        self.assertEqual(chunks[b"IHDR"][9], png.COLOR_TYPE_INDEXED)
        plte = chunks[b"PLTE"]
        self.assertEqual(len(plte), 16 * 3)
        self.assertEqual(list(chunks[b"tRNS"])[TRANSPARENT_INDEX], 0)
        # Index 15 holds the placeholder, which is none of the three real
        # line-dependent values -- so no consumer can mistake it for one.
        slot = LINE_DEPENDENT_INDEX * 3
        self.assertEqual(tuple(plte[slot:slot + 3]), PLACEHOLDER_COLOR)
        real = {value for _, _, index, value in UI_COLOR_SITES if index == LINE_DEPENDENT_INDEX}
        self.assertNotIn(0x0000, real)

    def test_the_eleven_table_words_travel_with_each_enemy(self):
        for entry in self.entries:
            with self.subTest(symbol=entry["symbol"]):
                self.assertEqual(len(entry["palette_words"]), 11)
                for word in entry["palette_words"]:
                    self.assertRegex(word, r"^0x[0-9A-F]{4}$")

    def test_the_body_hole_split(self):
        complete = [e for e in self.entries if e["body_complete"]]
        holed = [e for e in self.entries if not e["body_complete"]]
        self.assertEqual(len(complete), BODIES_COMPLETE)
        self.assertEqual(len(holed), BODIES_HOLED)
        self.assertTrue(all(e["body_holes"] == 0 for e in complete))
        self.assertTrue(all(e["body_holes"] > 0 for e in holed))
        self.assertEqual(sum(e["body_holes"] for e in self.entries), TOTAL_BODY_HOLES)

    def test_the_two_enemies_whose_mapping_is_not_enigma(self):
        raw = {e["id"] for e in self.entries if e["mapping"]["format"] == "raw_words"}
        self.assertEqual(raw, set(RAW_MAPPING_ENEMY_IDS))
        self.assertEqual(self.payload["raw_mapping_enemy_ids"], sorted(RAW_MAPPING_ENEMY_IDS))

    def test_each_enemy_names_its_three_art_blobs_in_bank_order(self):
        self.assertEqual(self.payload["art_bank_order"], [3, 1, 2])
        for entry in self.entries:
            with self.subTest(symbol=entry["symbol"]):
                self.assertEqual(len(entry["art"]), 3)
                self.assertEqual(entry["base_pattern"], 0)
                self.assertLessEqual(entry["patterns_used"], entry["bank_patterns"])
                # `first_pattern` places each blob in the concatenated bank.
                self.assertEqual(
                    sorted(a["first_pattern"] for a in entry["art"])[0], 0
                )


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestCharacterPoses(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root, cls.fragment = emitted()
        cls.payload = json.loads((cls.root / CHARACTER_ART_NAME).read_text())
        cls.entries = cls.payload["characters"]

    def test_eleven_characters_and_forty_three_poses(self):
        self.assertEqual(len(self.entries), CHARACTER_COUNT)
        self.assertEqual(self.payload["pose_count"], TOTAL_POSES)
        self.assertEqual(sum(e["pose_count"] for e in self.entries), TOTAL_POSES)

    def test_a_pose_is_keyed_on_its_index_and_labelled_advisory(self):
        for entry in self.entries:
            labels = POSE_LABELS[entry["symbol"]]
            for position, pose in enumerate(entry["poses"]):
                with self.subTest(symbol=entry["symbol"], pose=position):
                    # The index is the key; the position in the list is it.
                    self.assertEqual(pose["pose"], position)
                    self.assertEqual(pose["label"], labels[position])
                    self.assertEqual(pose["label_source"], POSE_LABEL_SOURCE)
            self.assertEqual(entry["poses"][0]["label"], "Idle")
        self.assertTrue(self.payload["pose_labels"]["advisory"])

    def test_every_pose_png_is_a_six_by_six_grid(self):
        self.assertEqual(self.payload["grid_cells"], [6, 6])
        for entry in self.entries:
            for pose in entry["poses"]:
                with self.subTest(symbol=entry["symbol"], pose=pose["pose"]):
                    image = (self.root / pose["png"]).read_bytes()
                    self.assertEqual(pose["png_sha256"], hashlib.sha256(image).hexdigest())
                    width, height, _ = png_pixels(image)
                    self.assertEqual((width, height), (48, 48))
                    self.assertTrue(pose["png"].startswith(CHARACTER_PNG_DIRECTORY))

    def test_the_shared_line_three_palette_ships_all_four_variants(self):
        palette = self.payload["palette"]
        self.assertEqual(palette["cram_line"], 3)
        variants = palette["variants"]
        self.assertEqual(len(variants), CHARACTER_PALETTE_TABLE["entry_count"])
        self.assertEqual([v["name"] for v in variants], list(VEHICLE_VARIANTS))
        for variant in variants:
            with self.subTest(variant=variant["name"]):
                self.assertEqual(len(variant["words"]), 15)
                self.assertEqual(len(variant["colors"]), 16)
        # The three UI immediates `loc_7634` writes, verified against the ROM.
        self.assertEqual(len(palette["ui_colors"]), len(UI_COLOR_SITES))
        self.assertEqual(palette["png_baked_variant"], 0)

    def test_the_two_characters_that_carry_a_second_art_blob(self):
        # Demi and Wren load their gun art mid-block; it applies to the poses
        # after it, which is why a pose names its own art pointer.
        multiple = {e["symbol"] for e in self.entries if len(e["art"]) > 1}
        self.assertEqual(multiple, {"Demi", "Wren"})
        for entry in self.entries:
            offsets = {a["rom_offset"] for a in entry["art"]}
            for pose in entry["poses"]:
                self.assertIn(pose["art_rom_offset"], offsets)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestBackgrounds(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root, cls.fragment = emitted()
        cls.payload = json.loads((cls.root / BACKGROUND_ART_NAME).read_text())
        cls.entries = cls.payload["backgrounds"]
        cls.selection = cls.payload["selection"]
        cls.data = read_rom(ROM)

    def test_thirty_two_entries_over_twenty_art_blobs(self):
        self.assertEqual(len(self.entries), BACKGROUNDS)
        self.assertEqual([e["index"] for e in self.entries], list(range(BACKGROUNDS)))
        # Several backgrounds share one art blob under different palettes,
        # which is why the two counts differ.
        self.assertEqual(self.payload["distinct_art_blobs"], BACKGROUND_ART_BLOBS)

    def test_every_background_is_a_full_plane(self):
        self.assertEqual(self.payload["plane_size_cells"], [64, 24])
        for entry in self.entries:
            with self.subTest(symbol=entry["symbol"]):
                image = (self.root / entry["png"]).read_bytes()
                self.assertEqual(entry["png_sha256"], hashlib.sha256(image).hexdigest())
                self.assertEqual(png_pixels(image)[:2], (512, 192))
                self.assertTrue(entry["png"].startswith(BACKGROUND_PNG_DIRECTORY))

    def test_a_background_bakes_its_palette_because_it_has_only_one(self):
        # The opposite of an enemy body: `loc_6C3C` loads exactly one palette
        # with the art, so there is no slot choice to keep out of the image.
        for entry in self.entries:
            with self.subTest(symbol=entry["symbol"]):
                palette = entry["palette"]
                self.assertEqual(palette["cram_line"], 0)
                self.assertEqual(palette["first_index"], 1)
                self.assertEqual(palette["color_count"], 13)
                self.assertTrue(palette["index_0_forced_black"])
                self.assertEqual(len(palette["colors"]), 13)

    def test_the_three_selection_tables_are_where_the_routine_points(self):
        # Located by following `Battle_SetupBackground`'s own operands, so this
        # pins the routine and all three tables at once.
        self.assertEqual(
            self.data[BATTLE_SETUP_BACKGROUND:
                      BATTLE_SETUP_BACKGROUND + len(BATTLE_SETUP_SIGNATURE)],
            BATTLE_SETUP_SIGNATURE,
        )
        self.assertEqual(self.selection["order"],
                         ["event_battle", "field_map", "motavia_terrain"])
        self.assertEqual(
            self.selection["vehicle_mounted"]["selector"], "Vehicle_Index != 0"
        )
        for key, offset, count in (
            ("event_battle", EVENT_BG_INDEXES, EVENT_BG_COUNT),
            ("field_map", FIELD_MAP_BG_INDEXES, FIELD_MAP_BG_COUNT),
            ("motavia_terrain", MOTA_BG_INDEXES, MOTA_BG_COUNT),
        ):
            with self.subTest(table=key):
                block = self.selection[key]
                self.assertEqual(block["rom_offset"], f"0x{offset:06X}")
                self.assertEqual(block["count"], count)
                self.assertEqual(len(block["indexes"]), count)
                self.assertEqual(block["indexes"], list(self.data[offset:offset + count]))

    def test_each_table_ends_where_the_next_thing_begins(self):
        # None of the three counts is a guess: the event table's end rounds up
        # to the map table's start, the map table's end is `loc_6E48`, and the
        # Motavia table's end is `loc_58716`.
        self.assertEqual((EVENT_BG_INDEXES + EVENT_BG_COUNT + 1) & ~1, FIELD_MAP_BG_INDEXES)
        self.assertEqual(FIELD_MAP_BG_INDEXES + FIELD_MAP_BG_COUNT, 0x006E48)
        self.assertEqual(MOTA_BG_INDEXES + MOTA_BG_COUNT, 0x058716)

    def test_the_map_table_is_one_short_of_the_map_id_space(self):
        # The same off-by-one `Battle_EnemyFormationIndexes` has: 416 bytes for
        # 417 map ids. `MapID_AirCastleSpace` would read into `loc_6E48`.
        self.assertEqual(FIELD_MAP_BG_COUNT, 416)
        self.assertGreater(MAP_AIR_CASTLE_SPACE, FIELD_MAP_BG_COUNT - 1)
        self.assertEqual(len(self.selection["field_map"]["indexes"]), 416)

    def test_every_background_is_reachable_and_five_needs_a_flag(self):
        # A background nobody can select would be dead data. All 32 are
        # selectable, and index 5 only through the Dark Force 2 swap.
        self.assertTrue(all(entry["selectable"] for entry in self.entries))
        swap = self.selection["dark_force_2_swap"]
        self.assertEqual((swap["from"], swap["to"]), DARK_FORCE_2_SWAP)
        five = self.entries[DARK_FORCE_2_SWAP[1]]
        self.assertEqual(sum(five["selected_by"].values()), 0)
        self.assertTrue(five["selectable"])
        # Everything else earns its place through a table.
        for entry in self.entries:
            if entry["index"] != DARK_FORCE_2_SWAP[1]:
                with self.subTest(symbol=entry["symbol"]):
                    self.assertGreater(sum(entry["selected_by"].values()), 0)

    def test_the_motavia_table_stores_index_plus_one(self):
        # `GetMotaBattleBGIndex`: `beq.s + / subq.b #1,d0`, so a stored 0 and a
        # stored 1 both mean background 0.
        block = self.selection["motavia_terrain"]
        self.assertEqual(block["stored_bias"], MOTA_STORED_BIAS)
        reachable = {max(v - MOTA_STORED_BIAS, 0) for v in block["indexes"]}
        # The Motavia overworld reaches the four Mota backgrounds and no more.
        self.assertEqual(sorted(reachable), [0, 1, 2, 3])
        self.assertEqual(
            [self.entries[i]["symbol"] for i in sorted(reachable)],
            ["MotaDesert", "MotaBeach", "MotaGrass", "MotaSea"],
        )

    def test_maps_without_a_background_say_so(self):
        indexes = self.selection["field_map"]["indexes"]
        self.assertEqual(self.selection["field_map"]["none"], NO_BACKGROUND)
        self.assertEqual(indexes.count(NO_BACKGROUND), 263)
        # MapID 0 is Motavia, whose $FF is never read: the routine branches to
        # the terrain table before it gets there.
        self.assertEqual(indexes[0], NO_BACKGROUND)
        # MapID 1 is Dezolis, which does take the map path -- and its index 4
        # is exactly the one the Dark Force 2 swap turns into 5.
        self.assertEqual(indexes[1], DARK_FORCE_2_SWAP[0])


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    DISASM.exists(),
    "reference/ps4disasm is not checked out; clone it to run the oracle tests",
)
class TestPoseLabelsAgainstTheDisassembly(unittest.TestCase):
    """The advisory labels are the clone's, transcribed rather than invented."""

    def test_the_three_selection_tables_match_the_clone(self):
        # A clean oracle is a finding: all three tables are byte-for-byte what
        # the clone lists, so the Grand Cross build did not touch them.
        text = DISASM.read_text(errors="replace")
        data = read_rom(ROM)
        for label, offset, count in (
            ("EventBattleBGIndexes", EVENT_BG_INDEXES, EVENT_BG_COUNT),
            ("Battle_BackgroundIndexes", FIELD_MAP_BG_INDEXES, FIELD_MAP_BG_COUNT),
            ("MotaBattleBGIndexes", MOTA_BG_INDEXES, MOTA_BG_COUNT),
        ):
            with self.subTest(label=label):
                match = re.search(
                    rf"^{label}:\s*\n((?:\tdc\.b\s+\$?[0-9A-Fa-f]+[^\n]*\n)+)", text, re.M
                )
                self.assertIsNotNone(match)
                clone = [
                    int(v, 16)
                    for v in re.findall(r"^\tdc\.b\s+\$?([0-9A-Fa-f]+)", match.group(1), re.M)
                ]
                self.assertEqual(len(clone), count)
                self.assertEqual(clone, list(data[offset:offset + count]))

    def test_every_label_is_the_clones_own(self):
        text = DISASM.read_text(errors="replace")
        found: dict[str, tuple[str, ...]] = {}
        for match in re.finditer(
            r"^PLC_(\w+?)BattleLongRange:\s*\n(.*?)^; =+$", text, re.M | re.S
        ):
            found[match.group(1)] = tuple(
                re.findall(r"^\tdc\.l\s+MapEni_\w+?Battle(\w+)\s*$", match.group(2), re.M)
            )
        self.assertEqual(found, POSE_LABELS)


if __name__ == "__main__":
    unittest.main()
