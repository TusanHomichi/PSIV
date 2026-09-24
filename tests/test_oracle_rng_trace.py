"""Tests for oracle/rng_trace.py, the RNG trace checker.

The fixtures are hand-built rows: the arithmetic those rows are built with is
written out here (and cross-checked against the transcription in
oracle/checks.py) instead of being borrowed from the module under test, so a
wrong rotation, a wrong roll or a broken chain has to fail the checker rather
than agree with it.
"""
import io
import os
import tempfile
import unittest
from unittest import mock

from oracle.checks import update_rng_seed
from oracle.rng_trace import check, rotate_seed, roll_for

HEADER = ("frame,call_index_in_frame,pc,hv,frame_count,seed_before,roll,"
          "seed_after")
COLUMNS = HEADER.split(",")
M16 = 0xFFFF
M32 = 0xFFFFFFFF


def cartridge_multiply(seed):
    """UpdateRNGSeed (ps4.asm:86066) written out again, independently."""
    d1 = 0x2A6D365B if seed & M16 == 0 else seed
    d1 = (d1 * 41) & M32  # d1*2, d1*2, +d1, <<3, +d1
    lo, hi = d1 & M16, (d1 >> 16) & M16
    return (((lo + hi) & M16) << 16) | lo


def ror_high(seed):
    """`ror RNG_Seed.w` written out again, independently."""
    hi, lo = (seed >> 16) & M16, seed & M16
    return (((hi >> 1) | ((hi & 1) << 15)) << 16) | lo


class Fixture:
    """Builds a trace and a matching RAM log from per-frame descriptions.

    A frame's log row carries the seed the frame ended with and the frame
    counter as it stood at the end of the frame; `stepped` says whether the
    VBlank handler's block (UpdateRNGSeed + Main_Frame_Count) ran in it.
    """

    def __init__(self, tmpdir, start=5000):
        self.tmpdir = tmpdir
        self.start = start
        self.rows = []
        self.log = []

    def frame(self, number, calls, seed=0x12345678, stepped=True):
        if self.log:
            opened = self.log[-1]["seed"]
            count = self.log[-1]["main_frame_count"]
        else:
            opened = seed
            count = self.start
            self.log.append({"frame": number - 1, "seed": opened,
                             "main_frame_count": count})
        seed = cartridge_multiply(opened) if stepped else opened
        count = (count + 1) & M16 if stepped else count
        for index, hv in enumerate(calls):
            self.rows.append(self.build_row(number, index, hv, count, seed))
            seed = ror_high(seed)
        self.log.append({"frame": number, "seed": seed,
                         "main_frame_count": count})
        return self

    @staticmethod
    def build_row(frame, index, hv, frame_count, seed, pc="0423A2"):
        return {
            "frame": str(frame),
            "call_index_in_frame": str(index),
            "pc": pc,
            "hv": f"{hv:04X}",
            "frame_count": str(frame_count),
            "seed_before": f"{seed:08X}",
            "roll": f"{roll_for(hv, frame_count, seed):04X}",
            "seed_after": f"{ror_high(seed):08X}",
        }

    def find(self, frame, index):
        for entry in self.rows:
            if entry["frame"] == str(frame) and \
                    entry["call_index_in_frame"] == str(index):
                return entry
        raise AssertionError(f"no row f{frame} call {index}")

    def edit(self, frame, index, **changes):
        self.find(frame, index).update(changes)
        return self

    def restart(self, frame, index, seed, **changes):
        """Rewrite a row so it starts from `seed` and stays self-consistent."""
        entry = self.find(frame, index)
        entry["seed_before"] = f"{seed:08X}"
        entry["roll"] = f"{roll_for(int(entry['hv'], 16), int(
            entry['frame_count']), seed):04X}"
        entry["seed_after"] = f"{ror_high(seed):08X}"
        entry.update(changes)
        return self

    def count_of(self, frame):
        for entry in self.log:
            if entry["frame"] == frame:
                return entry["main_frame_count"]
        raise AssertionError(f"the log has no frame {frame}")

    def set_count(self, frame, count):
        """Move a frame's counter without moving its seed (a contradiction)."""
        for entry in self.rows:
            if entry["frame"] == str(frame):
                hv = int(entry["hv"], 16)
                seed = int(entry["seed_before"], 16)
                entry["frame_count"] = str(count)
                entry["roll"] = f"{roll_for(hv, count, seed):04X}"
        for entry in self.log:
            if entry["frame"] == frame:
                entry["main_frame_count"] = count
        return self

    def drop_log_frame(self, frame):
        self.log = [entry for entry in self.log if entry["frame"] != frame]
        return self

    def write(self, name="trace"):
        trace = os.path.join(self.tmpdir, f"{name}.csv")
        log = os.path.join(self.tmpdir, f"{name}-log.csv")
        with open(trace, "w") as handle:
            handle.write("# hand-built\n" + HEADER + "\n")
            for entry in self.rows:
                handle.write(",".join(entry[c] for c in COLUMNS) + "\n")
        with open(log, "w") as handle:
            handle.write("# hand-built\nframe,mark,rng_seed,main_frame_count\n")
            for entry in self.log:
                handle.write(f"{entry['frame']},.,{entry['seed']:08X},"
                             f"{entry['main_frame_count']}\n")
        return trace, log


class TraceCase(unittest.TestCase):
    """Shared temp directory and helper for the fixtures."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def fixture(self, name="trace"):
        return Fixture(self.tmp.name)

    def run_check(self, fixture, name="trace"):
        trace, log = fixture.write(name)
        out = io.StringIO()
        return check(trace, log, out=out), out.getvalue()


class TestArithmetic(unittest.TestCase):
    def test_rotate_is_a_one_bit_right_rotation_of_the_high_word(self):
        self.assertEqual(rotate_seed(0xAABBCCDD), 0xD55DCCDD)
        self.assertEqual(rotate_seed(0x0001CCDD), 0x8000CCDD)
        self.assertEqual(rotate_seed(0x0000CCDD), 0x0000CCDD)

    def test_rotating_sixteen_times_returns_the_seed(self):
        seed = 0x9E3779B9
        for _ in range(16):
            seed = rotate_seed(seed)
        self.assertEqual(seed, 0x9E3779B9)

    def test_roll_is_the_three_instructions_written_out(self):
        # move.w $8(a5),d0 / add.w Main_Frame_Count,d0 / sub.w RNG_Seed.w,d0
        self.assertEqual(roll_for(0x293E, 23931, 0x6EA56A13), 0x1CA6)
        self.assertEqual(roll_for(0x0000, 0, 0x00000000), 0x0000)
        self.assertEqual(roll_for(0x0000, 0, 0x00000001), 0xFFFF)

    def test_the_multiply_matches_checks_py(self):
        for seed in (0x00000000, 0x00010000, 0x12345678, 0x6EA56A13,
                     0xFFFFFFFF, 0xDEADBEEF):
            self.assertEqual(cartridge_multiply(seed), update_rng_seed(seed),
                             f"seed {seed:08X}")


class TestCheckAccepts(TraceCase):
    def test_a_chain_across_frames_is_accepted(self):
        fixture = (self.fixture()
                   .frame(100, [0x1234], seed=0xAABBCCDD, stepped=False)
                   .frame(101, [0x2000, 0x2010], stepped=True)
                   .frame(102, [0x3000], stepped=False))
        status, out = self.run_check(fixture)
        self.assertEqual(status, 0, out)
        self.assertIn("3 frame(s) chain to the log's rng_seed", out)

    def test_a_multiplied_frame_and_a_kept_frame_both_pass(self):
        fixture = (self.fixture()
                   .frame(10, [0x0F00], seed=0x11223344, stepped=False)
                   .frame(11, [0x0F10], stepped=True)
                   .frame(12, [0x0F20], stepped=False)
                   .frame(13, [0x0F30], stepped=True))
        status, out = self.run_check(fixture)
        self.assertEqual(status, 0, out)
        self.assertIn("4 frame(s) chain", out)

    def test_frames_the_log_lacks_are_counted_and_skipped(self):
        fixture = (self.fixture()
                   .frame(20, [0x0A00], seed=0x01020304, stepped=False)
                   .frame(21, [0x0A10], stepped=True)
                   .frame(22, [0x0A20], stepped=False)
                   .drop_log_frame(22))
        status, out = self.run_check(fixture)
        self.assertEqual(status, 0, out)
        self.assertIn("1 frame(s) skipped", out)

    def test_a_multiply_that_leaves_the_seed_alone_is_not_a_mismatch(self):
        fixture = self.fixture().frame(30, [0x0B00], seed=0x11223344,
                                       stepped=False)
        trace, log = fixture.write()
        with mock.patch("oracle.rng_trace.update_rng_seed", lambda seed: seed):
            out = io.StringIO()
            status = check(trace, log, out=out)
        self.assertEqual(status, 0, out.getvalue())
        self.assertIn("where the frame's seed and its UpdateRNGSeed are "
                      "the same", out.getvalue())

    def test_the_pc_column_is_reported(self):
        fixture = self.fixture().frame(40, [0x0C00], seed=0x0BADCAFE)
        status, out = self.run_check(fixture)
        self.assertEqual(status, 0, out)
        self.assertIn("$0423A2", out)


class TestCheckRejects(TraceCase):
    def fixture(self, name="trace"):
        return (Fixture(self.tmp.name)
                .frame(50, [0x1000], seed=0xAABBCCDD, stepped=False)
                .frame(51, [0x1100, 0x1110, 0x1120], stepped=True)
                .frame(52, [0x1200, 0x1210], stepped=False))

    def test_a_wrong_roll_is_rejected(self):
        fixture = self.fixture()
        status, out = self.run_check(fixture)
        self.assertEqual(status, 0, out)
        fixture.edit(51, 1, roll="0001")
        status, out = self.run_check(fixture, name="trace-b")
        self.assertEqual(status, 1)
        self.assertIn("f51 call 1: roll 0001 is not", out)

    def test_a_seed_after_that_is_not_the_rotation_is_rejected(self):
        fixture = self.fixture().edit(50, 0, seed_after="12345678")
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("is not ror16 of seed_before", out)

    def test_a_broken_chain_between_calls_is_rejected(self):
        fixture = self.fixture().restart(51, 2, 0x0BADF00D)
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("the previous call left", out)

    def test_a_frame_that_ends_on_the_wrong_seed_is_rejected(self):
        fixture = self.fixture()
        fixture.log[-1]["seed"] ^= 0x00010000
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("the log's rng_seed for the frame is", out)

    def test_a_call_that_starts_from_neither_seed_is_rejected(self):
        fixture = self.fixture().restart(50, 0, 0x0F0F0F0F)
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("neither the frame's seed", out)

    def test_a_frame_count_the_log_does_not_have_is_rejected(self):
        fixture = self.fixture().edit(52, 0, frame_count="999")
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("is not the log's Main_Frame_Count", out)

    def test_a_multiplied_anchor_without_a_counter_step_is_rejected(self):
        fixture = self.fixture().frame(53, [0x1300, 0x1310], stepped=True)
        # the calls were built from a multiplied seed; take the step back out
        # of the counter so the anchor and Main_Frame_Count disagree
        fixture.set_count(53, fixture.count_of(52))
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("but Main_Frame_Count did not step", out)

    def test_a_kept_anchor_with_a_counter_step_is_rejected(self):
        fixture = self.fixture().frame(53, [0x1300], stepped=False)
        # the frame kept its seed, but claim the VBlank block ran
        fixture.set_count(53, fixture.count_of(52) + 1)
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("but Main_Frame_Count stepped", out)

    def test_rows_that_are_not_grouped_by_frame_are_rejected(self):
        fixture = self.fixture()
        fixture.rows.append(fixture.rows[0])
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("not written together", out)

    def test_call_numbering_that_skips_is_rejected(self):
        fixture = self.fixture().edit(51, 1, call_index_in_frame="7")
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("call 1 is numbered 7", out)


class TestCheckInput(TraceCase):
    def test_an_empty_trace_is_rejected(self):
        trace = os.path.join(self.tmp.name, "trace.csv")
        log = os.path.join(self.tmp.name, "log.csv")
        with open(trace, "w") as handle:
            handle.write("# nothing here\n")
        with open(log, "w") as handle:
            handle.write("frame,rng_seed,main_frame_count\n")
        self.assertEqual(check(trace, log, out=io.StringIO()), 1)

    def test_a_log_without_the_seed_column_is_rejected(self):
        fixture = self.fixture().frame(60, [0x1400], seed=0x0BADCAFE)
        trace, log = fixture.write()
        with open(log, "w") as handle:
            handle.write("frame,mark,main_frame_count\n60,.,5060\n")
        out = io.StringIO()
        self.assertEqual(check(trace, log, out=out), 2)
        self.assertIn("--groups rng", out.getvalue())

    def test_a_trace_without_a_roll_column_is_rejected(self):
        fixture = self.fixture().frame(70, [0x1500], seed=0x0BADCAFE)
        trace, log = fixture.write()
        with open(trace, "w") as handle:
            handle.write("# hand-built\nframe,call_index_in_frame,pc,hv,"
                         "frame_count,seed_before,seed_after\n"
                         "70,0,0423A2,1500,5001,0BADCAFE,055E6CAF\n")
        out = io.StringIO()
        self.assertEqual(check(trace, log, out=out), 2)
        self.assertIn("no roll column", out.getvalue())


if __name__ == "__main__":
    unittest.main()
