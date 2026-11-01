"""Cases for tools/pack_parity.py, the gate's psiv-pack parity step (#216).

    PYTHONPATH=. python3 -m unittest tests.test_pack_parity -v

Hermetic: synthetic packs in temp directories and stand-in programs, no ROM,
no cargo. Every way the step can be fooled gets a negative control: a
differing byte, a differing pixel, a missing, extra or unclaimed file, a claim
the Python pack lacks, a link, a refusal worded differently, and malformed
progress output.
"""
import hashlib
import json
import os
import stat
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

from psiv_tools.core import EXPECTED_SHA256, RomError, read_rom, wrong_rom_message
from psiv_tools.png import encode_indexed
from tools import pack_parity
from tools.pack_parity import CENSUS, census_entries, compare_decodes
from tools.pack_parity import PROBES, compare_probe
from tools.pack_parity import (FIXTURE, check_output, check_progress, check_wrong_rom, compare,
                               fixture_text)

PALETTE = [(0, 0, 0), (255, 255, 255)]
#: What `check_wrong_rom` writes as the wrong ROM: 1 KiB of zeros.
ZEROS_SHA = hashlib.sha256(bytes(0x400)).hexdigest()


def recompressed(png: bytes, level: int) -> bytes:
    """The same image with its IDAT deflated at another level."""
    out, pos = bytearray(png[:8]), 8
    while pos < len(png):
        length = int.from_bytes(png[pos:pos + 4], "big")
        kind, payload = png[pos + 4:pos + 8], png[pos + 8:pos + 8 + length]
        if kind == b"IDAT":
            payload = zlib.compress(zlib.decompress(payload), level)
        out += len(payload).to_bytes(4, "big") + kind + payload
        out += (zlib.crc32(kind + payload) & 0xFFFFFFFF).to_bytes(4, "big")
        pos += 12 + length
    return bytes(out)


class FixtureCase(unittest.TestCase):
    def test_committed_fixture_is_what_python_writes(self):
        self.assertEqual(FIXTURE.read_text(encoding="utf-8"), fixture_text(),
                         "run python3 tools/pack_parity.py --write-fixtures")

    def test_fixture_covers_escapes_floats_and_order(self):
        text = fixture_text()
        for needle in ("\\\\ud83d\\\\ude00", "\\\\u007f", "1e-05", "1e+16", "Infinity"):
            self.assertIn(needle, text)


class CompareCase(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="psiv-pack-parity-")
        root = Path(self.tmp.name)
        self.rust, self.python = root / "rust", root / "python"
        image = encode_indexed(4, 2, bytes([0, 1, 1, 0, 1, 0, 0, 1]), PALETTE, [0])
        for pack in (self.rust, self.python):
            (pack / "sec").mkdir(parents=True)
            (pack / "sec" / "a.json").write_bytes(b'{\n  "a": 1\n}\n')
            (pack / "sec" / "b.bin").write_bytes(bytes(range(16)))
            (pack / "sec" / "c.png").write_bytes(image)
            (pack / "one.json").write_bytes(b"{}\n")
        (self.python / "unclaimed.json").write_bytes(b"{}\n")
        self.claims = ["sec", "one.json"]

    def tearDown(self):
        self.tmp.cleanup()

    def problems(self):
        return compare(self.rust, self.python, self.claims)[0]

    def test_identical_claimed_paths_match(self):
        self.assertEqual(compare(self.rust, self.python, self.claims), ([], 4))

    def test_png_with_another_deflate_matches(self):
        path = self.rust / "sec" / "c.png"
        other = recompressed(path.read_bytes(), 1)
        self.assertNotEqual(other, path.read_bytes())
        path.write_bytes(other)
        self.assertEqual(self.problems(), [])

    def test_png_with_another_pixel_fails(self):
        (self.rust / "sec" / "c.png").write_bytes(
            encode_indexed(4, 2, bytes([0, 1, 1, 0, 1, 0, 1, 1]), PALETTE, [0]))
        self.assertEqual(self.problems(), ["sec/c.png: the decoded image differs"])

    def test_png_with_another_transparency_fails(self):
        (self.rust / "sec" / "c.png").write_bytes(
            encode_indexed(4, 2, bytes([0, 1, 1, 0, 1, 0, 0, 1]), PALETTE, []))
        self.assertEqual(self.problems(), ["sec/c.png: the decoded image differs"])

    def test_unreadable_png_fails(self):
        (self.rust / "sec" / "c.png").write_bytes(b"not a png")
        self.assertTrue(self.problems()[0].startswith("sec/c.png: not a readable PNG"))

    def test_json_byte_fails(self):
        (self.rust / "sec" / "a.json").write_bytes(b'{\n  "a": 2\n}\n')
        self.assertEqual(self.problems(), ["sec/a.json: bytes differ from offset 9 "
                                           "(13 bytes against 13)"])

    def test_trailing_newline_fails(self):
        (self.rust / "one.json").write_bytes(b"{}")
        self.assertEqual(self.problems(), ["one.json: bytes differ from offset 2 "
                                           "(2 bytes against 3)"])

    def test_missing_file_fails(self):
        (self.rust / "sec" / "b.bin").unlink()
        self.assertEqual(self.problems(), ["sec/b.bin: missing from psiv-pack's pack"])

    def test_extra_file_under_a_claim_fails(self):
        (self.rust / "sec" / "d.bin").write_bytes(b"")
        self.assertEqual(self.problems(),
                         ["sec/d.bin: psiv-pack wrote it, the Python pack has none"])

    def test_file_outside_the_claims_fails(self):
        (self.rust / "secret.json").write_bytes(b"{}\n")
        self.assertEqual(self.problems(), ["secret.json: psiv-pack wrote it outside its claims"])

    def test_claim_prefix_is_a_path_not_a_string(self):
        (self.rust / "sec2").mkdir()
        (self.rust / "sec2" / "x.json").write_bytes(b"{}\n")
        self.assertIn("sec2/x.json: psiv-pack wrote it outside its claims", self.problems())

    def test_claim_the_python_pack_lacks_fails(self):
        self.claims.append("nowhere")
        self.assertEqual(self.problems(), ["claim nowhere: the Python pack has no such path"])

    def test_link_fails(self):
        (self.rust / "sec" / "b.bin").unlink()
        os.symlink(self.python / "sec" / "b.bin", self.rust / "sec" / "b.bin")
        self.assertEqual(self.problems(), ["sec/b.bin: psiv-pack wrote a symbolic link"])


class NoClaimsCase(unittest.TestCase):
    """With nothing claimed the Python build is skipped, not the output check."""

    def test_an_empty_installed_pack_passes(self):
        with tempfile.TemporaryDirectory(prefix="psiv-pack-parity-none-") as tmp:
            (Path(tmp) / "rust").mkdir()
            self.assertEqual(check_output(Path(tmp) / "rust", []), [])

    def test_any_file_or_link_fails(self):
        with tempfile.TemporaryDirectory(prefix="psiv-pack-parity-none-") as tmp:
            rust = Path(tmp) / "rust"
            (rust / "sec").mkdir(parents=True)
            (rust / "sec" / "a.json").write_bytes(b"{}\n")
            os.symlink("a.json", rust / "sec" / "b.json")
            self.assertEqual(check_output(rust, []),
                             ["sec/a.json: psiv-pack wrote it outside its claims",
                              "sec/b.json: psiv-pack wrote a symbolic link"])

    def test_no_output_fails(self):
        with tempfile.TemporaryDirectory(prefix="psiv-pack-parity-none-") as tmp:
            self.assertEqual(check_output(Path(tmp) / "rust", []),
                             ["psiv-pack left no pack directory at rust"])


class ProbeCase(unittest.TestCase):
    """The probe comparison: the documents must be identical text."""

    DOC = '{\n  "names": {\n    "a": 1\n  }\n}'

    def test_identical_documents_pass(self):
        self.assertEqual(compare_probe(self.DOC, self.DOC), [])

    def test_a_changed_value_names_its_line(self):
        problems = compare_probe(self.DOC, self.DOC.replace("1", "2"))
        self.assertEqual(len(problems), 1)
        self.assertTrue(problems[0].startswith("probe: line 3 differs"), problems)

    def test_a_missing_tail_fails(self):
        problems = compare_probe(self.DOC, self.DOC.rsplit("\n", 1)[0])
        self.assertIn("<end>", problems[0])

    def test_key_order_counts(self):
        """The Rust writer sorts as json.dumps(sort_keys=True); an unsorted
        document is a difference, not an equivalent."""
        unsorted = '{\n  "b": 1,\n  "a": 2\n}'
        self.assertNotEqual(compare_probe('{\n  "a": 2,\n  "b": 1\n}', unsorted), [])

    def test_the_probes_are_named(self):
        self.assertEqual(sorted(PROBES), ["cram", "layouts", "map_effects", "map_updates", "maps", "names", "warp_rects"])


class DecodeCase(unittest.TestCase):
    """The decoder comparison and the committed census (no ROM needed)."""

    CALLS = [{"decoder": "nemesis", "offset": 0x10},
             {"decoder": "enigma", "offset": 0x20, "base_tile": 0x6400, "max_words": 2048}]
    PYTHON = [{"consumed": 9, "sha256": "aa"}, {"consumed": 12, "sha256": "bb"}]

    def test_matching_results_pass(self):
        self.assertEqual(compare_decodes(self.CALLS, self.PYTHON, [dict(r) for r in self.PYTHON]), [])

    def test_matching_refusals_pass(self):
        refused = [{"error": "x"}, {"error": "y"}]
        self.assertEqual(compare_decodes(self.CALLS, refused, [{"error": "other words"}] * 2), [])

    def test_differences_fail(self):
        cases = {
            "output": [{"consumed": 9, "sha256": "ab"}, self.PYTHON[1]],
            "consumed length": [{"consumed": 10, "sha256": "aa"}, self.PYTHON[1]],
            "decoded it": [{"error": "refused"}, self.PYTHON[1]],
        }
        for what, rust in cases.items():
            with self.subTest(what=what):
                problems = compare_decodes(self.CALLS, self.PYTHON, rust)
                self.assertEqual(len(problems), 1, problems)
                self.assertTrue(problems[0].startswith("nemesis at 0x10: "), problems)
                self.assertIn(what, problems[0])

    def test_a_missing_result_fails(self):
        self.assertEqual(len(compare_decodes(self.CALLS, self.PYTHON, self.PYTHON[:1])), 1)

    def test_the_census_lists_refused_calls_once_each(self):
        from psiv_tools import enigma, nemesis
        from psiv_tools.decode_cache import scope
        # Byte 0 is past the end once the offset is 2, so both decoders refuse
        # there; a caller catches the refusal, as battle_art._is_mapping does.
        rom = bytes(2)
        with scope() as table:
            for _ in range(2):
                for call in (lambda: nemesis.decompress(rom, 2),
                             lambda: enigma.decompress(rom, 2, 0x10, 2048),
                             lambda: enigma.decompress(rom, 2, 0x10, None),
                             lambda: enigma.decompress(rom, 2, 0x20, 2048),
                             lambda: enigma.decompress(rom, 2, 0x10)):
                    with self.assertRaises(ValueError):
                        call()
        self.assertEqual(table.decodes, {}, "every call was refused")
        self.assertEqual(census_entries(table, rom), [
            {"decoder": "enigma", "offset": 2, "base_tile": 0x10, "max_words": None},
            {"decoder": "enigma", "offset": 2, "base_tile": 0x10, "max_words": 2048},
            {"decoder": "enigma", "offset": 2, "base_tile": 0x20, "max_words": 2048},
            {"decoder": "nemesis", "offset": 2},
        ])
        # And both builders' refusals are compared like any result.
        calls = census_entries(table, rom)
        refused = [{"error": "x"}] * len(calls)
        decoded = [{"consumed": 2, "sha256": "aa"}] + refused[1:]
        self.assertEqual(compare_decodes(calls, refused, refused), [])
        self.assertEqual(len(compare_decodes(calls, refused, decoded)), 1)

    def test_the_census_refuses_inputs_it_cannot_name(self):
        from psiv_tools import kosinski
        from psiv_tools.decode_cache import scope
        rom = bytes(4)
        with scope() as table:
            with self.assertRaises(ValueError):
                kosinski.decompress(bytearray(rom), 8)
        with self.assertRaisesRegex(ValueError, "mutable buffer"):
            census_entries(table, rom)
        with scope() as table:
            with self.assertRaises(ValueError):
                kosinski.decompress(rom[1:], 8)
        with self.assertRaisesRegex(ValueError, "other than the ROM"):
            census_entries(table, rom)

    def test_the_census_names_calls_only(self):
        census = json.loads(CENSUS.read_text(encoding="utf-8"))
        calls = census["calls"]
        self.assertGreater(len(calls), 1000)
        keys = {"nemesis": {"decoder", "offset"}, "kosinski": {"decoder", "offset"},
                "enigma": {"decoder", "offset", "base_tile", "max_words"}}
        for call in calls:
            self.assertEqual(set(call), keys[call["decoder"]], call)
            self.assertIsInstance(call["offset"], int)
        frozen = [json.dumps(c, sort_keys=True) for c in calls]
        self.assertEqual(len(frozen), len(set(frozen)), "a call is listed twice")


class ProgressCase(unittest.TestCase):
    def test_one_line_per_section_then_installed(self):
        out = "progress 1/2 sound\nprogress 2/2 maps\ninstalled runtime-pack\n"
        self.assertEqual(check_progress(out, 2), [])
        self.assertEqual(check_progress("installed runtime-pack\n", 0), [])

    def test_bad_progress_fails(self):
        for out in ("progress 1/2 sound\nprogress 2/2 maps\n",  # never installed
                    "progress 1/2 sound\ninstalled p\n",  # a section unreported
                    "progress 1/3 sound\nprogress 2/3 maps\ninstalled p\n",  # wrong total
                    "progress 2/2 sound\nprogress 1/2 maps\ninstalled p\n",  # out of order
                    "progress 1/2 sound\nsound done\nprogress 2/2 maps\ninstalled p\n",
                    ""):
            with self.subTest(out=out):
                self.assertNotEqual(check_progress(out, 2), [])


class WrongRomCase(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="psiv-pack-parity-rom-")
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def stand_in(self, message, code=2):
        """A program that refuses the way `psiv-pack` would, saying `message`
        with the ROM's file name in place of `{}`."""
        path = self.root / "stand-in"
        path.write_text(f"#!{sys.executable}\nimport os, sys\n"
                        f"print('psiv-pack: error: ' + {message!r}.format("
                        "os.path.basename(sys.argv[1])),"
                        f" file=sys.stderr)\nsys.exit({code})\n", encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)
        return path

    def test_python_wording(self):
        rom = self.root / "zeros"
        rom.write_bytes(bytes(0x400))
        with self.assertRaises(RomError) as raised:
            read_rom(rom)
        message = str(raised.exception)
        self.assertEqual(message, wrong_rom_message(rom, ZEROS_SHA))
        self.assertTrue(message.startswith(f"{rom.name} is not the US retail ROM PSIV builds from: "))
        self.assertNotIn(str(rom.parent), message)
        self.assertTrue(message.endswith(f", expected {EXPECTED_SHA256}"))

    def test_a_tiny_file_is_refused_in_the_same_words(self):
        rom = self.root / "tiny"
        rom.write_bytes(b"x")
        with self.assertRaisesRegex(RomError, "is not the US retail ROM PSIV builds from"):
            read_rom(rom)

    def test_same_wording_passes(self):
        binary = self.stand_in(wrong_rom_message("{}", ZEROS_SHA))
        self.assertEqual(check_wrong_rom(binary, self.root), [])

    def test_other_wording_fails(self):
        binary = self.stand_in("{} is not a ROM I know")
        problems = check_wrong_rom(binary, self.root)
        self.assertEqual(len(problems), 1)
        self.assertTrue(problems[0].startswith("wrong ROM: psiv-pack says"))

    def test_accepting_a_wrong_rom_fails(self):
        binary = self.stand_in(wrong_rom_message("{}", ZEROS_SHA), code=0)
        self.assertEqual(check_wrong_rom(binary, self.root), ["wrong ROM: psiv-pack exited 0"])


class CommandLineCase(unittest.TestCase):
    def test_missing_rom_is_a_failed_step(self):
        with tempfile.TemporaryDirectory(prefix="psiv-pack-parity-cli-") as tmp:
            self.assertEqual(pack_parity.main(["--rom", str(Path(tmp) / "absent")]), 1)


if __name__ == "__main__":
    unittest.main()
