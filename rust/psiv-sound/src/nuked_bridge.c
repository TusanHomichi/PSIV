#include <stdint.h>
#include <stdlib.h>

#include "ym3438.h"

typedef struct {
    ym3438_t chip;
    int16_t cycles[24][2];
    int16_t previous[2];
    int16_t current[2];
    double source_step;
    double source_phase;
    int initialized;
} PsivYm;

#define PSIV_YM_NATIVE_RATE (7670454.0 / 144.0)

static int16_t clamp_sample(int32_t sample) {
    if (sample > 32767) return 32767;
    if (sample < -32768) return -32768;
    return (int16_t)sample;
}

static void native_sample(PsivYm *ym, int16_t *out) {
    int32_t left = 0;
    int32_t right = 0;
    for (unsigned int i = 0; i < 24; ++i) {
        OPN2_Clock(&ym->chip, ym->cycles[i]);
        left += ym->cycles[i][0];
        right += ym->cycles[i][1];
    }
    out[0] = clamp_sample(left * 11);
    out[1] = clamp_sample(right * 11);
}

static void clock_once(PsivYm *ym) {
    int16_t ignored[2];
    OPN2_Clock(&ym->chip, ignored);
}

PsivYm *psiv_ym_new(uint32_t sample_rate) {
    PsivYm *ym = (PsivYm *)calloc(1, sizeof(PsivYm));
    if (ym != NULL) {
        if (sample_rate == 0) sample_rate = 44100;
        ym->source_step = PSIV_YM_NATIVE_RATE / (double)sample_rate;
        OPN2_SetChipType(ym3438_mode_ym2612);
        OPN2_Reset(&ym->chip);
    }
    return ym;
}

void psiv_ym_free(PsivYm *ym) {
    free(ym);
}

void psiv_ym_reset(PsivYm *ym) {
    OPN2_Reset(&ym->chip);
    for (unsigned int i = 0; i < 24; ++i) {
        ym->cycles[i][0] = 0;
        ym->cycles[i][1] = 0;
    }
    ym->previous[0] = 0;
    ym->previous[1] = 0;
    ym->current[0] = 0;
    ym->current[1] = 0;
    ym->source_phase = 0.0;
    ym->initialized = 0;
}

void psiv_ym_write_register(PsivYm *ym, uint8_t port, uint8_t reg, uint8_t value) {
    const uint32_t address_port = (uint32_t)(port & 1u) * 2u;
    OPN2_Write(&ym->chip, address_port, reg);
    clock_once(ym);
    OPN2_Write(&ym->chip, address_port + 1u, value);
    clock_once(ym);
}

void psiv_ym_render_sample(PsivYm *ym, int16_t *out) {
    if (!ym->initialized) {
        native_sample(ym, ym->previous);
        native_sample(ym, ym->current);
        ym->initialized = 1;
    }

    const double fraction = ym->source_phase;
    const double left = (double)ym->previous[0] +
                        ((double)ym->current[0] - (double)ym->previous[0]) * fraction;
    const double right = (double)ym->previous[1] +
                         ((double)ym->current[1] - (double)ym->previous[1]) * fraction;
    out[0] = clamp_sample((int32_t)left);
    out[1] = clamp_sample((int32_t)right);

    ym->source_phase += ym->source_step;
    while (ym->source_phase >= 1.0) {
        ym->previous[0] = ym->current[0];
        ym->previous[1] = ym->current[1];
        native_sample(ym, ym->current);
        ym->source_phase -= 1.0;
    }
}
