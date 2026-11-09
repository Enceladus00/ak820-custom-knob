//! Optional features beyond the knob: macros, mouse mode and click hints.
//! All of them are off until the user sets them up.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::config::{Action, Config, MouseButton};
use crate::hint_style::HintStyle;
use crate::keys::{Key, KeyCombo};

/// A saved sequence of steps, started by a hotkey or a knob gesture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacroDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<KeyCombo>,
    #[serde(default = "one")]
    pub repeat: u32,
    pub steps: Vec<MacroStep>,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum MacroStep {
    Keys { keys: KeyCombo },
    Text { text: String },
    Wait { ms: u32 },
    Click { button: MouseButton },
}

pub const MAX_REPEAT: u32 = 100;
pub const MAX_WAIT_MS: u32 = 60_000;
pub const MAX_STEPS: usize = 500;

/// Moving the pointer with the arrow keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct MouseSettings {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toggle: Option<KeyCombo>,
    /// Top speed in pixels per second.
    pub speed: u32,
}

pub const MOUSE_SPEED_RANGE: std::ops::RangeInclusive<u32> = 200..=5000;

impl Default for MouseSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            toggle: "Ctrl+Alt+M".parse().ok(),
            speed: 1400,
        }
    }
}

/// Letter labels on everything clickable in the app in front.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct HintSettings {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<KeyCombo>,
    pub style: HintStyle,
}

impl Default for HintSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            hotkey: "Ctrl+Alt+H".parse().ok(),
            style: HintStyle::default(),
        }
    }
}

const fn is_modifier_vk(vk: u16) -> bool {
    matches!(vk, 0x10..=0x12 | 0xA0..=0xA5 | 0x5B | 0x5C)
}

/// A hotkey must not get in the way of normal typing: it needs Ctrl, Alt or
/// Win, unless it is a function key.
pub fn is_usable_hotkey(combo: &KeyCombo) -> bool {
    let function_key = matches!(combo.key, Key::Vk(0x70..=0x87));
    let not_modifier = !matches!(combo.key, Key::Vk(vk) if is_modifier_vk(vk));
    not_modifier && (combo.mods.ctrl || combo.mods.alt || combo.mods.win || function_key)
}

impl Config {
    pub(crate) fn check_extras(&self, errors: &mut Vec<String>) {
        let mut names = HashSet::new();
        let mut hotkeys = HashSet::new();
        let mut hotkey = |combo: &Option<KeyCombo>, ctx: &str, errors: &mut Vec<String>| {
            let Some(combo) = combo else { return };
            let text = combo.to_string();
            if !is_usable_hotkey(combo) {
                errors.push(format!(
                    "{ctx}: the hotkey {text} needs Ctrl, Alt or Win, or a function key"
                ));
            } else if !hotkeys.insert(text.to_lowercase()) {
                errors.push(format!("{ctx}: the hotkey {text} is already used"));
            }
        };

        if self.mouse.enabled {
            hotkey(&self.mouse.toggle, "mouse mode", errors);
        }
        if !MOUSE_SPEED_RANGE.contains(&self.mouse.speed) {
            errors.push(format!(
                "mouse mode: speed must be {} to {}",
                MOUSE_SPEED_RANGE.start(),
                MOUSE_SPEED_RANGE.end()
            ));
        }
        if self.hints.enabled {
            hotkey(&self.hints.hotkey, "click hints", errors);
        }

        self.hints.style.check(errors);

        for item in &self.macros {
            let ctx = format!("macro '{}'", item.name);
            if item.name.trim().is_empty() {
                errors.push("a macro has no name".into());
            } else if !names.insert(item.name.as_str()) {
                errors.push(format!("{ctx}: the name is used twice"));
            }
            hotkey(&item.hotkey, &ctx, errors);
            if !(1..=MAX_REPEAT).contains(&item.repeat) {
                errors.push(format!("{ctx}: repeat must be 1 to {MAX_REPEAT}"));
            }
            if item.steps.is_empty() {
                errors.push(format!("{ctx}: it has no steps"));
            } else if item.steps.len() > MAX_STEPS {
                errors.push(format!("{ctx}: more than {MAX_STEPS} steps"));
            }
            for step in &item.steps {
                match step {
                    MacroStep::Wait { ms } if *ms > MAX_WAIT_MS => {
                        errors.push(format!("{ctx}: a wait can't be over {MAX_WAIT_MS} ms"));
                    }
                    MacroStep::Text { text } if text.is_empty() => {
                        errors.push(format!("{ctx}: a text step is empty"));
                    }
                    _ => {}
                }
            }
        }

        for mode in &self.modes {
            for action in mode.all_actions() {
                let Action::RunMacro { name } = action else {
                    continue;
                };
                if !self.macros.iter().any(|m| &m.name == name) {
                    errors.push(format!(
                        "mode '{}' runs macro '{name}', which doesn't exist",
                        mode.id
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_CONFIG;

    fn combo(text: &str) -> KeyCombo {
        text.parse().unwrap()
    }

    fn config() -> Config {
        Config::default_config()
    }

    fn sample() -> MacroDef {
        MacroDef {
            name: "Hello".into(),
            hotkey: Some(combo("Ctrl+Alt+1")),
            repeat: 1,
            steps: vec![
                MacroStep::Keys { keys: combo("Tab") },
                MacroStep::Wait { ms: 50 },
                MacroStep::Text { text: "hi".into() },
                MacroStep::Click {
                    button: MouseButton::Left,
                },
            ],
        }
    }

    fn errors(config: &Config) -> Vec<String> {
        let mut errors = Vec::new();
        config.check_extras(&mut errors);
        errors
    }

    #[test]
    fn everything_is_off_by_default() {
        let config = config();
        assert!(!config.mouse.enabled);
        assert!(!config.hints.enabled);
        assert!(config.macros.is_empty());
    }

    #[test]
    fn older_files_load_with_the_features_off() {
        let config = Config::from_toml(DEFAULT_CONFIG).unwrap();
        assert_eq!(config.mouse, MouseSettings::default());
        assert_eq!(config.hints, HintSettings::default());
    }

    #[test]
    fn macros_round_trip_through_toml() {
        let mut config = config();
        config.macros.push(sample());
        config.mouse.enabled = true;
        let text = toml::to_string(&config).unwrap();
        assert_eq!(Config::from_toml(&text).unwrap(), config);
    }

    #[test]
    fn typing_keys_can_not_be_hotkeys() {
        assert!(!is_usable_hotkey(&combo("A")));
        assert!(!is_usable_hotkey(&combo("Shift+A")));
        assert!(is_usable_hotkey(&combo("Ctrl+A")));
        assert!(is_usable_hotkey(&combo("Win+K")));
        assert!(is_usable_hotkey(&combo("F13")));
    }

    #[test]
    fn duplicate_hotkeys_are_reported() {
        let mut config = config();
        config.macros.push(sample());
        let mut second = sample();
        second.name = "Other".into();
        config.macros.push(second);
        assert_eq!(errors(&config).len(), 1);
    }

    #[test]
    fn a_disabled_feature_can_not_clash() {
        let mut config = config();
        let mut item = sample();
        item.hotkey = config.mouse.toggle;
        config.macros.push(item);
        assert!(errors(&config).is_empty());
        config.mouse.enabled = true;
        assert_eq!(errors(&config).len(), 1);
    }

    #[test]
    fn bad_macros_are_reported() {
        let cases: [fn(&mut MacroDef); 5] = [
            |m| m.name = " ".into(),
            |m| m.repeat = 0,
            |m| m.steps.clear(),
            |m| {
                m.steps = vec![MacroStep::Wait {
                    ms: MAX_WAIT_MS + 1,
                }]
            },
            |m| m.steps = vec![MacroStep::Text { text: "".into() }],
        ];
        for change in cases {
            let mut config = config();
            let mut item = sample();
            change(&mut item);
            config.macros.push(item);
            assert_eq!(errors(&config).len(), 1, "{:?}", config.macros);
        }
    }

    #[test]
    fn a_knob_gesture_must_run_a_macro_that_exists() {
        let mut config = config();
        config.modes[0].long_press = Some(Action::RunMacro {
            name: "Hello".into(),
        });
        assert_eq!(errors(&config).len(), 1);
        config.macros.push(sample());
        assert!(errors(&config).is_empty());
    }
}
