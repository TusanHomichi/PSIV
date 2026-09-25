"""A fixture names files, never directories.

Two captures that differ only in where they were swept are the same capture,
so everything a fixture records about them has to read the same: the tape is
recorded by its file name, and the provenance lines a capture's log opens with
are stored with their path fields cut to the file's own name
(`oracle/fixture/logs.py`; `oracle/host/provenance.h` is the same rule on the
host's side, pinned by `tests/test_oracle_force_battle_provenance.py`).

Nothing here starts the emulator: the capture is hand-built, as in
`tests/test_oracle_battle_fixture_forced.py`.
"""
import json
import os
import shutil
import tempfile
import unittest

from oracle import fixture as fx
from oracle.fixture.__main__ import main as extractor
from tests.test_oracle_battle_fixture import cartridge_roll, trace_row
from tests.test_oracle_battle_fixture_forced import (FORCED_MAP, ForcedLog,
                                                     ForcedFixture)

#: A log header of the shape the host wrote before it named its inputs by
#: their basenames: the ROM's path carries spaces, the tape's does not.
CAPTURE_HEADER = [
    "# core=Genesis Plus GX v1.7.4 2d7131c region=NTSC",
    "# rom=/home/peter/PSIV/Phantasy Star IV (USA).md size=3145728",
    "# tape=build/lane-evidence/sweep/formation_5E/forced_5E_attack.tape "
    "steps=2412 frames=26612",
    "# rng-trace=build/lane-evidence/sweep/formation_5E/capture/"
    "forced_5E_attack_rolls.csv",
]


class ProvenanceLines(unittest.TestCase):
    """`provenance_lines` keeps the value, drops the directory."""

    def lines(self, text):
        path = os.path.join(self.tmpdir, "header.csv")
        with open(path, "w") as handle:
            handle.write("\n".join(text) + "\n")
        return fx.provenance_lines(path)

    def setUp(self):
        self.tmpdir = tempfile.mkdtemp(prefix="psiv-provenance-")
        self.addCleanup(shutil.rmtree, self.tmpdir, ignore_errors=True)

    def test_a_rom_path_with_spaces_is_cut_to_its_name(self):
        self.assertEqual(
            self.lines([CAPTURE_HEADER[1]]),
            ["# rom=Phantasy Star IV (USA).md size=3145728"])

    def test_the_tape_keeps_its_own_fields(self):
        self.assertEqual(
            self.lines([CAPTURE_HEADER[2]]),
            ["# tape=forced_5E_attack.tape steps=2412 frames=26612"])

    def test_the_roll_trace_is_cut_to_its_name(self):
        self.assertEqual(
            self.lines([CAPTURE_HEADER[3]]),
            ["# rng-trace=forced_5E_attack_rolls.csv"])

    def test_a_line_that_names_no_path_is_untouched(self):
        self.assertEqual(self.lines([CAPTURE_HEADER[0], "# a note", "#  "]),
                         [CAPTURE_HEADER[0], "# a note", "#  "])

    def test_a_header_the_host_already_wrote_path_free_reads_the_same(self):
        once = self.lines(CAPTURE_HEADER)
        path = os.path.join(self.tmpdir, "again.csv")
        with open(path, "w") as handle:
            handle.write("\n".join(once) + "\n")
        self.assertEqual(fx.provenance_lines(path), once)


class ExtractionIsPathFree(ForcedFixture):
    """The same capture, extracted twice, is one fixture."""

    def capture(self, directory, tape):
        """A four-frame capture on disk: the trace, the log, and the tape name.

        The frames are `tests/test_oracle_battle_fixture_forced.py`'s vehicle
        shape, which the extractor accepts as it stands.
        """
        os.makedirs(directory, exist_ok=True)
        builder = ForcedLog()
        builder.frame(20, enemy_count=0)
        builder.frame(21, enemy_count=1, e1_id=81, e1_hp=1040, e1_maxhp=1040)
        builder.frame(22, turn_00=6, turn_01=20)
        builder.frame(23, battle_actor=6, hit_00="00", dmg_00=57,
                      alys_hp=683, e1_hp=1040)
        header = [entry["name"] for entry in FORCED_MAP["fields"]]
        log_path = os.path.join(directory, "capture.csv")
        with open(log_path, "w") as handle:
            for line in CAPTURE_HEADER:
                handle.write(line + "\n")
            handle.write(",".join(header) + "\n")
            for row in builder.rows:
                handle.write(",".join(row[column] for column in header) + "\n")
        trace_path = os.path.join(directory, "rolls.csv")
        with open(trace_path, "w") as handle:
            handle.write("# PSIV rolls\n")
            handle.write("frame,call_index_in_frame,pc,hv,frame_count,"
                         "seed_before,roll,seed_after\n")
            for frame, count in {20: 1, 21: 1, 22: 13, 23: 17}.items():
                for index in range(count):
                    hv = self.HV + frame
                    row = trace_row(frame, index, hv, self.FRAME_COUNT,
                                    self.SEED,
                                    cartridge_roll(hv, self.FRAME_COUNT,
                                                   self.SEED))
                    handle.write(",".join(row[name] for name in (
                        "frame", "call_index_in_frame", "pc", "hv",
                        "frame_count", "seed_before", "roll", "seed_after"))
                        + "\n")
        ram_map = os.path.join(directory, "ram_map.json")
        with open(ram_map, "w") as handle:
            json.dump(FORCED_MAP, handle)
        return log_path, trace_path, ram_map, tape

    def extract(self, out, tape, capture, root):
        log_path, trace_path, ram_map, _ = capture
        status = extractor([
            "--trace", trace_path, "--log", log_path, "--out", out,
            "--ram-map", ram_map, "--tape", tape, "--battle-first", "20",
            "--battle-last", "23", "--minified", "--core", "c", "--patch", "p",
        ])
        self.assertEqual(status, 0)
        with open(out, "rb") as handle:
            return handle.read()

    def test_two_directories_and_two_tape_spellings_are_one_fixture(self):
        first = tempfile.mkdtemp(prefix="psiv-sweep-a-")
        second = tempfile.mkdtemp(prefix="psiv-sweep-b-")
        self.addCleanup(shutil.rmtree, first, ignore_errors=True)
        self.addCleanup(shutil.rmtree, second, ignore_errors=True)
        one = self.extract(os.path.join(first, "fixture.json"),
                           "/sweeps/one/forced_5E_attack.tape",
                           self.capture(first, "/sweeps/one/forced_5E_attack.tape"),
                           first)
        two = self.extract(os.path.join(second, "fixture.json"),
                           "sweeps/two/forced_5E_attack.tape",
                           self.capture(second, "sweeps/two/forced_5E_attack.tape"),
                           second)
        self.assertEqual(one, two, "the same capture is one fixture")
        fixture = json.loads(one)
        provenance = fixture["provenance"]
        self.assertEqual(provenance["tape"], "forced_5E_attack.tape")
        self.assertEqual(provenance["trace"], "rolls.csv")
        for line in provenance["log_header"] + provenance["trace_header"]:
            self.assertNotIn("lane-evidence", line, line)
            self.assertNotIn("/sweeps", line, line)


if __name__ == "__main__":
    unittest.main()
