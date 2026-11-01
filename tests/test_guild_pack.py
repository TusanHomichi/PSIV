"""The guild section of the pack: each anchor is checked against the code naming it."""
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from psiv_tools.guild_pack import (
    DATA,
    EVENT_START,
    GuildError,
    MAP_PALETTE_POINTER,
    PENDING_LEA,
    emit_guild,
    extract_guild,
)

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class GuildPack(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.rom = ROM.read_bytes()

    def test_the_record_has_eight_jobs_and_three_whole_lines(self) -> None:
        record = extract_guild(self.rom)
        self.assertEqual(len(record["names"]), 8)
        self.assertTrue(all(record["names"]))
        self.assertEqual(len(record["rows"]), 8)
        self.assertEqual(len(record["money"]), 8)
        for line in ("font_line", "done_line", "closed_line"):
            self.assertEqual(len(record[line]), 16)

    def test_the_list_window_is_where_the_cursor_says(self) -> None:
        # `InitWindowCursor` at x $108, y $98: the window's first box cell,
        # with the sprite plane's $80 origin.
        window = extract_guild(self.rom)["window"]
        self.assertEqual((window["x"] + 1) * 8 + 0x80, 0x108)
        self.assertEqual((window["y"] + 1) * 8 + 0x80, 0x98)

    def test_a_moved_anchor_is_refused(self) -> None:
        for offset in (EVENT_START + 2, PENDING_LEA + 2, MAP_PALETTE_POINTER):
            broken = bytearray(self.rom)
            broken[offset] ^= 0xFF
            with self.subTest(offset=hex(offset)), self.assertRaises(GuildError):
                extract_guild(bytes(broken))

    def test_the_manifest_entry_names_the_file_it_wrote(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            entry = emit_guild(self.rom, Path(directory))
            data = (Path(directory) / entry["path"]).read_bytes()
        self.assertEqual(entry["sha256"], hashlib.sha256(data).hexdigest())
        self.assertEqual(json.loads(data)["kind"], "guild")
        self.assertGreater(DATA, EVENT_START)


if __name__ == "__main__":
    unittest.main()
