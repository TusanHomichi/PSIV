#ifndef PSIV_ORACLE_TAPE_H
#define PSIV_ORACLE_TAPE_H

#include <stdint.h>

/* Genesis pad bits, in the order used by the tape's button letters. */
enum {
	PAD_UP = 1 << 0,
	PAD_DOWN = 1 << 1,
	PAD_LEFT = 1 << 2,
	PAD_RIGHT = 1 << 3,
	PAD_A = 1 << 4,
	PAD_B = 1 << 5,
	PAD_C = 1 << 6,
	PAD_START = 1 << 7
};

#define PSIV_TAPE_MAX_MARK_LEN 48

struct psiv_tape_step {
	uint32_t frames;
	uint32_t buttons;
	char mark[PSIV_TAPE_MAX_MARK_LEN];
};

int psiv_tape_load(const char *path);
int psiv_tape_count(void);
uint64_t psiv_tape_total_frames(void);
const struct psiv_tape_step *psiv_tape_step_at(int index);
const char *psiv_tape_error(void);

#endif
