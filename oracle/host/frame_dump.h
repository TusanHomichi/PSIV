#ifndef PSIV_ORACLE_FRAME_DUMP_H
#define PSIV_ORACLE_FRAME_DUMP_H

#include <stddef.h>
#include <stdint.h>

#include "libretro.h"

/* Configure an explicit, 1-based list of retro_run() frame numbers and the
 * directory in which frame_N.png files are written. */
int frame_dump_set_frames(const char *list);
int frame_dump_set_directory(const char *directory);
int frame_dump_enabled(void);

/* The host supplies the core's post-load AV geometry. Genesis Plus GX starts
 * at its reset-time 256x192 mode and switches to the retail 320x224 viewport
 * on the first VDP frame; only the latter is accepted by the callback. */
int frame_dump_set_geometry(unsigned base_width, unsigned base_height);

/* Return 1 for every packed software format the encoder can decode. */
int frame_dump_accept_pixel_format(enum retro_pixel_format format);
enum retro_pixel_format frame_dump_negotiated_pixel_format(void);
const char *frame_dump_pixel_format_name(enum retro_pixel_format format);

/* Called immediately before retro_run() and from the libretro video hook. */
void frame_dump_begin_frame(uint64_t frame);
void frame_dump_video_refresh(const void *data, unsigned width,
                              unsigned height, size_t pitch);

int frame_dump_failed(void);
const char *frame_dump_error(void);
int frame_dump_finish(void);
void frame_dump_shutdown(void);

#endif
