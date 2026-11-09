/* Deterministic core-hook double for the reload timing trace transport. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <string.h>
#include "pc_trace.h"

static unsigned records;
static int enabled;
static const char *mode;
static unsigned configured_pc;

int psiv_pc_trace_configure(const unsigned *pcs, unsigned count)
{
    if (!count || count > 64) return -1;
    configured_pc = pcs[0];
    return 0;
}
void psiv_pc_trace_enable(int on) { enabled = on; }
void psiv_pc_trace_reset(void) { records = 0; }
unsigned psiv_pc_trace_count(void) { return records; }
unsigned psiv_pc_trace_dropped(void)
{
    return !strcmp(mode, "drop") && records ? 1 : 0;
}
int psiv_pc_trace_get(unsigned index, unsigned *pc, unsigned *cycles)
{
    if (!strcmp(mode, "badget") || index >= records) return 0;
    *pc = configured_pc;
    *cycles = 100 + index * 100;
    return 1;
}

static int failure(void)
{
    fprintf(stderr, "%s\n", pc_trace_error());
    pc_trace_finish();
    return 2;
}

int main(int argc, char **argv)
{
    void *core;
    const char *path;
    if (argc != 5) return 99;
    mode = argv[1];
    path = strcmp(argv[2], "-") ? argv[2] : NULL;
    if (strcmp(argv[3], "-") && pc_trace_add_watch(argv[3]) != 0) return failure();
    if (strcmp(argv[4], "-") && pc_trace_set_from(argv[4]) != 0) return failure();
    if (pc_trace_validate(path) != 0) return failure();
    if (pc_trace_validate_total(!strcmp(mode, "short") ? 2 : 10) != 0) return failure();
    if (!strcmp(mode, "config")) return 0;
    core = dlopen(!strcmp(mode, "missing") ? "libc.so.6" : NULL, RTLD_NOW);
    if (!core) return 98;
    if (pc_trace_begin(core, path) != 0) return failure();
    pc_trace_prepare_frame(1);
    if (enabled) { fprintf(stderr, "enabled before requested frame\n"); return 97; }
    if (pc_trace_frame(1) != 0) return failure();
    pc_trace_prepare_frame(3);
    if (!enabled) { fprintf(stderr, "not enabled at requested frame\n"); return 96; }
    records = 2;
    if (pc_trace_frame(3) != 0) return failure();
    if (pc_trace_finish() != 0) return failure();
    dlclose(core);
    return 0;
}
