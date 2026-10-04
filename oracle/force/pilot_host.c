/* Read-only frame hook around the stock oracle host. Python supplies pad blocks;
 * the final evidence is always rerun by the unmodified host on a frozen tape. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static void force_run(void (*run)(void));
#define rt_run() force_run(rt_run)
#include "stock_host.c"
#undef rt_run

static void force_run(void (*run)(void))
{
	static uint64_t frame;
	static unsigned remaining, buttons;
	const char *start = getenv("PSIV_FORCE_START_FRAME");
	uint32_t address;

	if (start && frame >= strtoull(start, NULL, 10)) {
		if (!remaining) {
			printf("%llu ", (unsigned long long)frame);
			for (address = 0; address < 0x10000; address++)
				printf("%02x", ram_u8(address));
			putchar('\n');
			fflush(stdout);
			if (scanf("%u %u", &remaining, &buttons) != 2 || !remaining)
				exit(0);
		}
		g_cur_buttons = buttons;
		remaining--;
	}
	run();
	frame++;
}
