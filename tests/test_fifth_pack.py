"""The fifth-member section of the pack: each table is found through the code naming it."""
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from psiv_tools.fifth_pack import (
    CLEARS,
    EVENT_START,
    RECORDS_LEA,
    ROWS,
    ROWS_LEA,
    FifthError,
    emit_fifth,
    extract_fifth,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class FifthPack(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ROM.read_bytes()

    def test_each_row_names_a_record_the_routine_refills(self) -> None:
        record = extract_fifth(self.rom)
        self.assertEqual(len(record["rows"]), ROWS)
        self.assertEqual(
            sorted(row["character"] for row in record["rows"]), sorted(record["records"])
        )
        # Distinct stands on whole cells, distinct lines and flags.
        stands = {(row["x"], row["y"]) for row in record["rows"]}
        self.assertEqual(len(stands), ROWS)
        self.assertTrue(all(x % 16 == 0 and y % 16 == 0 for x, y in stands))
        self.assertEqual(len({row["picked_flag"] for row in record["rows"]}), ROWS)
        self.assertEqual(len({row["first_line"] for row in record["rows"]}), ROWS)

    def test_a_moved_anchor_is_refused(self) -> None:
        for offset in (EVENT_START + 2, ROWS_LEA, RECORDS_LEA, CLEARS, CLEARS + 3):
            broken = bytearray(self.rom)
            broken[offset] ^= 0xFF
            with self.subTest(offset=hex(offset)), self.assertRaises(FifthError):
                extract_fifth(bytes(broken))

    def test_a_record_that_is_no_character_is_refused(self) -> None:
        records = extract_fifth(self.rom)["provenance"]["records"]
        broken = bytearray(self.rom)
        broken[int(records, 16) + 3] ^= 0x01
        with self.assertRaises(FifthError):
            extract_fifth(bytes(broken))

    def test_the_manifest_entry_names_the_file_it_wrote(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            entry = emit_fifth(self.rom, Path(directory))
            data = (Path(directory) / entry["path"]).read_bytes()
        self.assertEqual(entry["sha256"], hashlib.sha256(data).hexdigest())
        self.assertEqual(json.loads(data)["kind"], "fifth")


if __name__ == "__main__":
    unittest.main()
