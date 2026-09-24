#ifndef PSIV_ORACLE_RNG_TRACE_H
#define PSIV_ORACLE_RNG_TRACE_H

#include <stdint.h>

/* Binds the core's HV counter trace hooks and opens the roll CSV at path.
 * Fails when the loaded core carries no hooks, which is how a core built
 * without oracle/patches gets caught before a frame is emulated. */
int rng_trace_begin(void *core, const char *path);
int rng_trace_enabled(void);

/* One frame: the work-RAM values the host read around it, then whatever HV
 * reads the core recorded during it. Writes one CSV row per UpdateRNGSeed2
 * call. Returns -1 when a row cannot be written. */
int rng_trace_frame(uint64_t frame, uint32_t seed_before,
                    uint32_t frame_count_before, uint32_t seed_after,
                    uint32_t frame_count_after);

/* Flushes and closes the CSV, reports what the run captured on stderr, and
 * returns 0 only when the capture can be trusted (no dropped records and every
 * frame's calls account for that frame's seed transition). */
int rng_trace_finish(void);
const char *rng_trace_error(void);

#endif
