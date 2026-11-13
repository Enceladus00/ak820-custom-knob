# Valheim: keyboard effects that follow your character

A small BepInEx plugin that tells ak820-custom-knob what is happening to your
character. The knob program plays the keyboard effects you bound to those
situations. Effects run on the keyboard itself (LIVE v2 firmware) and only
use its RAM, so playing them as often as the game likes causes no flash wear.

## Binding effects

In Settings → Effects, open an effect and pick a situation from **Game…**
under "Plays automatically when". Or write it in `config.toml`:

```toml
[[effects]]
name = "Fire"
slot = 1
pattern = "flicker"
colors = ["#FF2000", "#FF7000", "#FFC040"]
speed = 6
brightness = 100
duration_ms = 0
when = ["game:effects=burning"]
```

| Situation | Holds while |
|---|---|
| `game:valheim` | Valheim is running and reporting |
| `game:effects=burning` | Burning (also `frost`, `freezing`, `poison`, `lightning`, `resting`, `wet`, `cold`, `smoked`) |
| `game:health<0.25` | Health is below 25 % (any number 0–1; `>` works too) |
| `game:dead` | Dead |
| `game:boss` | A boss within 80 m is fighting |
| `game:night` | Night |
| `game:weather~thunder` | The weather's name contains "thunder" |
| `game:biome=swamp` | In that biome: `meadows`, `blackforest`, `swamp`, `mountain`, `plains`, `mistlands`, `ashlands`, `deepnorth`, `ocean` |

**Moments** hold briefly after they happen: for the effect's duration, or
1.5 s if its duration is 0. The same moment again restarts the effect.

| Situation | Moment |
|---|---|
| `game:event=hurt` | You take a hit (not fire, poison or other damage over time) |
| `game:event=hit` | You land a hit |
| `game:event=kill` | You kill something |
| `game:event=bosskill` | A boss dies |
| `game:event=parry` | Perfect block |
| `game:event=levelup` | A skill levels up |
| `game:event=power_used` | You use the Forsaken power |
| `game:event=power_ready` | The Forsaken power is ready again |

When several effects apply, the one **higher in the list** wins. Put short
moment effects (a hit flash) above steady ones (burning), so they cut in and
the steady effect comes back after. When nothing applies, the effect stops
and the keyboard returns to its normal lighting. If the game closes or stops
reporting, its situations end within 3 seconds.

In multiplayer, kills and hits on monsters that another player's game
controls may not be reported. Your own hits taken, level-ups, parries and
power always are.

## Is it safe?

The plugin only reads the game. Its Harmony patches watch five combat methods
(`Player.OnDamaged`, `Character.Damage`, `Character.OnDeath`,
`Humanoid.BlockAttack`, `Player.OnSkillLevelup`) without changing what they
do, and each catches its own errors so a bug here can't break a fight. Other
players don't need it, and it sends nothing beyond this PC (`127.0.0.1`).

## Install

1. Valheim needs BepInEx (the *BepInExPack Valheim* from Thunderstore, which
   mod managers install for you).
2. Build the plugin (needs the .NET SDK):

   ```
   dotnet build -c Release
   ```

   If Valheim isn't in the default Steam folder, add
   `-p:ValheimDir="E:\SteamLibrary\steamapps\common\Valheim"`.
3. Copy `bin\Release\netstandard2.1\AK820KeyboardLights.dll` into
   `Valheim\BepInEx\plugins\`.
4. Bind some effects (above) and keep the knob program running.

The plugin sends to port 50820. That can be changed in
`BepInEx\config\ak820customknob.valheim.keyboardlights.cfg`, but the knob
program always listens on 50820.

If nothing happens, `BepInEx\LogOutput.log` shows whether the plugin loaded
and any warning it had, and the knob program's log shows `game: valheim is
reporting` once it hears from it.

## Protocol

One UDP datagram of text per line to 127.0.0.1:50820. State lines whenever
the state changes and at least every 0.5 s:

```
knob-game 1 valheim health=0.85 dead=0 effects=burning,resting biome=swamp night=1 weather=ThunderStorm boss=0
```

Event lines once, the moment something happens (`amount` is the hit's share
of the target's health):

```
knob-game 1 valheim event=hurt amount=0.30
```

Other games can send the same format with their own fields; `game:` tests
work on any field. See `crates/knob-core/src/game.rs`.
