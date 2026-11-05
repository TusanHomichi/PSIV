"""The fresh-Continue verifier's byte rules, without Godot.

    PYTHONPATH=. python3 -m unittest tests.test_verify_native_continue -v

tools/native/verify_native_continue.py compares a source slot with the slot
saved after CONTINUE and one Down step. These pin the standing-Y rule (#129):
a native slot written before the `SY` marker stored the standing row, a
marked slot stores the cartridge's object Y, and the resave always carries
the marker. Each control is one way a run can go wrong.
"""

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERIFIER = ROOT / "tools" / "native" / "verify_native_continue.py"


def slot_file(slot, y_word, marker, roster_mask=1, exit_index=0, native=True):
    """A 0x1600-byte slot file, logical header and payload interleaved. A
    native one carries what the writer adds: the roster mask and `TR`, the
    exit index and its complement (`with_dungeon_exit`); `native=False` is a
    cartridge image, whose header `InitSRAM` zeroes."""
    header, payload = bytearray(0x100), bytearray(0xA00)
    header[0x02:0x11] = b"PHANTASY STAR 4"
    header[0x12:0x14] = slot.to_bytes(2, "big")
    if native:
        header[0x20:0x22] = roster_mask.to_bytes(2, "big")
        header[0x22:0x26] = bytes([ord("T"), ord("R"), exit_index, exit_index ^ 0xFF])
    if marker:
        header[0x2E:0x30] = b"SY"
    payload[0x306:0x308] = (38 * 16).to_bytes(2, "big")
    payload[0x308:0x30A] = y_word.to_bytes(2, "big")
    payload[0x400] = 0x41
    header[0x14 + slot * 2:0x16 + slot * 2] = (sum(payload) & 0xFFFF).to_bytes(2, "big")
    data = bytearray(0x1600)
    data[1:0x200:2] = header
    data[0x201::2] = payload
    return bytes(data)


def receipt(row):
    state = {"map": 19, "world": 0, "flags": [], "money": 0, "leader": 0,
             "inventory": [], "party_status": [], "party_resources": []}
    return {"complete": True, "observations": [
        {"label": "continued", "state": dict(state, cell=[38, row])},
        {"label": "moved", "state": dict(state, cell=[38, row + 1])},
        {"label": "resaved", "state": dict(state, cell=[38, row + 1])},
    ]}


class ContinueVerifierTest(unittest.TestCase):
    def verify(self, source, resaved, row=18):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            (tmp / "source.sram").write_bytes(source)
            (tmp / "resaved.sram").write_bytes(resaved)
            (tmp / "receipt.json").write_text(json.dumps(receipt(row)))
            return subprocess.run(
                [sys.executable, str(VERIFIER),
                 "--source", str(tmp / "source.sram"),
                 "--source-sha256", hashlib.sha256(source).hexdigest(),
                 "--resaved", str(tmp / "resaved.sram"),
                 "--receipt", str(tmp / "receipt.json"),
                 "--out", str(tmp / "out.json")],
                capture_output=True, text=True, check=False)

    def assertPasses(self, source, resaved):
        run = self.verify(source, resaved)
        self.assertEqual(run.returncode, 0, run.stderr)

    def assertFails(self, source, resaved, message):
        run = self.verify(source, resaved)
        self.assertNotEqual(run.returncode, 0)
        self.assertIn(message, run.stderr)

    def test_a_marked_source_steps_one_object_row(self):
        # Standing row 18 is object row 17; one step down is object row 18.
        self.assertPasses(slot_file(0, 17 * 16, True), slot_file(1, 18 * 16, True))

    def test_a_legacy_source_resaves_in_the_retail_convention(self):
        # The legacy word is the standing row; the marked resave after one
        # step holds the same word, since it is now the object row.
        self.assertPasses(slot_file(0, 18 * 16, False), slot_file(1, 18 * 16, True))

    def test_a_resave_without_the_marker_fails(self):
        self.assertFails(slot_file(0, 18 * 16, False), slot_file(1, 19 * 16, False),
                         "the resave lacks the SY marker")

    def test_a_resave_that_did_not_step_fails(self):
        self.assertFails(slot_file(0, 17 * 16, True), slot_file(1, 17 * 16, True),
                         "the step is not one row down")
        self.assertFails(slot_file(0, 18 * 16, False), slot_file(1, 17 * 16, True),
                         "the step is not one row down")

    def test_a_cartridge_image_reads_as_the_object_row(self):
        # No native metadata, no marker: the cartridge's own convention. Its
        # resave adds what the writer always adds: mask, `TR` and `SY`.
        self.assertPasses(slot_file(0, 17 * 16, False, native=False),
                          slot_file(1, 18 * 16, True))

    def test_a_legacy_slot_with_no_roster_is_still_legacy(self):
        # A session without battle files seats no roster: a zero mask, but
        # `TR` still marks the file native.
        self.assertPasses(slot_file(0, 18 * 16, False, roster_mask=0),
                          slot_file(1, 18 * 16, True, roster_mask=0))

    def test_a_cartridge_resave_may_not_add_other_metadata(self):
        resaved = bytearray(slot_file(1, 18 * 16, True))
        resaved[1 + 0x26 * 2] = ord("P")
        self.assertFails(slot_file(0, 17 * 16, False, native=False), bytes(resaved),
                         "non-checksum header state changed")
        malformed = bytearray(slot_file(1, 18 * 16, True))
        malformed[1 + 0x25 * 2] = 0
        self.assertFails(slot_file(0, 17 * 16, False, native=False), bytes(malformed),
                         "the resave's dungeon exit is not the writer's")

    def test_a_native_source_may_not_gain_a_roster_or_exit(self):
        self.assertFails(slot_file(0, 17 * 16, True, exit_index=0),
                         slot_file(1, 18 * 16, True, exit_index=3),
                         "non-checksum header state changed")

    def test_another_header_change_fails(self):
        resaved = bytearray(slot_file(1, 18 * 16, True))
        resaved[1 + 0x30 * 2] = 0x01
        self.assertFails(slot_file(0, 17 * 16, True), bytes(resaved),
                         "non-checksum header state changed")


if __name__ == "__main__":
    unittest.main()
