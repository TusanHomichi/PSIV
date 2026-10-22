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
