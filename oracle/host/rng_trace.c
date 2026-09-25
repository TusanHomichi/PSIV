/* psiv_oracle --rng-trace: capture every roll UpdateRNGSeed2 returns.
 *
 * PSIV's battle rolls are not pseudo-random in the usual sense. UpdateRNGSeed2
 * (ps4.asm:86097, ROM $04239E) is four instructions and an rts:
 *
 *     30 2D 00 08     move.w  $8(a5), d0      ; d0 = VDP HV counter ($C00008)
 *     D0 78 EF 1C     add.w   (Main_Frame_Count).w, d0
 *     90 78 EF 0C     sub.w   (RNG_Seed).w, d0   ; d0 = the roll
 *     E6 F8 EF 0C     ror     (RNG_Seed).w       ; and the seed's high word
 *                                                ;  ($FFFFEF0C) is rotated
 *
 * so a roll is the beam position of the 68000's read, the frame counter and
 * the seed's *high* word: `(RNG_Seed).w` is an absolute-short word operand at
 * $FFFFEF0C, where RNG_Seed (ps4.constants.asm:2328) is a longword, so the
 * 68000 reads $FFFFEF0C/$FFFFEF0D - the same word `ror` rotates - and not the
 * low half at $FFFFEF0E. `rng_trace_roll` (rng_trace.h) is the derivation,
 * with the evidence for it. The beam position is hardware timing that
 * psiv-core deliberately does not reproduce (docs/RUNTIME_DESIGN.md, "RNG design"), and
 * the port takes rolls through its Rolls trait instead: this module captures
 * the stream that trait replays.
 *
 * oracle/patches/0001-rng-hv-trace.patch has the core record (pc, hv) for every
 * HV counter read. The records below are filtered to the reads made by
 * UpdateRNGSeed2's `move.w $8(a5),d0`, and each becomes one row:
 *
 *   frame,call_index_in_frame,pc,hv,frame_count,seed_before,roll,seed_after
 *
 * roll = rng_trace_roll(seed, hv, frame_count) - the arithmetic of the second
 * and third instructions, subtracting the seed longword's *high* word, which
 * is what a 68000 word read at $FFFFEF0C returns. seed_before/seed_after are
 * the whole RNG_Seed longword around the call, with seed_after =
 * ror16(seed_before's high word) and the low word carried, which is what
 * `ror (RNG_Seed).w` leaves behind.
 *
 * The seed and the counter are not in the core's records; they are read from
 * work RAM, sampled by the host before and after each frame, and each call's
 * seed is chained within its frame:
 *
 *   - UpdateRNGSeed2 only rotates the seed's high word, so consecutive calls
 *     chain: the next seed_before is the previous seed_after.
 *   - The VBlank handler (ps4.asm:612-625) applies UpdateRNGSeed, which rewrites
 *     the high word as high+low, and bumps Main_Frame_Count, in one block that
 *     the handler skips when it is not entered during vblank. Main_Frame_Count
 *     stepping by exactly one across a frame is therefore the cartridge telling
 *     us that block ran, and the frame's rolls chain from the multiplied seed
 *     and see the bumped counter.
 *   - Genesis Plus GX triggers VINT at the top of the emulated frame (system.c,
 *     system_frame_gen: the VCount is set to bitmap.viewport.h and the
 *     interrupt is taken before the vblank and visible lines run), so that
 *     block comes before every read of the frame, not after it. That is why a
 *     frame's calls chain from update_rng_seed(seed before the frame).
 *
 * A frame's chain has to land exactly on the seed the frame ended with. When
 * it does not, the derivation is not a description of that frame and the run
 * fails instead of quietly writing rows nobody can replay. That failure is a
 * count on stderr, and python3 -m oracle.rng_trace check re-derives the same
 * thing from the CSV and the RAM log of the same run.
 */
#define _GNU_SOURCE
#include "rng_trace.h"

#include <dlfcn.h>
#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* UpdateRNGSeed2's HV read: `move.w $8(a5), d0` is at ROM $04239E (four bytes,
 * the extension word included), the instructions after it are `add.w
 * (Main_Frame_Count).w,d0` at $0423A2, `sub.w (RNG_Seed).w,d0` at $0423A6 and
 * `ror (RNG_Seed).w` at $0423AA; none of those other three reads the counter,
 * so only the first can produce a record. Genesis Plus GX reports the 68000
 * program counter with the instruction's extension word already fetched, so a
 * record arrives as $0423A2 - the address of the instruction after it. The
 * accepted window runs from the read itself to $0423A6, where `sub.w
 * (RNG_Seed).w,d0` starts, so a core that reports the PC elsewhere in or just
 * after the access still matches. The PC of every matched record is written to
 * the CSV, so which point the core actually reports stays visible in the
 * data. */
#define RNG_HV_PC_FIRST 0x04239Eu
#define RNG_HV_PC_LAST  0x0423A6u

/* The V counter value the vblank lines start at in 224-line NTSC (the value
 * the VBlank handler runs at, core/system.c: `if (v_counter !=
 * bitmap.viewport.h)`). Only reported: the derivation above does not need it,
 * but a run whose rolls sit in the vblank rather than in the visible lines is
 * worth seeing. */
#define RNG_HV_VBLANK_LINE 0xE0u

#define MAX_PC_BUCKETS 8
#define MAX_REPORTED_MISMATCHES 8

struct pc_bucket {
	unsigned pc;
	uint64_t count;
};

static FILE *g_out;
static char g_error[256];
static char g_path[1024];

static uint64_t g_records_total;
static uint64_t g_records_matched;
static uint64_t g_records_unmatched;
static uint64_t g_records_dropped;
static uint64_t g_rows;
static uint64_t g_frames_with_calls;
static uint64_t g_frames_not_closed;
static uint64_t g_frames_count_step;
static uint64_t g_calls_before_vblank;
static uint64_t g_calls_after_vblank;
static unsigned g_calls_min, g_calls_max;

static struct pc_bucket g_matched_pcs[MAX_PC_BUCKETS];
static int g_nmatched_pcs;
static struct pc_bucket g_unmatched_pcs[MAX_PC_BUCKETS];
static int g_nunmatched_pcs;

static void (*core_trace_enable)(int);
static void (*core_trace_reset)(void);
static unsigned (*core_trace_count)(void);
static unsigned (*core_trace_dropped)(void);
static int (*core_trace_get)(unsigned, unsigned *, unsigned *);

static void failf(const char *fmt, ...)
{
	va_list ap;

	va_start(ap, fmt);
	vsnprintf(g_error, sizeof g_error, fmt, ap);
	va_end(ap);
}

static void bucket_add(struct pc_bucket *buckets, int *count, unsigned pc)
{
	int i;
	int smallest = 0;

	for (i = 0; i < *count; i++)
		if (buckets[i].pc == pc) {
			buckets[i].count++;
			return;
		}
	if (*count < MAX_PC_BUCKETS) {
		buckets[*count].pc = pc;
		buckets[*count].count = 1;
		(*count)++;
		return;
	}
	/* Keep the busiest few when more instructions read the counter than we
	 * have room to name. */
	for (i = 1; i < *count; i++)
		if (buckets[i].count < buckets[smallest].count)
			smallest = i;
	buckets[smallest].count++;
}

/* UpdateRNGSeed, ps4.asm:86066-86092, transcribed the same way oracle/checks.py
 * does it (an all-zero low word is replaced by the cartridge's constant, the
 * longword is multiplied by 41 with 32-bit wrap, and the new value is the
 * product's low word with its word sum in the high half). Both files have to
 * agree; the RNG checks in oracle/verify.sh are the reason. */
static uint32_t update_rng_seed(uint32_t seed)
{
	uint32_t d1 = seed;
	uint32_t lo, hi;

	if ((d1 & 0xFFFFu) == 0)
		d1 = 0x2A6D365Bu;
	d1 = d1 * 41u;
	lo = d1 & 0xFFFFu;
	hi = (d1 >> 16) & 0xFFFFu;
	return ((((lo + hi) & 0xFFFFu) << 16) | lo);
}

/* `ror (RNG_Seed).w` at ROM $0423AA: E6F8 with the absolute-short operand
 * $EF0C, so the word it rotates is the one at $FFFFEF0C - the RNG_Seed
 * longword's high half, the same word the `sub.w` before it subtracts. The low
 * half at $FFFFEF0E is carried through untouched, which is why a call's
 * seed_after differs from its seed_before in the high word alone. */
static uint32_t rotate_seed_high(uint32_t seed)
{
	uint32_t hi = (seed >> 16) & 0xFFFFu;

	return ((uint32_t)(((hi >> 1) | ((hi & 1u) << 15)) & 0xFFFFu) << 16) |
	       (seed & 0xFFFFu);
}

static void resolve(void *core, const char *name, void **target)
{
	*target = dlsym(core, name);
	if (!*target && !g_error[0]) /* keep the first missing symbol */
		failf("the core has no %s; rebuild it with oracle/build_core.sh "
		      "so oracle/patches reaches the build", name);
}

int rng_trace_begin(void *core, const char *path)
{
	void *enable, *reset, *count, *dropped, *get;

	if (!core || !path) {
		failf("rng trace needs the core handle and a path");
		return -1;
	}
	resolve(core, "psiv_hv_trace_enable", &enable);
	resolve(core, "psiv_hv_trace_reset", &reset);
	resolve(core, "psiv_hv_trace_count", &count);
	resolve(core, "psiv_hv_trace_dropped", &dropped);
	resolve(core, "psiv_hv_trace_get", &get);
	if (g_error[0])
		return -1;

	*(void **)(&core_trace_enable) = enable;
	*(void **)(&core_trace_reset) = reset;
	*(void **)(&core_trace_count) = count;
	*(void **)(&core_trace_dropped) = dropped;
	*(void **)(&core_trace_get) = get;

	snprintf(g_path, sizeof g_path, "%s", path);
	g_out = fopen(path, "w");
	if (!g_out) {
		failf("cannot write the rng trace %s: %s", path, strerror(errno));
		return -1;
	}
	fprintf(g_out,
	        "# PSIV rolls from UpdateRNGSeed2 (ps4.asm:86097), one row per "
	        "call\n"
	        "# rom instruction: $04239E move.w $8(a5),d0 (the (pc) column is "
	        "the counter the core\n"
	        "# reported for that access; the window accepts $%06X-$%06X)\n"
	        "# roll = (hv + frame_count - seed_high) & $FFFF (the word at "
	        "$FFFFEF0C, which is\n"
	        "# what `sub.w (RNG_Seed).w,d0` at $0423A6 reads and `ror "
	        "(RNG_Seed).w` at $0423AA rotates),\n"
	        "# seed_after = ror16(seed_before's high word)\n"
	        "# frame_count is Main_Frame_Count, seed the RNG_Seed longword "
	        "($FFFFEF0C), both from\n"
	        "# work RAM sampled around the frame; see oracle/README.md "
	        "(\"RNG trace\")\n"
	        "frame,call_index_in_frame,pc,hv,frame_count,seed_before,roll,"
	        "seed_after\n",
	        RNG_HV_PC_FIRST, RNG_HV_PC_LAST);
	if (ferror(g_out)) {
		failf("cannot write the rng trace header to %s", path);
		return -1;
	}
	core_trace_reset();
	core_trace_enable(1);
	return 0;
}

int rng_trace_enabled(void)
{
	return g_out != NULL;
}

int rng_trace_frame(uint64_t frame, uint32_t seed_before,
                    uint32_t frame_count_before, uint32_t seed_after,
                    uint32_t frame_count_after)
{
	unsigned count = core_trace_count();
	unsigned dropped = core_trace_dropped();
	unsigned index;
	unsigned call = 0;
	uint32_t seed;
	int stepped;
	int after_vblank_seen = 0;

	g_records_total += count;
	g_records_unmatched += count; /* matched records leave again below */
	if (dropped) {
		g_records_dropped += dropped;
		core_trace_reset();
	}

	/* Main_Frame_Count and the seed's UpdateRNGSeed are bumped by the same
	 * block of the VBlank handler, so a step of one is what says the frame's
	 * rolls start from a multiplied seed. Anything else is not a frame this
	 * derivation describes. */
	stepped = (frame_count_after == ((frame_count_before + 1u) & 0xFFFFu));
	if (!stepped && frame_count_after != frame_count_before)
		g_frames_count_step++;
	seed = stepped ? update_rng_seed(seed_before) : seed_before;

	for (index = 0; index < count; index++) {
		unsigned pc = 0, hv = 0;
		uint32_t roll, next;
		int after_vblank;

		if (!core_trace_get(index, &pc, &hv))
			continue;
		if (pc < RNG_HV_PC_FIRST || pc > RNG_HV_PC_LAST) {
			bucket_add(g_unmatched_pcs, &g_nunmatched_pcs, pc);
			continue;
		}
		bucket_add(g_matched_pcs, &g_nmatched_pcs, pc);
		g_records_unmatched--;
		g_records_matched++;

		after_vblank = ((hv >> 8) & 0xFFu) >= RNG_HV_VBLANK_LINE;
		if (after_vblank)
			g_calls_after_vblank++;
		else
			g_calls_before_vblank++;
		if (after_vblank_seen && !after_vblank)
			fprintf(stderr, "psiv_oracle: rng trace f%llu: a read below "
			        "the vblank line follows one at or past it\n",
			        (unsigned long long)frame);
		after_vblank_seen |= after_vblank;

		/* The cartridge's own roll: subtract the seed longword's high
		 * word, the word `sub.w (RNG_Seed).w,d0` reads at $FFFFEF0C
		 * (rng_trace.h). */
		roll = rng_trace_roll(seed, hv, frame_count_after);
		next = rotate_seed_high(seed);

		if (fprintf(g_out, "%llu,%u,%06X,%04X,%u,%08X,%04X,%08X\n",
		            (unsigned long long)frame, call, pc & 0xFFFFFFu,
		            hv & 0xFFFFu, frame_count_after & 0xFFFFu, seed, roll,
		            next) < 0) {
			failf("cannot write the rng trace row for frame %llu",
			      (unsigned long long)frame);
			return -1;
		}
		seed = next;
		call++;
		g_rows++;
	}

	if (call) {
		g_frames_with_calls++;
		if (call < g_calls_min || g_calls_min == 0)
			g_calls_min = call;
		if (call > g_calls_max)
			g_calls_max = call;
		if (seed != seed_after) {
			g_frames_not_closed++;
			if (g_frames_not_closed <= MAX_REPORTED_MISMATCHES)
				fprintf(stderr, "psiv_oracle: rng trace f%llu: "
				        "%u call(s) from %08X%s chain to %08X but the "
				        "frame ended at %08X\n",
				        (unsigned long long)frame, call, seed_before,
				        stepped ? " (multiplied)" : "", seed,
				        seed_after);
		}
	}

	core_trace_reset();
	return 0;
}

static void report_pcs(const char *label, const struct pc_bucket *buckets,
                       int count)
{
	int i;

	if (!count)
		return;
	fprintf(stderr, "psiv_oracle: %s", label);
	for (i = 0; i < count; i++)
		fprintf(stderr, " $%06X x%llu", buckets[i].pc,
		        (unsigned long long)buckets[i].count);
	fprintf(stderr, "\n");
}

int rng_trace_finish(void)
{
	int failed = 0;

	if (!g_out)
		return 0;
	if (fclose(g_out) != 0) {
		failf("cannot close %s: %s", g_path, strerror(errno));
		g_out = NULL;
		return -1;
	}
	g_out = NULL;

	/* Appended records that were never drained would describe frames that
	 * already went past, so the count has to come back empty here. */
	if (g_records_total && core_trace_count()) {
		g_records_dropped += core_trace_count();
		core_trace_reset();
	}
	if (g_records_dropped || g_frames_not_closed || g_frames_count_step) {
		fprintf(stderr, "psiv_oracle: rng trace %s is not trustworthy: "
		        "%llu dropped record(s), %llu frame(s) whose calls do not "
		        "explain the frame's seed, %llu frame(s) where "
		        "Main_Frame_Count stepped by neither zero nor one\n", g_path,
		        (unsigned long long)g_records_dropped,
		        (unsigned long long)g_frames_not_closed,
		        (unsigned long long)g_frames_count_step);
		failed = 1;
	}
	fprintf(stderr,
	        "psiv_oracle: rng trace %s: %llu roll(s) in %llu frame(s) "
	        "(min %u, max %u per frame) from %llu HV read(s), %llu matched "
	        "UpdateRNGSeed2, %llu other, %llu dropped\n",
	        g_path, (unsigned long long)g_rows,
	        (unsigned long long)g_frames_with_calls, g_calls_min, g_calls_max,
	        (unsigned long long)g_records_total,
	        (unsigned long long)g_records_matched,
	        (unsigned long long)g_records_unmatched,
	        (unsigned long long)g_records_dropped);
	fprintf(stderr, "psiv_oracle: rng trace rolls in the visible lines / in "
	        "the vblank lines (>= 0x%02X): %llu/%llu\n", RNG_HV_VBLANK_LINE,
	        (unsigned long long)g_calls_before_vblank,
	        (unsigned long long)g_calls_after_vblank);
	report_pcs("rng trace matched PCs:", g_matched_pcs, g_nmatched_pcs);
	report_pcs("rng trace other HV readers:", g_unmatched_pcs,
	           g_nunmatched_pcs);
	if (failed)
		failf("the rng trace did not close; see the messages above");
	return failed ? -1 : 0;
}

const char *rng_trace_error(void)
{
	return g_error[0] ? g_error : "unknown rng trace error";
}
