#ifndef AK_EFFECTS_H
#define AK_EFFECTS_H
#include <stdint.h>

#define AK_STATE_ADDR 0x20006E00u   /* 512 bytes up to 0x20007000 */
#define AK_MAGIC 0x564C4B41u        /* "AKLV" */
#define AK_SLOTS 8
#define AK_DEFAULT_FRAME_US 16000u  /* until measured; settable with 0x3B */

enum { AK_SOLID = 0, AK_PULSE = 1, AK_WAVE = 2, AK_FLICKER = 3, AK_SPARKLE = 4 };

struct ak_slot {
    uint8_t valid;
    uint8_t pattern;
    uint8_t speed;       /* 1..10 */
    uint8_t brightness;  /* 1..100 */
    uint8_t ncolors;     /* 1..4 */
    uint8_t rgb[12];
    uint8_t pad;
    uint16_t duration10; /* 10 ms units, 0 = until stopped */
};

struct ak_state {
    uint32_t magic;
    uint32_t frame;
    uint32_t start_frame;
    uint32_t frame_us;
    uint8_t active;          /* 0 = none, else slot number */
    uint8_t restore_pending; /* an effect ended by itself: redraw stock lighting */
    uint8_t pad[2];
    struct ak_slot slot[AK_SLOTS];
};

_Static_assert(sizeof(struct ak_state) <= 512, "state must fit 512 bytes");

struct ak_state *ak_state(void);
void ak_frame(void);
void ak_reapply_current(void);   /* in live.c: redraw the lighting held in RAM */

#endif
