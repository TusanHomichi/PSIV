#ifndef PSIV_ORACLE_CORE_VDP_H
#define PSIV_ORACLE_CORE_VDP_H

#include <stdint.h>

/* Genesis Plus GX keeps these VDP objects local to the core shared object.
 * The host binds them through the ELF symbol table instead of changing the
 * third-party core's libretro ABI. */
struct core_vdp {
	const uint8_t *registers;
	const uint8_t *vram;
	const uint8_t *vsram;
	const uint16_t *hscroll_base;
	const uint8_t *hscroll_mask;
};

int core_vdp_bind(void *core_anchor, struct core_vdp *vdp);
const char *core_vdp_error(void);

#endif
