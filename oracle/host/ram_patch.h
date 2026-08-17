#ifndef PSIV_ORACLE_RAM_PATCH_H
#define PSIV_ORACLE_RAM_PATCH_H

#include <stddef.h>
#include <stdint.h>

/* A patch writes bytes in 68000 address order at the start of one emulated
 * frame. It is an oracle-fixture tool, not a general cheat interface: every
 * requested frame must be reached and every address must be inside work RAM. */
int ram_patch_add_spec(const char *spec);
int ram_patch_validate_total(uint64_t total_frames);
void ram_patch_apply(uint64_t frame, uint8_t *ram, size_t ram_size);
int ram_patch_finish(void);
int ram_patch_count(void);
const char *ram_patch_error(void);

#endif
