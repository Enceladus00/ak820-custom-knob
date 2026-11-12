/* LIVE v2: on-keyboard effect engine. See docs/effects-format.md.
 *
 * Effects are defined in RAM (0x38), started (0x39) and stopped (0x3A)
 * with one command each. Rendering hooks the frame loop's LED push
 * (0x1054E: bl 0x14B90): ak_frame() draws the active effect into the
 * firmware's R/G/B arrays, then calls the original push. Nothing here
 * writes flash.
 *
 * State: 512 bytes at 0x20006E00, above the initial stack pointer
 * (0x20006D98), which nothing in the firmware reaches. Integer maths only.
 */
#include <stdint.h>
#include "effects.h"

#define LED_R   ((volatile uint8_t *)0x20004984u)
#define LED_G   ((volatile uint8_t *)0x20004A02u)
#define LED_B   ((volatile uint8_t *)0x20004A80u)
#define KEY_LED ((const uint8_t *)0x00015B8Au)   /* key k -> LED index, 82 entries */
#define KEYS    82

typedef void (*fn_t)(void);
#define LED_PUSH() ((fn_t)(0x14B90u | 1u))()

static const uint8_t ROW_LEN[6] = {14, 15, 15, 14, 14, 10};

/* (1 - cos(2*pi*i/64)) / 2 * 255, for pulse. */
static const uint8_t HALF_COS[64] = {
    0, 1, 2, 5, 10, 15, 21, 28, 37, 46, 56, 67, 78, 90, 102, 115,
    127, 140, 153, 165, 177, 188, 199, 209, 218, 227, 234, 240, 245, 250, 253, 254,
    255, 254, 253, 250, 245, 240, 234, 227, 218, 209, 199, 188, 177, 165, 153, 140,
    128, 115, 102, 90, 78, 67, 56, 46, 37, 28, 21, 15, 10, 5, 2, 1};

struct ak_state *ak_state(void)
{
    struct ak_state *s = (struct ak_state *)AK_STATE_ADDR;
    if (s->magic != AK_MAGIC) {
        volatile uint32_t *w = (volatile uint32_t *)s;
        for (unsigned i = 0; i < sizeof(*s) / 4; i++)
            w[i] = 0;
        s->magic = AK_MAGIC;
        s->frame_us = AK_DEFAULT_FRAME_US;
    }
    return s;
}

static uint8_t hash8(uint32_t k, uint32_t n)
{
    uint32_t x = k * 0x9E3779B1u ^ n * 0x85EBCA77u;
    x ^= x >> 15;
    x *= 0x2C1B3C6Du;
    x ^= x >> 12;
    return (uint8_t)(x >> 8);
}

static uint8_t mix8(uint8_t a, uint8_t b, uint32_t f /* 0..256 */)
{
    return (uint8_t)((a * (256 - f) + b * f) >> 8);
}

static void mix(const uint8_t *a, const uint8_t *b, uint32_t f, uint8_t *out)
{
    for (int i = 0; i < 3; i++)
        out[i] = mix8(a[i], b[i], f);
}

/* u: 0..65535 across the palette. loop: wraps from last colour to first. */
static void palette(const struct ak_slot *e, uint32_t u, int loop, uint8_t *out)
{
    uint32_t n = e->ncolors;
    const uint8_t *c = e->rgb;
    if (n <= 1) {
        out[0] = c[0]; out[1] = c[1]; out[2] = c[2];
        return;
    }
    uint32_t segs = loop ? n : n - 1;
    uint32_t pos = (u & 0xFFFF) * segs;          /* 16.16 */
    uint32_t i = pos >> 16, f = (pos & 0xFFFF) >> 8;
    uint32_t j = i + 1 >= n ? 0 : i + 1;
    mix(&c[i * 3], &c[j * 3], f, out);
}

static void render_key(const struct ak_slot *e, uint32_t k, uint32_t x16,
                       uint32_t t, uint32_t period, uint8_t *out)
{
    static const uint8_t black[3] = {0, 0, 0}, white[3] = {255, 255, 255};
    switch (e->pattern) {
    case AK_SOLID:
    default:
        out[0] = e->rgb[0]; out[1] = e->rgb[1]; out[2] = e->rgb[2];
        break;
    case AK_PULSE: {
        uint32_t f = HALF_COS[((t % period) * 64) / period];
        const uint8_t *low = e->ncolors > 1 ? &e->rgb[3] : black;
        mix(low, e->rgb, f + (f >> 7), out);
        break;
    }
    case AK_WAVE:
        palette(e, x16 - (((t % period) << 16) / period), 1, out);
        break;
    case AK_FLICKER: {
        uint32_t roll = period / 8 ? period / 8 : 1;
        uint32_t n = t / roll, f = ((t % roll) << 8) / roll;
        uint32_t v = mix8(hash8(k, n), hash8(k, n + 1), f);
        palette(e, v << 8, 0, out);
        uint32_t level = 140 + (115 * v) / 255;   /* 0.55 + 0.45 v */
        for (int i = 0; i < 3; i++)
            out[i] = (uint8_t)((out[i] * level) >> 8);
        break;
    }
    case AK_SPARKLE: {
        uint32_t w = period / 4 ? period / 4 : 1;
        uint32_t n = t / w;
        uint8_t h = hash8(k, n);
        out[0] = e->rgb[0]; out[1] = e->rgb[1]; out[2] = e->rgb[2];
        if ((uint32_t)(h % 100) < (uint32_t)e->speed * 2) {
            const uint8_t *fl = white;
            if (e->ncolors > 1)
                fl = &e->rgb[3 * (1 + hash8(k + 97, n) % (e->ncolors - 1))];
            uint32_t fade = 256 - ((t % w) << 8) / w;
            mix(e->rgb, fl, fade, out);
        }
        break;
    }
    }
}

/* Called instead of the LED push, once per LED frame. */
void ak_frame(void)
{
    struct ak_state *s = ak_state();
    s->frame++;
    if (s->restore_pending) {
        s->restore_pending = 0;
        ak_reapply_current();
    }
    if (s->active >= 1 && s->active <= AK_SLOTS) {
        const struct ak_slot *e = &s->slot[s->active - 1];
        /* ms since start; 32-bit safe for days at any sane frame period */
        uint32_t t = ((s->frame - s->start_frame) * (s->frame_us / 100u)) / 10u;
        if (e->duration10 && t >= (uint32_t)e->duration10 * 10u) {
            s->active = 0;
            s->restore_pending = 1;
        } else if (e->valid) {
            uint32_t period = 4000u / (e->speed ? e->speed : 1);
            uint32_t k = 0;
            for (uint32_t row = 0; row < 6; row++) {
                uint32_t len = ROW_LEN[row];
                for (uint32_t p = 0; p < len; p++, k++) {
                    uint8_t c[3];
                    uint32_t x16 = len > 1 ? (p * 65535u) / (len - 1) : 0;
                    render_key(e, k, x16, t, period, c);
                    uint32_t led = KEY_LED[k];
                    LED_R[led] = (uint8_t)((c[0] * e->brightness) / 100);
                    LED_G[led] = (uint8_t)((c[1] * e->brightness) / 100);
                    LED_B[led] = (uint8_t)((c[2] * e->brightness) / 100);
                }
            }
        }
    }
    LED_PUSH();
}
