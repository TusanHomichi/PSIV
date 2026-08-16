#ifndef PSIV_ORACLE_RAM_DUMP_H
#define PSIV_ORACLE_RAM_DUMP_H

#include <stdint.h>

int ram_dump_add_spec(const char *spec);
int ram_dump_write(uint64_t frame, const uint8_t *ram);
const char *ram_dump_error(void);

#endif
