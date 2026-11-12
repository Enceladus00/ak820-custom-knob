//! Key remapping: one key or shortcut becomes another key, a character or a
//! shortcut, in every app or only in some. Off until the user adds a rule.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::keys::{Key, KeyCombo};
use crate::signal::Modifiers;

/// One `[[remap]]` rule as written in the config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemapDef {
    /// A single key, or a key with Ctrl, Shift, Alt or Win (Alt+1).
    pub from: KeyCombo,
    /// A key, a character or a shortcut.
    pub to: KeyCombo,
    /// Exe file names the rule is limited to. Empty means every app.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<String>,
}

pub const MAX_REMAPS: usize = 200;

/// The volume and mute keys carry the knob's signals, so they stay with it.
const KNOB_KEYS: [u16; 3] = [0xAD, 0xAE, 0xAF];

/// What a remapped key sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemapOutput {
    /// Another key, held down for as long as the original is held. Used when
    /// the target is a single key, so Caps Lock → Ctrl works as a real Ctrl.
    Hold(u16),
    /// A shortcut or a character, sent once per press. A plain character also
    /// repeats while held, like typing; a shortcut doesn't, so holding a key
    /// mapped to Win+D doesn't flicker the desktop. With `lift`, the modifiers
    /// held for the original shortcut are let go while it is sent, so Alt+1
    /// can type ">" instead of Alt+>.
    Tap {
        combo: KeyCombo,
        repeats: bool,
        lift: bool,
    },
}

impl RemapOutput {
    pub fn for_target(to: KeyCombo) -> Self {
        match to.key {
            Key::Vk(vk) if to.mods.is_empty() => Self::Hold(vk),
            Key::Char(_) => Self::Tap {
                combo: to,
                repeats: to.mods.is_empty(),
                lift: false,
            },
            Key::Vk(_) => Self::Tap {
                combo: to,
                repeats: false,
                lift: false,
            },
        }
    }

    /// What a rule sends. A shortcut (Alt+1) can't hold a key, because its
    /// modifiers would mix in, so it always taps with the modifiers lifted.
    pub fn for_rule(from_mods: Modifiers, to: KeyCombo) -> Self {
        if from_mods.is_empty() {
            return Self::for_target(to);
        }
        Self::Tap {
            combo: to,
            repeats: to.mods.is_empty(),
            lift: true,
        }
    }
}

/// A rule ready for the router: the key to change as a virtual-key code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remap {
    pub vk: u16,
    /// Modifiers that must be held with the key. None: the key alone.
    pub mods: Modifiers,
    pub output: RemapOutput,
    /// Lower-case exe names. Empty means every app.
    pub apps: Vec<String>,
}

impl Remap {
    pub fn new(vk: u16, def: &RemapDef) -> Self {
        Self::with_mods(vk, def.from.mods, def)
    }

    /// A rule whose key needs these modifiers, e.g. Shift for a character
    /// that the current layout types with Shift.
    pub fn with_mods(vk: u16, mods: Modifiers, def: &RemapDef) -> Self {
        Self {
            vk,
            mods,
            output: RemapOutput::for_rule(mods, def.to),
            apps: def.apps.iter().map(|a| a.trim().to_lowercase()).collect(),
        }
    }

    pub fn applies_to(&self, exe: &str) -> bool {
        self.apps.is_empty() || self.apps.iter().any(|a| a == exe)
    }
}

/// A rule in force in the app in front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveRemap {
    pub vk: u16,
    pub mods: Modifiers,
    pub output: RemapOutput,
}

/// For each key (with its modifiers), the rule that applies in this app: one
/// limited to the app wins over one for every app.
pub fn active_for(remaps: &[Remap], exe: &str) -> Vec<ActiveRemap> {
    let mut active: Vec<ActiveRemap> = Vec::new();
    let specific = remaps.iter().filter(|r| !r.apps.is_empty());
    let everywhere = remaps.iter().filter(|r| r.apps.is_empty());
    for rule in specific.chain(everywhere) {
        if rule.applies_to(exe)
            && !active
                .iter()
                .any(|a| a.vk == rule.vk && a.mods == rule.mods)
        {
            active.push(ActiveRemap {
                vk: rule.vk,
                mods: rule.mods,
                output: rule.output,
            });
        }
    }
    active
}

impl Config {
    pub(crate) fn check_remaps(&self, errors: &mut Vec<String>) {
        if self.remaps.len() > MAX_REMAPS {
            errors.push(format!("more than {MAX_REMAPS} key remaps"));
        }
        // (key, app or "" for every app) pairs already taken.
        let mut taken = HashSet::new();
        for rule in &self.remaps {
            let from = rule.from.to_string();
            let ctx = format!("remap {from}");
            if matches!(rule.from.key, Key::Vk(vk) if KNOB_KEYS.contains(&vk)) {
                errors.push(format!(
                    "{ctx}: the volume and mute keys are used by the knob"
                ));
            }
            if rule.from == rule.to {
                errors.push(format!("{ctx}: it changes the key into itself"));
            }
            if rule.apps.iter().any(|a| a.trim().is_empty()) {
                errors.push(format!("{ctx}: an app name is empty"));
            }
            let scopes: Vec<String> = if rule.apps.is_empty() {
                vec![String::new()]
            } else {
                rule.apps.iter().map(|a| a.trim().to_lowercase()).collect()
            };
            for scope in scopes {
                if !taken.insert((from.to_lowercase(), scope.clone())) {
                    let place = if scope.is_empty() {
                        "for every app".to_string()
                    } else {
                        format!("in {scope}")
                    };
                    errors.push(format!("{ctx}: the key is changed twice {place}"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(text: &str) -> KeyCombo {
        text.parse().unwrap()
    }

    fn rule(from: &str, to: &str, apps: &[&str]) -> RemapDef {
        RemapDef {
            from: combo(from),
            to: combo(to),
            apps: apps.iter().map(|a| a.to_string()).collect(),
        }
    }

    fn errors(rules: Vec<RemapDef>) -> Vec<String> {
        let mut config = Config::default_config();
        config.remaps = rules;
        let mut errors = Vec::new();
        config.check_remaps(&mut errors);
        errors
    }

    #[test]
    fn nothing_is_remapped_by_default() {
        assert!(Config::default_config().remaps.is_empty());
    }

    #[test]
    fn a_single_key_is_held_and_a_shortcut_is_tapped() {
        assert_eq!(
            RemapOutput::for_target(combo("Esc")),
            RemapOutput::Hold(0x1B)
        );
        assert_eq!(
            RemapOutput::for_target(combo("LeftCtrl")),
            RemapOutput::Hold(0xA2)
        );
        assert_eq!(
            RemapOutput::for_target(combo("Ctrl+Shift+M")),
            RemapOutput::Tap {
                combo: combo("Ctrl+Shift+M"),
                repeats: false,
                lift: false
            }
        );
        assert_eq!(
            RemapOutput::for_target(combo(";")),
            RemapOutput::Tap {
                combo: combo(";"),
                repeats: true,
                lift: false
            }
        );
    }

    #[test]
    fn a_shortcut_taps_with_its_modifiers_lifted() {
        let from = combo("Alt+1");
        assert_eq!(
            RemapOutput::for_rule(from.mods, combo(">")),
            RemapOutput::Tap {
                combo: combo(">"),
                repeats: true,
                lift: true
            }
        );
        assert_eq!(
            RemapOutput::for_rule(from.mods, combo("Esc")),
            RemapOutput::Tap {
                combo: combo("Esc"),
                repeats: true,
                lift: true
            }
        );
    }

    #[test]
    fn rules_round_trip_through_toml() {
        let mut config = Config::default_config();
        config.remaps = vec![
            rule("CapsLock", "Esc", &[]),
            rule("F13", "Ctrl+Shift+M", &["Teams.exe"]),
        ];
        let text = toml::to_string(&config).unwrap();
        assert!(text.contains("[[remap]]"));
        assert_eq!(Config::from_toml(&text).unwrap(), config);
    }

    #[test]
    fn good_rules_pass() {
        assert!(
            errors(vec![
                rule("CapsLock", "Esc", &[]),
                rule("CapsLock", "LeftCtrl", &["code.exe"]),
                rule("A", "B", &[]),
                rule("B", "A", &[]),
                rule("Alt+1", ">", &[]),
                rule("1", "2", &[]),
            ])
            .is_empty()
        );
    }

    #[test]
    fn bad_rules_are_reported() {
        for rules in [
            vec![rule("Alt+1", "Alt+1", &[])],
            vec![rule("Alt+1", "B", &[]), rule("alt+1", "C", &[])],
            vec![rule("VolumeUp", "B", &[])],
            vec![rule("A", "A", &[])],
            vec![rule("A", "B", &[" "])],
            vec![rule("A", "B", &[]), rule("a", "C", &[])],
            vec![rule("A", "B", &["x.exe"]), rule("A", "C", &["X.EXE"])],
        ] {
            assert_eq!(errors(rules.clone()).len(), 1, "{rules:?}");
        }
    }

    #[test]
    fn an_app_rule_wins_over_the_rule_for_every_app() {
        let remaps = vec![
            Remap::new(0x14, &rule("CapsLock", "Esc", &[])),
            Remap::new(0x14, &rule("CapsLock", "LeftCtrl", &["Code.exe"])),
            Remap::new(0x7C, &rule("F13", "F14", &["code.exe"])),
        ];
        let pairs = |exe| {
            active_for(&remaps, exe)
                .into_iter()
                .map(|a| (a.vk, a.output))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            pairs("code.exe"),
            vec![
                (0x14, RemapOutput::Hold(0xA2)),
                (0x7C, RemapOutput::Hold(0x7D))
            ]
        );
        assert_eq!(pairs("notepad.exe"), vec![(0x14, RemapOutput::Hold(0x1B))]);
    }
}
