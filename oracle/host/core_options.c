/* The core's option set, and the proof that what this host pins is in it.
 *
 * Split out of psiv_oracle.c (issue #61) so the runner stays inside the
 * repository's file-size limit; nothing here touches emulated state, and the
 * pinned values below are the only decision it makes on its own.
 *
 * Genesis Plus GX does not report an unrecognised option value: it runs atoi()
 * or a strcmp chain and falls through to whatever that yields, which is how
 * "1x" for the overclock option quietly clocked the 68000 at 1% and produced a
 * log full of zeroes. An oracle cannot afford silent misconfiguration, so the
 * declared set is captured as the core announces it and every pinned value is
 * checked against it before the first frame.
 */
#include "core_options.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

/* Options we pin explicitly. Anything not listed here falls through to the
 * core's compiled-in default, which is deterministic for a fixed core build;
 * --dump-options prints the full declared set with defaults so the pinned list
 * can be audited against a new core revision. */
struct pinned_opt {
	const char *key;
	const char *value;
};

static const struct pinned_opt g_pinned[] = {
	/* Hardware and timing: these decide instruction and frame cadence. */
	{ "genesis_plus_gx_system_hw", "mega drive / genesis" },
	{ "genesis_plus_gx_region_detect", "ntsc-u" },
	{ "genesis_plus_gx_vdp_mode", "60hz" },
	/* Must be "100", not "100%": the core does atoi() on this value, so a
	 * label-shaped or otherwise unparseable string silently becomes a tiny
	 * clock divisor instead of an error. validate() below exists because of
	 * exactly this failure mode. */
	{ "genesis_plus_gx_overclock", "100" },
	{ "genesis_plus_gx_force_dtack", "enabled" },
	{ "genesis_plus_gx_addr_error", "enabled" },
	{ "genesis_plus_gx_lock_on", "disabled" },
	{ "genesis_plus_gx_add_on", "none" },
	/* Sprite limit is not cosmetic: it drives VDP status bits the 68000 can
	 * read, so the accurate (limited) behaviour is required. */
	{ "genesis_plus_gx_no_sprite_limit", "disabled" },
	/* Rendering stays on and accurate. With no frame flags the video callback's
	 * pixels are discarded, but VDP state feeds status registers, so we do not
	 * skip work the hardware does. */
	{ "genesis_plus_gx_render", "single field" },
	{ "genesis_plus_gx_frameskip", "disabled" },
	{ "genesis_plus_gx_overscan", "disabled" },
	{ NULL, NULL }
};

/* Declared-option index, captured when the core announces its options, so we
 * can prove every pinned value is one the core actually accepts. */
#define MAX_DECLARED_OPTS 128
#define MAX_OPT_VALUES 64

struct declared_opt {
	const char *key;
	const char *values[MAX_OPT_VALUES];
	int nvalues;
};

static struct declared_opt g_declared[MAX_DECLARED_OPTS];
static int g_ndeclared;
static int g_dump_declared;

const char *core_options_pinned_value(const char *key)
{
	const struct pinned_opt *p;

	for (p = g_pinned; p->key; p++)
		if (strcmp(p->key, key) == 0)
			return p->value;
	return NULL;
}

void core_options_dump(int enabled)
{
	g_dump_declared = enabled;
}

static void record_declared(const char *key,
                            const struct retro_core_option_value *values)
{
	struct declared_opt *o;
	if (g_ndeclared >= MAX_DECLARED_OPTS)
		return;
	o = &g_declared[g_ndeclared++];
	o->key = key;
	o->nvalues = 0;
	for (; values && values->value && o->nvalues < MAX_OPT_VALUES; values++)
		o->values[o->nvalues++] = values->value;
}

static void announce(const char *key,
                     const struct retro_core_option_value *values,
                     const char *default_value)
{
	record_declared(key, values);
	if (g_dump_declared)
		printf("option\t%s\t%s\n", key,
		       default_value ? default_value : "(none)");
}

int core_options_validate(void)
{
	const struct pinned_opt *p;
	int bad = 0;

	if (g_ndeclared == 0) {
		fprintf(stderr, "psiv_oracle: core declared no options; cannot "
		        "validate the pinned set\n");
		return -1;
	}

	for (p = g_pinned; p->key; p++) {
		const struct declared_opt *found = NULL;
		int i, ok = 0;

		for (i = 0; i < g_ndeclared; i++) {
			if (strcmp(g_declared[i].key, p->key) == 0) {
				found = &g_declared[i];
				break;
			}
		}
		if (!found) {
			fprintf(stderr, "psiv_oracle: pinned option '%s' is not declared "
			        "by this core build\n", p->key);
			bad = 1;
			continue;
		}
		for (i = 0; i < found->nvalues; i++)
			if (strcmp(found->values[i], p->value) == 0) {
				ok = 1;
				break;
			}
		if (!ok) {
			fprintf(stderr, "psiv_oracle: pinned option '%s' value '%s' is not "
			        "one of the core's accepted values:", p->key, p->value);
			for (i = 0; i < found->nvalues; i++)
				fprintf(stderr, " '%s'", found->values[i]);
			fprintf(stderr, "\n");
			bad = 1;
		}
	}
	return bad ? -1 : 0;
}

void core_options_log(enum retro_log_level level, const char *fmt, ...)
{
	va_list ap;
	if (level < RETRO_LOG_WARN)
		return; /* core chatter would pollute stderr determinism checks */
	va_start(ap, fmt);
	vfprintf(stderr, fmt, ap);
	va_end(ap);
}

void core_options_ingest_v2(const struct retro_core_option_v2_definition *definitions)
{
	for (; definitions && definitions->key; definitions++)
		announce(definitions->key, definitions->values,
		         definitions->default_value);
}

void core_options_ingest_v1(const struct retro_core_option_definition *definitions)
{
	for (; definitions && definitions->key; definitions++)
		announce(definitions->key, definitions->values,
		         definitions->default_value);
}
