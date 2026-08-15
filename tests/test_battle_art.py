import hashlib
import itertools
import tempfile
import unittest
from pathlib import Path

from psiv_tools.battle_art import (
    BLANK_PATTERN,
    CHARACTER_BASE_WORD,
    CHARACTER_COLUMNS,
    CHARACTER_COUNT,
    CHARACTER_CRAM_LINE,
    CHARACTER_PALETTE_COLORS,
    CHARACTER_PALETTE_TABLE,
    CHARACTER_PLC_TABLE,
    CHARACTER_ROWS,
    CHARACTER_SLOT_WORDS,
    CHARACTER_SYMBOLS,
    ENEMY_ART_BANK_ORDER,
    ENEMY_ART_FIELDS,
    ENEMY_ART_TABLE,
    ENEMY_COUNT,
    ENEMY_CRAM_LINES,
    ENEMY_FIXED_COLORS,
    ENEMY_PALETTE_COLORS,
    ENEMY_PALETTE_FIRST_INDEX,
    ENEMY_PALETTE_TABLE,
    RAW_MAPPING_ENEMY_IDS,
    UI_COLOR_14,
    BattleArtError,
    battle_cram,
    build_tile_bank,
    character_art_extents,
    character_blocks,
    character_cram_line,
    character_records,
    enemy_art_bounds,
    enemy_cram_line,
    enemy_mapping,
    enemy_palette_words,
    enemy_records,
    export_battle_art_pngs,
    extract_battle_art,
    verify_ui_colors,
)
from psiv_tools.core import read_rom
from psiv_tools.enigma import decompress as enigma_decompress
from psiv_tools.nemesis import TILE_SIZE
from psiv_tools.nemesis import decompress as nemesis_decompress
from psiv_tools.planes import CRAM_COLORS
from psiv_tools.gfx import decode_tile

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"
CHARACTER_MAPPING_DIR = REFERENCE / "plane mappings" / "characters"
CHARACTER_ART_DIR = REFERENCE / "graphics" / "characters"

# Every character art blob, keyed by the disassembly's own file name. The
# lengths are the assertion: this module derives them from the mappings alone.
CHARACTER_ART_FILES = {
    "Chaz Long Range": 864,
    "Alys Long Range": 1120,
    "Hahn Long Range": 768,
    "Rune Long Range": 1504,
    "Gryz Long Range": 1536,
    "Rika Long Range": 928,
    "Demi Long Range": 768,
    "Demi Gun": 960,
    "Wren Long Range": 1184,
    "Wren Gun": 1536,
    "Raja Long Range": 1248,
    "Kyra Long Range": 1728,
    "Seth Long Range": 768,
}


class TestConstants(unittest.TestCase):
    """Shape checks that need neither the ROM nor the disassembly."""

    def test_bank_order_is_a_permutation_of_the_three_art_fields(self):
        self.assertEqual(sorted(ENEMY_ART_BANK_ORDER), list(range(ENEMY_ART_FIELDS)))

    def test_character_slot_is_the_drawn_grid(self):
        """loc_86D6 draws 6x6 and loc_9A80's buffers are 36 words apart."""
        self.assertEqual(CHARACTER_SLOT_WORDS, CHARACTER_COLUMNS * CHARACTER_ROWS)
        self.assertEqual((CHARACTER_COLUMNS, CHARACTER_ROWS), (6, 6))

    def test_character_base_word_is_priority_plus_line_3(self):
        self.assertEqual(CHARACTER_BASE_WORD, 0x8000 | (CHARACTER_CRAM_LINE << 13))

    def test_enemy_palette_covers_indices_1_to_13(self):
        used = set(ENEMY_FIXED_COLORS) | set(
            range(ENEMY_PALETTE_FIRST_INDEX, ENEMY_PALETTE_FIRST_INDEX + ENEMY_PALETTE_COLORS)
        )
        self.assertEqual(used, set(range(1, 14)))

    def test_symbol_lists_match_their_tables(self):
        self.assertEqual(len(CHARACTER_SYMBOLS), CHARACTER_COUNT)
        self.assertEqual(CHARACTER_SYMBOLS[0], "Chaz")


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestBattleArtFromRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.result = extract_battle_art(cls.data)
        cls.enemies = cls.result["enemies"]
        cls.characters = cls.result["characters"]
        cls.by_symbol = {e["symbol"]: e for e in cls.enemies["enemies"]}

    # -- tables ------------------------------------------------------------
    def test_the_two_enemy_tables_abut_exactly(self):
        """The 153rd graphics record ends on the first palette record.

        Neither table has a terminator, so this adjacency is what proves both
        the 20-byte stride and the 153 count without trusting a label.
        """
        art = self.enemies["art_table"]
        self.assertEqual(art["rom_offset"], "0x27F3AE")
        self.assertEqual(art["entry_size"], 20)
        self.assertEqual(art["rom_end_exclusive"], self.enemies["palette_table"]["rom_offset"])
        self.assertEqual(self.enemies["palette_table"]["rom_offset"], "0x27FFA2")
        self.assertEqual(self.enemies["palette_table"]["rom_end_exclusive"], "0x280CC8")
        self.assertEqual(len(self.enemies["enemies"]), ENEMY_COUNT)

    def test_record_geometry_is_in_family(self):
        records = enemy_records(self.data)
        widths = {r["half_width_cells"] for r in records}
        heights = {r["height_cells"] for r in records}
        self.assertTrue(widths <= set(range(1, 21)), widths)
        self.assertTrue(heights <= set(range(5, 16)), heights)
        for record in records:
            self.assertGreater(record["half_width_cells"], 0)
            self.assertGreater(record["height_cells"], 0)

    def test_vram_word_is_an_allocation_not_a_tile_count(self):
        """It is never smaller than art #1, and usually is not equal to it."""
        records = enemy_records(self.data)
        equal = 0
        for record in records:
            from psiv_tools.nemesis import read_header

            art1 = read_header(self.data, record["art_offsets"][0]).tile_count
            self.assertGreaterEqual(record["vram_patterns"], art1)
            equal += record["vram_patterns"] == art1
        self.assertEqual(equal, 49)

    # -- art ---------------------------------------------------------------
    def test_every_art_blob_decompresses_within_its_neighbours(self):
        records = enemy_records(self.data)
        bounds = enemy_art_bounds(records, len(self.data))
        offsets = {o for r in records for o in r["art_offsets"]}
        self.assertEqual(len(offsets), 199)
        for record in records:
            with self.subTest(record["symbol"]):
                build_tile_bank(self.data, record, bounds)

    def test_art_census(self):
        self.assertEqual(self.enemies["distinct_art_blobs"], 199)
        self.assertEqual(self.enemies["total_art_patterns"], 11541)
        self.assertEqual(self.enemies["art_bank_order"], [3, 1, 2])

    def test_bank_order_beats_every_alternative(self):
        """Re-derive the ordering rather than trusting the constant.

        Two things separate the right order from the other five: pattern 0 has
        to be blank, because every mapping uses 0 for an empty cell, and the
        number of cells the bank cannot fill has to be as small as possible.
        The chosen order wins both, and by a wide margin on the second.
        """
        records = enemy_records(self.data)
        cache: dict[int, list[bytes]] = {}

        def art(offset: int) -> list[bytes]:
            if offset not in cache:
                blob, _ = nemesis_decompress(self.data, offset)
                cache[offset] = [
                    decode_tile(blob[i:i + TILE_SIZE]) for i in range(0, len(blob), TILE_SIZE)
                ]
            return cache[offset]

        mappings = {r["id"]: enemy_mapping(self.data, r)[0] for r in records}
        scores = {}
        for order in itertools.permutations(range(ENEMY_ART_FIELDS)):
            holes = blanks = 0
            for record in records:
                bank: list[bytes] = []
                for field in order:
                    bank.extend(art(record["art_offsets"][field]))
                blanks += bank[0] == BLANK_PATTERN
                for word in mappings[record["id"]]:
                    tile = word & 0x07FF
                    if tile and bank[tile] == BLANK_PATTERN:
                        holes += 1
            scores[order] = (holes, blanks)

        best = min(scores, key=lambda o: scores[o][0])
        self.assertEqual(best, ENEMY_ART_BANK_ORDER)
        self.assertEqual(scores[best], (303, 141))
        runner_up = min((o for o in scores if o != best), key=lambda o: scores[o][0])
        self.assertGreater(scores[runner_up][0], scores[best][0] * 1.5)

    # -- mappings ----------------------------------------------------------
    def test_mapping_cell_count_is_twice_the_half_width(self):
        """loc_10ED4 doubles the width byte before calling PlaneMapToRAM."""
        for record in enemy_records(self.data):
            with self.subTest(record["symbol"]):
                words, mapping = enemy_mapping(self.data, record)
                self.assertEqual(mapping["columns"], 2 * record["half_width_cells"])
                self.assertEqual(len(words), mapping["columns"] * record["height_cells"])

    def test_most_but_not_all_mappings_are_mirrored(self):
        """Symmetry is common enough to explain the half-width byte, and it is
        the reason the raw ProfoundDarkness mappings were recognisable. It is
        not universal, so nothing in the decoder may rely on it."""

        def mirrored(words, columns, rows) -> bool:
            for row in range(rows):
                line = words[row * columns:(row + 1) * columns]
                for column in range(columns // 2):
                    left, right = line[column], line[-1 - column]
                    if left & 0x07FF != right & 0x07FF:
                        return False
                    if left & 0x07FF and left & 0x0800 == right & 0x0800:
                        return False
            return True

        symmetric = []
        for record in enemy_records(self.data):
            words, mapping = enemy_mapping(self.data, record)
            if mirrored(words, mapping["columns"], mapping["rows"]):
                symmetric.append(record["symbol"])
        self.assertEqual(len(symmetric), 69)
        self.assertIn("Helex", symmetric)
        self.assertNotIn("Igglanova", symmetric)

    def test_only_the_two_known_enemies_store_a_raw_mapping(self):
        self.assertEqual(self.enemies["mapping_formats"], {"enigma": 151, "raw_words": 2})
        self.assertEqual(set(self.enemies["raw_mapping_enemy_ids"]), set(RAW_MAPPING_ENEMY_IDS))
        raw = {e["id"] for e in self.enemies["enemies"] if e["mapping"]["format"] == "raw_words"}
        self.assertEqual(raw, set(RAW_MAPPING_ENEMY_IDS))
        self.assertEqual(
            {self.by_symbol[s]["id"] for s in ("ProfoundDarkness2", "ProfoundDarkness3")},
            set(RAW_MAPPING_ENEMY_IDS),
        )

    def test_a_raw_mapping_is_not_a_decodable_enigma_stream(self):
        """The fallback is not a guess: Enigma genuinely cannot read these."""
        from psiv_tools.enigma import EnigmaError

        for enemy_id in sorted(RAW_MAPPING_ENEMY_IDS):
            record = enemy_records(self.data)[enemy_id]
            with self.subTest(hex(enemy_id)):
                with self.assertRaises(EnigmaError):
                    enigma_decompress(self.data, record["mapping_offset"], max_words=4096)

    def test_an_unexpected_raw_mapping_would_fail_closed(self):
        record = dict(enemy_records(self.data)[0])
        record["mapping_offset"] = 0x27F3AE  # a table, not a stream
        with self.assertRaises(BattleArtError):
            enemy_mapping(self.data, record)

    def test_body_completeness_census(self):
        """69 enemies have cells the animated sprite pieces fill in."""
        self.assertEqual(self.enemies["bodies_complete"], 84)
        self.assertEqual(self.enemies["total_body_holes"], 303)
        incomplete = [e for e in self.enemies["enemies"] if not e["body_complete"]]
        self.assertEqual(len(incomplete), ENEMY_COUNT - 84)
        for entry in self.enemies["enemies"]:
            self.assertLessEqual(entry["patterns_used"], entry["bank_patterns"])

    # -- pins --------------------------------------------------------------
    def test_pinned_enemies(self):
        helex = self.by_symbol["Helex"]
        self.assertEqual(helex["id"], 0)
        self.assertEqual(helex["record_offset"], "0x27F3AE")
        self.assertEqual(helex["size_cells"], [8, 8])
        self.assertEqual(helex["size_pixels"], [64, 64])
        self.assertEqual([a["rom_offset"] for a in helex["art"]], ["0x20D5F0", "0x20D2C8", "0x20D2A6"])
        self.assertEqual([a["tile_count"] for a in helex["art"]], [23, 72, 3])
        self.assertEqual([a["first_pattern"] for a in helex["art"]], [3, 26, 0])
        self.assertEqual(
            helex["art"][0]["decompressed_sha256"],
            "7a775d125723107c" + helex["art"][0]["decompressed_sha256"][16:],
        )
        self.assertEqual(helex["palette"]["rom_offset"], "0x27FFA2")
        self.assertEqual(
            [c["hex"] for c in helex["palette"]["colors"][:3]],
            ["#240000", "#6D2400", "#B60000"],
        )

        igglanova = self.by_symbol["Igglanova"]
        self.assertEqual(igglanova["id"], 12)
        self.assertEqual(igglanova["size_cells"], [10, 10])
        self.assertEqual(igglanova["body_holes"], 0)
        self.assertEqual(igglanova["bank_patterns"], 213)

        dark_force = self.by_symbol["DarkForce1"]
        self.assertEqual(dark_force["id"], 130)
        self.assertEqual(dark_force["size_cells"], [40, 15])
        self.assertEqual(dark_force["size_pixels"], [320, 120])
        self.assertEqual(dark_force["bank_patterns"], 393)
        self.assertEqual(dark_force["mapping"]["format"], "enigma")

    def test_all_enemy_art_is_pinned(self):
        digests = sorted(
            {a["decompressed_sha256"] for e in self.enemies["enemies"] for a in e["art"]}
        )
        self.assertEqual(len(digests), 187)  # 199 offsets, 12 of them duplicate content
        self.assertEqual(
            hashlib.sha256("\n".join(digests).encode()).hexdigest(),
            "10c98ef24e1a5b3ed3ae930742c60ea5d511ad7d4e5940d8ee4e7a3e83bfa73a",
        )

    # -- palettes ----------------------------------------------------------
    def test_enemy_palette_line_is_assembled_from_three_sources(self):
        line = enemy_cram_line(self.data, 0, 1)
        self.assertEqual(len(line), 16)
        self.assertEqual(line[1], ENEMY_FIXED_COLORS[1])
        self.assertEqual(line[2], ENEMY_FIXED_COLORS[2])
        self.assertEqual(line[3:14], enemy_palette_words(self.data, 0))
        self.assertEqual(line[14], UI_COLOR_14)
        self.assertEqual(line[15], 0x0CC4)  # line 1
        self.assertEqual(enemy_cram_line(self.data, 0, 2)[15], 0x062E)
        # Only the last colour differs between the two lines an enemy can use.
        one, two = enemy_cram_line(self.data, 0, 1), enemy_cram_line(self.data, 0, 2)
        self.assertEqual(one[:15], two[:15])

    def test_enemy_palette_rejects_a_line_enemies_never_use(self):
        for line in (0, 3):
            with self.assertRaises(BattleArtError):
                enemy_cram_line(self.data, 0, line)
        self.assertEqual(ENEMY_CRAM_LINES, (1, 2))

    def test_ui_colour_immediates_are_where_the_module_says(self):
        sites = verify_ui_colors(self.data)
        self.assertEqual([s["rom_offset"] for s in sites], ["0x007672", "0x007678", "0x00767E"])
        self.assertEqual([s["cram_line"] for s in sites], [0, 1, 2])
        self.assertTrue(all(s["color_index"] == 15 for s in sites))

    def test_character_palette(self):
        table = self.characters["palette_table"]
        self.assertEqual(table["rom_offset"], "0x0076A4")
        self.assertEqual(table["cram_line"], 3)
        self.assertEqual(table["entry_count"], 4)
        self.assertEqual(len(table["colors"]), CHARACTER_PALETTE_COLORS)
        self.assertEqual(
            [c["hex"] for c in table["colors"][:4]],
            ["#000000", "#B6B6B6", "#6D6D6D", "#FFB66D"],
        )
        line = character_cram_line(self.data, 0)
        self.assertEqual(len(line), 16)
        self.assertEqual(line[0], 0)  # index 0 is never written by loc_7634
        with self.assertRaises(BattleArtError):
            character_cram_line(self.data, 4)

    def test_battle_cram_is_a_whole_cram_image(self):
        cram = battle_cram(self.data, 0, 1)
        self.assertEqual(len(cram), CRAM_COLORS)

    # -- characters --------------------------------------------------------
    def test_plc_blocks_tile_the_table(self):
        blocks = character_blocks(self.data)
        self.assertEqual(len(blocks), CHARACTER_COUNT)
        self.assertEqual(blocks[0][0], 0x008274)
        self.assertEqual(blocks[-1][1], CHARACTER_PLC_TABLE["rom_end_exclusive"])
        for (_, end), (start, _) in zip(blocks, blocks[1:]):
            self.assertEqual(end, start)

    def test_pose_census(self):
        self.assertEqual(self.characters["total_poses"], 43)
        self.assertEqual(self.characters["distinct_art_blobs"], 13)
        counts = {c["symbol"]: c["pose_count"] for c in self.characters["characters"]}
        self.assertEqual(counts, {
            "Chaz": 2, "Alys": 6, "Hahn": 3, "Rune": 3, "Gryz": 3, "Rika": 2,
            "Demi": 5, "Wren": 5, "Raja": 4, "Kyra": 8, "Seth": 2,
        })
        self.assertEqual(sum(counts.values()), 43)

    def test_only_demi_and_wren_carry_a_second_art_blob(self):
        extra = {c["symbol"] for c in self.characters["characters"] if len(c["art"]) > 1}
        self.assertEqual(extra, {"Demi", "Wren"})
        for record in self.characters["characters"]:
            self.assertLessEqual(len(record["art"]), 2)

    def test_character_art_length_is_derived_from_the_mappings(self):
        """Uncompressed art carries no length; the poses supply it.

        The highest pattern any pose names is the last one its blob holds, and
        that reproduces all thirteen documented lengths exactly.
        """
        extents = character_art_extents(self.data, character_records(self.data))
        self.assertEqual(len(extents), 13)
        sizes = sorted(patterns * TILE_SIZE for patterns in extents.values())
        self.assertEqual(sizes, sorted(CHARACTER_ART_FILES.values()))

    def test_plc_size_word_is_a_vram_allocation(self):
        """It matches the largest art blob for ten of the eleven characters."""
        extents = character_art_extents(self.data, character_records(self.data))
        exact = []
        for record in character_records(self.data):
            largest = max(extents[o] * TILE_SIZE for o in record["art_offsets"])
            if record["vram_bytes"] == largest:
                exact.append(record["symbol"])
            self.assertGreaterEqual(record["vram_bytes"], largest, record["symbol"])
        self.assertEqual(set(CHARACTER_SYMBOLS) - set(exact), {"Chaz"})

    def test_wren_charge_overruns_its_plane_buffer(self):
        """A retail anomaly: one pose decompresses 42 words into a 36-word slot.

        `loc_9A80`'s per-slot buffers are $48 bytes apart and `loc_86D6` draws
        6x6, so the last six words land in the next party member's buffer and
        are never drawn. Every other pose is exactly 36 words.
        """
        oversized = [
            (c["symbol"], p["pose"], p["cell_count"])
            for c in self.characters["characters"]
            for p in c["poses"]
            if p["cell_count"] != CHARACTER_SLOT_WORDS
        ]
        self.assertEqual(oversized, [("Wren", 3, 42)])
        for record in self.characters["characters"]:
            for pose in record["poses"]:
                self.assertGreaterEqual(pose["cell_count"], CHARACTER_SLOT_WORDS)

    def test_pose_zero_is_the_idle_pose_and_art_precedes_its_poses(self):
        for record in self.characters["characters"]:
            with self.subTest(record["symbol"]):
                poses = record["poses"]
                self.assertEqual([p["pose"] for p in poses], list(range(len(poses))))
                first_art = record["art"][0]["rom_offset"]
                self.assertEqual(poses[0]["art_rom_offset"], first_art)
                for pose in poses:
                    self.assertLess(pose["patterns_used"], 64)

    def test_all_character_art_is_pinned(self):
        digests = sorted({a["sha256"] for c in self.characters["characters"] for a in c["art"]})
        self.assertEqual(len(digests), 13)
        self.assertEqual(
            hashlib.sha256("\n".join(digests).encode()).hexdigest(),
            "74dd79f30a394f9f5bfcab46e5fd0c40b5b93ff6433455d82a5ea758b47e011e",
        )

    # -- rendering ---------------------------------------------------------
    def test_png_export(self):
        with tempfile.TemporaryDirectory() as tmp:
            written = export_battle_art_pngs(self.data, tmp)
            self.assertEqual(len(written), ENEMY_COUNT + 43)
            kinds = {w["kind"] for w in written}
            self.assertEqual(kinds, {"enemy", "character"})
            for entry in written:
                self.assertTrue(Path(entry["path"]).exists())
                self.assertGreater(entry["bytes"], 0)


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    CHARACTER_MAPPING_DIR.is_dir() and CHARACTER_ART_DIR.is_dir(),
    f"disassembly oracle not present at {REFERENCE}",
)
class TestBattleArtAgainstDisassembly(unittest.TestCase):
    """Enemy art is inline `dc.b` in ps4.asm, so it has no binclude oracle.

    Character art and pose mappings do, and they are strong ones: each file is
    the retail payload byte for byte, so matching by content fixes both the
    offset and the pose name.
    """

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.records = character_records(cls.data)

    def test_every_pose_mapping_is_a_named_disassembly_file(self):
        by_offset = {}
        for path in sorted(CHARACTER_MAPPING_DIR.iterdir()):
            blob = path.read_bytes()
            found = self.data.find(blob)
            self.assertNotEqual(found, -1, path.name)
            self.assertEqual(self.data.count(blob), 1, path.name)
            by_offset[found] = path.stem.replace(" Enigma", "")
        self.assertEqual(len(by_offset), 43)

        named = 0
        for record in self.records:
            for pose in record["poses"]:
                with self.subTest(f"{record['symbol']} pose {pose['pose']}"):
                    name = by_offset.get(pose["mapping_offset"])
                    self.assertIsNotNone(name, f"0x{pose['mapping_offset']:06X}")
                    self.assertTrue(name.startswith(record["symbol"]), name)
                    named += 1
                    if pose["pose"] == 0:
                        self.assertEqual(name, f"{record['symbol']} Idle")
        self.assertEqual(named, 43)

    def test_character_art_offsets_and_lengths_match(self):
        extents = character_art_extents(self.data, self.records)
        by_offset = {}
        for path in sorted(CHARACTER_ART_DIR.iterdir()):
            if path.stem not in CHARACTER_ART_FILES:
                continue
            blob = path.read_bytes()
            self.assertEqual(len(blob), CHARACTER_ART_FILES[path.stem], path.name)
            found = self.data.find(blob)
            self.assertNotEqual(found, -1, path.name)
            self.assertEqual(self.data.count(blob), 1, path.name)
            by_offset[found] = path.stem
        self.assertEqual(len(by_offset), len(CHARACTER_ART_FILES))

        for offset, patterns in extents.items():
            with self.subTest(hex(offset)):
                name = by_offset.get(offset)
                self.assertIsNotNone(name, f"0x{offset:06X}")
                # The length this module derived from the mappings alone.
                self.assertEqual(patterns * TILE_SIZE, CHARACTER_ART_FILES[name])

    def test_pose_naming_covers_every_action_the_battle_code_selects(self):
        """Pose names come from the disassembly, pose indices from the ROM.

        The index is the load-bearing fact -- `movea.l (a0,d0.w),a0` with
        `d0 = 4 * pose` -- so the names are recorded as labels for the design
        session, not treated as cartridge text.
        """
        by_offset = {
            self.data.find(path.read_bytes()): path.stem.replace(" Enigma", "")
            for path in sorted(CHARACTER_MAPPING_DIR.iterdir())
        }
        actions = set()
        for record in self.records:
            for pose in record["poses"]:
                actions.add(by_offset[pose["mapping_offset"]][len(record["symbol"]) + 1:])
        self.assertIn("Idle", actions)
        self.assertIn("Cast", actions)
        self.assertEqual(
            actions,
            {
                "Idle", "Cast", "Cast 2", "Arms Raised", "Arms Raised 2",
                "Charge", "Charge Magic", "Load Gun", "Fire Gun",
                "Slasher", "Two Slashers", "Throw Slasher", "Throw Two Slashers",
            },
        )


if __name__ == "__main__":
    unittest.main()
