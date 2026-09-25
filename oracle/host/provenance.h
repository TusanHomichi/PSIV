#ifndef PSIV_ORACLE_PROVENANCE_H
#define PSIV_ORACLE_PROVENANCE_H

#include <string.h>

/* The basename of a path, for the provenance lines that name a path.
 *
 * Where a run *reads* its inputs from, and where it *writes* its outputs, is
 * not what the run observed: a path is not a property of the capture. A
 * provenance line that names one therefore names the file, not the path it was
 * given: two runs that differ only in their directories then produce
 * byte-identical files, so a capture can be compared - or pinned by sha256 -
 * without depending on where it was written or re-run from. The RAM log's
 * `# tape=` and `# rng-trace=` lines are those lines today; the trace itself
 * carries no path at all.
 *
 * A run's `# rom=` line is the exception: the ROM by its full path is what a
 * capture was taken against, and the oracle's own ledgers pin that spelling.
 *
 * tests/test_oracle_rng_trace.py compiles this header into a probe and checks
 * the basenames it returns; tests/test_oracle_force_battle.py carries the same
 * probe for the tape paths a capture is re-run with. */
static inline const char *path_basename(const char *path)
{
	const char *slash = strrchr(path, '/');

	return slash ? slash + 1 : path;
}

#endif
