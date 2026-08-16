#define _GNU_SOURCE

#include "state_dump.h"

#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX_STATE_DUMPS 16
#define STATE_DUMP_PATH_MAX 4096

struct state_dump {
	uint64_t frame;
	char path[STATE_DUMP_PATH_MAX];
};

struct state_region {
	const char *name;
	const char *symbol;
	uint32_t address;
	size_t size;
};

static const struct state_region g_regions[] = {
	{ "plane_a", "Plane_A_Buffer", 0x8000, 0x1000 },
	{ "plane_b", "Plane_B_Buffer", 0x9000, 0x1000 },
	{ "cram", "Palette_Table_Buffer", 0xFB00, 0x0080 },
	{ "sprite_table", "Sprite_Table_Buffer", 0xFC00, 0x0280 },
};

static struct state_dump g_dumps[MAX_STATE_DUMPS];
static int g_ndumps;
static char g_error[256];

static void failf(const char *fmt, ...)
{
	va_list ap;

	va_start(ap, fmt);
	vsnprintf(g_error, sizeof g_error, fmt, ap);
	va_end(ap);
}

int state_dump_add_spec(const char *spec)
{
	const char *colon;
	char *end;
	uint64_t frame;
	size_t path_len;

	if (g_ndumps >= MAX_STATE_DUMPS) {
		failf("--dump-state accepts at most %d dumps", MAX_STATE_DUMPS);
		return -1;
	}
	colon = spec ? strchr(spec, ':') : NULL;
	if (!colon || colon == spec || !colon[1]) {
		failf("--dump-state wants <frame>:<path> (max %d)",
		      MAX_STATE_DUMPS);
		return -1;
	}
	errno = 0;
	frame = strtoull(spec, &end, 10);
	if (errno == ERANGE || end != colon || frame == 0) {
		failf("--dump-state frame is not a positive decimal number: %.*s",
		      (int)(colon - spec), spec);
		return -1;
	}
	path_len = strlen(colon + 1);
	if (path_len >= sizeof g_dumps[0].path) {
		failf("--dump-state path is too long");
		return -1;
	}
	g_dumps[g_ndumps].frame = frame;
	memcpy(g_dumps[g_ndumps].path, colon + 1, path_len + 1);
	g_ndumps++;
	return 0;
}

static uint8_t ram_byte(const uint8_t *ram, uint32_t address)
{
	/* The JSON bytes are in 68000 address order, unlike the core's LSB_FIRST
	 * host array. This is the same accessor used by the CSV RAM logger. */
	return ram[(address & 0xFFFF) ^ 1];
}

static int write_hex(FILE *out, const uint8_t *ram, uint32_t address,
	                     size_t size)
{
	static const char digits[] = "0123456789ABCDEF";
	size_t i;

	for (i = 0; i < size; i++) {
		uint8_t byte = ram_byte(ram, address + (uint32_t)i);
		if (fputc(digits[byte >> 4], out) == EOF ||
		    fputc(digits[byte & 0x0F], out) == EOF)
			return -1;
	}
	return 0;
}

static int write_state(FILE *out, uint64_t frame, const uint8_t *ram)
{
	size_t i;

	if (fprintf(out,
	            "{\n"
	            "  \"format_version\": 1,\n"
	            "  \"kind\": \"psiv_oracle_state\",\n"
	            "  \"frame\": %llu,\n"
	            "  \"address_space\": \"68000_work_ram\",\n"
	            "  \"byte_order\": \"68000_address_order\",\n"
	            "  \"visible_screen\": {\"width_pixels\": 320, "
	            "\"height_pixels\": 224, \"width_cells\": 40, "
	            "\"height_cells\": 28},\n"
	            "  \"regions\": {\n",
	            (unsigned long long)frame) < 0)
		return -1;

	for (i = 0; i < sizeof g_regions / sizeof g_regions[0]; i++) {
		const struct state_region *region = &g_regions[i];
		if (fprintf(out,
		            "    \"%s\": {\"symbol\": \"%s\", "
		            "\"address\": \"0xFFFF%04X\", "
		            "\"ram_offset\": \"0x%04X\", "
		            "\"size_bytes\": %zu, "
		            "\"bytes_hex\": \"",
		            region->name, region->symbol, region->address,
		            region->address, region->size) < 0)
			return -1;
		if (write_hex(out, ram, region->address, region->size) != 0)
			return -1;
		if (fprintf(out, "\"}%s\n",
		            i + 1 == sizeof g_regions / sizeof g_regions[0] ? "" : ",") < 0)
			return -1;
	}
	return fprintf(out, "  }\n}\n") < 0 ? -1 : 0;
}

int state_dump_write(uint64_t frame, const uint8_t *ram)
{
	int i;

	for (i = 0; i < g_ndumps; i++) {
		FILE *out;

		if (g_dumps[i].frame != frame)
			continue;
		out = fopen(g_dumps[i].path, "w");
		if (!out) {
			failf("cannot write %s: %s", g_dumps[i].path, strerror(errno));
			return -1;
		}
		if (write_state(out, frame, ram) != 0) {
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

const char *state_dump_error(void)
{
	return g_error[0] ? g_error : "unknown state dump error";
}
