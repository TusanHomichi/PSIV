#ifndef PSIV_ORACLE_CORE_OPTIONS_H
#define PSIV_ORACLE_CORE_OPTIONS_H

#include <stdio.h>

#include "libretro.h"

/* The core's option set, and the proof that what this host pins is in it
 * (core_options.c). The host has no say in what the core declares; it only
 * decides which of those values it hands back, and refuses to start when a pin
 * is not one the core announced. */

/* The value pinned for `key`, or NULL to let the core's compiled-in default
 * stand - what `RETRO_ENVIRONMENT_GET_VARIABLE` answers with. */
const char *core_options_pinned_value(const char *key);

/* Whether the declared set is printed as the core announces it
 * (`--dump-options`). Set before `retro_set_environment`, which is what fires
 * the declaration. */
void core_options_dump(int enabled);

/* Capture what the core announced, one call per
 * `RETRO_ENVIRONMENT_SET_CORE_OPTIONS*` variant. */
void core_options_ingest_v2(const struct retro_core_option_v2_definition *definitions);
void core_options_ingest_v1(const struct retro_core_option_definition *definitions);

/* Prove every pinned value is one the core declared: -1, after naming the pins
 * it could not find, when it is not. */
int core_options_validate(void);

/* The `RETRO_ENVIRONMENT_GET_LOG_INTERFACE` callback: warnings and errors the
 * core logs go to stderr, below them nothing (core chatter would pollute the
 * deterministic stderr the determinism check compares). */
void core_options_log(enum retro_log_level level, const char *fmt, ...);

#endif
