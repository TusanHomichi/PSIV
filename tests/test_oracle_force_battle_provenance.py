"""Tests for the provenance lines a capture's RAM log opens with.

A capture is only reproducible if its log says what the run was given *and*
nothing about where the run happened to write: `tests/test_oracle_rng_trace.py`
pins that for `--rng-trace`, and this file pins it for the ROM and the tape -
the same capture re-run from another directory has to produce the same bytes,
which is what makes a capture's sha256 worth pinning in a ledger.

The line itself needs the emulator core and the ROM to produce, so its call
site is read out of `oracle/host/psiv_oracle.c` and the helper it hands to
`fprintf` is compiled from `oracle/host/provenance.h` and run.
"""
import os
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

#: The paths a capture is composed with, and what the lines must say. The ROM
#: is named by the file alone as well - what identifies it is the size the line
#: carries, not the directory it was read from - and its name carries spaces,
#: which is why the line keeps its value and its own fields apart.
PROBE_PATHS = [
    ("build/forced/helex/forced_5E_attack.tape", "forced_5E_attack.tape"),
    ("/tmp/somewhere/else/forced_53_attack.tape", "forced_53_attack.tape"),
    ("forced_37_attack.tape", "forced_37_attack.tape"),
    ("./nested/dir/forced_5E_attack_d5.tape", "forced_5E_attack_d5.tape"),
    ("build/lane-evidence/f94/forced_5E_attack.tape",
     "forced_5E_attack.tape"),
    ("/home/peter/PSIV/Phantasy Star IV (USA).md",
     "Phantasy Star IV (USA).md"),
]

#: The `# tape=` and `# rom=` provenance lines, and the argument each must hand
#: to fprintf.
TAPE_LINE = re.compile(r'fprintf\(out,\s*"# tape=%s steps=%d frames=%llu\\n"\s*,\s*(.+?),')
ROM_LINE = re.compile(
    r'fprintf\(out,\s*"# rom=%s size=%zu\\n"\s*,\s*(.+?)\);')

PROBE_SOURCE = """
#include <stdio.h>
#include <stdlib.h>
#include "oracle/host/provenance.h"

int main(int argc, char **argv)
{
	int i;

	if (argc > 1 && !strcmp(argv[1], "basename")) {
		for (i = 2; i < argc; i++)
			printf("%s\\n", path_basename(argv[i]));
		return 0;
	}
	return 2;
}
"""


class TapeProvenance(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.compiler = (shutil.which("cc") or shutil.which("gcc")
                        or shutil.which("clang"))
        if not cls.compiler:
            raise unittest.SkipTest("no C compiler for the oracle/host probe")
        cls.tmp = tempfile.mkdtemp(prefix="psiv-provenance-probe-")
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

    def test_the_hosts_basename_drops_every_directory(self):
        result = subprocess.run(
            [self.binary, "basename", *[path for path, _ in PROBE_PATHS]],
            capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        # One name per line: a ROM's name carries spaces, so the probe's
        # output cannot be split on whitespace.
        self.assertEqual(result.stdout.splitlines(),
                         [name for _, name in PROBE_PATHS])

    def test_the_host_names_the_tape_by_basename_in_the_log(self):
        """Two captures that differ only in their directory are one capture.

        The tape is this run's own *input*, but where it sits is not what the
        run observed: the log hands `path_basename` the path it was given, so a
        capture reproduced from another directory has the same bytes and can be
        pinned by sha256.
        """
        with open(os.path.join(ROOT, "oracle", "host",
                               "psiv_oracle.c")) as handle:
            source = handle.read()
        line = TAPE_LINE.search(source)
        self.assertIsNotNone(line, "no `# tape=` provenance line")
        self.assertEqual(line.group(1).strip(), "path_basename(tape_path)")

    def test_the_host_names_the_rom_by_basename_in_the_log(self):
        """The ROM too is named by its file, and the size beside it is what
        identifies the dump: a capture taken against the same ROM from another
        directory has the same bytes, so its sha256 can be pinned in a ledger
        regardless of where the ROM sits.
        """
        with open(os.path.join(ROOT, "oracle", "host",
                               "psiv_oracle.c")) as handle:
            source = handle.read()
        line = ROM_LINE.search(source)
        self.assertIsNotNone(line, "no `# rom=` provenance line")
        self.assertEqual(line.group(1).strip(), "path_basename(rom_path), rom_size")


if __name__ == "__main__":
    unittest.main()
