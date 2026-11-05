"""Every cartridge write to `World_Index` has a cited native owner (#126).

    PYTHONPATH=. python3 -m unittest tests.test_world_index_writes -v

The world byte decides the RYUKA/TELEPIPE town lists and the ship's
destination list, so a write the runtime misses strands a later save on the
wrong planet. `Runtime::set_world_index` (rust/psiv-runtime/src/travel.rs) is
the one runtime writer; its documentation cites every write the scenes and the
destination menu make, and the new-game initializer's own write is the pack's
`world_index` start field. This census reads the disassembly's writes and
fails when one is uncited, so a newly found write cannot be dropped silently.
The disassembly is a local input: without it the case is skipped.
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DISASM = ROOT / "reference" / "ps4disasm" / "ps4.asm"
OWNER = ROOT / "rust" / "psiv-runtime" / "src" / "travel.rs"
INITIALIZER = ROOT / "rust" / "psiv-pack" / "src" / "game_start" / "mod.rs"

#: An instruction that writes `World_Index`: its last operand, unless the
#: instruction only compares or tests (`cmp*`, `tst`, `btst`), or the one
#: operand of a one-operand write.
WRITE = re.compile(r"^\s+(?:(?!cmp|tst|btst)\S+\s+[^;]*,\s*|(?:clr|st|sf|not|neg|tas)\.?[bwl]?\s+)"
                   r"\(World_Index\)")


def writes():
    lines = DISASM.read_text(encoding="latin-1").splitlines()
    return [number for number, line in enumerate(lines, 1) if WRITE.match(line)]


def cited(path):
    """Line numbers a file cites, as `ps4.asm:N` or a continuing `:N`."""
    return {int(n) for n in re.findall(r"(?:ps4\.asm|(?<![\w$])):(\d{5,6})\b", path.read_text())}


@unittest.skipUnless(DISASM.is_file(), "the disassembly is a local input")
class WorldIndexWritesTest(unittest.TestCase):
    def test_the_pattern_reads_destinations_only(self):
        for line in ["\tmove.b\t#3, (World_Index).w", "\tclr.w\t(World_Index).w",
                     "\tmove.b\t(a0,d0.w), (World_Index).w"]:
            self.assertTrue(WRITE.match(line), line)
        for line in ["\tmove.b\t(World_Index).w, d0", "\tcmpi.b\t#2, (World_Index).w",
                     "\tbtst\t#0, (World_Index).w", "\ttst.b\t(World_Index).w",
                     "\tmove.b\t(World_Index).w, ($FFFFEC29).w"]:
            self.assertFalse(WRITE.match(line), line)

    def test_the_census_finds_the_known_writes(self):
        # The US disassembly's eight: the new-game initializer, the two copies
        # of the destination menu, and five scene writes.
        self.assertEqual(len(writes()), 8, writes())

    def test_every_write_is_cited_by_its_native_owner(self):
        owners = cited(OWNER) | cited(INITIALIZER)
        missing = [line for line in writes() if line not in owners]
        self.assertEqual(missing, [], "World_Index writes no native owner cites")


if __name__ == "__main__":
    unittest.main()
