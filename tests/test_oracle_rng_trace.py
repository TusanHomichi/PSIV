"""Tests for oracle/rng_trace.py, the RNG trace checker.

The fixtures are hand-built rows: the arithmetic those rows are built with is
written out here (and cross-checked against the transcription in
oracle/checks.py) instead of being borrowed from the module under test, so a
wrong rotation, a wrong roll or a broken chain has to fail the checker rather
than agree with it. The roll column in particular is built by
`cartridge_roll`, spelled out from the disassembly of `UpdateRNGSeed2`
(ps4.asm:86097) - the word `sub.w (RNG_Seed).w,d0` subtracts is the one at
$FFFFEF0C, the `RNG_Seed` longword's *high* half, because `RNG_Seed`
(ps4.constants.asm:2328) is a longword and the 68000 reads big-endian.

`HostAgreement` closes the loop the other way: it compiles
oracle/host/rng_trace.h and oracle/host/provenance.h into a probe and runs
them, so the derivation the C host writes into a trace and the derivation this
module checks are the same one, and neither can drift into a convention of its
own. That is how the low-half roll survived: the host computed it and `check`
re-derived it with the same mistake, so only an outside anchor could see it.
"""
import io
import os
import re
import shutil
import subprocess
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
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


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


def cartridge_roll(hv, frame_count, seed):
    """The trace's `roll` column, written out again from the disassembly.

    `sub.w (RNG_Seed).w, d0` reads the word at $FFFFEF0C: the high half of the
    longword there, and the word `ror (RNG_Seed).w` rotates next."""
    return (hv + frame_count - ((seed >> 16) & M16)) & M16


def low_word_roll(hv, frame_count, seed):
    """The subtraction the host wrote before it was fixed ($FFFFEF0E)."""
    return (hv + frame_count - (seed & M16)) & M16


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
            "roll": f"{cartridge_roll(hv, frame_count, seed):04X}",
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
        entry["roll"] = f"{cartridge_roll(int(entry['hv'], 16), int(
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
                entry["roll"] = f"{cartridge_roll(hv, count, seed):04X}"
        for entry in self.log:
            if entry["frame"] == frame:
                entry["main_frame_count"] = count
        return self

    def low_word_rolls(self):
        """Every row's roll column as the pre-fix host wrote it: low half."""
        for entry in self.rows:
            entry["roll"] = f"{low_word_roll(
                int(entry['hv'], 16), int(entry['frame_count']),
                int(entry['seed_before'], 16)):04X}"
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
        # move.w $8(a5),d0 / add.w Main_Frame_Count,d0 / sub.w RNG_Seed.w,d0,
        # with the word $FFFFEF0C being the seed longword's high half.
        self.assertEqual(roll_for(0x293E, 23931, 0x6EA56A13), 0x1814)
        self.assertEqual(roll_for(0x0000, 0, 0x00000000), 0x0000)
        self.assertEqual(roll_for(0x0000, 0, 0x00010000), 0xFFFF)

    def test_the_subtrahend_is_the_seeds_high_half_not_its_low_half(self):
        # A seed whose halves differ: 0x00010000 subtracts 1 (high) or 0
        # (low); 0x0000FFFF subtracts 0 (high) or 65535 (low).
        self.assertEqual(roll_for(0x1000, 0x0010, 0x00010000), 0x100F)
        self.assertEqual(low_word_roll(0x1000, 0x0010, 0x00010000), 0x1010)
        self.assertEqual(roll_for(0x1000, 0x0010, 0x0000FFFF), 0x1010)
        self.assertEqual(low_word_roll(0x1000, 0x0010, 0x0000FFFF), 0x1011)
        self.assertNotEqual(roll_for(0x1000, 0x0010, 0x00010000),
                            low_word_roll(0x1000, 0x0010, 0x00010000))

    def test_the_traces_own_first_row_derives_to_the_cartridges_roll(self):
        # Tape 07 frame 29711, call 0: hv 2292, Main_Frame_Count 28815, seed
        # 21E817F3. The pre-fix column read 7B2E, which is the low half's
        # subtraction; the high half's is 7139 and is what the ledger's
        # damage table resolves against (docs/oracle/BATTLE_ORACLE_REPLAY.md).
        self.assertEqual(roll_for(0x2292, 28815, 0x21E817F3), 0x7139)
        self.assertEqual(low_word_roll(0x2292, 28815, 0x21E817F3), 0x7B2E)

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

    def test_a_low_word_roll_column_is_rejected(self):
        # What oracle/host/rng_trace.c wrote before it was fixed: the seeds,
        # the chain and the frame counter are all exactly right, and only the
        # roll column subtracts the longword's low half. That is why the
        # checker re-deriving its own `roll_for` could not see it, and why the
        # column has to be the disassembly's arithmetic and nothing else.
        fixture = self.fixture().low_word_rolls()
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("f50 call 0: roll", out)
        self.assertIn("seed_high", out)

    def test_the_low_word_column_is_rejected_at_the_row_that_uses_it(self):
        fixture = self.fixture()
        entry = fixture.find(51, 1)
        entry["roll"] = f"{low_word_roll(int(entry['hv'], 16), int(
            entry['frame_count']), int(entry['seed_before'], 16)):04X}"
        status, out = self.run_check(fixture)
        self.assertEqual(status, 1)
        self.assertIn("f51 call 1: roll", out)


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


#: A probe built from the host's own headers. Its two modes print what the C
#: side computes, so the Python side can be compared with the code that
#: actually writes a trace instead of with a description of it.
PROBE_SOURCE = r'''
#include <stdio.h>
#include <string.h>

#include "oracle/host/provenance.h"
#include "oracle/host/rng_trace.h"

int main(int argc, char **argv)
{
	char line[256];
	int i;

	if (argc < 2)
		return 2;
	if (strcmp(argv[1], "roll") == 0) {
		/* seed hv frame_count, one per line */
		while (fgets(line, sizeof line, stdin)) {
			unsigned long seed, hv, count;

			if (sscanf(line, "%lx %lx %lx", &seed, &hv, &count) != 3)
				return 2;
			printf("%04X\n", (unsigned)rng_trace_roll((uint32_t)seed,
			                                          (unsigned)hv,
			                                          (unsigned)count));
		}
		return 0;
	}
	if (strcmp(argv[1], "basename") == 0) {
		for (i = 2; i < argc; i++)
			printf("%s\n", path_basename(argv[i]));
		return 0;
	}
	return 2;
}
'''

#: What the C probe should answer. Written out here from the disassembly, so a
#: host and a checker that drift into the same wrong convention together still
#: fail: the numbers do not come from either implementation.
PROBE_ROWS = [
    # tape 07, frame 29711, calls 0 and 1 (docs/oracle/BATTLE_ORACLE_REPLAY.md)
    (0x21E817F3, 0x2292, 28815),
    (0x10F417F3, 0x23F3, 28815),
    # seeds whose halves differ, so the two conventions cannot coincide
    (0x00010000, 0x1000, 0x0010),
    (0x0000FFFF, 0x1000, 0x0010),
    (0x6EA56A13, 0x293E, 23931),
    (0x00000000, 0x0000, 0x0000),
]

#: Where a run wrote its trace is not part of what it observed, so the
#: provenance line names the file: (path given, what the log must say).
PROBE_PATHS = [
    ("build/tape07_rolls.csv", "tape07_rolls.csv"),
    ("/tmp/somewhere/else/first-battle.csv", "first-battle.csv"),
    ("trace.csv", "trace.csv"),
    ("./nested/trace-final.csv", "trace-final.csv"),
    ("oracle/logs/tape07_rolls.csv", "tape07_rolls.csv"),
]

#: The `--rng-trace` provenance line, and the argument it must hand to fprintf.
RNG_TRACE_LINE = re.compile(
    r'fprintf\(out,\s*"# rng-trace=%s\\n"\s*,\s*(.+?)\)\s*;')


class HostAgreement(unittest.TestCase):
    """oracle/host and this module must derive the same roll.

    oracle/host/rng_trace.c and `oracle/rng_trace.py check` re-derive the
    cartridge's roll independently, and a mistake they share is invisible to
    both: that is exactly how the low-half subtraction survived in the trace.
    This case compiles the host's `rng_trace_roll` and `path_basename` and runs
    them, comparing against numbers written out here from the disassembly.
    """

    @classmethod
    def setUpClass(cls):
        cls.compiler = (shutil.which("cc") or shutil.which("gcc")
                        or shutil.which("clang"))
        if not cls.compiler:
            raise unittest.SkipTest("no C compiler for the oracle/host probe")
        cls.tmp = tempfile.mkdtemp(prefix="psiv-oracle-probe-")
        source = os.path.join(cls.tmp, "probe.c")
        with open(source, "w") as handle:
            handle.write(PROBE_SOURCE)
        cls.binary = os.path.join(cls.tmp, "probe")
        built = subprocess.run(
            [cls.compiler, "-O0", "-Wall", "-Wextra", "-I", ROOT,
             "-o", cls.binary, source],
            capture_output=True, text=True, cwd=ROOT)
        if built.returncode != 0:
            shutil.rmtree(cls.tmp, ignore_errors=True)
            raise AssertionError(
                f"the oracle/host probe did not build:\n{built.stderr}")

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def probe(self, *arguments, stdin=""):
        result = subprocess.run([self.binary, *arguments], input=stdin,
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.split()

    def test_the_host_computes_the_cartridges_roll(self):
        out = self.probe("roll", stdin="".join(
            f"{seed:x} {hv:x} {count:x}\n" for seed, hv, count in PROBE_ROWS))
        self.assertEqual(len(out), len(PROBE_ROWS))
        for (seed, hv, count), text in zip(PROBE_ROWS, out):
            roll = int(text, 16)
            self.assertEqual(
                roll, roll_for(hv, count, seed),
                f"the host's roll for seed {seed:08X} is {text}, "
                f"oracle/rng_trace.py check derives "
                f"{roll_for(hv, count, seed):04X}")

    def test_the_host_does_not_subtract_the_seeds_low_half(self):
        # The negative control for the defect this file was fixed for: for
        # every seed whose halves differ, the host must answer the high half's
        # roll and not the low half's.
        rows = [(seed, hv, count) for seed, hv, count in PROBE_ROWS
                if (seed >> 16) != (seed & M16)]
        self.assertTrue(rows, "the probe table needs unequal seed halves")
        out = self.probe("roll", stdin="".join(
            f"{seed:x} {hv:x} {count:x}\n" for seed, hv, count in rows))
        for (seed, hv, count), text in zip(rows, out):
            self.assertEqual(int(text, 16), cartridge_roll(hv, count, seed))
            self.assertNotEqual(int(text, 16), low_word_roll(hv, count, seed))

    def test_the_hosts_basename_drops_every_directory(self):
        out = self.probe("basename", *[path for path, _ in PROBE_PATHS])
        self.assertEqual(out, [name for _, name in PROBE_PATHS])

    def test_the_host_names_the_rng_trace_by_basename_in_the_log(self):
        """Two runs to different output paths must produce identical bytes.

        The `# rng-trace=` provenance line is the one place a run writes one of
        its own output paths into a file, so it has to name the file alone. The
        line itself needs the emulator core and the ROM to produce, so its call
        site is read here and its helper is what the probe above runs.
        """
        with open(os.path.join(ROOT, "oracle", "host",
                               "psiv_oracle.c")) as handle:
            source = handle.read()
        line = RNG_TRACE_LINE.search(source)
        self.assertIsNotNone(line, "no `# rng-trace=` provenance line")
        self.assertEqual(line.group(1).strip(),
                         "path_basename(rng_trace_path)")


if __name__ == "__main__":
    unittest.main()
