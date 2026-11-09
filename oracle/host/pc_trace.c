#define _GNU_SOURCE
#include "pc_trace.h"

#include <dlfcn.h>
#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX_PC_WATCHES 64

static unsigned g_watches[MAX_PC_WATCHES];
static unsigned g_nwatches;
static uint64_t g_from = 1;
static int g_from_set;
static FILE *g_out;
static int g_active;
static uint64_t g_rows;
static char g_error[256];

static int (*core_configure)(const unsigned *, unsigned);
static void (*core_enable)(int);
static void (*core_reset)(void);
static unsigned (*core_count)(void);
static unsigned (*core_dropped)(void);
static int (*core_get)(unsigned, unsigned *, unsigned *);

static void failf(const char *fmt, ...)
{
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(g_error, sizeof g_error, fmt, ap);
	va_end(ap);
}

int pc_trace_add_watch(const char *hex)
{
	const char *p;
	char *end;
	unsigned long value;
	unsigned i;

	if (!hex || !*hex) {
		failf("--pc-watch requires an even 24-bit hexadecimal PC");
		return -1;
	}
	p = hex;
	if (p[0] == '0' && (p[1] == 'x' || p[1] == 'X'))
		p += 2;
	if (!*p || strspn(p, "0123456789abcdefABCDEF") != strlen(p)) {
		failf("invalid --pc-watch hexadecimal PC: %s", hex);
		return -1;
	}
	errno = 0;
	value = strtoul(p, &end, 16);
	if (errno || *end || value > 0xFFFFFFul || (value & 1ul)) {
		failf("--pc-watch PC must be even and at most FFFFFE: %s", hex);
		return -1;
	}
	for (i = 0; i < g_nwatches; i++)
		if (g_watches[i] == value) {
			failf("duplicate --pc-watch PC: %s", hex);
			return -1;
		}
	if (g_nwatches == MAX_PC_WATCHES) {
		failf("at most %u --pc-watch PCs are supported", MAX_PC_WATCHES);
		return -1;
	}
	g_watches[g_nwatches++] = (unsigned)value;
	return 0;
}

int pc_trace_set_from(const char *decimal)
{
	char *end;
	unsigned long long value;

	if (!decimal || !*decimal ||
	    strspn(decimal, "0123456789") != strlen(decimal)) {
		failf("--pc-trace-from requires a positive decimal tape frame");
		return -1;
	}
	errno = 0;
	value = strtoull(decimal, &end, 10);
	if (errno || *end || value == 0 || value > UINT64_MAX) {
		failf("invalid --pc-trace-from frame: %s", decimal);
		return -1;
	}
	g_from = (uint64_t)value;
	g_from_set = 1;
	return 0;
}

int pc_trace_validate(const char *path)
{
	if ((!path || !*path) && (path || g_nwatches || g_from_set)) {
		failf("--pc-watch and --pc-trace-from require --pc-trace <path.csv>");
		return -1;
	}
	if (path && !g_nwatches) {
		failf("--pc-trace requires at least one --pc-watch PC");
		return -1;
	}
	return 0;
}

int pc_trace_validate_total(uint64_t total)
{
	if (g_nwatches && g_from > total) {
		failf("--pc-trace-from frame %llu is past tape end %llu",
		      (unsigned long long)g_from, (unsigned long long)total);
		return -1;
	}
	return 0;
}

static void resolve(void *core, const char *name, void **target)
{
	*target = dlsym(core, name);
	if (!*target && !g_error[0])
		failf("the core has no %s; rebuild it with oracle/build_core.sh "
		      "so oracle/patches reaches the build", name);
}

int pc_trace_begin(void *core, const char *path)
{
	void *configure, *enable, *reset, *count, *dropped, *get;

	if (pc_trace_validate(path) != 0)
		return -1;
	if (!core || !path) {
		failf("pc trace needs the core handle and a path");
		return -1;
	}
	resolve(core, "psiv_pc_trace_configure", &configure);
	resolve(core, "psiv_pc_trace_enable", &enable);
	resolve(core, "psiv_pc_trace_reset", &reset);
	resolve(core, "psiv_pc_trace_count", &count);
	resolve(core, "psiv_pc_trace_dropped", &dropped);
	resolve(core, "psiv_pc_trace_get", &get);
	if (g_error[0])
		return -1;
	*(void **)(&core_configure) = configure;
	*(void **)(&core_enable) = enable;
	*(void **)(&core_reset) = reset;
	*(void **)(&core_count) = count;
	*(void **)(&core_dropped) = dropped;
	*(void **)(&core_get) = get;
	core_enable(0);
	core_reset();
	if (core_configure(g_watches, g_nwatches) != 0) {
		failf("the core rejected the pc trace watch configuration");
		return -1;
	}
	g_out = fopen(path, "w");
	if (!g_out) {
		failf("cannot write the pc trace %s: %s", path, strerror(errno));
		return -1;
	}
	if (fprintf(g_out, "# instruction boundaries before opcode fetch; cycles "
	                  "are frame-relative master cycles\nframe,pc,cycles\n") < 0) {
		failf("cannot write the pc trace header");
		fclose(g_out);
		g_out = NULL;
		return -1;
	}
	return 0;
}

int pc_trace_enabled(void)
{
	return g_out != NULL;
}

void pc_trace_prepare_frame(uint64_t frame)
{
	if (g_out && !g_active && frame >= g_from) {
		core_reset();
		core_enable(1);
		g_active = 1;
	}
}

int pc_trace_frame(uint64_t frame)
{
	unsigned index, count;

	if (!g_out || !g_active)
		return 0;
	if (core_dropped()) {
		failf("pc trace dropped %u record(s) in frame %llu",
		      core_dropped(), (unsigned long long)frame);
		return -1;
	}
	count = core_count();
	for (index = 0; index < count; index++) {
		unsigned pc, cycles, watch;
		if (!core_get(index, &pc, &cycles)) {
			failf("cannot read pc trace record %u in frame %llu", index,
			      (unsigned long long)frame);
			return -1;
		}
		for (watch = 0; watch < g_nwatches; watch++)
			if (pc == g_watches[watch])
				break;
		if (watch == g_nwatches) {
			failf("pc trace returned an unwatched PC %08X", pc);
			return -1;
		}
		if (fprintf(g_out, "%llu,%06X,%u\n", (unsigned long long)frame,
		            pc, cycles) < 0) {
			failf("cannot write the pc trace row for frame %llu",
			      (unsigned long long)frame);
			return -1;
		}
		g_rows++;
	}
	core_reset();
	return 0;
}

int pc_trace_finish(void)
{
	int failed = g_error[0] != 0;

	if (!g_out)
		return failed ? -1 : 0;
	core_enable(0);
	g_active = 0;
	if (core_count() || core_dropped()) {
		failf("pc trace finished with undrained or dropped records");
		failed = 1;
	}
	core_reset();
	if (fclose(g_out) != 0) {
		failf("cannot close the pc trace: %s", strerror(errno));
		failed = 1;
	}
	g_out = NULL;
	fprintf(stderr, "psiv_oracle: pc trace: %llu instruction boundary record(s)\n",
	        (unsigned long long)g_rows);
	return failed ? -1 : 0;
}

const char *pc_trace_error(void)
{
	return g_error[0] ? g_error : "unknown pc trace error";
}
