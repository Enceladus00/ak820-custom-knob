# RGB lighting research

Goal: control the AK820 MAX lighting from this program (effects, colours,
brightness, per-key colours), and later tie it to knob modes and apps.

Status (2026-11-09): the protocol is known from the vendor's own web driver
and implemented in `crates/knob-rgb`. **Reads are confirmed on the keyboard**
(firmware 1.36, battery 58 %).

Write results by read-back (command `0x23`), on the real keyboard:

| Change | Read back |
|---|---|
| Brightness 1 and 5 | Matches |
| Effect 7 (breathing) | Matches |
| Effect 11 (Wave) | Matches |
| Colour, colour cycling off | Not reported, but **seen working** on the keys (red, `#66CCFF`) |
| Effect 0 (off) | **Reads as effect `0B`**; the user believes the lights went off |
| Restoring the original block | Matches |

The keyboard accepts bytes 1-4, 8 and 14-15 on a write, but read-back doesn't
show them changing, so verification ignores them. A colour change can only be
confirmed by looking at the keyboard.

The first real read of the lighting block was
`01 FF FF FF 00 00 00 00 01 03 03 00 00 00 00 00`: effect 1, white, colour
cycling on, brightness 3, speed 3. Those fields fit the layout below, but
bytes 4 and 14-15 are zero, where the driver writes 255 and the `AA 55`
marker. Those bytes stay zero after a write too.

## Which software speaks to this keyboard

The dongle is `0C45:FEFC`, product string `HFD AJAZZ AK820MAX`.

| Software | Works with `0C45:FEFC`? | Notes |
|---|---|---|
| AJAZZ `OemDrv.exe` (installed) | **No** | Looks only for `258A:010C` and `3554:FA09`, a different chip. Logged run: finds 0 devices, sends nothing. |
| [uzuw's Linux configurator](https://github.com/uzuw/ajazz-ak820-max-software-for-linux) | No | Same `258A`/`3554` hardware, despite the same model name. |
| AULA's web driver, `hev.aulacn.com` | **Yes** | Lists `productId 65276` as `AJAZZ AK820MAX`, on usage page `0xFF80`. The source of everything below. |
| [HaloBattery `aula.py`](https://github.com/ahmedkhursheed23/HaloBattery/blob/main/providers/aula.py) | Yes | Battery only, confirmed on an AULA F87 Pro V2C using the same dongle ID. |

Two AK820 MAX versions are sold under the same name, so advice and tools for
one often don't apply to the other.

The web driver's `FEFC` entry is marked as a magnetic-switch model. The dongle
ID is shared across many HFD keyboards, so the lighting block has to be
checked against the real keyboard before it is trusted; see
[Next steps](#next-steps).

## Protocol

Interface MI_02, usage page `0xFF80`, usage `0x01`. Output and input reports
are 32 bytes after report id 0, and there are no feature reports.

### Frames

```
byte   0    1    2      3-4         5   6      7   8...
       AA   cmd  chunk  offset LE   0   last   0   payload
```

* Payloads are split into 24-byte chunks.
* `chunk` is 24 except on the short tail of a transfer of two or more frames.
  A single short frame still claims 24.
* `last` is 1 on the final frame.
* The keyboard acknowledges each frame with `55 cmd ...`, payload at byte 8.
* Reads use the same framing with a zero payload the size of the block.
* Pushed `55 37 ...` frames are status updates, not acknowledgements.
* The receiver pushes `55 FC 04` when the keyboard isn't linked, for example
  asleep or switched off; the web driver treats it as a lost connection, and
  `55 FC 05` as a cue to reload. Seen on this dongle while the keyboard slept.

### Lighting block (read `0x13`, write `0x23`, 16 bytes)

| Offset | Meaning |
|---|---|
| 0 | Effect; 0 is off |
| 1-3 | Colour R, G, B |
| 4 | Always 255 in the driver |
| 5-7 | Secondary colour |
| 8 | 1 cycles colours, 0 uses the colour above |
| 9 | Brightness, 1-5 |
| 10 | Speed, 1-5 |
| 11 | Direction |
| 12 | Gear mode (some models) |
| 14-15 | Marker `AA 55` |

Effect values, translated from the driver's Chinese names:

| Value | Effect | Value | Effect |
|---|---|---|---|
| `01` | Static | `0B` | Wave |
| `02` | Light up on press | `0C` | Winding path |
| `03` | Go dark on press | `0D` | Burst on press |
| `04` | Drifting | `0E` | Double strike |
| `05` | Snowfall | `0F` | Ripple |
| `06` | Blossoms | `10` | Stream |
| `07` | Breathing | `11` | Layered peaks |
| `08` | Spectrum cycle | `12` | Slanting rain |
| `09` | Fountain | `13` | Shuttle |
| `0A` | Colour crossing | `80` | Per-key custom |

### Device information (read `0x10`, 32 bytes)

Firmware is BCD at bytes 8-9; battery percent at byte 17. A sleeping or
switched-off keyboard doesn't answer at all, so silence means offline.

### Commands that must never be sent

These exist in the same protocol. `knob-rgb` has no way of encoding them.

| Frame | Effect |
|---|---|
| `AA 0F FF` | Factory reset |
| `AA 0F 05` | Reset the key map |
| `AA 41 ...`, `AA 42 ...` | Write flash (images, frames) |
| `AA 50 ...` | Write the screen's flash |
| `AA 42 89 5A FF 71 62 CC` (feature report) | [Reboots into the bootloader](https://github.com/fpb/ajazz-ak820-pro/issues/5) on an AK820 Pro |

## Tools

```
knob-probe hid-info
knob-probe rgb-read [--wait <seconds>]
knob-probe rgb-set [--effect N] [--color RRGGBB] [--brightness N] [--speed N] [--apply]
knob-probe rgb-set --restore <32 hex digits> --apply
```

`rgb-read` only reads. `rgb-set` reads the current state, prints a backup, and
without `--apply` shows what it would send and stops. With `--apply` it writes,
reads back and compares. All output is saved in `probe-logs\`.

## Next steps

1. Wake the keyboard and run `rgb-read`. Expect the marker `AA 55` at offset
   14-15 and values matching the current lighting. Anything else means this
   keyboard's block differs, and writing must not go ahead.
2. Write one visible change with `rgb-set --brightness`, check the keyboard,
   and restore it from the printed backup.
3. Try one effect and one colour.
4. Find out whether writes survive a power cycle. The driver writes on every
   change, but repeated writes may wear flash, so frequent automatic changes
   need checking first.
5. Layouts in Settings are done: `[[lighting]]` in the config, applied only
   when the user clicks Apply. Still to do is a `lighting` entry per mode,
   applied on mode switch, which depends on step 4. Effect 0 isn't offered
   until "off" is understood.
6. Per-key colours use command `0x24` with `index R G B` entries. Not
   implemented yet.

## Ideas this opens up

* **Mode colour:** the keyboard shows which knob mode is active.
* **Knob as a lighting control:** a mode whose turn changes brightness, using
  an action like any other.
* **Battery level in the tray:** from the information block, with no lighting
  writes involved. The smallest useful feature here.
* **Per-app lighting:** follows the existing per-app rules.
