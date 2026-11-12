/* AK820 MAX Ultra v1.36 patch "live lighting", phase 1.
 *
 * New commands in the 0x3X range, which the stock firmware ignores:
 *   0x30  apply a 16-byte lighting block (same layout as 0x23) in RAM only
 *   0x31  re-apply the stored block from flash (0x9800), again RAM only
 *   0x3F  identify: the reply payload carries "AKLIVE" and the patch version
 *
 * This mirrors the stock apply routine at 0x1984 step by step, minus its
 * final tail call to 0x6418, which writes the settings record at 0x8200.
 * Nothing here calls the flash writer at 0x5CB8.
 *
 * No globals, no .data, no .bss: the code lives in free flash at 0x17400
 * and touches only the firmware's own RAM variables.
 */
#include <stdint.h>
#include "effects.h"

#define PATCH_VERSION 2

#define R8(a) (*(volatile uint8_t *)(a))

/* Firmware RAM variables used by the stock apply routine. */
#define MODE_FLAG    0x2000022Bu   /* cleared at the start of apply */
#define LIGHT_STATE  0x200001CCu   /* 8 bytes copied from the block */
#define DIR_FLAG     0x200001B4u
#define EFFECT_INDEX 0x200001A1u
#define BRIGHTNESS   0x200001B0u
#define COLOR_INDEX  0x200001AAu
#define SPEED        0x200001B2u
#define DIRTY        0x200001A2u

#define SAVED_BLOCK  0x00009800u   /* where 0x23 stores the block */

typedef void (*fn_t)(void);
#define CALL(addr) ((fn_t)((addr) | 1u))()

/* Reply status in payload byte 15 of the reply (offset 8 + 15). */
enum { OK = 0, BAD_EFFECT = 1, BAD_LEVEL = 2, BAD_DIRECTION = 3,
       BAD_SLOT = 4, BAD_PATTERN = 5, NOT_DEFINED = 6 };

static int check(const uint8_t *b)
{
    uint8_t effect = b[0];
    if (!(effect <= 0x13 || effect == 0x80))
        return BAD_EFFECT;
    if (b[9] > 5 || b[10] > 5)          /* brightness, speed */
        return BAD_LEVEL;
    if (b[11] > 3)                      /* direction */
        return BAD_DIRECTION;
    return OK;
}

/* The stock apply routine (0x1984) with the block taken from `b`. */
static void live_apply(const volatile uint8_t *b)
{
    volatile uint8_t *s = (volatile uint8_t *)LIGHT_STATE;

    R8(MODE_FLAG) = 0;
    s[0] = b[0];   /* effect */
    s[1] = b[1];   /* R */
    s[2] = b[2];   /* G */
    s[3] = b[3];   /* B */
    s[4] = b[8];   /* colour cycling */
    s[5] = b[9];   /* brightness */
    s[6] = b[10];  /* speed */
    s[7] = b[11];  /* direction */

    R8(DIR_FLAG) = (s[7] == 1 || s[7] == 2) ? 1 : 0;

    uint8_t e = s[0];
    if (e == 0x80)
        R8(EFFECT_INDEX) = 0x14;
    else if (e == 0)
        R8(EFFECT_INDEX) = 10;
    else if (e < 11)
        R8(EFFECT_INDEX) = (uint8_t)(e - 1);
    else
        R8(EFFECT_INDEX) = e;

    R8(BRIGHTNESS) = s[5];
    R8(COLOR_INDEX) = 0xFF;
    CALL(0x7C84);
    CALL(0xE528);
    R8(SPEED) = s[6];
    R8(DIRTY) = 1;
    CALL(0xAF8C);
    /* stock continues with `b.w 0x6418` (flash write): deliberately omitted */
}

/* Redraws the lighting currently held in RAM (whatever 0x23, 0x30 or the
 * keyboard last applied), so the stock renderer takes over cleanly after
 * an effect. No flash. */
void ak_reapply_current(void)
{
    const volatile uint8_t *s = (const volatile uint8_t *)LIGHT_STATE;
    uint8_t b[12];             /* live_apply reads bytes 0-3 and 8-11 */
    b[4] = b[5] = b[6] = b[7] = 0;
    b[0] = s[0]; b[1] = s[1]; b[2] = s[2]; b[3] = s[3];
    b[8] = s[4]; b[9] = s[5]; b[10] = s[6]; b[11] = s[7];
    live_apply(b);
}

static int define_effect(const uint8_t *p)
{
    uint8_t slot = p[0], pattern = p[1], speed = p[2], bright = p[3], n = p[4];
    if (slot < 1 || slot > AK_SLOTS)
        return BAD_SLOT;
    if (pattern > AK_SPARKLE)
        return BAD_PATTERN;
    if (speed < 1 || speed > 10 || bright < 1 || bright > 100 || n < 1 || n > 4)
        return BAD_LEVEL;
    struct ak_state *s = ak_state();
    struct ak_slot *e = &s->slot[slot - 1];
    if (s->active == slot)       /* redefining the playing effect: stop first */
        s->active = 0;
    e->pattern = pattern;
    e->speed = speed;
    e->brightness = bright;
    e->ncolors = n;
    for (int i = 0; i < 12; i++)
        e->rgb[i] = p[5 + i];
    e->duration10 = (uint16_t)(p[17] | p[18] << 8);
    e->valid = 1;
    return OK;
}

/* Called from the hook stubs for any 0x3X command.
 * frame: the received 32-byte frame, payload at +8.
 * reply: the reply frame, already a copy of `frame` with byte 0 = 0x55. */
void ak_cmd3x(uint32_t cmd, const uint8_t *frame, uint8_t *reply)
{
    uint8_t status = OK;

    switch (cmd & 0xFF) {
    case 0x30:
        status = (uint8_t)check(frame + 8);
        if (status == OK)
            live_apply(frame + 8);
        break;
    case 0x31:
        if (check((const uint8_t *)SAVED_BLOCK) == OK)
            live_apply((const volatile uint8_t *)SAVED_BLOCK);
        else
            status = BAD_EFFECT;
        break;
    case 0x38:   /* define: slot, pattern, speed, brightness, n, rgb x4, dur10 LE */
        status = (uint8_t)define_effect(frame + 8);
        break;
    case 0x39: { /* play: slot */
        struct ak_state *s = ak_state();
        uint8_t slot = frame[8];
        if (slot < 1 || slot > AK_SLOTS)
            status = BAD_SLOT;
        else if (!s->slot[slot - 1].valid)
            status = NOT_DEFINED;
        else {
            s->start_frame = s->frame;
            s->active = slot;
        }
        break;
    }
    case 0x3A: { /* stop */
        struct ak_state *s = ak_state();
        if (s->active) {
            s->active = 0;
            ak_reapply_current();
        }
        break;
    }
    case 0x3B: { /* frame period in microseconds, u32 LE; 1000..100000 */
        uint32_t us = frame[8] | frame[9] << 8 | frame[10] << 16 | (uint32_t)frame[11] << 24;
        if (us < 1000 || us > 100000)
            status = BAD_LEVEL;
        else
            ak_state()->frame_us = us;
        break;
    }
    case 0x3F: {
        static const char sig[6] = {'A', 'K', 'L', 'I', 'V', 'E'};
        struct ak_state *s = ak_state();
        for (int i = 0; i < 6; i++)
            reply[8 + i] = (uint8_t)sig[i];
        reply[14] = PATCH_VERSION;
        /* RAM self-test in the engine's state area. */
        volatile uint8_t *probe = s->pad;
        probe[0] = 0xA5; probe[1] = 0x5A;
        reply[15] = (probe[0] == 0xA5 && probe[1] == 0x5A) ? 1 : 0;
        for (int i = 0; i < 4; i++)
            reply[16 + i] = (uint8_t)(s->frame >> (8 * i));
        reply[20] = s->active;
        return;   /* payload byte 15 is the self-test, not a status */
    }
    default:
        return;   /* other 0x3X: plain echo, as stock */
    }
    reply[8 + 15] = status;
}
