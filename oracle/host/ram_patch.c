#define _GNU_SOURCE

#include "ram_patch.h"

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX_RAM_PATCHES 128
#define MAX_RAM_PATCH_BYTES 64

struct ram_patch {
	uint64_t frame;
	uint32_t address;
	uint8_t bytes[MAX_RAM_PATCH_BYTES];
	size_t count;
	int applied;
};

static struct ram_patch g_patches[MAX_RAM_PATCHES];
static int g_npatches;
static char g_error[256];

static void failf(const char *fmt, ...)
{
	va_list ap;

	va_start(ap, fmt);
	vsnprintf(g_error, sizeof g_error, fmt, ap);
	va_end(ap);
}

static int hex_digit(char c)
{
	if (c >= '0' && c <= '9') return c - '0';
	if (c >= 'a' && c <= 'f') return c - 'a' + 10;
	if (c >= 'A' && c <= 'F') return c - 'A' + 10;
	return -1;
}

int ram_patch_add_spec(const char *spec)
{
	char *copy, *frame_text, *address_text, *bytes_text, *save;
	struct ram_patch *patch;
	char *end;
	size_t length, i;
	unsigned long long frame;
	unsigned long address;

	if (g_npatches >= MAX_RAM_PATCHES) {
		failf("too many --ram-patch options (max %d)", MAX_RAM_PATCHES);
		return -1;
	}
	copy = strdup(spec);
	if (!copy) {
		failf("out of memory parsing --ram-patch");
		return -1;
	}
	frame_text = strtok_r(copy, ":", &save);
	address_text = strtok_r(NULL, ":", &save);
	bytes_text = strtok_r(NULL, ":", &save);
	if (!frame_text || !address_text || !bytes_text || strtok_r(NULL, ":", &save)) {
		failf("--ram-patch wants <frame>:<68000-address>:<hex-bytes>");
		free(copy);
		return -1;
	}
	frame = strtoull(frame_text, &end, 10);
	if (*frame_text == 0 || *end != 0 || frame == 0) {
		failf("--ram-patch frame is not a positive decimal number: %s", frame_text);
		free(copy);
		return -1;
	}
	if (address_text[0] == '$') address_text++;
	else if (address_text[0] == '0' &&
	         (address_text[1] == 'x' || address_text[1] == 'X')) address_text += 2;
	address = strtoul(address_text, &end, 16);
	if (*address_text == 0 || *end != 0 || address > 0xFFFFFFFFUL) {
		failf("--ram-patch address is not hexadecimal: %s", address_text);
		free(copy);
		return -1;
	}
	length = strlen(bytes_text);
	if (length == 0 || (length & 1) != 0 || length / 2 > MAX_RAM_PATCH_BYTES) {
		failf("--ram-patch hex payload must be 1..%d bytes and even-length",
		      MAX_RAM_PATCH_BYTES);
		free(copy);
		return -1;
	}
	patch = &g_patches[g_npatches];
	memset(patch, 0, sizeof *patch);
	patch->frame = (uint64_t)frame;
	patch->address = (uint32_t)address;
	patch->count = length / 2;
	for (i = 0; i < patch->count; i++) {
		int high = hex_digit(bytes_text[i * 2]);
		int low = hex_digit(bytes_text[i * 2 + 1]);
		if (high < 0 || low < 0) {
			failf("--ram-patch payload is not hexadecimal: %s", bytes_text);
			free(copy);
			return -1;
		}
		patch->bytes[i] = (uint8_t)((high << 4) | low);
	}
	g_npatches++;
	free(copy);
	return 0;
}

int ram_patch_validate_total(uint64_t total_frames)
{
	int i;

	for (i = 0; i < g_npatches; i++) {
		if (g_patches[i].address < 0xFFFF0000U) {
			failf("--ram-patch address %08X is outside 68000 work RAM",
			      g_patches[i].address);
			return -1;
		}
		if (g_patches[i].frame > total_frames) {
			failf("--ram-patch frame %llu is past tape end %llu",
			      (unsigned long long)g_patches[i].frame,
			      (unsigned long long)total_frames);
			return -1;
		}
		if ((uint64_t)(g_patches[i].address & 0xFFFF) + g_patches[i].count > 0x10000) {
			failf("--ram-patch at %08X crosses the 64KB work-RAM boundary",
			      g_patches[i].address);
			return -1;
		}
	}
	return 0;
}

void ram_patch_apply(uint64_t frame, uint8_t *ram, size_t ram_size)
{
	int i;

	for (i = 0; i < g_npatches; i++) {
		struct ram_patch *patch = &g_patches[i];
		size_t j;
		uint32_t address;

		if (patch->frame != frame) continue;
		address = patch->address & 0xFFFF;
		for (j = 0; j < patch->count; j++) {
			/* The core exposes little-endian host words, while the spec is
			 * written in the retail CPU's byte-address order. */
			ram[(address + j) ^ 1] = patch->bytes[j];
		}
		patch->applied = 1;
	}
	(void)ram_size;
}

int ram_patch_finish(void)
{
	int i;

	for (i = 0; i < g_npatches; i++) {
		if (!g_patches[i].applied) {
			failf("--ram-patch frame %llu was not applied",
			      (unsigned long long)g_patches[i].frame);
			return -1;
		}
	}
	return 0;
}

int ram_patch_count(void) { return g_npatches; }

const char *ram_patch_error(void)
{
	return g_error[0] ? g_error : "unknown RAM patch error";
}
