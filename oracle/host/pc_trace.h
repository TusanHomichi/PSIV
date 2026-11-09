#ifndef PSIV_ORACLE_PC_TRACE_H
#define PSIV_ORACLE_PC_TRACE_H

#include <stdint.h>

/* Optional instruction-boundary observations. Watch addresses are distinct,
 * even 24-bit 68000 PCs; the start frame is a positive decimal tape frame. */
int pc_trace_add_watch(const char *hex);
int pc_trace_set_from(const char *decimal);
int pc_trace_validate(const char *path);
int pc_trace_validate_total(uint64_t total);

/* Bind the patched core and open frame,pc,cycles CSV. cycles is the core's
 * frame-relative master-cycle counter immediately before opcode fetch, not
 * elapsed host time or a count of 68000 clock ticks. No emulated state is
 * changed. Tracing stays disabled until prepare_frame reaches the start. */
int pc_trace_begin(void *core, const char *path);
int pc_trace_enabled(void);
void pc_trace_prepare_frame(uint64_t frame);
int pc_trace_frame(uint64_t frame);
int pc_trace_finish(void);
const char *pc_trace_error(void);

#endif
