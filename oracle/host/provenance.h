#ifndef PSIV_ORACLE_PROVENANCE_H
#define PSIV_ORACLE_PROVENANCE_H

#include <string.h>

/* The basename of a path, for the provenance lines of a run's outputs.
 *
 * Where a run *reads* its inputs from is part of what it observed and belongs
 * in the log verbatim; where it *writes* one of its own outputs is not. A
 * provenance line that names an output therefore names the file, not the path
 * it was given: two runs that differ only in their output directories then
 * produce byte-identical files, so a capture can be compared - or pinned by
 * sha256 - without depending on where it was written. `--rng-trace`'s
 * `# rng-trace=` line in the RAM log is the one such line today; the trace
 * itself carries no path at all.
 *
 * tests/test_oracle_rng_trace.py compiles this header into a probe and checks
 * the basenames it returns. */
static inline const char *path_basename(const char *path)
{
	const char *slash = strrchr(path, '/');

	return slash ? slash + 1 : path;
}

#endif
