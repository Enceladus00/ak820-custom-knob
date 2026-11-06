# Probe findings: AJAZZ AK820 MAX knob

Measured with `knob-probe` on 2026-11-04 (Windows 10, **2.4 GHz dongle**).
Wired USB and Bluetooth are not measured yet; they will likely show up with
other IDs and may have different timing.

## Device

`HFD AJAZZ AK820MAX`, VID `0C45` (Sonix), PID `FEFC` (the 2.4 GHz dongle).

| Interface | Collection | Usage page / usage | Readable by apps |
|---|---|---|---|
| MI_00 | | 0x01 / 0x02 Mouse | no (OS-owned) |
| MI_01 | COL01, COL02 | 0x01 / 0x06 Keyboard | no (OS-owned) |
| MI_01 | COL03 | 0x0C / 0x01 Consumer Control | **yes** |
| MI_01 | COL04 | 0x01 / 0x80 System Control | yes |
| MI_02 | | 0xFF80 / 0x01 Vendor | yes |
| MI_03 | | 0xFF70 / 0x71 Vendor | yes |

## What the knob sends

All knob events arrive as 3-byte consumer reports on **MI_01 COL03**
(`[report id 03][usage lo][usage hi]`), followed by a release `03 00 00`:

| Action | Report | Windows key | Held (press → release) |
|---|---|---|---|
| Knob clockwise, 1 detent | `03 E9 00` | VK_VOLUME_UP | **2.9 – 6.1 ms** |
| Knob counter-clockwise | `03 EA 00` | VK_VOLUME_DOWN | **2.9 – 6.1 ms** |
| Knob click | `03 E2 00` | VK_VOLUME_MUTE | 134 – 180 ms (finger) |
| Fn+F3 | `03 E9 00` | VK_VOLUME_UP | 77 – 89 ms |
| Fn+F2 | `03 EA 00` | VK_VOLUME_DOWN | 91 – 113 ms |
| Fn+F4 (mute) | `03 E2 00` | VK_VOLUME_MUTE | 101 – 106 ms |

Fast spin: detents arrive 8 – 38 ms apart; each one is still released within ~5 ms.

Nothing was sent on the vendor interfaces (MI_02, MI_03) during knob use.

## Conclusions

1. **Bytes and interface are identical** for the knob and Fn+F2/F3/F4.
   The difference is in **timing**: a knob detent is released after ~5 ms,
   a finger press after 75 ms or more. A threshold around 30 ms separates
   rotation from Fn+F2/F3 with a wide margin on both sides.
2. **Knob click and Fn+F4 can't be told apart** (both are finger presses;
   see the timing test below).
3. **Fn has no effect on the knob**: Fn+rotate and Fn+click send the same
   reports as without Fn. Fn layers are not possible.
4. **Ctrl / Alt / Shift work as layers**: the modifier is visible in the
   hook (and Alt is even flagged on the volume event itself).
5. **The keyboard hook can't see the device**: Windows turns consumer
   reports into *injected* key events (`LLKHF_INJECTED`, `dwExtraInfo = 0`,
   Raw Input device handle = none). The device is only visible in the HID
   report itself (Raw Input `RIM_TYPEHID`, or hidapi on COL03), which arrives
   ~0.2 – 0.5 ms *before* the hook event.

## Resulting detection strategy

On a volume key in the hook (not our own injection):

1. Swallow it and wait for the release, at most ~30 ms.
2. Check whether a matching consumer report came from VID_0C45&PID_FEFC COL03.
3. Decide:
   - AK820 + released within 30 ms → **knob rotation** → run the mode's action.
   - AK820 + still held at 30 ms → **Fn+F2/F3** → replay the volume key
     (marked as ours). Could later become its own configurable trigger.
   - Not from the AK820 → replay unchanged (other keyboards keep working).
4. Mute: AK820 mute = **knob click** (shared with Fn+F4, documented limitation).
   The press duration gives long-press detection for free.

## Knob press vs Fn+F4 (2026-11-06)

`knob-probe mute`: ten presses per row, on the 2.4 GHz dongle. Both send
`03 E2 00` on `MI_01 COL03`; only the hold time differs.

| Press | Hold times (ms) |
|---|---|
| Knob, as quick as possible | 73 – 131 (one 278) |
| Fn+F4, as quick as possible | 47 – 76 |
| Knob, normal | 130 – 227 |
| Fn+F4, normal | 90 – 162 |

Fn+F4 is shorter on average, but the ranges overlap (a quick knob tap at
73 ms vs a normal Fn+F4 at 90 – 162 ms), so no hold-time threshold separates
them reliably. The limitation stays.

Pressing the knob sometimes nudges it by one detent (a 3 – 5 ms volume step
just before or after the press): 3 times in 20 presses.

**Fn+F1 is not mute** on the AK820 MAX: it sends Media Select (`03 83 01`,
VK_LAUNCH_MEDIA, opens the media app). The program doesn't touch it. The
mute row in the first table was recorded with Fn+F4.
