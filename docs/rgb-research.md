# RGB lighting research

Goal: control the AK820 MAX Ultra lighting from this program (effects, colours,
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
| AULA's web driver, `hev.aulacn.com` | **No** (refuses it) | Its code bundle lists `productId 65276` as `AJAZZ AK820MAX` in the generic and AJAZZ lists, on usage page `0xFF80`, and is the source of everything below; the AULA site itself only uses its AULA list and answers "This device is not supported!!". |
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
knob-probe rgb-bench [--count N]
knob-probe rgb-bench --reads
knob-probe rgb-stream [--wired] [--seconds N]
```

`rgb-bench` times colour-only writes (default 30, at most 200), restores the
original block, and prints the steps of the persistence test; every write is
a stored write, so keep runs rare. `--reads` times reads only and changes
nothing. `rgb-stream` sends the driver's live per-key stream (`0x35`, or
`0x32` over the cable with `--wired`) and stores nothing; on this keyboard
it shows nothing either.

Sound-reactive and game-driven lighting were built on top of `0x23` and then
parked on the `live-lighting` branch: they would write several times a
second, and each write is saved to flash. This project never risks wearing
out the keyboard.

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
   **Result (2026-11-12, firmware 1.36, 2.4 GHz):** writes survive. Brightness
   set to 2, keyboard switched off and on, read back 2. A `0x23` write takes
   29.8–31.9 ms (median 30.8, about 33 per second), steady from one write to
   the next. So the lighting block is stored, most likely in flash, and
   streaming colours through `0x23` should be treated as wearing it.
   Unknowns: whether a colour-only write is stored too (the read-back never
   shows colour, so this can't be tested), and whether per-key `0x24` writes
   behave differently.
5. Layouts in Settings are done: `[[lighting]]` in the config, applied only
   when the user clicks Apply. Still to do is a `lighting` entry per mode,
   applied on mode switch, which depends on step 4. Effect 0 isn't offered
   until "off" is understood.
6. Per-key colours use command `0x24` with `index R G B` entries. Not
   implemented. The driver sends it with the key map, lighting and macros
   when a profile is saved, so it is most likely stored like `0x23`.
7. **Live per-key stream, `0x35`** (found 2026-11-12, not yet tried on the
   keyboard). The driver's "music" tab (`musicVersion: 2` on product 65278)
   computes every key's colour itself and sends a frame about every 50 ms,
   with no start command. On 32-byte receivers each frame is 6 packets:
   `AA 35 <packet 0-5> 00` then 28 bytes, 14 keys of RGB565 each, high byte
   first, 12 ms apart, without waiting for replies. Keys follow the driver's
   fixed 82-key order (`knob_rgb::KEYS`); the last two slots are padding.
   64-byte receivers use `0x32` with `index R G B` entries instead.
   **Model caution:** this keyboard is the AK820 MAX **Ultra**. The driver
   has no entry with that name: about a dozen entries are called "AK820MAX"
   or "AJAZZ AK820MAX" under different product IDs, two of them on this
   receiver's 65276 (one with music, one without), and nothing here shows
   which one the driver would pick for the Ultra. The key order used comes
   from the 65276 music entry (`keys: kt()`) and is unconfirmed for the
   Ultra. 65278 is a different receiver.
   **Tried 2026-11-12 over 2.4 GHz: nothing shows.** `rgb-stream` sent 15
   frames/s; every packet was answered with `55 35`, echoing the data, and
   a write took about 0.4 ms. The keys didn't change, in Static (`0x01`) or
   in Custom. Writing effect `0x80` reads back as `0x14` and turns the keys
   off (empty per-key table); the stream didn't show there either. The echo
   may come from the receiver alone, without reaching the keyboard. Untried:
   the USB cable, and a music mode switched on from the keyboard itself.
   **Why, most likely:** the driver bundle keeps one device list per brand
   site (`wr(supName)`). The 65276 "AJAZZ AK820MAX" entry with music is in
   the generic list (`""`); AJAZZ's own list (`ajazz`) has the same entry
   with **no `musicVersion` and no key list**, so AJAZZ doesn't offer live
   colours for this receiver. The AULA site (`hev.aulacn.com`) uses
   `supName: "aula"`, whose list has no entry for 65276, and answers
   "This device is not supported!!" when connecting. Conclusion: no live
   per-key stream on this keyboard over 2.4 GHz is known; every colour
   change goes through the stored `0x23` block.
   **USB cable, also tried 2026-11-12: nothing shows either.** Wired, the
   keyboard is `0C45:80A0` "KEYBOARD AK820MAX"; its vendor interface is
   `MI_02` (usage page `0xFF68`, usage `0x61`, 64-byte reports). `MI_03`
   (`0xFF67`, 4 KB output) is the bulk flash/image channel, never to be
   used. `rgb-stream --wired` sent the driver's `0x32` stream (`[index R G
   B]` per key, 56 bytes per packet, 6 packets) at 20 frames/s for 15 s and
   again for 5 s: every packet was answered with `55 32` echoing the data,
   in about 4 ms, and the keys stayed cyan throughout. AJAZZ's list has the
   wired 32928 entry with no music either.
   **Timing:** a `0x13` read over 2.4 GHz takes 3 ms (2.7-5.0, `rgb-bench
   --reads`), a `0x23` write 31 ms, so each write costs about 28 ms of work
   on the keyboard: consistent with a flash save on every write.

## Firmware analysis (v1.36)

On 2026-11-12 the firmware was read statically in Ghidra 12.2. The official
`AK820MAX Ultra Upgrade.exe` from ajazzdriver.com is a rebranded Sonix ISP
tool. It carries the image unencrypted as RCDATA resource 4000: 516,096 bytes
for a Sonix **SN34F280** (Cortex-M3/M4 class). Its USB descriptor is
`0C45:80A0`, bcdDevice `0x0136`, so it is exactly this keyboard's firmware.
The updater was never run, and nothing was sent to the keyboard. The firmware
image itself isn't in this repo.

### Command dispatch

Two near-identical handlers, one per transport (2.4 GHz at `0x0000ea90`,
wired at `0x0000f940`), check for `AA` in byte 0. They then copy the frame
into the reply, set byte 0 to `55`, and act on the high nibble of the command:

| Group | Commands | What it does |
|---|---|---|
| `0x1X` | `10`–`18` | Read. Copies from a flash page into the reply (`13` is built from RAM). |
| `0x2X` | `21`–`28` | Write the payload to a **fixed** flash page (see below). |
| `0x6X` | `64`–`67` | Magnetic-switch calibration flags (RAM). `65` ends calibration and **writes** `0x8400`/`0x8500`. |
| `0xXF` | sub-code 1, 2, 4, 5, `FF` | Resets: erase groups of settings pages; `FF` erases all of them. |
| anything else | `3X`, `4X`, `5X`, … | **Not handled.** The frame is still echoed back as `55 cmd …`. |

Read and write pages: `10` → `0x9000` (device info), `11`/`21` → `0x9200`,
`12`/`22` → `0x9600`, `13`/`23` → `0x9800` (lighting block), `14`/`24` →
`0x9A00` (per-key colours), `15`/`25` → `0x9C00`, `16`/`26` → `0xB000`,
`17`/`27` → `0xB600`, `18`/`28` → `0xB200`.

### What this settles

* **Every write is a flash erase.** `FUN_00005cb8` copies the 512-byte
  page to RAM, erases it, patches in the new bytes and programs it back.
  The address is always the same: no wear levelling, and no "skip if
  unchanged" check.
* **A `0x23` costs two erase cycles.** After writing `0x9800`, the apply
  routine (`FUN_00001984`) calls `FUN_00006418`, which unconditionally
  rewrites a 16-byte settings record (marker `A5`) at `0x8200`. That fits
  the ~28 ms of extra time per write.
* **The firmware has no live or direct lighting mode.** The renderer reads
  its state from RAM, but the only code that fills that RAM from the host
  is the apply routine, which runs after the flash write. No command
  changes colours in RAM only.
* **Music mode doesn't exist in this firmware.** Neither handler has a
  `0x3X` branch, so `0x32`/`0x35` frames are just echoed and dropped. That
  is exactly what `rgb-stream` saw. It isn't a mode to be switched on.
* **The keyboard's own Fn shortcuts write flash too.** The Fn handler
  (`FUN_00003098`) calls the same `0x8200` writer on most lighting
  shortcuts, so AJAZZ's firmware already erases a page on every Fn
  brightness or effect press.
* **"Off" reads back as Wave: a reporting quirk.** Apply maps effect
  `0` to internal index 10 and `0x80` to `0x14`. A read reports
  `index + 1` for indexes ≤ 10, so "off" comes back as `0x0B`, and
  per-key custom as `0x14`.
* `0x41`, `0x42` and `0x50` (images and screen in the web driver) have no
  branch in either handler here. They stay on the never-send list anyway.

Still unknown: the SN34F280's rated erase/write cycles (needs the
datasheet), and the 2.4 GHz receiver's firmware, which isn't in the updater.

## Ideas this opens up

* **Mode colour:** the keyboard shows which knob mode is active.
* **Knob as a lighting control:** a mode whose turn changes brightness, using
  an action like any other.
* **Battery level in the tray:** from the information block, with no lighting
  writes involved. The smallest useful feature here.
* **Per-app lighting:** follows the existing per-app rules.
