#define _GNU_SOURCE

#include "frame_dump.h"

#include <errno.h>
#include <limits.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#define MAX_FRAME_REQUESTS 256
#define FRAME_WIDTH 320
#define FRAME_HEIGHT 224
#define FRAME_PATH_MAX 4096

struct frame_request {
	uint64_t frame;
	int captured;
};

static struct frame_request g_requests[MAX_FRAME_REQUESTS];
static int g_nrequests;
static char g_directory[FRAME_PATH_MAX];
static unsigned g_width;
static unsigned g_height;
static enum retro_pixel_format g_pixel_format = RETRO_PIXEL_FORMAT_0RGB1555;
static uint8_t *g_last_rgb;
static int g_last_valid;
static uint64_t g_current_frame;
static unsigned g_callback_width;
static unsigned g_callback_height;
static size_t g_callback_pitch;
static int g_callback_seen;
static int g_failed;
static char g_error[256];

static void failf(const char *fmt, ...)
{
	va_list ap;

	if (g_failed)
		return;
	g_failed = 1;
	va_start(ap, fmt);
	vsnprintf(g_error, sizeof g_error, fmt, ap);
	va_end(ap);
}

static int write_u32_be(FILE *f, uint32_t value)
{
	uint8_t bytes[4];
	bytes[0] = (uint8_t)(value >> 24);
	bytes[1] = (uint8_t)(value >> 16);
	bytes[2] = (uint8_t)(value >> 8);
	bytes[3] = (uint8_t)value;
	return fwrite(bytes, 1, sizeof bytes, f) == sizeof bytes;
}

static uint32_t crc32_update(uint32_t crc, const uint8_t *data, size_t size)
{
	size_t i;
	int bit;

	for (i = 0; i < size; i++) {
		crc ^= data[i];
		for (bit = 0; bit < 8; bit++)
			crc = (crc >> 1) ^ (0xEDB88320u & (uint32_t)-(int)(crc & 1));
	}
	return crc;
}

static int write_chunk(FILE *f, const char type[4], const uint8_t *data,
	                       uint32_t size)
{
	uint32_t crc = 0xFFFFFFFFu;

	if (!write_u32_be(f, size) || fwrite(type, 1, 4, f) != 4)
		return 0;
	crc = crc32_update(crc, (const uint8_t *)type, 4);
	if (size && fwrite(data, 1, size, f) != size)
		return 0;
	crc = crc32_update(crc, data, size) ^ 0xFFFFFFFFu;
	return write_u32_be(f, crc);
}

static uint32_t adler32(const uint8_t *data, size_t size)
{
	uint32_t a = 1;
	uint32_t b = 0;
	size_t i;

	for (i = 0; i < size; i++) {
		a = (a + data[i]) % 65521u;
		b = (b + a) % 65521u;
	}
	return (b << 16) | a;
}

/* PNG needs a zlib stream, but stored DEFLATE blocks need no compression
 * library. Reference frames are only 320x224, so this remains small and
 * fully auditable. */
static int write_png_rgb(const char *path, unsigned width, unsigned height,
	                        const uint8_t *rgb)
{
	static const uint8_t signature[8] = {
		0x89, 'P', 'N', 'G', '\r', '\n', 0x1A, '\n'
	};
	uint8_t ihdr[13];
	uint8_t *raw = NULL;
	uint8_t *zlib = NULL;
	FILE *f = NULL;
	size_t row_bytes = (size_t)width * 3;
	size_t raw_row = row_bytes + 1;
	size_t raw_size = raw_row * height;
	size_t blocks = (raw_size + 65534) / 65535;
	size_t zlib_size = 2 + raw_size + blocks * 5 + 4;
	size_t y, src, dst, left, len, pos;
	uint32_t sum;
	int ok = 0;

	if (width == 0 || height == 0 || raw_size / raw_row != height ||
	    zlib_size < raw_size || zlib_size > UINT32_MAX) {
		failf("invalid PNG dimensions %ux%u", width, height);
		return -1;
	}
	raw = malloc(raw_size);
	zlib = malloc(zlib_size);
	if (!raw || !zlib) {
		failf("out of memory encoding %s", path);
		goto done;
	}
	for (y = 0; y < height; y++) {
		dst = y * raw_row;
		raw[dst] = 0; /* filter: None */
		memcpy(raw + dst + 1, rgb + y * row_bytes, row_bytes);
	}

	/* zlib header: CM=8, CINFO=7, FCHECK valid, FDICT=0, level=fast. */
	zlib[0] = 0x78;
	zlib[1] = 0x01;
	pos = 2;
	for (src = 0; src < raw_size; src += len) {
		left = raw_size - src;
		len = left > 65535 ? 65535 : left;
		zlib[pos++] = (uint8_t)(src + len == raw_size ? 1 : 0);
		zlib[pos++] = (uint8_t)len;
		zlib[pos++] = (uint8_t)(len >> 8);
		zlib[pos++] = (uint8_t)~(uint16_t)len;
		zlib[pos++] = (uint8_t)(~(uint16_t)len >> 8);
		memcpy(zlib + pos, raw + src, len);
		pos += len;
	}
	sum = adler32(raw, raw_size);
	zlib[pos++] = (uint8_t)(sum >> 24);
	zlib[pos++] = (uint8_t)(sum >> 16);
	zlib[pos++] = (uint8_t)(sum >> 8);
	zlib[pos++] = (uint8_t)sum;
	if (pos != zlib_size) {
		failf("internal PNG size error for %s", path);
		goto done;
	}

	ihdr[0] = (uint8_t)(width >> 24);
	ihdr[1] = (uint8_t)(width >> 16);
	ihdr[2] = (uint8_t)(width >> 8);
	ihdr[3] = (uint8_t)width;
	ihdr[4] = (uint8_t)(height >> 24);
	ihdr[5] = (uint8_t)(height >> 16);
	ihdr[6] = (uint8_t)(height >> 8);
	ihdr[7] = (uint8_t)height;
	ihdr[8] = 8;  /* bit depth */
	ihdr[9] = 2;  /* truecolour RGB */
	ihdr[10] = 0; /* compression */
	ihdr[11] = 0; /* filter */
	ihdr[12] = 0; /* interlace */

	f = fopen(path, "wb");
	if (!f) {
		failf("cannot write %s: %s", path, strerror(errno));
		goto done;
	}
	ok = fwrite(signature, 1, sizeof signature, f) == sizeof signature &&
	     write_chunk(f, "IHDR", ihdr, sizeof ihdr) &&
	     write_chunk(f, "IDAT", zlib, (uint32_t)zlib_size) &&
	     write_chunk(f, "IEND", NULL, 0);
	if (!ok) {
		fclose(f);
		f = NULL;
		failf("short write while encoding %s", path);
		goto done;
	}
	if (fclose(f) != 0) {
		f = NULL;
		failf("short write while closing %s", path);
		goto done;
	}
	f = NULL;

done:
	if (f)
		fclose(f);
	free(zlib);
	free(raw);
	return g_failed ? -1 : 0;
}

static int ensure_directory(void)
{
	struct stat st;

	if (stat(g_directory, &st) == 0) {
		if (!S_ISDIR(st.st_mode)) {
			failf("frame directory is not a directory: %s", g_directory);
			return -1;
		}
		return 0;
	}
	if (errno != ENOENT || mkdir(g_directory, 0777) != 0) {
		failf("cannot create frame directory %s: %s", g_directory,
		      strerror(errno));
		return -1;
	}
	return 0;
}

int frame_dump_set_frames(const char *list)
{
	const char *p = list;

	if (g_nrequests || !list || !*list) {
		failf("--dump-frames wants a comma-separated list of positive frame numbers");
		return -1;
	}
	for (;;) {
		char *end;
		uint64_t frame;
		int i;

		if (g_nrequests >= MAX_FRAME_REQUESTS) {
			failf("--dump-frames accepts at most %d frame numbers",
			      MAX_FRAME_REQUESTS);
			return -1;
		}
		errno = 0;
		frame = strtoull(p, &end, 10);
		if (errno == ERANGE || end == p || frame == 0 ||
		    (*end != '\0' && *end != ',')) {
			failf("bad frame number in --dump-frames: %s", p);
			return -1;
		}
		for (i = 0; i < g_nrequests; i++) {
			if (g_requests[i].frame == frame) {
				failf("duplicate frame number in --dump-frames: %llu",
				      (unsigned long long)frame);
				return -1;
			}
		}
		g_requests[g_nrequests++].frame = frame;
		if (*end == '\0')
			break;
		p = end + 1;
		if (!*p) {
			failf("--dump-frames cannot end with a comma");
			return -1;
		}
	}
	return 0;
}

int frame_dump_set_directory(const char *directory)
{
	size_t length;

	if (!directory || !*directory) {
		failf("--dump-frames-dir wants a directory");
		return -1;
	}
	length = strlen(directory);
	if (length >= sizeof g_directory) {
		failf("frame directory path is too long");
		return -1;
	}
	memcpy(g_directory, directory, length + 1);
	return 0;
}

int frame_dump_enabled(void)
{
	return g_nrequests != 0;
}

int frame_dump_set_geometry(unsigned base_width, unsigned base_height)
{
	size_t pixels;

	if (!frame_dump_enabled())
		return 0;
	if (!((base_width == 256 && base_height == 192) ||
	      (base_width == FRAME_WIDTH && base_height == FRAME_HEIGHT))) {
		failf("core AV geometry is %ux%u; expected Genesis reset 256x192 or "
		      "visible %ux%u", base_width, base_height, FRAME_WIDTH,
		      FRAME_HEIGHT);
		return -1;
	}
	if (!g_directory[0]) {
		failf("--dump-frames requires --dump-frames-dir");
		return -1;
	}
	if (ensure_directory() != 0)
		return -1;
	g_width = FRAME_WIDTH;
	g_height = FRAME_HEIGHT;
	pixels = (size_t)g_width * g_height;
	if (pixels / g_width != g_height || pixels > SIZE_MAX / 3) {
		failf("frame geometry is too large");
		return -1;
	}
	g_last_rgb = calloc(pixels, 3);
	if (!g_last_rgb) {
		failf("out of memory for %ux%u frame buffer", g_width, g_height);
		return -1;
	}
	return 0;
}

int frame_dump_accept_pixel_format(enum retro_pixel_format format)
{
	switch (format) {
	case RETRO_PIXEL_FORMAT_0RGB1555:
	case RETRO_PIXEL_FORMAT_XRGB8888:
	case RETRO_PIXEL_FORMAT_RGB565:
		g_pixel_format = format;
		return 1;
	default:
		if (frame_dump_enabled())
			failf("core requested unsupported pixel format %d", (int)format);
		return 0;
	}
}

enum retro_pixel_format frame_dump_negotiated_pixel_format(void)
{
	return g_pixel_format;
}

const char *frame_dump_pixel_format_name(enum retro_pixel_format format)
{
	switch (format) {
	case RETRO_PIXEL_FORMAT_0RGB1555: return "0RGB1555";
	case RETRO_PIXEL_FORMAT_XRGB8888: return "XRGB8888";
	case RETRO_PIXEL_FORMAT_RGB565: return "RGB565";
	default: return "UNKNOWN";
	}
}

static int requested_index(uint64_t frame)
{
	int i;
	for (i = 0; i < g_nrequests; i++)
		if (g_requests[i].frame == frame)
			return i;
	return -1;
}

static int convert_frame(const void *data, unsigned width, unsigned height,
	                        size_t pitch)
{
	const uint8_t *src = data;
	size_t source_bpp;
	size_t row_bytes;
	unsigned x, y;

	if (width != g_width || height != g_height) {
		failf("video callback geometry is %ux%u; AV base is %ux%u",
		      width, height, g_width, g_height);
		return -1;
	}
	source_bpp = g_pixel_format == RETRO_PIXEL_FORMAT_XRGB8888 ? 4 : 2;
	row_bytes = (size_t)width * source_bpp;
	if (pitch < row_bytes) {
		failf("video callback pitch %zu is shorter than %zu bytes", pitch,
		      row_bytes);
		return -1;
	}
	g_callback_width = width;
	g_callback_height = height;
	g_callback_pitch = pitch;
	g_callback_seen = 1;
	for (y = 0; y < height; y++) {
		const uint8_t *row = src + y * pitch;
		uint8_t *dst = g_last_rgb + (size_t)y * width * 3;
		for (x = 0; x < width; x++) {
			uint32_t pixel;
			uint8_t r, g, b;

			if (source_bpp == 2) {
				uint16_t p;
				memcpy(&p, row + x * 2, sizeof p);
				pixel = p;
				if (g_pixel_format == RETRO_PIXEL_FORMAT_RGB565) {
					r = (uint8_t)(((pixel >> 11) & 0x1F) * 255 / 31);
					g = (uint8_t)(((pixel >> 5) & 0x3F) * 255 / 63);
					b = (uint8_t)((pixel & 0x1F) * 255 / 31);
				} else {
					r = (uint8_t)(((pixel >> 10) & 0x1F) * 255 / 31);
					g = (uint8_t)(((pixel >> 5) & 0x1F) * 255 / 31);
					b = (uint8_t)((pixel & 0x1F) * 255 / 31);
				}
			} else {
				memcpy(&pixel, row + x * 4, sizeof pixel);
				r = (uint8_t)(pixel >> 16);
				g = (uint8_t)(pixel >> 8);
				b = (uint8_t)pixel;
			}
			dst[x * 3 + 0] = r;
			dst[x * 3 + 1] = g;
			dst[x * 3 + 2] = b;
		}
	}
	g_last_valid = 1;
	return 0;
}

static int write_request(int index)
{
	char path[FRAME_PATH_MAX];
	int n;

	n = snprintf(path, sizeof path, "%s/frame_%llu.png", g_directory,
	             (unsigned long long)g_requests[index].frame);
	if (n < 0 || (size_t)n >= sizeof path) {
		failf("frame output path is too long for frame %llu",
		      (unsigned long long)g_requests[index].frame);
		return -1;
	}
	if (write_png_rgb(path, g_width, g_height, g_last_rgb) != 0)
		return -1;
	g_requests[index].captured = 1;
	return 0;
}

void frame_dump_begin_frame(uint64_t frame)
{
	if (frame_dump_enabled() && frame == 0)
		failf("frame numbers are 1-based");
	g_current_frame = frame;
}

void frame_dump_video_refresh(const void *data, unsigned width,
	                              unsigned height, size_t pitch)
{
	int index;

	if (!frame_dump_enabled() || g_failed)
		return;
	if (!g_current_frame) {
		failf("video callback arrived outside retro_run() frame");
		return;
	}
	index = requested_index(g_current_frame);
	if (data) {
		if (width == g_width && height == g_height) {
			if (convert_frame(data, width, height, pitch) != 0)
				return;
		} else if (width == 256 && height == 192) {
			/* The core emits one reset-mode frame before the VDP registers
			 * select the retail 320x224 display. It is not a reference frame. */
			if (index >= 0) {
				failf("requested frame %llu was emitted in reset geometry 256x192",
				      (unsigned long long)g_current_frame);
				return;
			}
			return;
		} else {
			failf("video callback geometry is %ux%u; expected visible %ux%u",
			      width, height, g_width, g_height);
			return;
		}
	} else if (index >= 0) {
		if (!g_last_valid) {
			failf("frame %llu was a NULL video duplicate with no prior frame",
			      (unsigned long long)g_current_frame);
			return;
		}
	}
	if (index >= 0 && !g_requests[index].captured)
		write_request(index);
}

int frame_dump_failed(void)
{
	return g_failed;
}

const char *frame_dump_error(void)
{
	return g_error[0] ? g_error : "unknown frame capture error";
}

int frame_dump_finish(void)
{
	int i;

	if (g_failed)
		return -1;
	for (i = 0; i < g_nrequests; i++) {
		if (!g_requests[i].captured) {
			failf("requested frame %llu was not emitted by the core",
			      (unsigned long long)g_requests[i].frame);
			return -1;
		}
	}
	if (g_callback_seen)
		fprintf(stderr, "psiv_oracle: video callback=%ux%u pitch=%zu; "
		        "PNGs=320x224\n", g_callback_width, g_callback_height,
		        g_callback_pitch);
	return 0;
}

void frame_dump_shutdown(void)
{
	free(g_last_rgb);
	g_last_rgb = NULL;
}
