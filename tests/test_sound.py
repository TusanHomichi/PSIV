"""Sound records, driver vocabulary and deterministic raw emission."""

from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from psiv_tools.core import read_rom
from psiv_tools.sound import (
    COMMAND_SPECS,
    META_SPECS,
    SoundError,
    emit_sound,
    extract_sound,
)


ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
DOC = Path(__file__).resolve().parents[1] / "docs" / "sound" / "SOUND_EXTRACTION.md"


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class SoundCase(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rom = read_rom(ROM)
        cls.payload = extract_sound(cls.rom)

    def test_complete_id_space_and_track_inventory(self):
        payload = self.payload
        census = payload["census"]
        self.assertEqual(len(payload["music"]["records"]), 52)
        self.assertEqual(len(payload["sfx"]["regular"]["records"]), 67)
        self.assertEqual(len(payload["sfx"]["special"]["records"]), 3)
        self.assertEqual(census["music_track_count"], 468)
        self.assertEqual(census["regular_sfx_track_count"], 86)
        self.assertEqual(census["special_sfx_track_count"], 3)
        self.assertEqual(census["track_count"], 557)
        self.assertEqual(census["unresolved"], [])
        self.assertEqual(payload["unresolved"], [])

    def test_command_census_is_documented_and_decoded(self):
        census = self.payload["census"]
        seen = {int(value, 16) for value in census["command_opcodes_seen"]}
        self.assertEqual(census["command_vocabulary_size"], len(seen))
        self.assertTrue(seen <= set(COMMAND_SPECS))
        self.assertEqual(
            set(int(value, 16) for value in census["meta_opcodes_seen"]),
            {0},
        )
        self.assertEqual(len(COMMAND_SPECS), 32)
        self.assertEqual(len(META_SPECS), 1)
        document = DOC.read_text()
        for opcode in COMMAND_SPECS:
            with self.subTest(opcode=f"{opcode:02X}"):
                self.assertIn(f"`{opcode:02X}`", document)
        self.assertIn("`00`", document)

    def test_every_track_carries_an_instrument_census(self):
        tracks = self.payload["census"]["instruments_used_per_track"]
        self.assertEqual(len(tracks), 557)
        self.assertEqual(len({track["track_id"] for track in tracks}), 557)
        for track in tracks:
            self.assertEqual(track["instruments_used"], sorted(set(track["instruments_used"])))
            self.assertEqual(
                track["psg_instruments_used"],
                sorted(set(track["psg_instruments_used"])),
            )

    def test_every_voice_table_emits_all_definitions_and_only_zero_pad(self):
        groups = (
            self.payload["music"]["records"],
            self.payload["sfx"]["regular"]["records"],
            self.payload["sfx"]["special"]["records"],
        )
        counts = []
        for group in groups:
            for record in group:
                counts.append(len(record["voices"]))
                self.assertEqual(record["header"]["voice_count"], len(record["voices"]))
                self.assertIn(record["header"]["voice_padding_bytes"], (0, 1))
        self.assertEqual(sum(counts[:52]), 234)
        self.assertEqual(sum(counts[52:119]), 88)
        self.assertEqual(sum(counts[119:]), 5)

    def test_raw_record_slices_are_byte_exact(self):
        payload = self.payload
        with tempfile.TemporaryDirectory() as directory:
            result = emit_sound(self.rom, directory)
            root = Path(directory) / "sound"
            for group in (
                payload["music"]["records"],
                payload["sfx"]["regular"]["records"],
                payload["sfx"]["special"]["records"],
            ):
                for record in group:
                    with self.subTest(record=record["symbol"]):
                        source = record["record"]
                        start = int(source["rom_offset"], 16)
                        end = int(source["end_offset_exclusive"], 16)
                        raw = (root / source["raw_file"]).read_bytes()
                        self.assertEqual(raw, self.rom[start:end])
                        self.assertEqual(
                            hashlib.sha256(raw).hexdigest(), source["raw_sha256"]
                        )
            self.assertEqual(result["unresolved"], [])

    def test_emit_is_deterministic_in_one_process(self):
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            emit_sound(self.rom, first)
            emit_sound(self.rom, second)
            left = Path(first) / "sound"
            right = Path(second) / "sound"
            left_files = sorted(path.relative_to(left) for path in left.rglob("*") if path.is_file())
            right_files = sorted(path.relative_to(right) for path in right.rglob("*") if path.is_file())
            self.assertEqual(left_files, right_files)
            for relative in left_files:
                with self.subTest(file=str(relative)):
                    self.assertEqual((left / relative).read_bytes(), (right / relative).read_bytes())

    def test_provenance_refuses_drift(self):
        changed = bytearray(self.rom)
        changed[0xD1C40] ^= 0x01
        with self.assertRaises(SoundError):
            extract_sound(bytes(changed))

    def test_index_inventory_hashes_its_emitted_files(self):
        with tempfile.TemporaryDirectory() as directory:
            emit_sound(self.rom, directory)
            root = Path(directory)
            index = json.loads((root / "sound/index.json").read_text())
            for entry in index["files"]:
                with self.subTest(file=entry["file"]):
                    data = (root / entry["file"]).read_bytes()
                    self.assertEqual(len(data), entry["size_bytes"])
                    self.assertEqual(hashlib.sha256(data).hexdigest(), entry["sha256"])
