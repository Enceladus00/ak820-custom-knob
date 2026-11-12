# Custom firmware: phase 0

Goal: patched firmware for the AK820 MAX Ultra that can switch lighting
without writing flash, and later play built-in effect templates (fire,
frost, …) that a game integration triggers with one command.

Rule: **nothing is loaded onto the keyboard without the owner's explicit
go-ahead for that specific action.** Everything below was done on the PC
only.

Firmware facts are in [rgb-research.md](rgb-research.md#firmware-analysis-v136).
The firmware image and the tools live outside this repo, in
`C:\Users\melih\tools\ak820-fw\`, because the image is AJAZZ's code. Only
patches (diffs) should ever be published.

## Recovery: what saves us and what doesn't

| Situation | Recovery |
|---|---|
| Bad settings (lighting, keymap, calibration) | **Fn+Space** for 3–5 s. This is application code (`FUN_0000d74c`): it blinks the keys 6 times, then rewrites every settings page with defaults. It only works while the firmware runs. |
| Flash interrupted or failed | Run the stock updater again. It erases the whole user flash and still talks to a separate "SONiX ISP Device" afterwards, so the bootloader lives outside user flash (most likely in ROM). The image ends with a `55 55 AA AA` marker, which is likely how the boot ROM tells a complete image. |
| Patched firmware crashes when a **new command** is sent | The HardFault handler resets the chip (`AIRCR` `SYSRESETREQ`), so the keyboard reboots into the same firmware, which works until that command is sent again. |
| Patched firmware crashes **while booting** | Could need the chip's boot pin, which means opening the case. **Avoided by design:** patches never touch the boot path, the vector table or the settings pages, and `fwpatch.py` enforces this. |

Side effect of any flash: the image's default settings pages get written,
including the magnetic-switch calibration (`0x8400`/`0x8500`). Calibrate
again afterwards (command `0x64`, then `0x65`, or through the driver).

## How the official updater flashes

The updater is a rebranded "SONiX USB MCU ISP Tool". Its strings give the
sequence:

1. Find the keyboard (`0C45:80A0`) and send the 64-byte feature report
   `AA 55 A5 5A FF 00 33 CC …`. In the firmware, the SET_REPORT handler
   (`FUN_00010f64`) recognises it and sets a flag. Where that flag hands
   over to the boot ROM hasn't been found yet; it's in code Ghidra didn't
   turn into a function.
2. *Enter ISP Mode (0x20)* → *Compare ISP Password (0x02/0x22)*, where
   the password is resource 4007, `12345678` → *Enable Erase* → **Chip
   Erase** → *Enable Program* → *Program* → *Data Verify* → *Get User
   Code CheckSum* → *Return Boot Mode*.
3. It refuses chips at code-security level CS3. This one is configured
   with `CodeSecurity=0`.

The checksum the tool compares (resource 4014, `67E7`) is the 16-bit sum
of the image's little-endian words over all `0x7E000` bytes.

### Will the stock updater always reflash? (checked in Ghidra)

* **No version check here.** The updater compares the device's version
  with `CurVersion` and shows *"Firmware is already updated"* only when
  `ProgramOneTime=1` (`FUN_00424f70`, field `0x5CD`). This updater's ini
  sets `ProgramOneTime=0`, so Start stays enabled whatever version the
  keyboard reports. It still shows the version it reads, as `… : V1.36`.
  `ProgramByDeviceVersionInfo` has a `Count` of 0, so that feature is off
  too.
* **It also finds a keyboard that is already in the bootloader.** The
  updater holds a table of 17 Sonix ROM bootloader USB IDs (`0C45:7010`,
  `7040`, `7050`, `7110`, `7120`, `7140`, `7160`, `7210`, `7300`–`7B00`,
  `75F0`), 51 bytes per entry, which the detection code indexes. A
  keyboard stuck in the bootloader after a failed flash enumerates under
  one of these and can be flashed again. Which entry is the SN34F280 isn't
  known yet; a rehearsal flash would show it.
* **Patched builds therefore keep version 1.36.** The tool doesn't need a
  different version to overwrite them. The USB descriptor sits on the
  `0x9000` page, which `fwpatch.py` protects anyway. A patched build
  identifies itself through its new command instead.

**Plan for flashing a patch:** keep AJAZZ's own tool and protocol, and
only swap its embedded image and checksum. No custom ISP code, so the
flashing itself is as safe as an official update.

## Tools (in `C:\Users\melih\tools\ak820-fw\`)

| Tool | What it does |
|---|---|
| Ghidra 12.2 + JDK 25 | Static analysis. Headless scripts in `scripts\`; decompiled output with pool values resolved in `out\decomp_r.c`. |
| `scripts\fwpatch.py` | Applies patches to the stock image only. Refuses changes outside allowed ranges or inside the vector table, settings pages or end marker. Recomputes the checksum. |
| `scripts\repack.py` | Builds a **copy** of the updater with the patched image and checksum, then reads both back to confirm. Never runs anything. |
| `scripts\emu.py` | Unicorn emulator. Boots the image to `main()`, then feeds frames to the 2.4 GHz command handler and logs every flash write. |
| xPack arm-none-eabi-gcc 15.2.1 | Compiles new code for the free flash area. |

Verified:
* An empty patch reproduces the stock image byte for byte (checksum `67E7`).
* A change to a settings page is refused.
* The emulator matches the hardware: `0x23` writes `0x9800`, then
  `0x8200`; `0x35` and `0x64` are echoed without a flash write; `0x13`
  answers.

## Free space

Code and data end at about `0x171C6`. From there to `0x7DFFC` is zeros
(about 370 KB), followed by the `55 55 AA AA` marker. New code goes from
`0x17400` upwards.

## Settings backup

`knob-probe rgb-dump` reads the lighting block and the stored blocks
`0x11`, `0x12` and `0x14`–`0x18` (512 bytes each). These reads are pure
copies; the emulator confirmed zero flash writes. The block reader in
`knob-rgb` (`ReadBlock`) can't encode anything else.

It leaves out `0x10`: if byte `0x9003` is still `0x12`, as in a freshly
flashed image, the first `0x10` read writes `0x92` there, and over the
cable it also resets the switch calibration. Expect that once after any
flash.

The backup from 2026-11-12 is in
`C:\Users\melih\tools\ak820-fw\stock\settings-backup-2026-11-12`.
Lighting was Ripple (`0x0F`), `#00FFFF`, brightness 5, speed 3. The key
map, macro and per-key blocks were empty, and block `0x11` held 4
non-default bytes.

## Phase 1: live lighting (built and emulated, not flashed)

Source is in [`firmware/live-lighting/`](../firmware/live-lighting). It
consists of 392 bytes of code at `0x17400` and two 6-byte hooks; the rest
of the image is unchanged.

| Command | Patched firmware | Stock firmware |
|---|---|---|
| `0x30` + 16-byte lighting block (same layout as `0x23`) | Checked (effect ≤ `0x13` or `0x80`, brightness and speed ≤ 5, direction ≤ 3), then applied **in RAM only**. Status in payload byte 15: 0 = ok, 1 = effect, 2 = level, 3 = direction. | Echoed, ignored |
| `0x31` | Re-applies the stored block at `0x9800`, RAM only | Echoed, ignored |
| `0x3F` | Reply payload `AKLIVE` + version (1) | Echoed with zeros |

How it works: both command handlers test "is it group 6?" with
`movs rX,#6; cmp.w rX, rCMD, lsr #4` (2.4 GHz at `0xED36`, wired at
`0xFC54`). Each test is replaced by `b.w stub; nop`. A stub calls the C
handler for `0x3X` commands and leaves through the handler's normal exit.
For anything else it runs the two original instructions and jumps back,
so every stock command behaves as before. The C handler mirrors the stock
apply routine (`0x1984`) instruction by instruction, but feeds it from the
frame instead of flash and drops its final `b.w 0x6418` (the `0x8200`
settings write). It never calls the flash writer, and nothing runs on the
boot path.

Emulated (`emu.py`, both transports):

* `0x30` sets the renderer state (effect, brightness, speed) and calls
  the three render-setup functions: **0 flash writes**, settings writer
  not called. `0x13` reads the new state back.
* Invalid blocks are refused with status 1 or 2, and the state is
  unchanged.
* `0x31` returns to the stored block with 0 flash writes.
* Stock `0x23` is unchanged: two writes, as on the real keyboard.
* The same frames sent to the stock image are echoed and ignored.

What the real keyboard has shown: `knob-probe rgb-identify` reports
"stock" (plain echo), and `rgb-live` refuses to send anything to stock
firmware.

Build: `python firmware/live-lighting/build.py`, with the stock image and
toolchain under `C:\Users\melih\tools\`. It produces `live-v1.bin`
(sha256 `a8fb86ed…94f9`, checksum `DD8B`). `repack.py` turns that into
`AK820MAX-Ultra-LIVE-v1-Upgrade.exe` (sha256 `2e98ea4a…c92b`), which has
**not been run**.

Not testable in the emulator: whether the LEDs really show the new state.
The emulator has no LED driver, only the same RAM variables and calls
that the stock `0x23` path uses.

Not flashed. Per the agreed rule, flashing needs an explicit go-ahead.

## Phase 2: LIVE v2 effect engine (built and emulated, not flashed)

Format and commands: [effects-format.md](effects-format.md). Source:
`firmware/live-lighting/effects.c`, `effects.h` and `live.c`. It is 1,764
bytes of code at `0x17400` and three hooks; v1's commands are kept.

**How the LEDs work** (found in v1.36):
* Effects draw into three 126-byte arrays: R at `0x20004984`, G at
  `0x20004A02`, B at `0x20004A80`. `FUN_0000E908` is `set_led(i, r, g, b)`.
* There are 82 LEDs, in serpentine order. The table at `0x15B8A` maps key
  `k` (row-major, rows of 14/15/15/14/14/10) to its LED index.
* `FUN_00014B90` encodes all of them into a WS2812-style GRB bitstream
  (12 bytes per LED, 985 bytes in all) and sends it out.
* The frame loop `FUN_000104D4` runs on a timer flag every 40 base ticks
  (`FUN_00005770`) and pushes the LEDs every 8th frame. The base tick length
  isn't in the code, so the frame period is measured on the keyboard.

**The patch:**
* The frame loop's `bl 0x14B90` at `0x1054E` becomes `bl ak_frame`.
  `ak_frame` counts frames, draws the active effect over the stock
  renderer's output, then calls the original push. Stopping an effect
  (`0x3A`, or the duration running out) re-applies the lighting held in RAM,
  so the stock renderer takes over cleanly.
* Engine state is 512 bytes at `0x20006E00`. The firmware's RAM use,
  stack included, ends at `0x20006D98`: the C runtime's zero-init region
  runs `0x200010C0`–`0x20006D98`, the initial stack pointer is
  `0x20006D98`, and the stack grows down. So nothing reaches above it.
  Sonix lists the SN34F2852 with 160 KB of RAM. Even the smallest standard
  size that fits the firmware (28 KB) contains `0x20006E00`–`0x20006FFF`.
  `0x3F` also self-tests that area.
* A magic word marks valid state, so random RAM at power-on is cleared
  before use.
* Integer maths only, using the hardware divide; no C library.

Emulated (`emu.py --frame=0x177E0`, over 2.4 GHz and the cable):
* `0x3F` reports v2 with the RAM self-test OK, plus a frame counter.
* Fire (flicker) gives each key its own red, orange or yellow that changes
  between frames. Frost (wave) sweeps colour along the rows. Shock
  (sparkle) flashes and fades on a dark background.
* A pulse with a 500 ms duration was still on at 320 ms and had ended by
  itself at 640 ms.
* `0x3A` stops the effect and re-applies the current lighting.
* An invalid slot is refused (status 4), and so is an undefined slot
  (status 6).
* No flash writes from any effect command. Stock `0x23` is unchanged.
* Cost: the stock LED push is ~26k instructions per frame; with Fire
  running it's ~40k, so the engine adds ~14k instructions per LED frame.
* Stock firmware ignores all the new commands.

Build: `python firmware/live-lighting/build.py` produces `live-v2.bin`
(sha256 `a42cc375…599a`, checksum `4E5A`). `repack.py` turns it into
`AK820MAX-Ultra-LIVE-v2-Upgrade.exe` (sha256 `2664fa9c…51c6`), which has
**not been run**.

Host side:
* `knob-rgb` has `EffectDef`, `Pattern`, and requests for define, play,
  stop and frame period. `LiveInfo` now also reports the RAM self-test,
  frame count and active slot.
* `knob-probe rgb-effect measure | define … | play N | stop | demo`. `demo`
  measures the frame rate, uploads the five default effects and plays each
  for 4 s.

### LIVE v2 on the keyboard (2026-11-12)

Flashed by the owner at about 16:00. `0x3F` answers v2 with the RAM
self-test OK, so the `0x20006E00` area exists on the real chip. Measured
over 3 s: 187 LED frames, so **16.06 ms per frame, 62.3 fps**. That
matches the 16 ms default and implies a 50 µs base tick (× 320).
`rgb-effect demo`, and a replay at 3 s per effect, ran with every command
accepted.

**Bug: stop turns the lights off.** `0x3A` and a duration running out
re-apply the lighting block held in RAM (`0x200001CC`). But only the
`0x23`/`0x30` apply path fills that block; the boot path (`FUN_0000AE10`,
from the `0x8200` record) doesn't. The flash also reset the stored block
to the image default, effect 0. So stop applied "off". Possible fixes for
v3:
* Snapshot the renderer state when an effect starts and restore it.
* Or skip the re-apply if the stock renderer redraws every frame anyway.

Check which on the keyboard before choosing. Workaround now: store one
lighting look with `rgb-set … --apply` (one write), after which stop
returns to it.

**Second cause of the dark replays: LED power.** `FUN_00013768` checks every
frame whether the stock renderer's output is all off (or brightness < 2)
and then cuts LED power (`FUN_00006080`; GPIO `0x40038000 + 0x10`). After
stop applied "off", later effects drew into an unpowered LED chain. Fix
for v3: keep LED power on while an effect plays.

**Confirmed on the keyboard (16:05):** first set Ripple cyan live with
`rgb-live --effect 0x0F --color 00FFFF --brightness 5 --speed 3` (RAM
only, powers the LEDs). Then Fire, Frost, Poison, Low health and Shock
played at 3 s each, and **the owner saw all five correctly**. Stop
returned to Ripple cyan, because the live command had filled the RAM
block that stop restores. No flash writes.

`rgb-live` now starts from a default block when the read-back fails
validation (speed 0 after an "off" state).

## Fallback rehearsal: stock reflash (2026-11-12, succeeded)

The owner ran the unmodified stock updater over the USB cable and pressed
Start. A USB recorder (`scripts\usbwatch.ps1`, polling VID `0C45` every
~200 ms) logged:

| Time | Event |
|---|---|
| 15:19:02.9 | Wired keyboard `0C45:80A0` connected |
| 15:20:15.5 | Keyboard drops off USB |
| 15:20:17.0 | Back as `0C45:80A0`, 1.5 s later; the updater reports success |

* **No separate bootloader ID appeared.** Windows' device registry
  (`Enum\USB`) holds only `0C45:80A0` and `0C45:FEFC`, so none of the 17
  ROM bootloader IDs in the updater's table has ever enumerated on this
  PC. Programming happens while the chip presents `80A0`: either the
  bootloader reuses the keyboard's ID, or the application firmware does
  the programming itself (in-application programming). Which one is still
  open.
* **Consequence for patches:** if programming is done by the application
  firmware, a patched build must keep that code working. LIVE v1 only
  changes two hooks in the command handlers and adds code in empty flash,
  so the ISP path is untouched. Keep it that way in future phases.
* **After the flash** (read over 2.4 GHz with the cable still plugged in):
  the pairing survived, and the keyboard reports firmware 1.36 and
  identifies as stock. Lighting went back to factory settings (brightness
  2, speed 3, colour cycling). The info block still had `0x12` at byte 3,
  the "first read after flashing" flag, which that read then set.
* Switch calibration is back to factory defaults and should be redone
  after the last flash of a session.

The stock fallback is now proven on this keyboard and PC.

## LIVE v1 on the keyboard (2026-11-12)

The owner flashed `AK820MAX-Ultra-LIVE-v1-Upgrade.exe` over the cable. The
keyboard dropped off USB at 15:28:54 and was back as `0C45:80A0` 0.9 s
later.

* **Identify:** `0x3F` answers `AKLIVE` v1 (stock answered zeros).
  `knob-probe` now picks the cable automatically when the keyboard is in
  cable mode (64-byte reports) and the receiver otherwise.
* **Live lighting works on the LEDs.** `rgb-live --effect 7 --color FF4000
  --brightness 5 --speed 4` returned status 0, read back as Breathing
  brightness 5 speed 4, and the owner saw orange-red breathing.
* **No flash work, measured** over the cable (`rgb-live --bench 100`): `0x30`
  median 3.01 ms (1.50–6.04), plain `0x13` read median 2.97 ms
  (1.50–5.02). A stored `0x23` write takes ~31 ms. So the live path costs
  the same as a read.
* **A flicker demo works:** 30 static colour changes (red, orange, yellow)
  in 2.98 s, limited by starting the probe process each time, not by the
  keyboard.
* **v1 flaw: `0x31` restores the wrong thing.** It re-applies the `0x9800`
  block, but the keyboard boots from the `0x8200` settings record
  (`FUN_0000ae10`). After a flash, `0x9800` holds the image default,
  effect 0 (off), so `--saved` turned the lighting off instead of
  restoring what was shown. Fix for v2: restore through the same path the
  boot code uses, or store one stored block via the official app first.
  **Workaround applied:** one stored write of Ripple cyan, brightness 5,
  speed 3 (`rgb-set … --apply`). Then the round trip worked: live
  Breathing, then `--saved` returned to Ripple brightness 5 speed 3.
