/* Compile the patch's actual observer against a sentinel CPU state. */
#include <assert.h>
#include <stddef.h>
struct { unsigned cycles, pc, sentinel; } m68k;
#define REG_PC m68k.pc
#include "core_observer.h"

int main(void)
{
    unsigned watches[] = {0x51790, 0x421d4};
    unsigned duplicates[] = {0x51790, 0x51790};
    unsigned odd[] = {0x51791};
    unsigned pc, cycles, i;
    assert(psiv_pc_trace_configure(watches, 2) == 0);
    assert(psiv_pc_trace_configure(duplicates, 2) == -1);
    assert(psiv_pc_trace_configure(odd, 1) == -1);
    assert(psiv_pc_trace_configure(NULL, 1) == -1);
    m68k.cycles = 123;
    m68k.pc = 0x51790;
    m68k.sentinel = 0x89abcdef;
    /* Same gate used at both instruction dispatch sites. */
    if (psiv_pc_trace_on) psiv_pc_trace_instruction();
    assert(psiv_pc_trace_count() == 0);
    psiv_pc_trace_enable(1);
    psiv_pc_trace_instruction();
    assert(psiv_pc_trace_get(0, &pc, &cycles) == 1);
    assert(pc == 0x51790 && cycles == 123);
    m68k.pc = 0x42;
    psiv_pc_trace_instruction();
    assert(psiv_pc_trace_count() == 1);
    m68k.pc = 0x421d4;
    m68k.cycles = 456;
    for (i = 0; i < PSIV_PC_TRACE_CAPACITY; ++i) psiv_pc_trace_instruction();
    assert(psiv_pc_trace_count() == PSIV_PC_TRACE_CAPACITY);
    assert(psiv_pc_trace_dropped() == 1);
    assert(psiv_pc_trace_get(PSIV_PC_TRACE_CAPACITY, &pc, &cycles) == 0);
    assert(m68k.pc == 0x421d4 && m68k.cycles == 456 && m68k.sentinel == 0x89abcdef);
    psiv_pc_trace_reset();
    assert(psiv_pc_trace_count() == 0 && psiv_pc_trace_dropped() == 0);
    return 0;
}
