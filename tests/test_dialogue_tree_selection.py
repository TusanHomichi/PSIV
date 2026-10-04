"""US retail world selection, including corruption controls (issue #79)."""

import struct
import unittest
from pathlib import Path

from psiv_tools.dialogue_pack.common import DialoguePackError
from psiv_tools.dialogue_pack.selection import SELECTION_ROUTINE, extract_selection
from psiv_tools.dialogue_pack.selection_census import compare_map, census, markdown
from psiv_tools.maps import extract_maps
from psiv_tools.text import dialogue_tree_specs

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"


@unittest.skipUnless(ROM.is_file(), "US retail ROM is absent")
class WorldDialogueSelectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = ROM.read_bytes()
        cls.selection = extract_selection(cls.rom)

    def test_table_identities_come_from_the_compressed_tree_starts(self):
        identities = {spec["start"]: spec["tree"] for spec in dialogue_tree_specs(self.rom)}
        table = int(self.selection["table_offset"], 16)
        actual = [identities[struct.unpack_from(">I", self.rom, table + i * 4)[0]]
                  for i in range(6)]
        self.assertEqual(self.selection["world_trees"], actual)
        self.assertEqual(self.selection["first_world_override"]["tree"],
                         identities[struct.unpack_from(">I", self.rom, table + 24)[0]])
        self.assertEqual(self.selection["first_world_override"]["entry_from"],
                         struct.unpack_from(">H", self.rom, SELECTION_ROUTINE + 18)[0])

    def test_negative_control_rejects_a_changed_unsigned_branch(self):
        corrupt = bytearray(self.rom)
        corrupt[SELECTION_ROUTINE + 20] ^= 1
        with self.assertRaisesRegex(DialoguePackError, "opcode mismatch"):
            extract_selection(corrupt)

    def test_negative_control_rejects_an_unknown_pointer(self):
        corrupt = bytearray(self.rom)
        table = int(self.selection["table_offset"], 16)
        struct.pack_into(">I", corrupt, table + 4, 0)
        with self.assertRaisesRegex(DialoguePackError, "is not a tree"):
            extract_selection(corrupt)

    def test_negative_control_rejects_a_truncated_table(self):
        table = int(self.selection["table_offset"], 16)
        with self.assertRaisesRegex(DialoguePackError, "past the end"):
            extract_selection(self.rom[:table + 4])

    def test_census_checks_every_map_and_valid_world(self):
        import os
        pack = Path(os.environ.get("PSIV_RUNTIME_PACK", str(ROM.parent / "runtime-pack")))
        if not (pack / "manifest.json").is_file():
            self.skipTest(f"runtime pack absent at {pack}")
        result = census(self.rom, pack)
        sources = [row for row in extract_maps(self.rom)["maps"] if not row["is_null"]]
        self.assertEqual(result["map_count"], len(sources))
        self.assertEqual(result["area_count"], sum(
            area["interaction_type"] == 0 for row in sources
            for area in row["interaction_areas"]["entries"]))
        self.assertEqual(result["worlds"], list(range(6)))
        self.assertEqual(len(markdown(result).splitlines()), len(sources) + 2)
        for row in result["maps"]:
            for area in row["areas"]:
                self.assertEqual(len(area["world_trees"]), 6)
                self.assertEqual(area["differs"], [tree != row["map_tree"]
                                                  for tree in area["world_trees"]])

    def test_census_negative_control_rejects_a_changed_map_binding(self):
        source = next(row for row in extract_maps(self.rom)["maps"] if not row["is_null"])
        with self.assertRaisesRegex(DialoguePackError, "tree disagrees"):
            compare_map(source, {"dialogue_tree": 0})
