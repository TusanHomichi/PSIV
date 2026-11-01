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
from tools.check_rom_tables import (MARKER_LINES, RECORD_WINDOW, REMOVED_BLOBS, STATS,
                                    TABLE_RECORDS, blob_id, run, tiles_in)

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
ROWS = ["KKKKKKKK", "KQQQQQQK", "KQZZZZQK", "KQZKKZQK",
        "KQZKKZQK", "KQZZZZQK", "KQQQQQQK", "KKKKKKKK"]


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

    def test_a_synthetic_picture_says_so(self):
        marked = "// A synthetic map.\n" + literal_block(ROWS)
        self.assertEqual(tiles_in(marked), [])
        far = "// A synthetic map.\n" + "\n" * MARKER_LINES + literal_block(ROWS)
        self.assertEqual(len(tiles_in(far)), 1, "a marker past the window does not count")

    def test_a_marker_exempts_one_tile(self):
        marked = "// A synthetic map.\n"
        self.assertEqual(tiles_in(marked + literal_block(ROWS + ROWS)), [11], "nested, one marker")
        flat = ",\n".join(f'"{row}"' for row in ROWS + ROWS)
        self.assertEqual(tiles_in(marked + flat), [10], "flat, one marker")
        self.assertEqual(tiles_in(marked + literal_block(ROWS * 3)), [11, 19])
        split = marked + literal_block(ROWS) + marked + literal_block(ROWS)
        self.assertEqual(tiles_in(split), [], "each marked")
        one_line = ", ".join(f'"{row}"' for row in ROWS)
        self.assertEqual(tiles_in(f"{marked}{one_line},\n\n\n{one_line}\n"), [5],
                         "the second tile's window ends short of the marker")

    def test_rows_after_an_exemption_are_still_read(self):
        names = [f"SYMBOL{n:02}" for n in range(1, 9)]
        self.assertEqual(tiles_in(literal_block(names + ROWS)), [10], "after a name list")
        self.assertEqual(tiles_in(literal_block(names * 2 + ROWS)), [18], "after two lists")
        self.assertEqual(tiles_in(literal_block(ROWS + names)), [2], "before a name list")
        self.assertEqual(tiles_in("// synthetic\n" + literal_block(ROWS + names + ROWS)), [19],
                         "a marked tile, names, then an unmarked tile")

    def test_only_a_comment_is_a_marker(self):
        tile = literal_block(ROWS)
        for above in ('let label = "synthetic";', 'let label = "// synthetic";',
                      "let synthetic = 1;", "fn synthetic() {}", "synthetic();",
                      "let c = '\"'; let label = \"// synthetic\";",
                      'let label = r#"// synthetic"#;', 'let label = "\n// synthetic\n";',
                      'let label = r#"\n// synthetic\n"#;', "// nonsynthetic",
                      '#[cfg(feature = "synthetic")]', "&'a str = \"// synthetic\";",
                      "const LABEL: &'static str = \"'// synthetic\";",
                      "fn f<'a, 'b>(x: &'a str, y: &'b str) { g(\"'// synthetic\") }",
                      "let c = '\\''; let s = \"// synthetic\";",
                      "let c = b'\"'; let s = \"// synthetic\";",
                      'let s = "\\"// synthetic";', 'let s = br##"x"#\n// synthetic\n"##;',
                      'let s = "unterminated\n// synthetic\n',
                      '/* /* */ " */ "// synthetic";'):
            with self.subTest(above=above):
                self.assertEqual(len(tiles_in(f"{above}\n{tile}")), 1)
        for above in ("// A synthetic map.", "/// Synthetic.", "/* synthetic */",
                      "/*\n * A synthetic map.\n */", 'let label = "x"; // synthetic',
                      "let c = '\"'; // synthetic", "fn f<'a>(x: &'a str) {} // synthetic",
                      '/* /* nested */ synthetic */', "let c = '\\u{22}'; // synthetic",
                      'let s = r#"a"b"#; // synthetic', "let r = 'r'; // synthetic"):
            with self.subTest(above=above):
                self.assertEqual(tiles_in(f"{above}\n{tile}"), [])
        expired = "/* synthetic */\n" + "\n" * MARKER_LINES + tile
        self.assertEqual(len(tiles_in(expired)), 1, "an expired comment")

    def test_comment_syntax_follows_the_file_type(self):
        tile = literal_block(ROWS)
        self.assertEqual(tiles_in(f"# A synthetic map.\n{tile}", ".py"), [])
        self.assertEqual(tiles_in(f"x = 1  # synthetic\n{tile}", ".py"), [])
        for above in ("x = a // synthetic", 'x = "# synthetic"', '"""\n# synthetic\n"""',
                      "\'\'\'\n# synthetic\n\'\'\'", "synthetic = 1",
                      '"""\\"""\n# synthetic\n"""', "\'\'\'\\\'\'\'\n# synthetic\n\'\'\'",
                      'x = f"{y!r:# synthetic}"', 'x = "\\\n# synthetic"'):
            with self.subTest(above=above):
                self.assertEqual(len(tiles_in(f"{above}\n{tile}", ".py")), 1)
        self.assertEqual(len(tiles_in(f"# synthetic\n(\n{tile}", ".py")), 1,
                         "a file that does not tokenize has no markers")
        self.assertEqual(len(tiles_in(f"# synthetic\n{chr(10) * MARKER_LINES}{tile}", ".py")), 1,
                         "an expired comment")
        self.assertEqual(tiles_in(f"# synthetic\n{tile}# synthetic\n{tile}", ".py"), [])
        self.assertEqual(len(tiles_in(f"# synthetic\n{tile}", ".rs")), 1, "not Rust's comment")
        for suffix, above in ((".md", "// synthetic"), (".txt", "# synthetic"),
                              (".js", "const a = `\n// synthetic\n`;"),
                              (".ts", "// synthetic"),
                              (".cpp", 'auto a = R"(\n// synthetic\n)";'),
                              (".sh", "a='\n# synthetic\n'"), (".sh", "echo $# synthetic"),
                              (".nix", "a = ''\n# synthetic\n'';"),
                              (".yml", "a: |\n  # synthetic\n"), (".gd", "# synthetic")):
            with self.subTest(suffix=suffix, above=above):
                self.assertEqual(len(tiles_in(f"{above}\n{tile}", suffix)), 1,
                                 "comments it cannot tell from strings are no markers")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "art.py").write_text(f"# synthetic\n{tile}")
            (root / "art.rs").write_text(f"# synthetic\n{tile}")
            self.assertEqual(run(root, root / "none", ["art.py"])[0], [])
            self.assertEqual(len(run(root, root / "none", ["art.rs"])[0]), 1, "the scan's suffix")

    def test_a_numbered_name_list_is_not_a_tile(self):
        names = [f"SYMBOL{n:02}" for n in range(1, 9)]
        self.assertEqual(tiles_in(literal_block(names)), [])
        self.assertEqual(tiles_in(literal_block([f"SYMBOL{n:02}" for n in range(1, 13)])), [],
                         "past eight names")

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
