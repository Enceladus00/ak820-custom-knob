//! The configuration file: modes (gesture → action bindings) and profiles
//! (which modes apply to which app).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::gesture::{GestureKind, GestureTiming};
use crate::keys::KeyCombo;
use crate::signal::{MS, Modifiers};

pub const CONFIG_VERSION: u32 = 1;
pub const DEFAULT_CONFIG: &str = include_str!("default-config.toml");

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid configuration:\n  - {}", .0.join("\n  - "))]
    Invalid(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub ui: UiPrefs,
    #[serde(default)]
    pub timing: Timing,
    #[serde(rename = "mode")]
    pub modes: Vec<Mode>,
    #[serde(rename = "profile")]
    pub profiles: Vec<Profile>,
    /// Saved lighting layouts. Files written before these existed get the
    /// built-in set.
    #[serde(rename = "lighting", default = "default_layouts")]
    pub layouts: Vec<LightingLayout>,
}

fn default_layouts() -> Vec<LightingLayout> {
    Config::default_config().layouts
}

/// A named lighting setup. It reaches the keyboard only when the user applies
/// it, because each write may be stored in the keyboard's flash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightingLayout {
    pub name: String,
    /// Built-in effect number, 1 to 19.
    pub effect: u8,
    /// `#RRGGBB`.
    pub color: String,
    /// Cycle through colours instead of using `color`.
    #[serde(default)]
    pub colorful: bool,
    /// 1 to 5.
    pub brightness: u8,
    /// 1 to 5.
    pub speed: u8,
}

impl LightingLayout {
    /// `color` as red, green, blue, if it is well formed.
    pub fn rgb(&self) -> Option<[u8; 3]> {
        let digits = self.color.strip_prefix('#')?;
        if digits.len() != 6 || !digits.is_ascii() {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
        Some([channel(0)?, channel(2)?, channel(4)?])
    }

    /// Adds a message to `errors` for everything wrong with this layout.
    pub fn check(&self, errors: &mut Vec<String>) {
        let ctx = format!("lighting layout '{}'", self.name);
        if self.name.trim().is_empty() {
            errors.push("a lighting layout has no name".into());
        }
        if !(1..=knob_rgb::EFFECT_MAX).contains(&self.effect) {
            errors.push(format!(
                "{ctx}: effect must be 1 to {}",
                knob_rgb::EFFECT_MAX
            ));
        }
        if self.rgb().is_none() {
            errors.push(format!("{ctx}: color must look like #66CCFF"));
        }
        if !knob_rgb::LEVEL_RANGE.contains(&self.brightness) {
            errors.push(format!("{ctx}: brightness must be 1 to 5"));
        }
        if !knob_rgb::LEVEL_RANGE.contains(&self.speed) {
            errors.push(format!("{ctx}: speed must be 1 to 5"));
        }
    }
}

/// Appearance and language of the program's own windows and popups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct UiPrefs {
    pub theme: Theme,
    /// `"auto"` follows the Windows display language; otherwise a language
    /// code such as `"en"`. Languages without a translation fall back to English.
    pub language: String,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: AUTO_LANGUAGE.into(),
        }
    }
}

pub const AUTO_LANGUAGE: &str = "auto";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow the Windows light / dark setting.
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Timing {
    pub long_press_ms: u64,
    pub double_click_ms: u64,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            long_press_ms: 500,
            double_click_ms: 250,
        }
    }
}

impl Timing {
    pub fn gesture_timing(&self) -> GestureTiming {
        GestureTiming {
            long_press: self.long_press_ms * MS,
            double_click: self.double_click_ms * MS,
        }
    }
}

/// What a gesture does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// System volume, `step` percent per detent.
    Volume {
        #[serde(default = "default_volume_step")]
        step: u32,
        #[serde(default = "yes")]
        accelerate: bool,
    },
    /// Volume of the focused app only.
    AppVolume {
        #[serde(default = "default_volume_step")]
        step: u32,
        #[serde(default = "yes")]
        accelerate: bool,
    },
    /// Toggle system mute.
    Mute,
    Scroll {
        #[serde(default = "one")]
        lines: u32,
        #[serde(default)]
        horizontal: bool,
        #[serde(default = "yes")]
        accelerate: bool,
    },
    /// Ctrl + mouse wheel.
    Zoom,
    /// Press `keys`; or for rotation, `cw` / `ccw`.
    Keys {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keys: Option<KeyCombo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cw: Option<KeyCombo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ccw: Option<KeyCombo>,
    },
    MouseClick {
        button: MouseButton,
    },
    /// Switch to the profile's next mode.
    CycleMode,
    /// Stock knob behaviour: let the volume / mute key through.
    Passthrough,
    /// Do nothing.
    #[serde(rename = "none")]
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

fn default_volume_step() -> u32 {
    2
}
fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}

impl Action {
    /// Can this action be bound to rotation?
    pub fn fits_rotate(&self) -> bool {
        match self {
            Action::Volume { .. }
            | Action::AppVolume { .. }
            | Action::Scroll { .. }
            | Action::Zoom
            | Action::Passthrough
            | Action::Nothing => true,
            Action::Keys { keys, cw, ccw } => keys.is_none() && cw.is_some() && ccw.is_some(),
            Action::Mute | Action::MouseClick { .. } | Action::CycleMode => false,
        }
    }

    /// Can this action be bound to click / double-click / long press?
    pub fn fits_press(&self) -> bool {
        match self {
            Action::Mute
            | Action::MouseClick { .. }
            | Action::CycleMode
            | Action::Passthrough
            | Action::Nothing => true,
            Action::Keys { keys, cw, ccw } => keys.is_some() && cw.is_none() && ccw.is_none(),
            Action::Volume { .. }
            | Action::AppVolume { .. }
            | Action::Scroll { .. }
            | Action::Zoom => false,
        }
    }
}

/// Gesture bindings. Used for a mode itself and for its modifier layers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bindings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotate: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub click: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_click: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_press: Option<Action>,
}

impl Bindings {
    pub fn get(&self, kind: GestureKind) -> Option<&Action> {
        match kind {
            GestureKind::Rotate => self.rotate.as_ref(),
            GestureKind::Click => self.click.as_ref(),
            GestureKind::DoubleClick => self.double_click.as_ref(),
            GestureKind::LongPress => self.long_press.as_ref(),
        }
    }

    fn validate(&self, context: &str, errors: &mut Vec<String>) {
        if let Some(a) = &self.rotate
            && !a.fits_rotate()
        {
            errors.push(format!(
                "{context}: '{}' can't be used for rotate",
                action_name(a)
            ));
        }
        for (name, action) in [
            ("click", &self.click),
            ("double_click", &self.double_click),
            ("long_press", &self.long_press),
        ] {
            if let Some(a) = action
                && !a.fits_press()
            {
                errors.push(format!(
                    "{context}: '{}' can't be used for {name}",
                    action_name(a)
                ));
            }
        }
    }
}

fn action_name(action: &Action) -> String {
    match action {
        Action::Keys { .. } => "keys (rotate needs cw + ccw, presses need keys)".into(),
        other => format!("{other:?}")
            .split([' ', '{'])
            .next()
            .unwrap_or("")
            .to_lowercase(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mode {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotate: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub click: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_click: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_press: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctrl: Option<Bindings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift: Option<Bindings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<Bindings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub win: Option<Bindings>,
}

impl Mode {
    /// The action for a gesture, honouring held modifiers. Layers are checked
    /// in the order Ctrl, Shift, Alt, Win; then the mode's own binding.
    pub fn binding(&self, kind: GestureKind, mods: Modifiers) -> Option<&Action> {
        let layers = [
            (mods.ctrl, &self.ctrl),
            (mods.shift, &self.shift),
            (mods.alt, &self.alt),
            (mods.win, &self.win),
        ];
        layers
            .into_iter()
            .filter(|(held, _)| *held)
            .find_map(|(_, layer)| layer.as_ref().and_then(|l| l.get(kind)))
            .or_else(|| self.own(kind))
    }

    pub fn uses_double_click(&self) -> bool {
        self.double_click.is_some() || self.layers().any(|(_, b)| b.double_click.is_some())
    }

    /// The mode's own binding for a gesture (no layers).
    fn own(&self, kind: GestureKind) -> Option<&Action> {
        match kind {
            GestureKind::Rotate => self.rotate.as_ref(),
            GestureKind::Click => self.click.as_ref(),
            GestureKind::DoubleClick => self.double_click.as_ref(),
            GestureKind::LongPress => self.long_press.as_ref(),
        }
    }

    fn own_bindings(&self) -> Bindings {
        Bindings {
            rotate: self.rotate.clone(),
            click: self.click.clone(),
            double_click: self.double_click.clone(),
            long_press: self.long_press.clone(),
        }
    }

    fn layers(&self) -> impl Iterator<Item = (&'static str, &Bindings)> {
        [
            ("ctrl", &self.ctrl),
            ("shift", &self.shift),
            ("alt", &self.alt),
            ("win", &self.win),
        ]
        .into_iter()
        .filter_map(|(name, layer)| layer.as_ref().map(|l| (name, l)))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    #[serde(rename = "match")]
    pub matcher: Matcher,
    pub modes: Vec<String>,
    /// Come back to the mode you left this app in (instead of the first one).
    #[serde(default = "yes")]
    pub remember_last_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Matcher {
    /// Exe file name(s), case-insensitive. `"*"` matches every app.
    pub exe: OneOrMany,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        let items: &[String] = match self {
            OneOrMany::One(s) => std::slice::from_ref(s),
            OneOrMany::Many(v) => v,
        };
        items.iter().map(String::as_str)
    }
}

impl Matcher {
    pub fn matches(&self, exe: &str) -> bool {
        self.exe
            .iter()
            .any(|pattern| pattern == "*" || pattern.eq_ignore_ascii_case(exe))
    }
}

impl Config {
    /// Parses and validates a config file.
    pub fn from_toml(text: &str) -> Result<Config, ConfigError> {
        let config: Config = toml::from_str(text)?;
        config.validate()?;
        Ok(config)
    }

    /// The built-in default configuration.
    pub fn default_config() -> Config {
        Self::from_toml(DEFAULT_CONFIG).expect("built-in default config is valid")
    }

    pub fn mode(&self, id: &str) -> Option<&Mode> {
        self.modes.iter().find(|m| m.id == id)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut errors = Vec::new();
        if self.version != CONFIG_VERSION {
            errors.push(format!(
                "version must be {CONFIG_VERSION}, found {}",
                self.version
            ));
        }
        let language = &self.ui.language;
        let looks_like_code = language.len() <= 16
            && !language.is_empty()
            && language
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !looks_like_code {
            errors.push(format!(
                "language must be \"{AUTO_LANGUAGE}\" or a language code like \"en\", found \"{language}\""
            ));
        }
        if self.modes.is_empty() {
            errors.push("at least one [[mode]] is needed".into());
        }
        let mut ids = HashSet::new();
        for mode in &self.modes {
            if mode.id.trim().is_empty() {
                errors.push(format!("mode '{}' has an empty id", mode.name));
            } else if !ids.insert(mode.id.as_str()) {
                errors.push(format!("mode id '{}' is used twice", mode.id));
            }
            let ctx = format!("mode '{}'", mode.id);
            mode.own_bindings().validate(&ctx, &mut errors);
            for (name, layer) in mode.layers() {
                layer.validate(&format!("{ctx}, {name} layer"), &mut errors);
            }
        }
        if self.profiles.is_empty() {
            errors.push("at least one [[profile]] is needed".into());
        }
        for profile in &self.profiles {
            if profile.modes.is_empty() {
                errors.push(format!("profile '{}' lists no modes", profile.name));
            }
            for id in &profile.modes {
                if !ids.contains(id.as_str()) {
                    errors.push(format!(
                        "profile '{}' uses unknown mode '{id}'",
                        profile.name
                    ));
                }
            }
        }
        for layout in &self.layouts {
            layout.check(&mut errors);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Invalid(errors))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = Config::default_config();
        assert_eq!(config.modes.len(), 3);
        let volume = config.mode("volume").unwrap();
        assert_eq!(
            volume.binding(GestureKind::Rotate, Modifiers::default()),
            Some(&Action::Volume {
                step: 2,
                accelerate: true
            })
        );
        assert!(!volume.uses_double_click());
    }

    #[test]
    fn layers_fall_back_to_the_mode() {
        let config = Config::default_config();
        let volume = config.mode("volume").unwrap();
        let ctrl = Modifiers {
            ctrl: true,
            ..Default::default()
        };
        assert_eq!(
            volume.binding(GestureKind::Rotate, ctrl),
            Some(&Action::AppVolume {
                step: 5,
                accelerate: true
            })
        );
        // No click in the ctrl layer: the mode's own click applies.
        assert_eq!(
            volume.binding(GestureKind::Click, ctrl),
            Some(&Action::Mute)
        );
    }

    #[test]
    fn config_round_trips_through_toml() {
        let config = Config::default_config();
        let text = toml::to_string(&config).unwrap();
        assert_eq!(Config::from_toml(&text).unwrap(), config);
    }

    #[test]
    fn reports_all_problems() {
        let text = r#"
            version = 1
            [[mode]]
            id = "a"
            name = "A"
            rotate = { action = "mute" }
            click = { action = "keys", cw = "Left", ccw = "Right" }
            [[profile]]
            name = "P"
            match = { exe = ["x.exe", "y.exe"] }
            modes = ["a", "missing"]
        "#;
        let Err(ConfigError::Invalid(errors)) = Config::from_toml(text) else {
            panic!("should be invalid")
        };
        assert_eq!(errors.len(), 3, "{errors:#?}");
    }

    #[test]
    fn bad_key_names_are_parse_errors() {
        let text = DEFAULT_CONFIG.replace("MediaNext", "MediaNxt");
        assert!(matches!(
            Config::from_toml(&text),
            Err(ConfigError::Parse(_))
        ));
    }

    #[test]
    fn ui_section_is_optional_and_checked() {
        // Files written before [ui] existed still load, with defaults.
        let old = DEFAULT_CONFIG
            .replace("[ui]", "")
            .replace("theme = \"system\"", "")
            .replace("language = \"auto\"", "");
        let config = Config::from_toml(&old).unwrap();
        assert_eq!(config.ui, UiPrefs::default());

        let dark = DEFAULT_CONFIG
            .replace("theme = \"system\"", "theme = \"dark\"")
            .replace("\"auto\"", "\"tr\"");
        let config = Config::from_toml(&dark).unwrap();
        assert_eq!(
            (config.ui.theme, config.ui.language.as_str()),
            (Theme::Dark, "tr")
        );

        let bad = DEFAULT_CONFIG.replace("language = \"auto\"", "language = \"not a code!\"");
        assert!(matches!(
            Config::from_toml(&bad),
            Err(ConfigError::Invalid(_))
        ));
    }

    fn without_layouts() -> String {
        let cut = DEFAULT_CONFIG
            .find("# ---------------------------------------------------------------- lighting")
            .unwrap();
        DEFAULT_CONFIG[..cut].to_string()
    }

    #[test]
    fn older_files_get_the_built_in_layouts() {
        let config = Config::from_toml(&without_layouts()).unwrap();
        assert!(!config.layouts.is_empty());
        assert_eq!(config.layouts, Config::default_config().layouts);
    }

    #[test]
    fn every_layout_can_be_deleted() {
        let mut config = Config::default_config();
        config.layouts.clear();
        let text = toml::to_string(&config).unwrap();
        assert!(Config::from_toml(&text).unwrap().layouts.is_empty());
    }

    fn layout() -> LightingLayout {
        LightingLayout {
            name: "Test".into(),
            effect: 1,
            color: "#66CCFF".into(),
            colorful: false,
            brightness: 3,
            speed: 3,
        }
    }

    fn problems(layout: &LightingLayout) -> usize {
        let mut errors = Vec::new();
        layout.check(&mut errors);
        errors.len()
    }

    #[test]
    fn colors_parse_in_either_case() {
        assert_eq!(layout().rgb(), Some([0x66, 0xCC, 0xFF]));
        let lower = LightingLayout {
            color: "#66ccff".into(),
            ..layout()
        };
        assert_eq!(lower.rgb(), Some([0x66, 0xCC, 0xFF]));
    }

    #[test]
    fn a_good_layout_has_no_problems() {
        assert_eq!(problems(&layout()), 0);
    }

    #[test]
    fn bad_layouts_are_reported() {
        for bad in [
            LightingLayout {
                name: " ".into(),
                ..layout()
            },
            LightingLayout {
                effect: 0,
                ..layout()
            },
            LightingLayout {
                effect: 0x14,
                ..layout()
            },
            LightingLayout {
                effect: knob_rgb::EFFECT_CUSTOM,
                ..layout()
            },
            LightingLayout {
                color: "blue".into(),
                ..layout()
            },
            LightingLayout {
                color: "#12345".into(),
                ..layout()
            },
            LightingLayout {
                color: "#ééé".into(),
                ..layout()
            },
            LightingLayout {
                brightness: 0,
                ..layout()
            },
            LightingLayout {
                speed: 6,
                ..layout()
            },
        ] {
            assert_eq!(problems(&bad), 1, "{bad:?}");
        }
    }

    #[test]
    fn matcher_is_case_insensitive() {
        let m = Matcher {
            exe: OneOrMany::Many(vec!["Resolve.exe".into()]),
        };
        assert!(m.matches("resolve.EXE"));
        assert!(!m.matches("code.exe"));
    }
}
