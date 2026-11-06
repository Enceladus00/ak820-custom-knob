# AK820 Custom Knob

Free, open-source software for the knob on the **AJAZZ AK820 MAX** and
**AK820 MAX Ultra** keyboards. It gives the knob new jobs on Windows:
scrolling, zooming, media control, the volume of a single app, any key
combination, and different functions in different apps.

Only the knob changes. Every other key, including Fn+F2 / Fn+F3 for volume,
works exactly as before.

[![Download for Windows](https://img.shields.io/badge/Download-for%20Windows-2f6feb?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/Enceladus00/ak820-custom-knob/releases/latest/download/ak820-custom-knob-setup.exe)

## What the knob can do

The knob has three gestures: **turn**, **press** and **long press**. Each
**mode** gives them a job, and a long press switches to the next mode. It
comes with three modes, and you can change them or add your own:

| Mode | Turn | Press | Holding Ctrl while turning | Holding Shift while turning |
|---|---|---|---|---|
| Volume | System volume | Mute | Volume of the app in front | |
| Scroll | Scroll | Middle click | Zoom | Scroll sideways |
| Media | Right / left arrow key | Play / pause | Next / previous track | System volume |

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

1. Click **Download for Windows** above. Your browser downloads
   [`ak820-custom-knob-setup.exe`](https://github.com/Enceladus00/ak820-custom-knob/releases/latest/download/ak820-custom-knob-setup.exe).
2. Open the downloaded file. If Windows SmartScreen warns about an unknown
   publisher, choose *More info → Run anyway*; the program isn't code-signed
   yet. See [Privacy](#privacy) for exactly what it does.
3. Click through the installer. It needs no administrator rights.
4. At the end, the knob program starts and Settings opens. Turn the knob.

Prefer not to install? The
[Releases](https://github.com/Enceladus00/ak820-custom-knob/releases) page
also has `ak820-custom-knob-portable.zip`: unpack it anywhere and run
`ak820-custom-knob.exe`, keeping both exe files in the same folder.

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

## Privacy

To see the knob, the program installs a Windows keyboard hook, which lets it
see every key press. It only ever acts on the volume and mute keys and lets
everything else through untouched. It doesn't record or store any keys,
its log files contain no key presses, and it never connects to the
internet.

## How it works

The knob sends the same volume-up / volume-down / mute signals as Fn+F2,
Fn+F3 and Fn+F4. The difference is timing: a knob step is released after
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

Published releases are built by GitHub Actions instead; see
[docs/releasing.md](docs/releasing.md).

The workspace:

| Crate | What it is |
|---|---|
| `knob-core` | Knob detection, gestures, modes and config, with no Windows code |
| `knob-win` | Keyboard hook, Raw Input, sending keys, per-app volume |
| `ak820-custom-knob` | The tray program |
| `knob-settings` | The Settings window ([Tauri](https://tauri.app)) |
| `knob-probe` | A diagnostic tool that logs what the keyboard sends |

## Contributing

Everyone is more than welcome to build on this project, with features for
the knob or for anything else on the keyboard. Have an idea? Open an issue
to talk it through, or send a pull request. Bug reports are just as welcome.

To add a language, see [docs/translating.md](docs/translating.md). If your
knob isn't recognised, for example over the cable or Bluetooth, a log from
`knob-probe` helps add support for it.

## Notes

**Fn+F4 (mute) counts as a knob press.** The keyboard sends exactly the same
signal for both, and both are finger presses of similar length, so they
can't be told apart. In a mode where the press does something else (Scroll
mode: middle click), Fn+F4 does that too.

## License

[Apache License 2.0](LICENSE).

This is an independent project, not affiliated with or endorsed by AJAZZ.
