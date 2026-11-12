//! The configuration file: modes (gesture → action bindings) and profiles
//! (which modes apply to which app).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::extras::{HintSettings, MacroDef, MouseSettings};
use crate::gesture::{GestureKind, GestureTiming};
use crate::keys::KeyCombo;
use crate::remap::RemapDef;
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
    /// Lighting effects for the patched (LIVE v2) firmware, see
    /// docs/effects-format.md. Files written before these existed get the
    /// built-in set.
    #[serde(rename = "effects", default = "default_effects")]
    pub effects: Vec<Effect>,
    #[serde(rename = "macro", default, skip_serializing_if = "Vec::is_empty")]
    pub macros: Vec<MacroDef>,
    #[serde(default)]
    pub mouse: MouseSettings,
    #[serde(default)]
    pub hints: HintSettings,
    #[serde(rename = "remap", default, skip_serializing_if = "Vec::is_empty")]
    pub remaps: Vec<RemapDef>,
}

fn default_layouts() -> Vec<LightingLayout> {
    Config::default_config().layouts
}

fn default_effects() -> Vec<Effect> {
    Config::default_config().effects
}

/// How an effect paints the keys. The formulas are in docs/effects-format.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pattern {
    Solid,
    Pulse,
    Wave,
    Flicker,
    Sparkle,
}

impl Pattern {
    /// The pattern's number in the keyboard's define command.
    pub fn code(self) -> u8 {
        match self {
            Pattern::Solid => 0,
            Pattern::Pulse => 1,
            Pattern::Wave => 2,
            Pattern::Flicker => 3,
            Pattern::Sparkle => 4,
        }
    }
}

pub const EFFECT_SLOTS: std::ops::RangeInclusive<u8> = 1..=8;
pub const EFFECT_MAX_COLORS: usize = 4;
pub const EFFECT_MAX_DURATION_MS: u32 = 60_000;

/// A lighting effect the keyboard's effect engine plays from RAM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub name: String,
    /// 1 to 8: where it lives on the keyboard.
    pub slot: u8,
    pub pattern: Pattern,
    /// 1 to 4 colours, `#RRGGBB`.
    pub colors: Vec<String>,
    /// 1 to 10.
    pub speed: u8,
    /// 1 to 100 percent.
    pub brightness: u8,
    /// 0 plays until stopped; otherwise it stops by itself (at most 60000).
    #[serde(default)]
    pub duration_ms: u32,
    /// Situations in which the effect plays by itself, see [`Situation`].
    /// Empty: only when started by hand.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub when: Vec<String>,
}

/// When an effect plays by itself, written as text in the config:
/// `app:<program.exe>` while that program is in front, `mode:<name>` while
/// that knob mode is active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Situation {
    App(String),
    Mode(String),
}

impl Situation {
    pub fn parse(text: &str) -> Option<Self> {
        let (kind, value) = text.split_once(':')?;
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        match kind.trim() {
            "app" => Some(Situation::App(value.to_lowercase())),
            "mode" => Some(Situation::Mode(value.to_lowercase())),
            _ => None,
        }
    }

    /// Whether this situation holds for the program in front (its exe name)
    /// and the active knob mode (its id or name).
    pub fn holds(&self, app: &str, mode_id: &str, mode_name: &str) -> bool {
        match self {
            Situation::App(exe) => app.eq_ignore_ascii_case(exe),
            Situation::Mode(m) => {
                mode_id.eq_ignore_ascii_case(m) || mode_name.eq_ignore_ascii_case(m)
            }
        }
    }
}

/// The effect that should be playing now: the first one in the list with a
/// situation that holds, so the list order is the priority.
pub fn effect_for<'a>(
    effects: &'a [Effect],
    app: &str,
    mode_id: &str,
    mode_name: &str,
) -> Option<&'a Effect> {
    effects.iter().find(|e| {
        e.when
            .iter()
            .filter_map(|w| Situation::parse(w))
            .any(|s| s.holds(app, mode_id, mode_name))
    })
}

impl Effect {
    /// The keyboard command form of this effect, if it is valid.
    pub fn to_def(&self) -> Option<knob_rgb::EffectDef> {
        let pattern = match self.pattern {
            Pattern::Solid => knob_rgb::Pattern::Solid,
            Pattern::Pulse => knob_rgb::Pattern::Pulse,
            Pattern::Wave => knob_rgb::Pattern::Wave,
            Pattern::Flicker => knob_rgb::Pattern::Flicker,
            Pattern::Sparkle => knob_rgb::Pattern::Sparkle,
        };
        let colors = self.rgb_colors()?;
        knob_rgb::EffectDef::new(
            self.slot,
            pattern,
            self.speed,
            self.brightness,
            &colors,
            self.duration_ms,
        )
        .ok()
    }

    /// The colours as red, green, blue, if they are all well formed.
    pub fn rgb_colors(&self) -> Option<Vec<[u8; 3]>> {
        self.colors
            .iter()
            .map(|c| crate::hint_style::parse_color(c))
            .collect()
    }

    /// Adds a message to `errors` for everything wrong with this effect on
    /// its own. Unique names and slots are checked by [`check_effects`].
    pub fn check(&self, errors: &mut Vec<String>) {
        let ctx = format!("effect '{}'", self.name);
        if self.name.trim().is_empty() {
            errors.push("an effect has no name".into());
        }
        if !EFFECT_SLOTS.contains(&self.slot) {
            errors.push(format!("{ctx}: slot must be 1 to 8"));
        }
        if !(1..=EFFECT_MAX_COLORS).contains(&self.colors.len()) {
            errors.push(format!("{ctx}: needs 1 to 4 colours"));
        }
        if self
            .colors
            .iter()
            .any(|c| crate::hint_style::parse_color(c).is_none())
        {
            errors.push(format!("{ctx}: colours must look like #FF2000"));
        }
        if !(1..=10).contains(&self.speed) {
            errors.push(format!("{ctx}: speed must be 1 to 10"));
        }
        if !(1..=100).contains(&self.brightness) {
            errors.push(format!("{ctx}: brightness must be 1 to 100"));
        }
        if self.duration_ms > EFFECT_MAX_DURATION_MS {
            errors.push(format!("{ctx}: duration must be 0 to 60000 ms"));
        }
        for when in &self.when {
            if Situation::parse(when).is_none() {
                errors.push(format!(
                    "{ctx}: '{when}' is not a situation (use app:<program.exe> or mode:<name>)"
                ));
            }
        }
    }
}

/// Checks every effect, and that no two share a name or a slot.
pub fn check_effects(effects: &[Effect], errors: &mut Vec<String>) {
    let mut names = HashSet::new();
    let mut slots = HashSet::new();
    for effect in effects {
        effect.check(errors);
        let name = effect.name.trim().to_lowercase();
        if !name.is_empty() && !names.insert(name) {
            errors.push(format!(
                "effect name '{}' is used twice",
                effect.name.trim()
            ));
        }
        if EFFECT_SLOTS.contains(&effect.slot) && !slots.insert(effect.slot) {
            errors.push(format!("effect slot {} is used twice", effect.slot));
        }
    }
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
        crate::hint_style::parse_color(&self.color)
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
    /// Volume of the microphone (the default recording device).
    MicVolume {
        #[serde(default = "default_mic_step")]
        step: u32,
        #[serde(default = "yes")]
        accelerate: bool,
    },
    /// Mute or unmute the microphone.
    MicMute,
    /// Make the next sound output the default; turning goes back and forth.
    AudioOutput,
    /// Open an app, a file, a folder or a web address.
    Open {
        target: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        args: String,
    },
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
    /// Turn arrow-key mouse control on or off.
    ToggleMouseMode,
    /// Show letter labels on what can be clicked in the app in front.
    ClickHints,
    RunMacro {
        name: String,
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
fn default_mic_step() -> u32 {
    5
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
            | Action::MicVolume { .. }
            | Action::AudioOutput
            | Action::Scroll { .. }
            | Action::Zoom
            | Action::Passthrough
            | Action::Nothing => true,
            Action::Keys { keys, cw, ccw } => keys.is_none() && cw.is_some() && ccw.is_some(),
            Action::Mute
            | Action::MicMute
            | Action::Open { .. }
            | Action::MouseClick { .. }
            | Action::CycleMode
            | Action::ToggleMouseMode
            | Action::ClickHints
            | Action::RunMacro { .. } => false,
        }
    }

    /// Can this action be bound to click / double-click / long press?
    pub fn fits_press(&self) -> bool {
        match self {
            Action::Mute
            | Action::MicMute
            | Action::AudioOutput
            | Action::Open { .. }
            | Action::MouseClick { .. }
            | Action::CycleMode
            | Action::ToggleMouseMode
            | Action::ClickHints
            | Action::RunMacro { .. }
            | Action::Passthrough
            | Action::Nothing => true,
            Action::Keys { keys, cw, ccw } => keys.is_some() && cw.is_none() && ccw.is_none(),
            Action::Volume { .. }
            | Action::AppVolume { .. }
            | Action::MicVolume { .. }
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
        for action in [
            &self.rotate,
            &self.click,
            &self.double_click,
            &self.long_press,
        ]
        .into_iter()
        .flatten()
        {
            if let Action::Open { target, .. } = action
                && target.trim().is_empty()
            {
                errors.push(format!("{context}: choose what to open"));
            }
            if let Action::MicVolume { step, .. } = action
                && !(1..=25).contains(step)
            {
                errors.push(format!("{context}: microphone step must be 1 to 25"));
            }
        }
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

    /// Every action bound anywhere in this mode.
    pub(crate) fn all_actions(&self) -> impl Iterator<Item = &Action> {
        let own = [
            &self.rotate,
            &self.click,
            &self.double_click,
            &self.long_press,
        ];
        let layers = self
            .layers()
            .flat_map(|(_, b)| [&b.rotate, &b.click, &b.double_click, &b.long_press]);
        own.into_iter().chain(layers).filter_map(Option::as_ref)
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
        check_effects(&self.effects, &mut errors);
        self.check_extras(&mut errors);
        self.check_remaps(&mut errors);
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
    fn sound_and_open_actions_load_and_check() {
        let text = DEFAULT_CONFIG.replacen(
            "[[mode]]",
            r#"[[mode]]
id = "sound"
name = "Sound"
rotate = { action = "mic_volume" }
click = { action = "mic_mute" }
double_click = { action = "audio_output" }
long_press = { action = "open", target = "https://example.com" }

[[mode]]"#,
            1,
        );
        let config = Config::from_toml(&text).unwrap();
        let mode = config.mode("sound").unwrap();
        assert_eq!(
            mode.rotate,
            Some(Action::MicVolume {
                step: 5,
                accelerate: true
            })
        );
        assert_eq!(mode.click, Some(Action::MicMute));
        assert!(Action::AudioOutput.fits_rotate() && Action::AudioOutput.fits_press());

        let empty = text.replace(r#"target = "https://example.com""#, r#"target = " ""#);
        assert!(Config::from_toml(&empty).is_err());
        let rotate_open = text.replace(
            r#"rotate = { action = "mic_volume" }"#,
            r#"rotate = { action = "open", target = "x" }"#,
        );
        assert!(Config::from_toml(&rotate_open).is_err());
    }

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

    fn effect() -> Effect {
        Effect {
            name: "Test".into(),
            slot: 1,
            pattern: Pattern::Pulse,
            colors: vec!["#FF2000".into(), "#200000".into()],
            speed: 5,
            brightness: 100,
            duration_ms: 0,
            when: vec![],
        }
    }

    #[test]
    fn situations_parse_and_match() {
        assert_eq!(
            Situation::parse("app:Valheim.exe"),
            Some(Situation::App("valheim.exe".into()))
        );
        assert_eq!(
            Situation::parse("mode: Volume "),
            Some(Situation::Mode("volume".into()))
        );
        assert_eq!(Situation::parse("app:"), None);
        assert_eq!(Situation::parse("weather:rain"), None);
        assert_eq!(Situation::parse("valheim.exe"), None);
        let s = Situation::parse("app:valheim.exe").unwrap();
        assert!(s.holds("Valheim.EXE", "m1", "Volume"));
        assert!(!s.holds("code.exe", "m1", "Volume"));
        let m = Situation::parse("mode:volume").unwrap();
        assert!(m.holds("x.exe", "m1", "Volume"));
        assert!(
            Situation::parse("mode:M1")
                .unwrap()
                .holds("x.exe", "m1", "Volume")
        );
    }

    #[test]
    fn the_first_matching_effect_wins() {
        let fire = Effect {
            name: "Fire".into(),
            when: vec!["app:valheim.exe".into()],
            ..effect()
        };
        let calm = Effect {
            name: "Calm".into(),
            slot: 2,
            when: vec!["mode:scroll".into(), "app:valheim.exe".into()],
            ..effect()
        };
        let manual = Effect {
            name: "Manual".into(),
            slot: 3,
            ..effect()
        };
        let effects = [manual, fire, calm];
        let pick = |app, mode| effect_for(&effects, app, "", mode).map(|e| e.name.as_str());
        assert_eq!(pick("valheim.exe", "volume"), Some("Fire"));
        assert_eq!(pick("code.exe", "Scroll"), Some("Calm"));
        assert_eq!(pick("code.exe", "volume"), None);
    }

    #[test]
    fn bad_situations_are_reported_and_good_ones_convert() {
        let bad = Effect {
            when: vec!["app:valheim.exe".into(), "sometimes".into()],
            ..effect()
        };
        assert_eq!(effect_problems(&[bad]).len(), 1);
        let def = effect().to_def().unwrap();
        assert_eq!(def.slot(), 1);
        assert_eq!(def.payload()[..5], [1, 1, 5, 100, 2]);
    }

    #[test]
    fn situations_round_trip_and_are_omitted_when_empty() {
        let mut config = Config::default_config();
        config.effects[0].when = vec!["app:valheim.exe".into()];
        let text = toml::to_string(&config).unwrap();
        assert_eq!(text.matches("when = ").count(), 1);
        assert_eq!(
            Config::from_toml(&text).unwrap().effects[0].when,
            ["app:valheim.exe"]
        );
    }

    fn effect_problems(effects: &[Effect]) -> Vec<String> {
        let mut errors = Vec::new();
        check_effects(effects, &mut errors);
        errors
    }

    #[test]
    fn default_effects_match_the_format_doc() {
        let effects = Config::default_config().effects;
        let summary: Vec<_> = effects
            .iter()
            .map(|e| (e.name.as_str(), e.slot, e.pattern, e.colors.len(), e.speed))
            .collect();
        assert_eq!(
            summary,
            [
                ("Fire", 1, Pattern::Flicker, 3, 6),
                ("Frost", 2, Pattern::Wave, 3, 3),
                ("Poison", 3, Pattern::Pulse, 2, 5),
                ("Low health", 4, Pattern::Pulse, 2, 8),
                ("Shock", 5, Pattern::Sparkle, 3, 9),
            ]
        );
        assert_eq!(effects[0].rgb_colors().unwrap()[0], [0xFF, 0x20, 0x00]);
    }

    #[test]
    fn older_files_get_the_built_in_effects() {
        let start = DEFAULT_CONFIG
            .find("# ---------------------------------------------------------------- effects")
            .unwrap();
        let len = DEFAULT_CONFIG[start..]
            .find(
                "# ---------------------------------------------------------------- key remapping",
            )
            .unwrap();
        let old = format!(
            "{}{}",
            &DEFAULT_CONFIG[..start],
            &DEFAULT_CONFIG[start + len..]
        );
        assert!(!old.contains("[[effects]]"));
        let config = Config::from_toml(&old).unwrap();
        assert_eq!(config.effects.len(), 5);
        assert_eq!(config.effects, Config::default_config().effects);
    }

    #[test]
    fn every_effect_can_be_deleted() {
        let mut config = Config::default_config();
        config.effects.clear();
        let text = toml::to_string(&config).unwrap();
        assert!(Config::from_toml(&text).unwrap().effects.is_empty());
    }

    #[test]
    fn patterns_are_lowercase_and_unknown_ones_fail() {
        let text = DEFAULT_CONFIG.replace(r#"pattern = "wave""#, r#"pattern = "Wave""#);
        assert!(matches!(
            Config::from_toml(&text),
            Err(ConfigError::Parse(_))
        ));
        let codes = [
            Pattern::Solid,
            Pattern::Pulse,
            Pattern::Wave,
            Pattern::Flicker,
            Pattern::Sparkle,
        ]
        .map(Pattern::code);
        assert_eq!(codes, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn good_effects_have_no_problems() {
        let edges = [
            effect(),
            Effect {
                name: "One colour".into(),
                slot: 8,
                colors: vec!["#ffffff".into()],
                speed: 10,
                brightness: 1,
                duration_ms: 60_000,
                ..effect()
            },
            Effect {
                name: "Four".into(),
                slot: 2,
                colors: vec!["#000000".into(); 4],
                speed: 1,
                ..effect()
            },
        ];
        assert_eq!(effect_problems(&edges), Vec::<String>::new());
    }

    #[test]
    fn bad_effects_are_reported() {
        for bad in [
            Effect {
                name: " ".into(),
                ..effect()
            },
            Effect {
                slot: 0,
                ..effect()
            },
            Effect {
                slot: 9,
                ..effect()
            },
            Effect {
                colors: vec![],
                ..effect()
            },
            Effect {
                colors: vec!["#FFFFFF".into(); 5],
                ..effect()
            },
            Effect {
                colors: vec!["#FFF".into()],
                ..effect()
            },
            Effect {
                colors: vec!["#FF2000".into(), "red".into()],
                ..effect()
            },
            Effect {
                speed: 0,
                ..effect()
            },
            Effect {
                speed: 11,
                ..effect()
            },
            Effect {
                brightness: 0,
                ..effect()
            },
            Effect {
                brightness: 101,
                ..effect()
            },
            Effect {
                duration_ms: 60_001,
                ..effect()
            },
        ] {
            assert_eq!(
                effect_problems(std::slice::from_ref(&bad)).len(),
                1,
                "{bad:?}"
            );
        }
    }

    #[test]
    fn effect_names_and_slots_must_be_unique() {
        let other = Effect {
            name: "Other".into(),
            slot: 2,
            ..effect()
        };
        assert!(effect_problems(&[effect(), other.clone()]).is_empty());
        let same_name = Effect {
            name: " test ".into(),
            ..other.clone()
        };
        assert_eq!(effect_problems(&[effect(), same_name]).len(), 1);
        let same_slot = Effect { slot: 1, ..other };
        assert_eq!(effect_problems(&[effect(), same_slot]).len(), 1);

        let mut config = Config::default_config();
        config.effects[1].slot = 1;
        assert!(matches!(config.validate(), Err(ConfigError::Invalid(_))));
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
