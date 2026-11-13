# Lighting effects: shared format

This is the contract between the settings page (where effects are made and
edited) and the keyboard's effect engine (LIVE v2 firmware, see
[custom-firmware.md](custom-firmware.md)). Effects live in the config, as many
as you like. The keyboard's **RAM** holds 8 at a time: the background
program keeps slots 1–7 as a cache, uploading an effect into a free (or the
least recently used) slot just before it plays it, and the Settings Play
button uses slot 8 so a preview never pushes out a cached effect. A game or
the knob starts and stops effects with one command each (two when the
effect isn't cached yet). Nothing about effects is ever written to the
keyboard's flash.

## Config (`config.toml`)

```toml
[[effects]]
name = "Fire"            # shown in the UI, unique, non-empty
pattern = "flicker"      # solid | pulse | wave | flicker | sparkle
colors = ["#FF2000", "#FF7000", "#FFC040"]   # 1–4 colours, "#RRGGBB"
speed = 6                # 1–10
brightness = 100         # 1–100, percent
duration_ms = 0          # 0 = until stopped, otherwise stops by itself (max 60000)
when = ["app:valheim.exe", "game:effects=burning"]   # optional, see below
```

`when` lists the situations in which the effect plays by itself; any one
of them is enough, and the first effect in the list whose situation holds
wins. `app:<program.exe>` while that program is in front, `mode:<name>`
while that knob mode is active, `game:<test>` while a game plugin reports it
(`game:effects=burning`, `game:health<0.25`, `game:event=hurt`...; the full
list is in [integrations/valheim/README.md](../integrations/valheim/README.md)).

Configs from older versions have a `slot = N` line per effect. It still
loads, is ignored, and disappears the next time the config is saved.

Validation, the same in the UI and in `knob-core`:
* name is non-empty and unique;
* pattern is one of the five values;
* 1–4 colours, each `#RRGGBB`;
* speed is 1–10, brightness 1–100, duration 0–60000.

Default effects shipped in `default-config.toml`:

| name | pattern | colours | speed |
|---|---|---|---|
| Fire | flicker | `#FF2000` `#FF7000` `#FFC040` | 6 |
| Frost | wave | `#A0E8FF` `#2060FF` `#FFFFFF` | 3 |
| Poison | pulse | `#40FF20` `#003000` | 5 |
| Low health | pulse | `#FF0000` `#200000` | 8 |
| Shock | sparkle | `#101030` `#FFFFFF` `#80C0FF` | 9 |

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
