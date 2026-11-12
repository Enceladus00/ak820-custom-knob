# Lighting effects: shared format

This is the contract between the settings page (where effects are made and
edited) and the keyboard's effect engine (LIVE v2 firmware, see
[custom-firmware.md](custom-firmware.md)). Effects live in the config. The
app uploads them to the keyboard's **RAM** when it connects, and a game or
the knob starts and stops them with one command each. Nothing about
effects is ever written to the keyboard's flash.

## Config (`config.toml`)

```toml
[[effects]]
name = "Fire"            # shown in the UI, unique, non-empty
slot = 1                 # 1–8: where it lives on the keyboard
pattern = "flicker"      # solid | pulse | wave | flicker | sparkle
colors = ["#FF2000", "#FF7000", "#FFC040"]   # 1–4 colours, "#RRGGBB"
speed = 6                # 1–10
brightness = 100         # 1–100, percent
duration_ms = 0          # 0 = until stopped, otherwise stops by itself (max 60000)
```

Validation, the same in the UI and in `knob-core`:
* name is non-empty and unique;
* slot is 1–8 and unique;
* pattern is one of the five values;
* 1–4 colours, each `#RRGGBB`;
* speed is 1–10, brightness 1–100, duration 0–60000.

Default effects shipped in `default-config.toml`:

| name | slot | pattern | colours | speed |
|---|---|---|---|---|
| Fire | 1 | flicker | `#FF2000` `#FF7000` `#FFC040` | 6 |
| Frost | 2 | wave | `#A0E8FF` `#2060FF` `#FFFFFF` | 3 |
| Poison | 3 | pulse | `#40FF20` `#003000` | 5 |
| Low health | 4 | pulse | `#FF0000` `#200000` | 8 |
| Shock | 5 | sparkle | `#101030` `#FFFFFF` `#80C0FF` | 9 |

## Patterns

`t` is time in ms since the effect started, and `P = 4000 / speed` ms is
one cycle. `palette(u)` blends the colours evenly over `u` in 0..1
(linear RGB blend; one colour means constant). `x` is the key's column
divided by the widest column (0..1). The key positions are
`knob_rgb::KEYS` (name, row, column). The firmware implements these exactly; the
UI preview only needs to look the same.

| pattern | per key |
|---|---|
| `solid` | `colors[0]` |
| `pulse` | `f = (1 − cos(2π·t/P)) / 2`; blend from `colors[1]` (black if only one) to `colors[0]` by `f` |
| `wave` | `palette(frac(x − t/P))` with the palette treated as a loop |
| `flicker` | every key gets its own value `v` in 0..1, re-rolled every `P/8` ms and smoothly blended between rolls; colour `palette(v)` at `(0.55 + 0.45·v)` of the brightness |
| `sparkle` | background `colors[0]`; each key flashes a random colour from `colors[1..]` (white if none) and fades out over `P/4` ms; about `speed × 2` % of keys start a flash each `P/4` ms |

`brightness` scales the result. The keyboard's own brightness setting
applies on top of it.

## Wire format (keyboard commands, patched firmware only)

| Command | Payload | Effect |
|---|---|---|
| `0x38` define | `slot, pattern, speed, brightness, n_colors, R,G,B ×4, duration_ms (u16 LE, in 10 ms units)` = 19 bytes | Stores the effect in a RAM slot (lost on power-off; the app re-uploads it). |
| `0x39` play | `slot` | Starts it; it replaces whatever lighting is showing. |
| `0x3A` stop | none | Stops any effect and goes back to the lighting from before. |

Pattern codes: `solid` 0, `pulse` 1, `wave` 2, `flicker` 3, `sparkle` 4.
`brightness` on the wire is 1–100, and the duration is rounded to 10 ms.
These are reserved now; the firmware side is phase 2 work in progress.
