"""The `ps4.asm` citation check (#158) on a tiny disassembly.

    PYTHONPATH=. python3 -m unittest tests.test_check_citations -v

The fixture citations are written `{asm}:N` and filled in at run time, so the
real check, which scans this file too, does not read them against the real
disassembly.
"""

import tempfile
import unittest
from pathlib import Path

from tools.check_citations import run

ASM = "ps4" + ".asm"

# Routine spans 1-3; Other spans 4-5 and Routine's body calls it (line 2).
DISASM = "Routine:\n\tjsr Other\n\trts\nOther:\n\trts\n"
DOC = """`Routine`, {asm}:2
`Other`, {asm}:5
`Other` (`{asm}:5`)
{asm}:5 (Other)
{asm}:2 (Other)
`Other`, {asm}:3
{asm}:99
{asm}:4
"""

# A routine with a branch target and a named loop inside it, then a separator
# and a second routine; then a named routine right after a branch target.
ROUTINES = """Main:
\tmoveq #0, d0
loc_1A2B:
\tmoveq #1, d0
Main_FillLoop:
\tdbf d7, Main_FillLoop
\trts
; -----------------------------------------------------------------
Next:
\trts
loc_2C3D:
\trts
Named:
\trts
"""


class Citations(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        (self.root / "reference/ps4disasm").mkdir(parents=True)

    def tearDown(self):
        self.tmp.cleanup()

    def check(self, doc=DOC, disasm=DISASM):
        (self.root / "README.md").write_text(doc.format(asm=ASM))
        (self.root / "reference/ps4disasm/ps4.asm").write_text(disasm)
        return run(self.root, files=["README.md"])

    def test_the_forms_agree_with_the_label_bodies(self):
        mismatches, summary = self.check()
        self.assertEqual(summary, "citations: 8 citation(s), 4 label-verified, "
                                  "1 use-site, 1 unverified, 2 mismatch(es)")
        self.assertIn("Other is at line 4-5, not ps4.asm:3-3", mismatches[0])
        self.assertIn("past the disassembly's 5 lines", mismatches[1])

    def test_a_range_within_the_body_passes_and_an_off_by_one_label_fails(self):
        mismatches, summary = self.check("`Routine`, {asm}:1-3\n`Other`, {asm}:3\n")
        self.assertEqual(summary, "citations: 2 citation(s), 1 label-verified, "
                                  "0 use-site, 0 unverified, 1 mismatch(es)")
        self.assertIn("Other is at line 4-5, not ps4.asm:3-3", mismatches[0])

    def test_a_reference_the_body_does_not_cover_but_the_lines_use_passes(self):
        _, summary = self.check("`Other`, {asm}:2\n")
        self.assertEqual(summary, "citations: 1 citation(s), 0 label-verified, "
                                  "1 use-site, 0 unverified, 0 mismatch(es)")

    def test_a_bare_citation_is_unverified_not_failed(self):
        mismatches, summary = self.check("{asm}:4\n")
        self.assertEqual(mismatches, [])
        self.assertIn("1 unverified", summary)

    def test_a_label_the_disassembly_does_not_define_is_unverified(self):
        mismatches, summary = self.check("`NotThere`, {asm}:4\n")
        self.assertEqual(mismatches, [])
        self.assertIn("1 unverified", summary)

    def test_an_absent_disassembly_is_skipped(self):
        (self.root / "README.md").write_text(DOC.format(asm=ASM))
        self.assertEqual(run(self.root, files=["README.md"]),
                         ([], "citations: skipped (no reference/ps4disasm/ps4.asm)"))

    def test_a_routine_runs_past_its_branch_targets_and_loops(self):
        # Line 6 sits past `loc_1A2B` and `Main_FillLoop`, both inside Main.
        mismatches, summary = self.check("`Main`, {asm}:6\n`Main` ({asm}:1-7)\n",
                                         ROUTINES)
        self.assertEqual(mismatches, [])
        self.assertIn("2 label-verified", summary)

    def test_a_routine_ends_at_the_separator(self):
        # Line 9 is `Next`, past the separator at line 8 that closes Main.
        mismatches, _ = self.check("`Main`, {asm}:9\n", ROUTINES)
        self.assertEqual(len(mismatches), 1)
        self.assertIn("Main is at line 1-7, not ps4.asm:9-9", mismatches[0])

    def test_a_routine_ends_at_the_next_named_label(self):
        # `loc_2C3D` stays inside Next; `Named` opens a routine of its own.
        mismatches, _ = self.check("`Next`, {asm}:12\n`Next`, {asm}:13\n", ROUTINES)
        self.assertEqual(len(mismatches), 1)
        self.assertIn("Next is at line 9-12, not ps4.asm:13-13", mismatches[0])

    def test_the_label_beside_a_citation_is_the_one_read(self):
        # Two labels then two lines read the second label against the first
        # line: the form the audit rewrote as one label per citation.
        mismatches, _ = self.check("`Routine` and `Other` (`{asm}:1`, `{asm}:4`)\n")
        self.assertEqual(len(mismatches), 1)
        self.assertIn("Other is at line 4-5, not ps4.asm:1-1", mismatches[0])
        mismatches, _ = self.check("`Routine` (`{asm}:1`) and `Other` (`{asm}:4`)\n")
        self.assertEqual(mismatches, [])

    def test_a_citation_with_the_disassembly_path_is_paired(self):
        path = "reference/ps4disasm/" + ASM
        mismatches, summary = self.check(f"`Other` (`{path}:5`)\n`Other`, {path}:3\n")
        self.assertIn("1 label-verified", summary)
        self.assertEqual(len(mismatches), 1)
        self.assertIn("Other is at line 4-5, not ps4.asm:3-3", mismatches[0])

    def test_a_compact_citation_after_a_full_one_is_read(self):
        # `:N` follows a full citation in the same file and pairs like one; a
        # `:N` in a file with no full citation is not a citation.
        mismatches, summary = self.check("`Routine` ({asm}:1) and `Other` (`:3`)\n"
                                         "| `Other`, `:4-5` |\n")
        self.assertEqual(summary, "citations: 3 citation(s), 2 label-verified, "
                                  "0 use-site, 0 unverified, 1 mismatch(es)")
        self.assertIn("Other is at line 4-5, not ps4.asm:3-3", mismatches[0])
        _, summary = self.check("`Other` (`:3`) at 12:30\n")
        self.assertEqual(summary, "citations: 0 citation(s), 0 label-verified, "
                                  "0 use-site, 0 unverified, 0 mismatch(es)")

    def test_a_bare_offset_continuing_a_list_is_read(self):
        # The list's later items are lines too; a count or a value after a
        # citation is not.
        big = DISASM + "\trts\n" * 200
        mismatches, summary = self.check("`Routine` ({asm}:1-3,105, `106`, 999)\n"
                                         "({asm}:4, 153 entries)\n(`:5`, 0)\n", big)
        self.assertEqual(summary, "citations: 6 citation(s), 1 label-verified, "
                                  "0 use-site, 4 unverified, 1 mismatch(es)")
        self.assertIn("ps4.asm:999-999 is past the disassembly's 205 lines", mismatches[0])

    def test_every_separator_in_use_is_read_and_an_address_fails(self):
        # An en dash range, a slash list and grep's trailing colon are read; a
        # ROM address after the file name is not a line and fails.
        big = DISASM + "\trts\n" * 200
        mismatches, summary = self.check("`Other` ({asm}:4\u20135)\n{asm}:101/999\n"
                                         "{asm}:3: rts\n{asm}:5D48\n", big)
        self.assertEqual(summary, "citations: 4 citation(s), 1 label-verified, "
                                  "0 use-site, 2 unverified, 2 mismatch(es)")
        self.assertIn("ps4.asm:999-999 is past", mismatches[0])
        self.assertIn(f"{ASM}:5D48 is not a line citation", mismatches[1])

    def test_a_malformed_range_end_fails_and_a_trailing_label_reads_a_dash(self):
        # The range end is part of the citation: a typo in it is not passed
        # over as the bare start, and `N–M (Label)` pairs like `N-M (Label)`.
        mismatches, summary = self.check("{asm}:4-999oops\n{asm}:4\u20135 (Routine)\n")
        self.assertEqual(summary, "citations: 1 citation(s), 0 label-verified, "
                                  "0 use-site, 0 unverified, 2 mismatch(es)")
        self.assertIn(f"{ASM}:4-999oops is not a line citation", mismatches[0])
        self.assertIn("Routine is at line 1-3, not ps4.asm:4-5", mismatches[1])

    def test_a_citation_must_be_its_whole_token(self):
        # Every malformed tail fails, compact ones included; a range split
        # across lines fails; an open end and a full stop are read.
        bad = ["4-abc", "4..abc", "4/999oops", "4-"]
        mismatches, _ = self.check("{asm}:4,999oops and {asm}:4, `101-9x`\n"
                                   "{asm}:4,999- and {asm}:4 / 999oops\n")
        self.assertEqual([m.split(": ", 1)[1] for m in mismatches],
                         [f"{tail} is not a line citation"
                          for tail in ("999oops", "101-9x", "999-", "999oops")])
        compact = ":" + "999oops"  # split, so this file's own scan passes it
        doc = "".join(f"{{asm}}:{tail}\n" for tail in bad) + f"{{asm}}:4+ and `{compact}`.\n"
        mismatches, summary = self.check(doc + "See {asm}:4.\n")
        self.assertEqual([m.split(": ", 1)[1] for m in mismatches],
                         [f"{ASM}:{tail} is not a line citation" for tail in bad]
                         + [f"{compact} is not a line citation"])
        self.assertIn("5 mismatch(es)", summary)

    def test_a_dotted_range_is_read_whole(self):
        mismatches, _ = self.check("`Other` ({asm}:4..6)\n`Routine`, {asm}:1..3\n")
        self.assertEqual(len(mismatches), 1)
        self.assertIn("ps4.asm:4-6 is past the disassembly's 5 lines", mismatches[0])

    def test_the_asm_of_a_citation_is_not_its_label(self):
        # Without a label beside it the citation is unverified, not read
        # against a routine named `asm`.
        mismatches, summary = self.check("see ({asm}:1)\n", DISASM + "asm:\n\trts\n")
        self.assertEqual(mismatches, [])
        self.assertIn("1 unverified", summary)

    def test_every_text_file_is_read_whatever_its_suffix(self):
        # A C comment and a tape comment are citations too; a binary file and
        # a dangling link are passed over, not a crash.
        self.check("")
        (self.root / "trace.c").write_text(f"/* Other, {ASM}:3 */\n")
        (self.root / "run.tape").write_text(f"# Other ({ASM}:3)\n")
        (self.root / "art.bin").write_bytes(f"\0Other, {ASM}:3".encode())
        (self.root / "AGENTS.md").symlink_to(self.root / "SECURITY.md")  # dangling
        mismatches, summary = run(self.root, files=["trace.c", "run.tape", "art.bin",
                                                    "AGENTS.md"])
        self.assertEqual(len(mismatches), 2)
        self.assertIn("2 citation(s)", summary)


if __name__ == "__main__":
    unittest.main()
