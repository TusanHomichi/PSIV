#define _GNU_SOURCE

#include "tape.h"

#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX_TAPE_STEPS 262144

static struct psiv_tape_step g_tape[MAX_TAPE_STEPS];
static int g_ntape;
static uint64_t g_total_frames;
static char g_error[256];

static void failf(const char *fmt, ...)
{
	va_list ap;

	va_start(ap, fmt);
	vsnprintf(g_error, sizeof g_error, fmt, ap);
	va_end(ap);
}

static char *trim_tape(char *s)
{
	char *e;

	while (*s == ' ' || *s == '\t' || *s == '\r' || *s == '\n')
		s++;
	e = s + strlen(s);
	while (e > s && (e[-1] == ' ' || e[-1] == '\t' || e[-1] == '\r' ||
	                 e[-1] == '\n'))
		*--e = 0;
	return s;
}

static int parse_buttons(const char *s, uint32_t *out)
{
	uint32_t b = 0;

	if (strcmp(s, ".") == 0) {
		*out = 0;
		return 0;
	}
	for (; *s; s++) {
		switch (*s) {
		case 'U': b |= PAD_UP; break;
		case 'D': b |= PAD_DOWN; break;
		case 'L': b |= PAD_LEFT; break;
		case 'R': b |= PAD_RIGHT; break;
		case 'A': b |= PAD_A; break;
		case 'B': b |= PAD_B; break;
		case 'C': b |= PAD_C; break;
		case 'S': b |= PAD_START; break;
		default: return -1;
		}
	}
	*out = b;
	return 0;
}

int psiv_tape_load(const char *path)
{
	FILE *f = fopen(path, "r");
	char line[512];
	int lineno = 0;
	int repeat_start = -1;
	uint32_t repeat_count = 0;

	g_ntape = 0;
	g_total_frames = 0;
	g_error[0] = 0;
	if (!f) {
		failf("cannot open tape %s: %s", path, strerror(errno));
		return -1;
	}

	while (fgets(line, sizeof line, f)) {
		char *p = trim_tape(line), *tok, *save;
		struct psiv_tape_step *st;
		lineno++;
		if (!*p || *p == '#')
			continue;

		if (strncmp(p, "repeat", 6) == 0 &&
		    (p[6] == ' ' || p[6] == '\t')) {
			if (repeat_start >= 0) {
				failf("tape line %d: repeat blocks do not nest", lineno);
				fclose(f);
				return -1;
			}
			repeat_count = (uint32_t)strtoul(p + 7, NULL, 10);
			if (repeat_count == 0) {
				failf("tape line %d: repeat count must be >= 1", lineno);
				fclose(f);
				return -1;
			}
			repeat_start = g_ntape;
			continue;
		}

		if (strcmp(p, "end") == 0) {
			int block_len = g_ntape - repeat_start;
			uint32_t rep;
			int j;

			if (repeat_start < 0) {
				failf("tape line %d: 'end' without 'repeat'", lineno);
				fclose(f);
				return -1;
			}
			if ((uint64_t)repeat_start + (uint64_t)block_len * repeat_count >
			    (uint64_t)MAX_TAPE_STEPS) {
				failf("tape line %d: repeat expands past the %d step limit",
				      lineno, MAX_TAPE_STEPS);
				fclose(f);
				return -1;
			}
			for (rep = 1; rep < repeat_count; rep++)
				for (j = 0; j < block_len; j++) {
					g_tape[g_ntape] = g_tape[repeat_start + j];
					g_total_frames += g_tape[g_ntape].frames;
					g_ntape++;
				}
			repeat_start = -1;
			continue;
		}

		if (g_ntape >= MAX_TAPE_STEPS) {
			failf("tape too long (max %d steps)", MAX_TAPE_STEPS);
			fclose(f);
			return -1;
		}
		st = &g_tape[g_ntape];
		memset(st, 0, sizeof *st);

		tok = strtok_r(p, " \t", &save);
		if (!tok)
			continue;
		st->frames = (uint32_t)strtoul(tok, NULL, 10);
		if (st->frames == 0) {
			failf("tape line %d: frame count must be >= 1", lineno);
			fclose(f);
			return -1;
		}

		tok = strtok_r(NULL, " \t", &save);
		if (!tok || parse_buttons(tok, &st->buttons) != 0) {
			failf("tape line %d: bad button set", lineno);
			fclose(f);
			return -1;
		}

		tok = strtok_r(NULL, " \t", &save);
		if (tok)
			snprintf(st->mark, sizeof st->mark, "%s", trim_tape(tok));

		g_total_frames += st->frames;
		g_ntape++;
	}
	if (repeat_start >= 0) {
		failf("tape ended inside an unterminated 'repeat' block");
		fclose(f);
		return -1;
	}
	fclose(f);
	return 0;
}

int psiv_tape_count(void)
{
	return g_ntape;
}

uint64_t psiv_tape_total_frames(void)
{
	return g_total_frames;
}

const struct psiv_tape_step *psiv_tape_step_at(int index)
{
	if (index < 0 || index >= g_ntape)
		return NULL;
	return &g_tape[index];
}

const char *psiv_tape_error(void)
{
	return g_error[0] ? g_error : "unknown tape error";
}
