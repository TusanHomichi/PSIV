#ifndef PSIV_ORACLE_STATE_DUMP_H
#define PSIV_ORACLE_STATE_DUMP_H

#include <stdint.h>

/* Each spec is <frame>:<path>; the path is a self-contained JSON state file. */
int state_dump_add_spec(const char *spec);
int state_dump_write(uint64_t frame, const uint8_t *ram);
const char *state_dump_error(void);

#endif
