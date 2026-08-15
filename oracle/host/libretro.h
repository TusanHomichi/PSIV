/* Minimal subset of the libretro API (libretro.h, public domain / MIT-0)
 * sufficient to host a Genesis Plus GX core headlessly.
 * Only the entry points and environment commands psiv_oracle actually uses
 * are declared; the ABI (struct layout, enum values, calling convention)
 * matches upstream libretro.h exactly and must not be edited casually. */
#ifndef PSIV_ORACLE_LIBRETRO_H
#define PSIV_ORACLE_LIBRETRO_H

#include <stddef.h>
#include <stdint.h>

#define RETRO_API_VERSION 1

/* Devices */
#define RETRO_DEVICE_JOYPAD 1

/* Joypad button ids. Genesis Plus GX maps these to the Mega Drive pad as:
 *   Genesis A <- Y(1), Genesis B <- B(0), Genesis C <- A(8), START <- START(3).
 * See gpgx-src/libretro/libretro.c, osd_input_update_internal_bitmasks(). */
#define RETRO_DEVICE_ID_JOYPAD_B 0
#define RETRO_DEVICE_ID_JOYPAD_Y 1
#define RETRO_DEVICE_ID_JOYPAD_SELECT 2
#define RETRO_DEVICE_ID_JOYPAD_START 3
#define RETRO_DEVICE_ID_JOYPAD_UP 4
#define RETRO_DEVICE_ID_JOYPAD_DOWN 5
#define RETRO_DEVICE_ID_JOYPAD_LEFT 6
#define RETRO_DEVICE_ID_JOYPAD_RIGHT 7
#define RETRO_DEVICE_ID_JOYPAD_A 8
#define RETRO_DEVICE_ID_JOYPAD_X 9
#define RETRO_DEVICE_ID_JOYPAD_L 10
#define RETRO_DEVICE_ID_JOYPAD_R 11
#define RETRO_DEVICE_ID_JOYPAD_L2 12
#define RETRO_DEVICE_ID_JOYPAD_R2 13
#define RETRO_DEVICE_ID_JOYPAD_L3 14
#define RETRO_DEVICE_ID_JOYPAD_R3 15
#define RETRO_DEVICE_ID_JOYPAD_MASK 256

/* Memory */
#define RETRO_MEMORY_SAVE_RAM 0
#define RETRO_MEMORY_SYSTEM_RAM 2

/* Regions */
#define RETRO_REGION_NTSC 0
#define RETRO_REGION_PAL 1

/* Pixel formats */
enum retro_pixel_format {
	RETRO_PIXEL_FORMAT_0RGB1555 = 0,
	RETRO_PIXEL_FORMAT_XRGB8888 = 1,
	RETRO_PIXEL_FORMAT_RGB565 = 2,
	RETRO_PIXEL_FORMAT_UNKNOWN = INT32_MAX
};

/* Environment commands (only the ones we answer). */
#define RETRO_ENVIRONMENT_SET_ROTATION 1
#define RETRO_ENVIRONMENT_GET_OVERSCAN 2
#define RETRO_ENVIRONMENT_GET_CAN_DUPE 3
#define RETRO_ENVIRONMENT_SET_MESSAGE 6
#define RETRO_ENVIRONMENT_SHUTDOWN 7
#define RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL 8
#define RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY 9
#define RETRO_ENVIRONMENT_SET_PIXEL_FORMAT 10
#define RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS 11
#define RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK 12
#define RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE 13
#define RETRO_ENVIRONMENT_SET_HW_RENDER 14
#define RETRO_ENVIRONMENT_GET_VARIABLE 15
#define RETRO_ENVIRONMENT_SET_VARIABLES 16
#define RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE 17
#define RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME 18
#define RETRO_ENVIRONMENT_GET_LIBRETRO_PATH 19
#define RETRO_ENVIRONMENT_SET_FRAME_TIME_CALLBACK 21
#define RETRO_ENVIRONMENT_SET_AUDIO_CALLBACK 22
#define RETRO_ENVIRONMENT_GET_RUMBLE_INTERFACE 23
#define RETRO_ENVIRONMENT_GET_INPUT_DEVICE_CAPABILITIES 24
#define RETRO_ENVIRONMENT_GET_LOG_INTERFACE 27
#define RETRO_ENVIRONMENT_GET_PERF_INTERFACE 28
#define RETRO_ENVIRONMENT_GET_CORE_ASSETS_DIRECTORY 30
#define RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY 31
#define RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO 32
#define RETRO_ENVIRONMENT_SET_SUBSYSTEM_INFO 34
#define RETRO_ENVIRONMENT_SET_CONTROLLER_INFO 35
#define RETRO_ENVIRONMENT_SET_MEMORY_MAPS 36
#define RETRO_ENVIRONMENT_SET_GEOMETRY 37
#define RETRO_ENVIRONMENT_GET_USERNAME 38
#define RETRO_ENVIRONMENT_GET_LANGUAGE 39
#define RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE 47
#define RETRO_ENVIRONMENT_GET_INPUT_BITMASKS 51
#define RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION 52
#define RETRO_ENVIRONMENT_SET_CORE_OPTIONS 53
#define RETRO_ENVIRONMENT_SET_CORE_OPTIONS_INTL 54
#define RETRO_ENVIRONMENT_SET_CORE_OPTIONS_DISPLAY 55
#define RETRO_ENVIRONMENT_GET_MESSAGE_INTERFACE_VERSION 59
#define RETRO_ENVIRONMENT_SET_MESSAGE_EXT 60
#define RETRO_ENVIRONMENT_GET_INPUT_MAX_USERS 61
#define RETRO_ENVIRONMENT_SET_CONTENT_INFO_OVERRIDE 65
#define RETRO_ENVIRONMENT_GET_GAME_INFO_EXT 66
#define RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2 67
#define RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2_INTL 68
#define RETRO_ENVIRONMENT_SET_CORE_OPTIONS_UPDATE_DISPLAY_CALLBACK 69
#define RETRO_ENVIRONMENT_SET_VARIABLE 70

/* Log levels */
enum retro_log_level {
	RETRO_LOG_DEBUG = 0,
	RETRO_LOG_INFO,
	RETRO_LOG_WARN,
	RETRO_LOG_ERROR
};

struct retro_log_callback {
	void (*log)(enum retro_log_level level, const char *fmt, ...);
};

struct retro_variable {
	const char *key;
	const char *value;
};

struct retro_game_info {
	const char *path;
	const void *data;
	size_t size;
	const char *meta;
};

struct retro_game_geometry {
	unsigned base_width;
	unsigned base_height;
	unsigned max_width;
	unsigned max_height;
	float aspect_ratio;
};

struct retro_system_timing {
	double fps;
	double sample_rate;
};

struct retro_system_av_info {
	struct retro_game_geometry geometry;
	struct retro_system_timing timing;
};

struct retro_system_info {
	const char *library_name;
	const char *library_version;
	const char *valid_extensions;
	int need_fullpath;
	int block_extract;
};

/* Core option descriptor shapes, needed so we can enumerate a core's declared
 * options and their defaults (SET_VARIABLES / SET_CORE_OPTIONS_V2). */
struct retro_core_option_value {
	const char *value;
	const char *label;
};

#define RETRO_NUM_CORE_OPTION_VALUES_MAX 128

struct retro_core_option_definition {
	const char *key;
	const char *desc;
	const char *info;
	struct retro_core_option_value values[RETRO_NUM_CORE_OPTION_VALUES_MAX];
	const char *default_value;
};

struct retro_core_option_v2_definition {
	const char *key;
	const char *desc;
	const char *desc_categorized;
	const char *info;
	const char *info_categorized;
	const char *category_key;
	struct retro_core_option_value values[RETRO_NUM_CORE_OPTION_VALUES_MAX];
	const char *default_value;
};

struct retro_core_option_v2_category {
	const char *key;
	const char *desc;
	const char *info;
};

struct retro_core_options_v2 {
	struct retro_core_option_v2_category *categories;
	struct retro_core_option_v2_definition *definitions;
};

struct retro_core_options_v2_intl {
	struct retro_core_options_v2 *us;
	struct retro_core_options_v2 *local;
};

struct retro_core_options_intl {
	struct retro_core_option_definition *us;
	struct retro_core_option_definition *local;
};

/* Callback typedefs */
typedef int (*retro_environment_t)(unsigned cmd, void *data);
typedef void (*retro_video_refresh_t)(const void *data, unsigned width,
                                      unsigned height, size_t pitch);
typedef void (*retro_audio_sample_t)(int16_t left, int16_t right);
typedef size_t (*retro_audio_sample_batch_t)(const int16_t *data,
                                             size_t frames);
typedef void (*retro_input_poll_t)(void);
typedef int16_t (*retro_input_state_t)(unsigned port, unsigned device,
                                       unsigned index, unsigned id);

#endif /* PSIV_ORACLE_LIBRETRO_H */
