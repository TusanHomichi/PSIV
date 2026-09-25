#ifndef PSIV_ORACLE_RNG_TRACE_H
#define PSIV_ORACLE_RNG_TRACE_H

#include <stdint.h>

/* UpdateRNGSeed2's roll (ps4.asm:86097, ROM $04239E), as the cartridge
 * computes it:
 *
 *     30 2D 00 08     move.w  $8(a5), d0             ; the VDP HV counter
 *     D0 78 EF 1C     add.w   (Main_Frame_Count).w, d0
 *     90 78 EF 0C     sub.w   (RNG_Seed).w, d0        ; d0 = the roll
 *
 * `sub.w (RNG_Seed).w, d0` is an absolute-short word operand at $FFFFEF0C,
 * where `RNG_Seed` (ps4.constants.asm:2328) is a *longword*: the 68000 reads
 * the word at $FFFFEF0C/$FFFFEF0D, the longword's HIGH half - the same word
 * the next instruction, `ror (RNG_Seed).w` (ROM $0423AA), rotates. Subtracting
 * the low half at $FFFFEF0E yields this roll shifted by a per-frame constant;
 * docs/oracle/BATTLE_ORACLE_REPLAY.md settles which one the cartridge uses against
 * tape 07's own turn order and damage numbers.
 *
 * `seed` is the RNG_Seed longword the call started from, `hv` and
 * `frame_count` the words the two earlier instructions saw, and the result the
 * word `d0` holds when the routine returns - the roll its caller gets.
 *
 * This is the one place the host computes a roll, and oracle/rng_trace.py's
 * `roll_for` re-derives it independently: tests/test_oracle_rng_trace.py
 * builds a program against this header and compares the two, so the host and
 * the checker cannot drift into agreeing on a convention the tests pin. */
static inline uint16_t rng_trace_roll(uint32_t seed, unsigned hv,
                                      unsigned frame_count)
{
	unsigned high = (unsigned)((seed >> 16) & 0xFFFFu);

	return (uint16_t)((hv + frame_count - high) & 0xFFFFu);
}

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
