/* psiv_oracle - headless deterministic Genesis behaviour oracle for PSIV.
 *
 * Hosts the Genesis Plus GX libretro core with no window, no audio device and
 * no wall clock: a tape of per-frame button states goes in, a CSV of named
 * 68000 work-RAM values comes out. Because nothing in the loop observes real
 * time, two runs of the same tape against the same core produce byte-identical
 * logs by construction rather than by careful configuration.
 *
 * Work-RAM byte order: Genesis Plus GX is built with -DLSB_FIRST, which stores
 * work_ram in host-native 16-bit word order. The 68000 core reads bytes as
 * READ_BYTE(base, addr) == base[addr^1] and words as *(uint16*)(base + addr)
 * (gpgx-src/core/macros.h, gpgx-src/core/m68k/m68kcpu.h:854-882). So a 68000
 * byte at address A lives at ram[A^1], and a word at even A is a plain
 * little-endian load. Getting this backwards silently corrupts every log line,
 * so oracle/verify.sh asserts it against a known value at runtime.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "frame_dump.h"
#include "libretro.h"
#include "ram_patch.h"
#include "ram_dump.h"
#include "state_dump.h"
#include "tape.h"

#define MAX_FIELDS 1024

/* ------------------------------------------------------------------ */
/* RAM map                                                            */
/* ------------------------------------------------------------------ */

struct field {
	char name[64];
	char group[32];
	uint32_t addr;   /* 68000 address, e.g. 0xFFFFEC28 */
	int size;        /* 1, 2 or 4 bytes */
	int is_signed;
	int hex;         /* render as hex in the log */
	int enabled;
};

static struct field g_fields[MAX_FIELDS];
static int g_nfields;

/* ------------------------------------------------------------------ */
/* Tape                                                               */
/* ------------------------------------------------------------------ */

/* ------------------------------------------------------------------ */
/* Core handle and callbacks                                          */
/* ------------------------------------------------------------------ */

static void *g_core;
static void (*rt_init)(void);
static void (*rt_deinit)(void);
static unsigned (*rt_api_version)(void);
static void (*rt_get_system_info)(struct retro_system_info *);
static void (*rt_get_system_av_info)(struct retro_system_av_info *);
static void (*rt_set_environment)(retro_environment_t);
static void (*rt_set_video_refresh)(retro_video_refresh_t);
static void (*rt_set_audio_sample)(retro_audio_sample_t);
static void (*rt_set_audio_sample_batch)(retro_audio_sample_batch_t);
static void (*rt_set_input_poll)(retro_input_poll_t);
static void (*rt_set_input_state)(retro_input_state_t);
static void (*rt_set_controller_port_device)(unsigned, unsigned);
static void (*rt_reset)(void);
static void (*rt_run)(void);
static int (*rt_load_game)(const struct retro_game_info *);
static void (*rt_unload_game)(void);
static void *(*rt_get_memory_data)(unsigned);
static size_t (*rt_get_memory_size)(unsigned);
static unsigned (*rt_get_region)(void);

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
	 * clock divisor instead of an error. validate_pinned_options() below
	 * exists because of exactly this failure mode. */
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

static char g_system_dir[1024];
static char g_save_dir[1024];
static int g_dump_options;

static int load_save_ram_file(const char *path, void *memory, size_t capacity)
{
	FILE *file;
	long length;
	size_t count;

	file = fopen(path, "rb");
	if (!file) {
		fprintf(stderr, "psiv_oracle: cannot open SRAM %s: %s\n", path,
		        strerror(errno));
		return -1;
	}
	if (fseek(file, 0, SEEK_END) != 0 || (length = ftell(file)) < 0 ||
	    fseek(file, 0, SEEK_SET) != 0) {
		fprintf(stderr, "psiv_oracle: cannot size SRAM %s\n", path);
		fclose(file);
		return -1;
	}
	if ((unsigned long)length > capacity) {
		fprintf(stderr, "psiv_oracle: SRAM %s is %ld bytes, max is %zu\n",
		        path, length, capacity);
		fclose(file);
		return -1;
	}
	count = (size_t)length;
	if (fread(memory, 1, count, file) != count || fclose(file) != 0) {
		fprintf(stderr, "psiv_oracle: cannot read SRAM %s\n", path);
		return -1;
	}
	/* Genesis Plus GX initializes the unprovided tail to erased SRAM. Keep
	 * the same frontend contract for compact .srm fixtures. */
	if (count < capacity)
		memset((uint8_t *)memory + count, 0xFF, capacity - count);
	return 0;
}

/* Declared-option index, captured when the core announces its options, so we
 * can prove every pinned value is one the core actually accepts. A core that
 * receives an unrecognised value does not report an error: Genesis Plus GX
 * runs atoi() or a strcmp chain and falls through to whatever that yields,
 * which is how "1x" for the overclock option quietly clocked the 68000 at 1%
 * and produced a log full of zeroes. An oracle cannot afford silent
 * misconfiguration, so a bad pin is a startup abort. */
#define MAX_DECLARED_OPTS 128
#define MAX_OPT_VALUES 64

struct declared_opt {
	const char *key;
	const char *values[MAX_OPT_VALUES];
	int nvalues;
};

static struct declared_opt g_declared[MAX_DECLARED_OPTS];
static int g_ndeclared;

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

static int validate_pinned_options(void)
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

static void core_log(enum retro_log_level level, const char *fmt, ...)
{
	va_list ap;
	if (level < RETRO_LOG_WARN)
		return; /* core chatter would pollute stderr determinism checks */
	va_start(ap, fmt);
	vfprintf(stderr, fmt, ap);
	va_end(ap);
}

static void ingest_options_v2(const struct retro_core_option_v2_definition *d)
{
	for (; d && d->key; d++) {
		record_declared(d->key, d->values);
		if (g_dump_options)
			printf("option\t%s\t%s\n", d->key,
			       d->default_value ? d->default_value : "(none)");
	}
}

static void ingest_options_v1(const struct retro_core_option_definition *d)
{
	for (; d && d->key; d++) {
		record_declared(d->key, d->values);
		if (g_dump_options)
			printf("option\t%s\t%s\n", d->key,
			       d->default_value ? d->default_value : "(none)");
	}
}

static int environment_cb(unsigned cmd, void *data)
{
	switch (cmd) {
	case RETRO_ENVIRONMENT_GET_CAN_DUPE:
		*(int *)data = 1;
		return 1;

	case RETRO_ENVIRONMENT_GET_OVERSCAN:
		*(int *)data = 0;
		return 1;

	case RETRO_ENVIRONMENT_SET_PIXEL_FORMAT:
		return frame_dump_accept_pixel_format(
			*(const enum retro_pixel_format *)data);

	case RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY:
		*(const char **)data = g_system_dir;
		return 1;

	case RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY:
		*(const char **)data = g_save_dir;
		return 1;

	case RETRO_ENVIRONMENT_GET_LOG_INTERFACE:
		((struct retro_log_callback *)data)->log = core_log;
		return 1;

	case RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION:
		*(unsigned *)data = 2;
		return 1;

	case RETRO_ENVIRONMENT_GET_INPUT_BITMASKS:
		return 1; /* we answer RETRO_DEVICE_ID_JOYPAD_MASK queries */

	case RETRO_ENVIRONMENT_GET_INPUT_MAX_USERS:
		*(unsigned *)data = 2;
		return 1;

	case RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE:
		*(int *)data = 0;
		return 1;

	case RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE:
		/* Bit 0 = video, bit 1 = audio. Both stay enabled so the core does
		 * exactly the work the hardware does; the video sink may capture it. */
		*(int *)data = 0x3;
		return 1;

	case RETRO_ENVIRONMENT_GET_VARIABLE: {
		struct retro_variable *var = data;
		const struct pinned_opt *p;
		var->value = NULL;
		for (p = g_pinned; p->key; p++) {
			if (strcmp(p->key, var->key) == 0) {
				var->value = p->value;
				return 1;
			}
		}
		return 0; /* core falls back to its compiled-in default */
	}

	case RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2:
		if (data)
			ingest_options_v2(
				((struct retro_core_options_v2 *)data)->definitions);
		return 1;

	case RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2_INTL:
		if (data) {
			struct retro_core_options_v2_intl *intl = data;
			if (intl->us)
				ingest_options_v2(intl->us->definitions);
		}
		return 1;

	case RETRO_ENVIRONMENT_SET_CORE_OPTIONS:
		if (data)
			ingest_options_v1(data);
		return 1;

	case RETRO_ENVIRONMENT_SET_CORE_OPTIONS_INTL:
		if (data)
			ingest_options_v1(
				((struct retro_core_options_intl *)data)->us);
		return 1;

	/* Accepted and ignored: presentation, notification and metadata hooks
	 * that cannot influence 68000-visible state. */
	case RETRO_ENVIRONMENT_SET_ROTATION:
	case RETRO_ENVIRONMENT_SET_MESSAGE:
	case RETRO_ENVIRONMENT_SET_MESSAGE_EXT:
	case RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL:
	case RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS:
	case RETRO_ENVIRONMENT_SET_VARIABLES:
	case RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME:
	case RETRO_ENVIRONMENT_SET_CONTROLLER_INFO:
	case RETRO_ENVIRONMENT_SET_MEMORY_MAPS:
	case RETRO_ENVIRONMENT_SET_GEOMETRY:
	case RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO:
	case RETRO_ENVIRONMENT_SET_SUBSYSTEM_INFO:
	case RETRO_ENVIRONMENT_SET_CORE_OPTIONS_DISPLAY:
	case RETRO_ENVIRONMENT_SET_CORE_OPTIONS_UPDATE_DISPLAY_CALLBACK:
		return 1;

	default:
		return 0;
	}
}

static void video_refresh_cb(const void *data, unsigned w, unsigned h,
                             size_t pitch)
{
	frame_dump_video_refresh(data, w, h, pitch);
}

static void audio_sample_cb(int16_t l, int16_t r) { (void)l; (void)r; }

static size_t audio_sample_batch_cb(const int16_t *data, size_t frames)
{
	(void)data;
	return frames;
}

static uint32_t g_cur_buttons;

static void input_poll_cb(void) {}

static int16_t input_state_cb(unsigned port, unsigned device, unsigned index,
                              unsigned id)
{
	int16_t mask = 0;
	(void)index;
	if (port != 0 || device != RETRO_DEVICE_JOYPAD)
		return 0;

	/* Genesis Plus GX maps libretro Y/B/A onto Mega Drive A/B/C. */
	if (g_cur_buttons & PAD_UP) mask |= 1 << RETRO_DEVICE_ID_JOYPAD_UP;
	if (g_cur_buttons & PAD_DOWN) mask |= 1 << RETRO_DEVICE_ID_JOYPAD_DOWN;
	if (g_cur_buttons & PAD_LEFT) mask |= 1 << RETRO_DEVICE_ID_JOYPAD_LEFT;
	if (g_cur_buttons & PAD_RIGHT) mask |= 1 << RETRO_DEVICE_ID_JOYPAD_RIGHT;
	if (g_cur_buttons & PAD_A) mask |= 1 << RETRO_DEVICE_ID_JOYPAD_Y;
	if (g_cur_buttons & PAD_B) mask |= 1 << RETRO_DEVICE_ID_JOYPAD_B;
	if (g_cur_buttons & PAD_C) mask |= 1 << RETRO_DEVICE_ID_JOYPAD_A;
	if (g_cur_buttons & PAD_START) mask |= 1 << RETRO_DEVICE_ID_JOYPAD_START;

	if (id == RETRO_DEVICE_ID_JOYPAD_MASK)
		return mask;
	if (id < 16)
		return (mask >> id) & 1;
	return 0;
}

/* ------------------------------------------------------------------ */
/* Work-RAM accessors                                                 */
/* ------------------------------------------------------------------ */

static const uint8_t *g_ram;
static size_t g_ram_size;

static uint32_t ram_u8(uint32_t addr)
{
	return g_ram[(addr & 0xFFFF) ^ 1];
}

static uint32_t ram_u16(uint32_t addr)
{
	uint32_t a = addr & 0xFFFE;
	return (uint32_t)g_ram[a] | ((uint32_t)g_ram[a + 1] << 8);
}

static uint32_t ram_u32(uint32_t addr)
{
	return (ram_u16(addr) << 16) | ram_u16(addr + 2);
}

static uint32_t ram_read(const struct field *f)
{
	switch (f->size) {
	case 1: return ram_u8(f->addr);
	case 2: return ram_u16(f->addr);
	default: return ram_u32(f->addr);
	}
}

/* ------------------------------------------------------------------ */
/* Parsing helpers                                                    */
/* ------------------------------------------------------------------ */

static char *trim(char *s)
{
	char *e;
	while (*s == ' ' || *s == '\t' || *s == '\r' || *s == '\n')
		s++;
	e = s + strlen(s);
	while (e > s && (e[-1] == ' ' || e[-1] == '\t' || e[-1] == '\r' ||
	                 e[-1] == '\n'))
		*--e = 0;
	return s;
}

/* The RAM map is a flat line-oriented table so the C host and any future Lua
 * or Rust consumer can read the same file without a JSON dependency:
 *   name<TAB>address<TAB>size<TAB>group<TAB>flags
 * Address is hex with or without 0x/$; flags is "hex", "signed", or "-". */
static int load_ram_map(const char *path)
{
	FILE *f = fopen(path, "r");
	char line[512];
	int lineno = 0;

	if (!f) {
		fprintf(stderr, "psiv_oracle: cannot open ram map %s: %s\n", path,
		        strerror(errno));
		return -1;
	}

	while (fgets(line, sizeof line, f)) {
		char *p = trim(line), *tok, *save;
		struct field *fl;
		lineno++;
		if (!*p || *p == '#')
			continue;
		if (g_nfields >= MAX_FIELDS) {
			fprintf(stderr, "psiv_oracle: too many fields (max %d)\n",
			        MAX_FIELDS);
			fclose(f);
			return -1;
		}
		fl = &g_fields[g_nfields];
		memset(fl, 0, sizeof *fl);
		fl->enabled = 1;

		tok = strtok_r(p, "\t", &save);
		if (!tok) continue;
		snprintf(fl->name, sizeof fl->name, "%s", trim(tok));

		tok = strtok_r(NULL, "\t", &save);
		if (!tok) goto bad;
		{
			char *a = trim(tok);
			if (*a == '$') a++;
			else if (a[0] == '0' && (a[1] == 'x' || a[1] == 'X')) a += 2;
			fl->addr = (uint32_t)strtoul(a, NULL, 16);
		}

		tok = strtok_r(NULL, "\t", &save);
		if (!tok) goto bad;
		fl->size = atoi(trim(tok));
		if (fl->size != 1 && fl->size != 2 && fl->size != 4) goto bad;

		tok = strtok_r(NULL, "\t", &save);
		snprintf(fl->group, sizeof fl->group, "%s",
		         tok ? trim(tok) : "misc");

		tok = strtok_r(NULL, "\t", &save);
		if (tok) {
			char *fs = trim(tok);
			if (strstr(fs, "hex")) fl->hex = 1;
			if (strstr(fs, "signed")) fl->is_signed = 1;
		}

		g_nfields++;
		continue;
	bad:
		fprintf(stderr, "psiv_oracle: bad ram map line %d\n", lineno);
		fclose(f);
		return -1;
	}
	fclose(f);
	return 0;
}

static void enable_groups(const char *csv)
{
	int i;
	for (i = 0; i < g_nfields; i++)
		g_fields[i].enabled = 0;
	for (i = 0; i < g_nfields; i++) {
		const char *p = csv;
		size_t glen = strlen(g_fields[i].group);
		while (*p) {
			const char *comma = strchr(p, ',');
			size_t n = comma ? (size_t)(comma - p) : strlen(p);
			if (n == glen && strncmp(p, g_fields[i].group, n) == 0) {
				g_fields[i].enabled = 1;
				break;
			}
			if (!comma) break;
			p = comma + 1;
		}
	}
}

static void usage(void)
{
	fprintf(stderr,
	        "usage: psiv_oracle --core <so> --rom <md> --tape <tape> "
	        "[--map <map>] [--out <csv>]\n"
	        "                   [--groups a,b,c] [--dump-options] "
	        "[--probe-endian]\n"
		"                   [--dump-frames N1,N2,...] "
		"--dump-frames-dir <dir>\n"
		"                   [--ram-patch <frame>:<address>:<hex-bytes>]\n"
		"                   [--dump-ram <frame>:<path>]\n"
	        "                   [--dump-state <frame>:<path>]\n"
	        "                   [--load-sram <path>]\n");
}

int main(int argc, char **argv)
{
	const char *core_path = NULL, *rom_path = NULL, *tape_path = NULL;
	const char *map_path = NULL, *out_path = NULL, *groups = NULL;
	const char *dump_frames = NULL, *dump_frames_dir = NULL;
	const char *load_sram_path = NULL;
	int probe_endian = 0;
	FILE *out = stdout;
	struct retro_system_info sysinfo;
	struct retro_system_av_info av_info;
	struct retro_game_info game;
	uint8_t *rom = NULL;
	size_t rom_size = 0;
	FILE *rf;
	int i, step, frame_dump_status;
	uint64_t frame = 0;

	for (i = 1; i < argc; i++) {
		if (!strcmp(argv[i], "--core") && i + 1 < argc) core_path = argv[++i];
		else if (!strcmp(argv[i], "--rom") && i + 1 < argc) rom_path = argv[++i];
		else if (!strcmp(argv[i], "--tape") && i + 1 < argc) tape_path = argv[++i];
		else if (!strcmp(argv[i], "--map") && i + 1 < argc) map_path = argv[++i];
		else if (!strcmp(argv[i], "--out") && i + 1 < argc) out_path = argv[++i];
		else if (!strcmp(argv[i], "--groups") && i + 1 < argc) groups = argv[++i];
		else if (!strcmp(argv[i], "--dump-options")) g_dump_options = 1;
		else if (!strcmp(argv[i], "--dump-frames") && i + 1 < argc)
			dump_frames = argv[++i];
		else if (!strcmp(argv[i], "--dump-frames-dir") && i + 1 < argc)
			dump_frames_dir = argv[++i];
		else if (!strcmp(argv[i], "--ram-patch") && i + 1 < argc) {
			if (ram_patch_add_spec(argv[++i]) != 0) {
				fprintf(stderr, "psiv_oracle: %s\n", ram_patch_error());
				return 2;
			}
		}
		else if (!strcmp(argv[i], "--dump-ram") && i + 1 < argc) {
			if (ram_dump_add_spec(argv[++i]) != 0) {
				fprintf(stderr, "psiv_oracle: %s\n", ram_dump_error());
				return 2;
			}
		}
		else if (!strcmp(argv[i], "--dump-state") && i + 1 < argc) {
			if (state_dump_add_spec(argv[++i]) != 0) {
				fprintf(stderr, "psiv_oracle: %s\n", state_dump_error());
				return 2;
			}
		}
		else if (!strcmp(argv[i], "--load-sram") && i + 1 < argc)
			load_sram_path = argv[++i];
		else if (!strcmp(argv[i], "--probe-endian")) probe_endian = 1;
		else { usage(); return 2; }
	}
	if ((dump_frames && !dump_frames_dir) ||
	    (!dump_frames && dump_frames_dir)) {
		fprintf(stderr, "psiv_oracle: --dump-frames and --dump-frames-dir "
		        "must be used together\n");
		return 2;
	}
	if (dump_frames && frame_dump_set_frames(dump_frames) != 0) {
		fprintf(stderr, "psiv_oracle: %s\n", frame_dump_error());
		return 2;
	}
	if (dump_frames_dir && frame_dump_set_directory(dump_frames_dir) != 0) {
		fprintf(stderr, "psiv_oracle: %s\n", frame_dump_error());
		return 2;
	}

	if (!core_path || !rom_path ||
	    (!tape_path && !g_dump_options && !probe_endian)) {
		usage();
		return 2;
	}

	snprintf(g_system_dir, sizeof g_system_dir, ".");
	snprintf(g_save_dir, sizeof g_save_dir, ".");

	g_core = dlopen(core_path, RTLD_LAZY | RTLD_LOCAL);
	if (!g_core) {
		fprintf(stderr, "psiv_oracle: dlopen %s: %s\n", core_path, dlerror());
		return 1;
	}

#define SYM(var, name)                                                        \
	do {                                                                      \
		*(void **)(&var) = dlsym(g_core, name);                               \
		if (!var) {                                                           \
			fprintf(stderr, "psiv_oracle: missing symbol %s\n", name);        \
			return 1;                                                         \
		}                                                                     \
	} while (0)

	SYM(rt_api_version, "retro_api_version");
	SYM(rt_init, "retro_init");
	SYM(rt_deinit, "retro_deinit");
	SYM(rt_get_system_info, "retro_get_system_info");
	SYM(rt_get_system_av_info, "retro_get_system_av_info");
	SYM(rt_set_environment, "retro_set_environment");
	SYM(rt_set_video_refresh, "retro_set_video_refresh");
	SYM(rt_set_audio_sample, "retro_set_audio_sample");
	SYM(rt_set_audio_sample_batch, "retro_set_audio_sample_batch");
	SYM(rt_set_input_poll, "retro_set_input_poll");
	SYM(rt_set_input_state, "retro_set_input_state");
	SYM(rt_set_controller_port_device, "retro_set_controller_port_device");
	SYM(rt_reset, "retro_reset");
	SYM(rt_run, "retro_run");
	SYM(rt_load_game, "retro_load_game");
	SYM(rt_unload_game, "retro_unload_game");
	SYM(rt_get_memory_data, "retro_get_memory_data");
	SYM(rt_get_memory_size, "retro_get_memory_size");
	SYM(rt_get_region, "retro_get_region");
#undef SYM

	if (rt_api_version() != RETRO_API_VERSION) {
		fprintf(stderr, "psiv_oracle: core reports libretro API %u, expected "
		        "%u\n", rt_api_version(), RETRO_API_VERSION);
		return 1;
	}

	rt_get_system_info(&sysinfo);

	rt_set_environment(environment_cb);
	rt_set_video_refresh(video_refresh_cb);
	rt_set_audio_sample(audio_sample_cb);
	rt_set_audio_sample_batch(audio_sample_batch_cb);
	rt_set_input_poll(input_poll_cb);
	rt_set_input_state(input_state_cb);

	/* set_environment above already fired the option declaration, so the
	 * pinned set can be checked before a single frame is emulated. */
	if (validate_pinned_options() != 0)
		return 1;

	if (g_dump_options) {
		printf("core\t%s\t%s\n", sysinfo.library_name, sysinfo.library_version);
		return 0;
	}

	rt_init();

	rf = fopen(rom_path, "rb");
	if (!rf) {
		fprintf(stderr, "psiv_oracle: cannot open rom %s: %s\n", rom_path,
		        strerror(errno));
		return 1;
	}
	fseek(rf, 0, SEEK_END);
	rom_size = (size_t)ftell(rf);
	fseek(rf, 0, SEEK_SET);
	rom = malloc(rom_size);
	if (!rom || fread(rom, 1, rom_size, rf) != rom_size) {
		fprintf(stderr, "psiv_oracle: cannot read rom\n");
		return 1;
	}
	fclose(rf);

	memset(&game, 0, sizeof game);
	game.path = rom_path;
	game.data = rom;
	game.size = rom_size;

	if (!rt_load_game(&game)) {
		fprintf(stderr, "psiv_oracle: core refused the rom\n");
		return 1;
	}
	if (load_sram_path) {
		void *sram = rt_get_memory_data(RETRO_MEMORY_SAVE_RAM);
		size_t sram_size = rt_get_memory_size(RETRO_MEMORY_SAVE_RAM);
		if (!sram || sram_size == 0 ||
		    load_save_ram_file(load_sram_path, sram, sram_size) != 0)
			return 1;
		fprintf(stderr, "psiv_oracle: loaded SRAM %s (%zu-byte core buffer)\n",
		        load_sram_path, sram_size);
	}
	rt_get_system_av_info(&av_info);
	if (frame_dump_set_geometry(av_info.geometry.base_width,
	                            av_info.geometry.base_height) != 0) {
		fprintf(stderr, "psiv_oracle: %s\n", frame_dump_error());
		return 1;
	}
	if (frame_dump_enabled())
		fprintf(stderr,
		        "psiv_oracle: video capture pixel_format=%s av_base=%ux%u "
		        "target=320x224 max=%ux%u\n",
		        frame_dump_pixel_format_name(
				frame_dump_negotiated_pixel_format()),
		        av_info.geometry.base_width, av_info.geometry.base_height,
		        av_info.geometry.max_width, av_info.geometry.max_height);

	rt_set_controller_port_device(0, RETRO_DEVICE_JOYPAD);
	rt_set_controller_port_device(1, RETRO_DEVICE_JOYPAD);

	g_ram = rt_get_memory_data(RETRO_MEMORY_SYSTEM_RAM);
	g_ram_size = rt_get_memory_size(RETRO_MEMORY_SYSTEM_RAM);
	if (!g_ram || g_ram_size < 0x10000) {
		fprintf(stderr, "psiv_oracle: system ram unavailable (size %zu)\n",
		        g_ram_size);
		return 1;
	}

	/* Endianness probe: run far enough for the game to have written its
	 * signature string to PS4_String ($FFFFF000), then print it under the
	 * byte accessor. A correct accessor spells the string in order. */
	if (probe_endian) {
		uint32_t a;
		size_t nz = 0;
		for (i = 0; i < 900; i++)
			rt_run();
		for (a = 0; a < 0x10000; a++)
			if (g_ram[a])
				nz++;
		printf("work_ram nonzero bytes after 900 frames: %zu / 65536\n", nz);
		printf("Map_Chunk_Addr ($FFFFEC2C) as long: %08X\n", ram_u32(0xFFFFEC2C));
		printf("Field_Map_Index ($FFFFEC28) as word: %04X\n", ram_u16(0xFFFFEC28));
		printf("PS4_String bytes at $FFFFF000: ");
		for (a = 0xFFFFF000; a < 0xFFFFF012; a++) {
			uint32_t c = ram_u8(a);
			printf("%c", (c >= 32 && c < 127) ? (char)c : '.');
		}
		printf("\nraw array order:               ");
		for (a = 0; a < 0x12; a++) {
			uint32_t c = g_ram[0xF000 + a];
			printf("%c", (c >= 32 && c < 127) ? (char)c : '.');
		}
		printf("\n");
		rt_unload_game();
		rt_deinit();
		return 0;
	}

	if (load_ram_map(map_path ? map_path : "oracle/ram_map.tsv") != 0)
		return 1;
	if (groups)
		enable_groups(groups);
	if (psiv_tape_load(tape_path) != 0) {
		fprintf(stderr, "psiv_oracle: %s\n", psiv_tape_error());
		return 1;
	}
	if (ram_patch_validate_total(psiv_tape_total_frames()) != 0) {
		fprintf(stderr, "psiv_oracle: %s\n", ram_patch_error());
		return 2;
	}

	if (out_path) {
		out = fopen(out_path, "w");
		if (!out) {
			fprintf(stderr, "psiv_oracle: cannot write %s: %s\n", out_path,
			        strerror(errno));
			return 1;
		}
	}

	/* Header carries the provenance needed to reproduce the run. */
	fprintf(out, "# core=%s %s region=%s\n", sysinfo.library_name,
	        sysinfo.library_version,
	        rt_get_region() == RETRO_REGION_PAL ? "PAL" : "NTSC");
	fprintf(out, "# rom=%s size=%zu\n", rom_path, rom_size);
	fprintf(out, "# tape=%s steps=%d frames=%llu\n", tape_path,
	        psiv_tape_count(), (unsigned long long)psiv_tape_total_frames());
	fprintf(out, "frame,mark,buttons");
	for (i = 0; i < g_nfields; i++)
		if (g_fields[i].enabled)
			fprintf(out, ",%s", g_fields[i].name);
	fprintf(out, "\n");

	for (step = 0; step < psiv_tape_count(); step++) {
		const struct psiv_tape_step *tape_step = psiv_tape_step_at(step);
		uint32_t k;
		g_cur_buttons = tape_step->buttons;
		for (k = 0; k < tape_step->frames; k++) {
			char btn[16];
			int n = 0;

			ram_patch_apply(frame + 1, (uint8_t *)g_ram, g_ram_size);
			frame_dump_begin_frame(frame + 1);
			rt_run();
			frame++;
			if (frame_dump_failed()) {
				fprintf(stderr, "psiv_oracle: %s\n", frame_dump_error());
				return 1;
			}

			if (ram_dump_write(frame, g_ram) != 0) {
				fprintf(stderr, "psiv_oracle: %s\n", ram_dump_error());
				return 1;
			}
			if (state_dump_write(frame, g_ram) != 0) {
				fprintf(stderr, "psiv_oracle: %s\n", state_dump_error());
				return 1;
			}

			if (g_cur_buttons & PAD_UP) btn[n++] = 'U';
			if (g_cur_buttons & PAD_DOWN) btn[n++] = 'D';
			if (g_cur_buttons & PAD_LEFT) btn[n++] = 'L';
			if (g_cur_buttons & PAD_RIGHT) btn[n++] = 'R';
			if (g_cur_buttons & PAD_A) btn[n++] = 'A';
			if (g_cur_buttons & PAD_B) btn[n++] = 'B';
			if (g_cur_buttons & PAD_C) btn[n++] = 'C';
			if (g_cur_buttons & PAD_START) btn[n++] = 'S';
			if (n == 0) btn[n++] = '.';
			btn[n] = 0;

			fprintf(out, "%llu,%s,%s", (unsigned long long)frame,
			        (k == 0) ? tape_step->mark : "", btn);

			for (i = 0; i < g_nfields; i++) {
				uint32_t v;
				if (!g_fields[i].enabled)
					continue;
				v = ram_read(&g_fields[i]);
				if (g_fields[i].hex) {
					fprintf(out, ",%0*X", g_fields[i].size * 2, v);
				} else if (g_fields[i].is_signed) {
					int32_t s = (g_fields[i].size == 2)
						? (int32_t)(int16_t)v
						: (g_fields[i].size == 1)
							? (int32_t)(int8_t)v
							: (int32_t)v;
					fprintf(out, ",%d", s);
				} else {
					fprintf(out, ",%u", v);
				}
			}
			fprintf(out, "\n");
		}
	}
	if (ram_patch_finish() != 0) {
		fprintf(stderr, "psiv_oracle: %s\n", ram_patch_error());
		return 1;
	}

	frame_dump_status = frame_dump_finish();
	if (frame_dump_status != 0)
		fprintf(stderr, "psiv_oracle: %s\n", frame_dump_error());
	if (out != stdout)
		fclose(out);
	rt_unload_game();
	rt_deinit();
	free(rom);
	frame_dump_shutdown();
	return frame_dump_status != 0;
}
