#define _GNU_SOURCE

#include "ram_dump.h"

#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX_RAM_DUMPS 8
#define RAM_DUMP_PATH_MAX 4096

struct ram_dump {
	uint64_t frame;
	char path[RAM_DUMP_PATH_MAX];
};

static struct ram_dump g_dumps[MAX_RAM_DUMPS];
static int g_ndumps;
static char g_error[256];

static void failf(const char *fmt, ...)
{
	va_list ap;

	va_start(ap, fmt);
	vsnprintf(g_error, sizeof g_error, fmt, ap);
	va_end(ap);
}

int ram_dump_add_spec(const char *spec)
{
	const char *colon;
	char *end;
	uint64_t frame;
	size_t path_len;

	if (g_ndumps >= MAX_RAM_DUMPS) {
		failf("--dump-ram accepts at most %d dumps", MAX_RAM_DUMPS);
		return -1;
	}
	colon = spec ? strchr(spec, ':') : NULL;
	if (!colon || colon == spec || !colon[1]) {
		failf("--dump-ram wants <frame>:<path> (max %d)", MAX_RAM_DUMPS);
		return -1;
	}
	errno = 0;
	frame = strtoull(spec, &end, 10);
	if (errno == ERANGE || end != colon || frame == 0) {
		failf("--dump-ram frame is not a positive decimal number: %.*s",
		      (int)(colon - spec), spec);
		return -1;
	}
	path_len = strlen(colon + 1);
	if (path_len >= sizeof g_dumps[0].path) {
		failf("--dump-ram path is too long");
		return -1;
	}
	g_dumps[g_ndumps].frame = frame;
	memcpy(g_dumps[g_ndumps].path, colon + 1, path_len + 1);
	g_ndumps++;
	return 0;
}

int ram_dump_write(uint64_t frame, const uint8_t *ram)
{
	int i;

	for (i = 0; i < g_ndumps; i++) {
		FILE *out;
		uint32_t address;

		if (g_dumps[i].frame != frame)
			continue;
		out = fopen(g_dumps[i].path, "wb");
		if (!out) {
			failf("cannot write %s: %s", g_dumps[i].path, strerror(errno));
			return -1;
		}
		for (address = 0; address < 0x10000; address++)
			if (fputc(ram[address ^ 1], out) == EOF) {
				fclose(out);
				failf("short write while writing %s", g_dumps[i].path);
				return -1;
			}
		if (fclose(out) != 0) {
			failf("short write while closing %s", g_dumps[i].path);
			return -1;
		}
	}
	return 0;
}

const char *ram_dump_error(void)
{
	return g_error[0] ? g_error : "unknown RAM dump error";
}
