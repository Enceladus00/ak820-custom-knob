# AK820 Custom Knob

Gives the knob on the **AJAZZ AK820 MAX** keyboard new jobs on Windows:
scrolling, zooming, media control, the volume of a single app, any key
combination, and different functions in different apps.

Only the knob changes. Every other key, including Fn+F2 / Fn+F3 for volume,
works exactly as before.

## What the knob can do

The knob has three gestures: **turn**, **press** and **long press**. Each
**mode** gives them a job, and a long press switches to the next mode. It
comes with three modes, and you can change them or add your own:

| Mode | Turn | Press | Holding Ctrl while turning | Holding Shift while turning |
|---|---|---|---|---|
| Volume | System volume | Mute | Volume of the app in front | |
| Scroll | Scroll | Middle click | Zoom | Scroll sideways |
| Media | Next / previous track | Play / pause | System volume | |

A short popup shows the new mode when you switch.

The available actions:

* system volume, or the volume of only the app in front
* mute
* scroll (up/down or sideways, faster when you spin fast), zoom
* press any key or key combination, with a different one per direction
  (for example `.` and `,` to step through a paused YouTube video frame by
  frame)
* mouse click (left, middle, right)
* switch to the next mode
* the knob's normal behaviour, or nothing

**Modifier layers:** each mode can give the knob other jobs while you hold
Ctrl, Shift, Alt or Win.

**Per-app modes:** choose which modes the knob has in which app, for example
only a "Timeline" mode in your video editor. It switches automatically when
another app comes to the front, and can remember the last mode you used
there.

**Double press** is available too, for rarely needed actions. Modes that use
it make a single press wait a moment to see whether a second one follows.

## Supported keyboards

| Keyboard | Connection | Status |
|---|---|---|
| AJAZZ AK820 MAX (tested on a MAX Ultra) | 2.4 GHz dongle | Supported |
| | USB cable | Not tested yet |
| | Bluetooth | Not tested yet |

Windows 10 or 11, 64-bit. When the knob isn't recognised (for example over
the cable), the program does nothing and the knob keeps its normal volume
function.

## Install

Download the latest version from the
[Releases](https://github.com/Enceladus00/ak820-custom-knob/releases) page:

* **Installer** (`ak820-custom-knob-<version>-setup.exe`): installs for your
  user account only, no administrator rights needed. Adds Start menu
  shortcuts and can start the program with Windows.
* **Portable** (`ak820-custom-knob-<version>-portable.zip`): unpack anywhere
  and run `ak820-custom-knob.exe`. Keep both exe files in the same folder.

The Settings window needs the Microsoft Edge WebView2 Runtime, which most
Windows PCs already have. The installer adds it if it's missing. The knob
itself works without it.

> **Windows SmartScreen** may warn about an unknown publisher, because the
> program isn't code-signed yet. Choose *More info → Run anyway*. Some
> antivirus programs are wary of any program that watches the keyboard; see
> [Privacy](#privacy) for exactly what it does.

## Use

The program runs in the notification area (tray), next to the clock.

* **Click** the tray icon to open **Settings**.
* **Right-click** it for the menu: the current mode, *Paused* (the knob
  works as normal until you turn it back on), *Start with Windows*,
  Settings, the config file, the log folder, and Quit.

In **Settings**:

* **Modes:** what the knob does in each mode, with and without a modifier
  held. To record a key, click the key field and press the key or
  combination.
* **Apps:** which modes the knob has in which app. Entries are checked from
  the top; the first that matches the app in front wins.
* **General:** Start with Windows, theme (light, dark, or same as Windows),
  language, long-press and double-press timing, and Restore defaults.

Changes apply as soon as you save. There's no need to restart the program.

### Config file

Settings are stored in `%APPDATA%\ak820-custom-knob\config.toml`. You can
also edit this file by hand; the program picks up saved changes right away,
and keeps the last working settings if the file has an error.
[`default-config.toml`](crates/knob-core/src/default-config.toml) describes
every option.

## Known limitations

* **Fn+F1 (mute) counts as a knob press.** The keyboard sends exactly the
  same signal for both, so they can't be told apart. In a mode where the
  press does something else (Scroll mode: middle click), Fn+F1 does that too.
* **Fn + knob does nothing.** The keyboard itself ignores the knob while Fn
  is held, so Fn can't be used as a layer. Ctrl, Shift, Alt and Win work.
* **Apps running as administrator:** while one is in front, Windows doesn't
  let a normal program see or send keys there, so the knob works as a plain
  volume knob.
* Volume keys (Fn+F2 / Fn+F3, or the volume keys of another keyboard) reach
  Windows up to 30 ms later than without the program, while it checks that
  they aren't the knob. This isn't noticeable in normal use.

## Privacy

To see the knob, the program installs a Windows keyboard hook, which lets it
see every key press. It only ever acts on the volume and mute keys and lets
everything else through untouched. It doesn't record or store any keys,
its log files contain no key presses, and it never connects to the
internet.

## How it works

The knob sends the same volume-up / volume-down / mute signals as Fn+F2,
Fn+F3 and Fn+F1. The difference is timing: a knob step is released after
about 5 ms, while a finger holds a key for 70 ms or more. The program holds
back volume keys for up to 30 ms. If one came from the AK820 and was released
by then, it's the knob. Otherwise it's a real key press, which is passed on
unchanged.

If the program ever fails, the knob falls back to its normal volume
function instead of going silent. Details are in [docs/design.md](docs/design.md)
and the measurements behind them in
[docs/probe-findings.md](docs/probe-findings.md).

## Uninstall

Use *Settings → Apps → AK820 Custom Knob → Uninstall* in Windows. It asks
whether to delete your settings and logs too. For the portable version, turn
off *Start with Windows* in the tray menu, quit, and delete the folder.

## Building from source

Needs [Rust](https://rustup.rs) (stable, with the MSVC toolchain) on
Windows.

```
cargo build --release -p ak820-custom-knob -p knob-settings
```

The two exe files end up in `target\release`. `cargo test --workspace` runs
the tests, including a replay of a real recording from the keyboard.

To build the installer and portable zip into `dist\`, install
[Inno Setup 6](https://jrsoftware.org/isinfo.php) and run:

```
powershell -ExecutionPolicy Bypass -File tools\build-release.ps1
```

The workspace:

| Crate | What it is |
|---|---|
| `knob-core` | Knob detection, gestures, modes and config, with no Windows code |
| `knob-win` | Keyboard hook, Raw Input, sending keys, per-app volume |
| `ak820-custom-knob` | The tray program |
| `knob-settings` | The Settings window ([Tauri](https://tauri.app)) |
| `knob-probe` | A diagnostic tool that logs what the keyboard sends |

## Contributing

Bug reports and pull requests are welcome. To add a language, see
[docs/translating.md](docs/translating.md). If your knob isn't recognised,
for example over the cable or Bluetooth, a log from `knob-probe` helps add
support for it.

## License

[Apache License 2.0](LICENSE).

This is an independent project, not affiliated with or endorsed by AJAZZ.
