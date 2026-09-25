#ifndef PSIV_ORACLE_PROVENANCE_H
#define PSIV_ORACLE_PROVENANCE_H

#include <string.h>

/* The basename of a path, for the provenance lines that name a path.
 *
 * Where a run *reads* its inputs from, and where it *writes* its outputs, is
 * not what the run observed: a path is not a property of the capture. Every
 * provenance line that names a file therefore names the file and not the path
 * it was given - `# rom=`, `# tape=` and `# rng-trace=` alike, and a fixture's
 * own `tape` field (`oracle/fixture/logs.py`'s `provenance_lines`) - so two
 * runs that differ only in their directories produce byte-identical files and
 * a capture can be compared, or pinned by sha256, without depending on where
 * it was written or re-run from. What identifies the ROM is the size the
 * `# rom=` line carries beside the name, not the directory it was read from.
 *
 * tests/test_oracle_rng_trace.py compiles this header into a probe and checks
 * the basenames it returns; tests/test_oracle_force_battle_provenance.py
 * carries the same probe for the ROM and tape paths a capture is taken with. */
static inline const char *path_basename(const char *path)
{
	const char *slash = strrchr(path, '/');

	return slash ? slash + 1 : path;
}

#endif
