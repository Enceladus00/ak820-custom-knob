# Design

## Guiding principle

**Never disturb other keys.** Only the knob is taken over. Every other key,
including other keyboards' volume keys and the AK820's own Fn+F2/F3, behaves
exactly as without this program. If anything goes wrong, the knob falls back
to its stock behaviour (volume) instead of being swallowed.

## Decisions

| Topic | Decision |
|---|---|
| Fn+F2 / Fn+F3 | Passed through as normal volume keys (delayed ~30 ms, not noticeable) |
| Fn+F1 (mute) | Can't be told apart from a knob click; acts as a knob click (documented limitation) |
| Mode switching | **Long press** cycles modes. Double-click is supported but only for rarely needed actions, and only costs click latency in modes that bind it |
| Connection | 2.4 GHz dongle first (`VID_0C45&PID_FEFC`); wired and Bluetooth later |
| Settings UI | Tauri (v0.3) |
| License | Apache-2.0 |
| Language | Rust |

## Detection (see probe-findings.md)

The knob and Fn+F2/F3 send identical HID reports; they differ only in how
long the key is held: ~5 ms for a knob detent, 75 ms or more for a finger.

1. The keyboard hook swallows every Volume Up/Down/Mute that isn't our own replay.
2. Raw Input reports from the AK820's consumer collection tell which device sent it.
3. Volume key from the AK820, released within 30 ms → **knob step**.
   Still held at 30 ms, or from another device → **replayed** unchanged.
4. Mute from the AK820 → **knob press** (release gives click / long press).

## Architecture

```
 Input thread                Engine thread (knob-core)       Action workers
 ────────────                ─────────────────────────       ──────────────
 keyboard hook ──┐           Classifier (30 ms rule)         SendInput (keys, scroll)
 Raw Input ──────┼─ channel ─► GestureRecognizer          ─► Windows audio
 foreground app ─┘           Resolver (app → profile → mode)
                             Engine → Output
                                   │
 UI thread  ◄──────────────────────┘  tray icon, on-screen popup
```

* The hook does no real work: swallow volume keys, forward them, return.
  It never reads config or takes locks, so Windows never removes it for
  being slow.
* `knob-core` is pure Rust with no Windows code. Time is passed in, so the
  whole pipeline is deterministic and tested against real probe recordings.
* `knob-win` does the I/O; the daemon wires the two together.

### knob-core pipeline

```
Input ──► Classifier ──► Classified ──► GestureRecognizer ──► Gesture ──► Engine ──► Output
          Step/Press/     (or Replay)    Rotate/Click/            (mode binding)   Replay / Run / ModeChanged
          Release/Replay                 DoubleClick/LongPress
```

## Config

TOML at `%APPDATA%\ak820-custom-knob\config.toml`, reloaded on save. An
invalid file keeps the last working config. See
`crates/knob-core/src/default-config.toml` for the built-in default.

* **Modes** bind gestures (rotate, click, double_click, long_press) to actions,
  with optional `ctrl` / `shift` / `alt` / `win` layers.
* **Profiles** match the foreground app's exe name and list the modes that a
  long press cycles through. First match wins. No match → stock knob.

## Roadmap

* **v0.1**: daemon with tray, volume / scroll / media modes, long-press mode
  cycling, modifier layers, config file, on-screen popup.
* **v0.2**: per-app volume, more actions, profile matching by window title.
* **v0.3**: Tauri settings app.
* Later: wired / Bluetooth support, keyboard RGB / screen as mode indicator.
