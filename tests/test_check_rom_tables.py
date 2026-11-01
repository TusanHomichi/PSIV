"""The copied-table guard (#149) on a synthetic pack, and on the real one.

    PYTHONPATH=. python3 -m unittest tests.test_check_rom_tables -v

The synthetic enemies have made-up names and values, so nothing here is the
cartridge's. The real-pack case writes its table to a temporary directory only
and skips without a pack.
"""

import json
import os
import tempfile
import unittest
from pathlib import Path

from tools import check_rom_tables
from tools.check_rom_tables import (RECORD_WINDOW, REMOVED_BLOBS, STATS, SYNTHETIC_TILES,
                                    TABLE_RECORDS, blob_id, run, tile_digest, tiles_in)

ROOT = Path(__file__).resolve().parent.parent


def enemy(index):
    """A synthetic record: distinct name, distinct values."""
    return {"id": index, "symbol": f"Quorbix{chr(65 + index)}",
            "display_name": f"QUORBIX {chr(65 + index)}", "hp": 9000 + index,
            "stats": {stat: 300 + 10 * index + slot for slot, stat in enumerate(STATS)}}


ENEMIES = [enemy(index) for index in range(TABLE_RECORDS + 4)]


def as_json(records):
    """The removed replay pack's shape: one object per enemy."""
    return json.dumps({"enemies": [
        {"id": r["id"], "name": r["symbol"].upper(), "hp": r["hp"], **r["stats"]}
        for r in records]})


class Guard(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / "pack/battle").mkdir(parents=True)
        (self.root / "pack/battle/enemies.json").write_text(json.dumps({"enemies": ENEMIES}))

    def check(self, text, pack=True):
        (self.root / "file.txt").write_text(text)
        return run(self.root, self.root / ("pack" if pack else "none"), ["file.txt"])

    def test_a_table_of_records_fails(self):
        problems, _ = self.check(as_json(ENEMIES[:TABLE_RECORDS]))
        self.assertEqual(len(problems), 1, problems)
        self.assertIn(f"{TABLE_RECORDS} enemy records", problems[0])

    def test_a_markdown_table_fails_too(self):
        rows = "\n".join(f"| {r['display_name']} | {r['hp']} | "
                         + " | ".join(str(v) for v in r["stats"].values()) + " |"
                         for r in ENEMIES)
        problems, _ = self.check(rows)
        self.assertEqual(len(problems), 1, problems)

    def test_fewer_records_than_a_table_pass(self):
        problems, summary = self.check(as_json(ENEMIES[:TABLE_RECORDS - 1]))
        self.assertEqual(problems, [])
        self.assertIn("1 file(s) scanned", summary)

    def test_names_alone_pass(self):
        problems, _ = self.check(" ".join(r["display_name"] for r in ENEMIES))
        self.assertEqual(problems, [])

    def test_a_record_needs_its_hp_and_enough_stats(self):
        def without(record, keep):
            stats = list(record["stats"].values())[:keep]
            return f"{record['symbol']} {record['hp']} " + " ".join(map(str, stats))
        too_few = "\n".join(without(r, check_rom_tables.RECORD_STATS - 1) for r in ENEMIES)
        self.assertEqual(self.check(too_few)[0], [])
        no_hp = "\n".join(f"{r['symbol']} " + " ".join(map(str, r["stats"].values()))
                          for r in ENEMIES)
        self.assertEqual(self.check(no_hp)[0], [])
        enough = "\n".join(without(r, check_rom_tables.RECORD_STATS) for r in ENEMIES)
        self.assertEqual(len(self.check(enough)[0]), 1)

    def test_values_past_the_window_are_not_the_record(self):
        far = "\n".join(f"{r['symbol']}{' ' * RECORD_WINDOW}{r['hp']} "
                        + " ".join(map(str, r["stats"].values())) for r in ENEMIES)
        self.assertEqual(self.check(far)[0], [])

    def test_without_a_pack_the_scan_is_skipped(self):
        problems, summary = self.check(as_json(ENEMIES), pack=False)
        self.assertEqual(problems, [])
        self.assertIn("records skipped", summary)

    def test_a_removed_copy_fails_without_a_pack(self):
        content = b"a removed copy\n"
        original = check_rom_tables.REMOVED_BLOBS
        check_rom_tables.REMOVED_BLOBS = frozenset({blob_id(content)})
        self.addCleanup(setattr, check_rom_tables, "REMOVED_BLOBS", original)
        (self.root / "file.txt").write_bytes(content)
        problems, _ = run(self.root, self.root / "none", ["file.txt"])
        self.assertEqual(len(problems), 1, problems)
        (self.root / "file.txt").write_bytes(content + b" ")
        self.assertEqual(run(self.root, self.root / "none", ["file.txt"])[0], [])

    def test_blob_ids_are_gits(self):
        # `git hash-object` of an empty file and of "hello\n".
        self.assertEqual(blob_id(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391")
        self.assertEqual(blob_id(b"hello\n"), "ce013625030ba8dba906f756967f9e9ca394464a")
        self.assertTrue(all(len(blob) == 40 for blob in REMOVED_BLOBS))


#: A synthetic 8x8 pattern, one row per literal.
ROWS = "KKKKKKKK KQQQQQQK KQZZZZQK KQZKKZQK KQZKKZQK KQZZZZQK KQQQQQQK KKKKKKKK".split()


def literal_block(rows, quote='"', per_line=1):
    lines = [", ".join(f"{quote}{row}{quote}" for row in rows[i:i + per_line])
             for i in range(0, len(rows), per_line)]
    return "let rows = [\n" + ",\n".join(f"    {line}" for line in lines) + ",\n];\n"


class Tiles(unittest.TestCase):
    def test_a_pixel_string_tile_fails(self):
        for quote, per_line in (('"', 1), ('"', 7), ("'", 4)):
            with self.subTest(quote=quote, per_line=per_line):
                self.assertEqual(tiles_in(literal_block(ROWS, quote, per_line)), [2])

    def test_nine_tiles_are_nine_problems(self):
        text = "".join(literal_block(ROWS) for _ in range(9))
        self.assertEqual(len(tiles_in(text)), 9)

    def test_a_listed_picture_passes_and_only_it(self):
        listed = frozenset({tile_digest(ROWS)})
        self.assertEqual(tiles_in(literal_block(ROWS), listed), [])
        changed = ROWS[:7] + ["KKKKKKKQ"]
        self.assertEqual(len(tiles_in(literal_block(changed), listed)), 1, "one pixel changed")
        self.assertEqual(len(tiles_in(literal_block(ROWS[1:] + ROWS[:1]), listed)), 1, "rows reordered")

    def test_a_listed_picture_exempts_itself_alone(self):
        listed = frozenset({tile_digest(ROWS)})
        other = ROWS[:7] + ["KKKKKKKQ"]
        self.assertEqual(tiles_in(literal_block(ROWS + other), listed), [10], "nested")
        flat = literal_block(ROWS + other, per_line=16)
        self.assertEqual(tiles_in(flat, listed), [2], "flat")
        self.assertEqual(tiles_in(literal_block(ROWS + ROWS), listed), [], "listed twice")

    def test_rows_after_an_exemption_are_still_read(self):
        names = [f"SYMBOL{n:02}" for n in range(1, 9)]
        self.assertEqual(len(tiles_in(literal_block(names + ROWS))), 1, "after a list")
        self.assertEqual(len(tiles_in(literal_block(names + names + ROWS))), 1, "after two")
        self.assertEqual(len(tiles_in(literal_block(ROWS + names))), 1, "before a list")
        listed = frozenset({tile_digest(ROWS)})
        other = ROWS[:7] + ["KKKKKKKQ"]
        self.assertEqual(tiles_in(literal_block(ROWS + names + other), listed), [18],
                         "listed tile, names, then a tile that is not listed")

    def test_no_text_in_the_file_exempts_a_tile(self):
        tile = literal_block(ROWS)
        for above in ("// A synthetic map.", "/* synthetic */", "# synthetic",
                      'let label = "// synthetic";', "const L: &'static str = \"'// synthetic\";",
                      "`// synthetic`", tile_digest(ROWS)):
            self.assertEqual(len(tiles_in(f"{above}\n{tile}")), 1, above)

    def test_the_listed_pictures_are_digests(self):
        for digest in SYNTHETIC_TILES:
            self.assertRegex(digest, r"^[0-9a-f]{64}$")

    def test_a_numbered_name_list_is_not_a_tile(self):
        names = [f"SYMBOL{n:02}" for n in range(1, 9)]
        self.assertEqual(tiles_in(literal_block(names)), [])
        self.assertEqual(tiles_in(literal_block([f"SYMBOL{n:02}" for n in range(1, 13)])), [],
                         "past eight names")

    def test_a_list_of_offsets_is_not_a_tile(self):
        offsets = [f"0x{0xD526 + n * 16:06X}" for n in range(8)]
        self.assertEqual(tiles_in(literal_block(offsets)), [])
        self.assertEqual(len(tiles_in(literal_block(offsets[:7] + ["KKKKKKKK"]))), 1,
                         "one row that is no number")
        self.assertEqual(len(tiles_in(literal_block(offsets + ROWS))), 1, "a tile after them")

    def test_a_tile_that_only_looks_numbered_still_fails(self):
        repeated = ["SYMBOL01"] * 8
        self.assertEqual(len(tiles_in(literal_block(repeated))), 1, "a repeated row")
        prefixes = [f"{letter * 6}{n:02}" for n, letter in enumerate("ABABABAB")]
        self.assertEqual(len(tiles_in(literal_block(prefixes))), 1, "letters that change")
        after = [f"SYMBOL{n:02}" for n in range(1, 9)] + ROWS
        self.assertEqual(len(tiles_in(literal_block(after))), 1, "a tile right after a list")

    def test_shapes_that_are_not_a_tile_pass(self):
        self.assertEqual(tiles_in(literal_block(ROWS[:7])), [], "seven rows")
        spaced = [row[:3] + " " + row[4:] for row in ROWS]
        self.assertEqual(tiles_in(literal_block(spaced)), [], "a space is text")
        apart = "\n".join(f'check("{row}");' for row in ROWS)
        self.assertEqual(tiles_in(apart), [], "rows separated by code")
        long = [row + "K" for row in ROWS]
        self.assertEqual(tiles_in(literal_block(long)), [], "nine columns")

    def test_the_tree_scan_reports_the_line(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "art.rs").write_text("fn f() {}\n" + literal_block(ROWS))
            problems, _ = run(root, root / "none", ["art.rs"])
        self.assertEqual(len(problems), 1, problems)
        self.assertTrue(problems[0].startswith("art.rs:3: an 8x8 tile"), problems)


class RealPack(unittest.TestCase):
    """The pack's own enemy table, written out, is caught."""

    def test_the_real_enemy_table_fails(self):
        pack = Path(os.environ.get("PSIV_RUNTIME_PACK", ROOT / "runtime-pack"))
        if not (pack / "battle/enemies.json").is_file():
            self.skipTest(f"no runtime pack at {pack}")
        enemies = json.loads((pack / "battle/enemies.json").read_text())["enemies"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "copy.json").write_text(as_json(enemies))
            problems, _ = run(root, pack, ["copy.json"])
        self.assertEqual(len(problems), 1, problems)


if __name__ == "__main__":
    unittest.main()
