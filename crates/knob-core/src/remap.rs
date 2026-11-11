//! Key remapping: one key becomes another key or a shortcut, in every app or
//! only in some. Off until the user adds a rule.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::keys::{Key, KeyCombo};

/// One `[[remap]]` rule as written in the config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemapDef {
    /// A single key, without Ctrl, Shift, Alt or Win.
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
    /// mapped to Win+D doesn't flicker the desktop.
    Tap { combo: KeyCombo, repeats: bool },
}

impl RemapOutput {
    pub fn for_target(to: KeyCombo) -> Self {
        match to.key {
            Key::Vk(vk) if to.mods.is_empty() => Self::Hold(vk),
            Key::Char(_) => Self::Tap {
                combo: to,
                repeats: to.mods.is_empty(),
            },
            Key::Vk(_) => Self::Tap {
                combo: to,
                repeats: false,
            },
        }
    }
}

/// A rule ready for the router: the key to change as a virtual-key code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remap {
    pub vk: u16,
    pub output: RemapOutput,
    /// Lower-case exe names. Empty means every app.
    pub apps: Vec<String>,
}

impl Remap {
    pub fn new(vk: u16, def: &RemapDef) -> Self {
        Self {
            vk,
            output: RemapOutput::for_target(def.to),
            apps: def.apps.iter().map(|a| a.trim().to_lowercase()).collect(),
        }
    }

    pub fn applies_to(&self, exe: &str) -> bool {
        self.apps.is_empty() || self.apps.iter().any(|a| a == exe)
    }
}

/// For each key, the rule that applies in this app: one limited to the app
/// wins over one for every app.
pub fn active_for(remaps: &[Remap], exe: &str) -> Vec<(u16, RemapOutput)> {
    let mut active: Vec<(u16, RemapOutput)> = Vec::new();
    let specific = remaps.iter().filter(|r| !r.apps.is_empty());
    let everywhere = remaps.iter().filter(|r| r.apps.is_empty());
    for rule in specific.chain(everywhere) {
        if rule.applies_to(exe) && !active.iter().any(|(vk, _)| *vk == rule.vk) {
            active.push((rule.vk, rule.output));
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
            if !rule.from.mods.is_empty() {
                errors.push(format!(
                    "{ctx}: the key to change must be a single key, without Ctrl, Shift, Alt or Win"
                ));
            }
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
                repeats: false
            }
        );
        assert_eq!(
            RemapOutput::for_target(combo(";")),
            RemapOutput::Tap {
                combo: combo(";"),
                repeats: true
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
            ])
            .is_empty()
        );
    }

    #[test]
    fn bad_rules_are_reported() {
        for rules in [
            vec![rule("Ctrl+A", "B", &[])],
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
        assert_eq!(
            active_for(&remaps, "code.exe"),
            vec![
                (0x14, RemapOutput::Hold(0xA2)),
                (0x7C, RemapOutput::Hold(0x7D))
            ]
        );
        assert_eq!(
            active_for(&remaps, "notepad.exe"),
            vec![(0x14, RemapOutput::Hold(0x1B))]
        );
    }
}
