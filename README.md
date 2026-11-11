# AK820 MAX Ultra Toolbox

Windows software for the AJAZZ AK820 MAX Ultra. Out of the box the knob only
controls volume. With this you can bind it to scrolling, zooming, media keys
or any shortcut, and give it a different job in each app.

It also does a few other things I wanted from my keyboard. I often use my PC
from across the room, so the arrow keys can drive the mouse, and letter hints
let me click a link without pressing Tab thirty times to reach it. You can
also turn any key into another key or a shortcut, and there are macros and
lighting layouts too.

Only the knob changes by default. Every other key behaves as before, including
Fn+F2 and Fn+F3 for volume, and the extras stay off until you turn them on.

**[Download the installer](https://github.com/Enceladus00/ak820-max-ultra-toolbox/releases/latest/download/ak820-custom-knob-setup.exe)**
· [Website](https://enceladus00.github.io/ak820-max-ultra-toolbox/)
· [All releases](https://github.com/Enceladus00/ak820-max-ultra-toolbox/releases)

This is an [unofficial community project](#unofficial-free-and-open-source),
not made by AJAZZ. It's free and all of the code is here.

<table>
<tr>
<td width="33%"><b>Any shortcut on the knob</b><br>A different one per direction</td>
<td width="33%"><b>Per-app modes</b><br>Switches with the app in front</td>
<td width="33%"><b>Modifier layers</b><br>New jobs while holding Ctrl, Shift, Alt or Win</td>
</tr>
<tr>
<td><b>Volume</b><br>System-wide or one app at a time</td>
<td><b>Scroll and zoom</b><br>Up, down or sideways</td>
<td><b>Media controls</b><br>Play, pause, next and previous</td>
</tr>
<tr>
<td><b>Key remapping</b><br>Any key becomes another key or a shortcut</td>
<td><b>Long and double press</b><br>More actions on one knob</td>
<td><b>Macros</b><br>Record keys, text, waits and clicks</td>
</tr>
<tr>
<td><b>Mouse mode</b><br>Move the pointer with the arrow keys</td>
<td><b>Click hints</b><br>Click anything by typing its letter</td>
<td><b>Lighting layouts</b><br>7 layouts and 19 effects</td>
</tr>
</table>

![The Modes page in Settings, showing what the knob does when turned, pressed and long-pressed](docs/images/settings-modes.png)

## Install

Needs Windows 10 or 11 (64-bit) and the keyboard's 2.4 GHz receiver.

1. Download and open
   [`ak820-custom-knob-setup.exe`](https://github.com/Enceladus00/ak820-max-ultra-toolbox/releases/latest/download/ak820-custom-knob-setup.exe).
   No administrator rights needed.
2. If SmartScreen warns about an unknown publisher, choose *More info → Run
   anyway*. The program isn't code-signed yet; [Privacy](#privacy) covers what
   it does on your PC.
3. When the installer finishes, Settings opens. Turn the knob.

If you'd rather not install anything, the
[Releases](https://github.com/Enceladus00/ak820-max-ultra-toolbox/releases) page
has `ak820-custom-knob-portable.zip`. Unpack it anywhere and run
`ak820-custom-knob.exe`, keeping both exe files in the same folder.

The program lives in the tray next to the clock. Click the icon for Settings,
or right-click it to pause, start with Windows, or quit. Changes apply as soon
as you save.

## The knob

You get turn, press and long press, plus double press if you want it. A set of
jobs for those is a **mode**, and a long press moves to the next one. Three
modes come ready to use, and you can edit them or add more:

| Mode | Turn | Press | Ctrl + turn | Shift + turn |
|---|---|---|---|---|
| Volume | System volume | Mute | Volume of the app in front | |
| Scroll | Scroll | Middle click | Zoom | Scroll sideways |
| Media | Right / left arrow key | Play / pause | Next / previous track | System volume |

You can bind system volume or the volume of a single app, mute, scrolling in
either direction, zoom, mouse clicks, switching modes, or any key combination.
Each direction can have its own: `.` and `,` step through a paused YouTube
video one frame at a time. Holding Ctrl, Shift, Alt or Win gives the knob a
separate set of jobs in every mode.

The **Apps** page picks which modes are available in which app, so your video
editor can have a Timeline mode that nothing else sees. The knob switches by
itself when another app comes to the front.

![The Apps page in Settings, with a video editor using its own Timeline mode and every other app using the default modes](docs/images/settings-apps.png)

One quirk: Fn+F4 counts as a knob press, because the keyboard sends the same
signal for both and they can't be told apart. In Scroll mode, Fn+F4 is a middle
click.

## Key remapping

Turn any key into another key or a shortcut, in every app or only in the ones
you pick. Caps Lock can become Esc or Ctrl, a key you never use can become
Play / Pause, and F13 can mute your mic in Teams.

A key that becomes another single key works exactly like it, held for as long
as you hold it. A key that becomes a shortcut sends it once per press. Remaps
never apply while Settings is in front, so you can always undo one there, and
*Paused* in the tray menu turns them all off.

## Macros

A macro is a list of key presses, typed text, waits and mouse clicks, started
by a hotkey or a knob gesture. Press **Record** to capture keys and the pauses
between them, then edit the steps. It can repeat a set number of times or until
you stop it with its hotkey or Esc.

Macro text sits unencrypted in the config file, so keep passwords out of it.
Some games ban macros; check before you use one there.

## Mouse mode

Turn it on in Settings, then press Ctrl+Alt+M (you can change this) or a knob
gesture.

| Key | What it does |
|---|---|
| Arrow keys | Move the pointer |
| Right Ctrl | Left click (hold to drag) |
| Right Shift | Right click |
| End | Middle click |
| Page Up / Page Down | Scroll |
| Home (hold) | Move slowly |
| Esc | Leave mouse mode |

Other keys keep typing. Mouse mode turns itself off after 10 idle minutes, in
case you forget it's on.

## Click hints

Turn it on in Settings, then press Ctrl+Alt+H (you can change this) or a knob
gesture. Everything clickable in the app in front gets a small letter label.
Type a label to click it, or hold Shift on the last letter to right-click. Esc
or any other key closes the labels.

It works in most apps and in Chrome and Edge. The look is up to you: pick a
preset or set the colours, size, roundness, opacity, position and shadow.

## Lighting

Save layouts with an effect, a colour or colour cycling, brightness and speed.
There are seven to start from and 19 effects. The lights only change when you
click **Apply to keyboard**.

![The Lighting page in Settings, with a list of saved layouts and the controls for one of them](docs/images/settings-lighting.png)

## Which keyboards work

Tested on one AK820 MAX Ultra over its 2.4 GHz receiver. The USB cable and
Bluetooth aren't tested yet.

Other keyboards sold as AK820 MAX can use different hardware. The program only
recognises a receiver that Windows sees as `VID_0C45&PID_FEFC`. With yours
plugged in, this command lists it if it's the right one:

```powershell
Get-PnpDevice -PresentOnly | Where-Object InstanceId -match 'VID_0C45&PID_FEFC'
```

You can also just install it. If the tray icon stays grey with a red badge
while the receiver is plugged in, your version isn't supported yet. The program
then does nothing and the knob goes on controlling volume.

## Privacy

To see the knob, the program installs a Windows keyboard hook, which means it
sees every key press. By default it only acts on the volume and mute keys and
passes everything else through. If you remap keys or turn on macros, mouse
mode or click hints, it also acts on the keys those use.

It doesn't record what you type, its logs contain no key presses, and it never
connects to the internet. Macro recording only captures keys pressed inside the
Settings window while you're recording.

## How it works

The knob sends the same volume-up, volume-down and mute signals as Fn+F2, Fn+F3
and Fn+F4. The difference is timing: a knob step is released after about 5 ms,
and a finger holds a key for 70 ms or more. The program holds volume keys back
for up to 30 ms. If one came from the AK820 and was already released, it's the
knob. Otherwise it's a real key press and goes through unchanged.

If the program fails, the knob falls back to volume instead of going dead.
[docs/design.md](docs/design.md) has the details and
[docs/probe-findings.md](docs/probe-findings.md) has the measurements.

## Questions

**How do I remap the knob on the AJAZZ AK820 MAX Ultra?**
Install this, open Settings, and on the Modes page pick what turn, press and
long press should do. It applies as soon as you save.

**Can I remap keys on the AK820 MAX Ultra?**
Yes. On the Remap keys page, click a key field, press the key to change, then
press what it should become: another key or a shortcut.

**Do I need the official AJAZZ software or driver?**
No.

**Does it change my keyboard's firmware?**
No. The knob functions all run on your PC. Lighting settings are sent to the
keyboard only when you click *Apply to keyboard*, and they don't touch the
firmware.

**Does it work with the regular AK820 MAX, AK820 Pro, MAX Plus or MAX HE?**
It's only been tested on the MAX Ultra. See
[Which keyboards work](#which-keyboards-work) to check yours.

**Why doesn't it work in an app I run as administrator?**
Windows doesn't let normal programs control apps running as administrator.
Hotkeys, remaps, mouse mode and click hints work everywhere else.

## Config file

Settings live in `%APPDATA%\ak820-custom-knob\config.toml`, and each save keeps
the previous one as `config.toml.bak`. You can edit the file by hand: the
program picks up changes right away and keeps the last working settings if the
file has an error. [`default-config.toml`](crates/knob-core/src/default-config.toml)
describes every option.

## Uninstall

Use *Settings → Apps → AK820 MAX Ultra Toolbox → Uninstall* in Windows. It asks
whether to delete your settings and logs too. For the portable version, turn
off *Start with Windows* in the tray menu, quit, and delete the folder.

## Building from source

You need [Rust](https://rustup.rs) (stable, MSVC toolchain) on Windows.

```
cargo build --release -p ak820-custom-knob -p knob-settings
```

The two exe files end up in `target\release`. `cargo test --workspace` runs the
tests, including a replay of a real recording from the keyboard.

For the installer and portable zip, install
[Inno Setup 6](https://jrsoftware.org/isinfo.php) and run this. Published
releases are built by GitHub Actions instead; see
[docs/releasing.md](docs/releasing.md).

```
powershell -ExecutionPolicy Bypass -File tools\build-release.ps1
```

| Crate | What it is |
|---|---|
| `knob-core` | Knob detection, gestures, modes and config, with no Windows code |
| `knob-rgb` | The lighting protocol, with no hardware code |
| `knob-win` | Keyboard hook, Raw Input, sending keys, per-app volume, applying lighting |
| `ak820-custom-knob` | The tray program |
| `knob-settings` | The Settings window ([Tauri](https://tauri.app)) |
| `knob-probe` | A diagnostic tool that logs what the keyboard sends |

## Contributing

Issues and pull requests are welcome, for the knob or anything else on the
keyboard. To add a language, see [docs/translating.md](docs/translating.md). If
your knob isn't recognised, a log from `knob-probe` helps add support for it.

## Unofficial, free and open source

This is a community project. It isn't made, endorsed or supported by AJAZZ.

All of the code is public, and releases are built from it by GitHub Actions
([how to check a download](docs/releasing.md#checking-a-download)).

As the license says, it comes as-is, without warranty, and the author isn't
responsible for problems that come from using it. If something doesn't work,
please [open an issue](https://github.com/Enceladus00/ak820-max-ultra-toolbox/issues).

## License

[Apache License 2.0](LICENSE).
