//! Key combinations written as text in the config, like `"Ctrl+Shift+Right"`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::signal::Modifiers;

/// A key plus modifiers. The key is a Windows virtual-key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct KeyCombo {
    pub mods: Modifiers,
    pub vk: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid key combination '{text}': {reason}")]
pub struct KeyComboError {
    pub text: String,
    pub reason: String,
}

/// Named keys. The first name for a code is the canonical one.
const NAMED_KEYS: &[(&str, u16)] = &[
    ("Space", 0x20),
    ("Enter", 0x0D),
    ("Return", 0x0D),
    ("Tab", 0x09),
    ("Esc", 0x1B),
    ("Escape", 0x1B),
    ("Backspace", 0x08),
    ("Delete", 0x2E),
    ("Del", 0x2E),
    ("Insert", 0x2D),
    ("Ins", 0x2D),
    ("Home", 0x24),
    ("End", 0x23),
    ("PageUp", 0x21),
    ("PgUp", 0x21),
    ("PageDown", 0x22),
    ("PgDn", 0x22),
    ("Left", 0x25),
    ("Up", 0x26),
    ("Right", 0x27),
    ("Down", 0x28),
    ("PrintScreen", 0x2C),
    ("Plus", 0xBB),
    ("Minus", 0xBD),
    ("Comma", 0xBC),
    ("Period", 0xBE),
    ("LeftBracket", 0xDB),
    ("RightBracket", 0xDD),
    ("MediaNext", 0xB0),
    ("MediaPrev", 0xB1),
    ("MediaStop", 0xB2),
    ("MediaPlayPause", 0xB3),
    ("VolumeUp", 0xAF),
    ("VolumeDown", 0xAE),
    ("VolumeMute", 0xAD),
    ("BrowserBack", 0xA6),
    ("BrowserForward", 0xA7),
    ("BrowserRefresh", 0xA8),
];

fn key_from_name(name: &str) -> Option<u16> {
    let upper = name.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    // Single letter or digit.
    if bytes.len() == 1 && bytes[0].is_ascii_alphanumeric() {
        return Some(bytes[0] as u16);
    }
    // F1..F24
    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u16>().ok())
        && (1..=24).contains(&n)
    {
        return Some(0x6F + n);
    }
    NAMED_KEYS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|&(_, vk)| vk)
}

fn key_name(vk: u16) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        _ => NAMED_KEYS
            .iter()
            .find(|&&(_, code)| code == vk)
            .map(|&(n, _)| n.to_string())
            .unwrap_or_else(|| format!("VK_{vk:02X}")),
    }
}

impl FromStr for KeyCombo {
    type Err = KeyComboError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let err = |reason: &str| KeyComboError {
            text: text.to_string(),
            reason: reason.to_string(),
        };
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (key, mod_names) = parts.split_last().ok_or_else(|| err("empty"))?;
        if key.is_empty() {
            return Err(err("missing key (use 'Plus' for the + key)"));
        }

        let mut mods = Modifiers::default();
        for name in mod_names {
            let flag = match name.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => &mut mods.ctrl,
                "shift" => &mut mods.shift,
                "alt" => &mut mods.alt,
                "win" | "super" | "meta" => &mut mods.win,
                _ => {
                    return Err(err(&format!(
                        "'{name}' is not a modifier (Ctrl, Shift, Alt, Win)"
                    )));
                }
            };
            *flag = true;
        }
        let vk = key_from_name(key).ok_or_else(|| err(&format!("unknown key '{key}'")))?;
        Ok(KeyCombo { mods, vk })
    }
}

impl TryFrom<String> for KeyCombo {
    type Error = KeyComboError;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.mods;
        for (held, name) in [
            (m.ctrl, "Ctrl"),
            (m.shift, "Shift"),
            (m.alt, "Alt"),
            (m.win, "Win"),
        ] {
            if held {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(&key_name(self.vk))
    }
}

impl From<KeyCombo> for String {
    fn from(combo: KeyCombo) -> Self {
        combo.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combos() {
        let c: KeyCombo = "ctrl + shift+Right".parse().unwrap();
        assert!(c.mods.ctrl && c.mods.shift && !c.mods.alt);
        assert_eq!(c.vk, 0x27);
        assert_eq!(c.to_string(), "Ctrl+Shift+Right");

        assert_eq!("F13".parse::<KeyCombo>().unwrap().vk, 0x7C);
        assert_eq!("a".parse::<KeyCombo>().unwrap().vk, b'A' as u16);
        assert_eq!(
            "MediaPlayPause".parse::<KeyCombo>().unwrap().to_string(),
            "MediaPlayPause"
        );
        assert_eq!(
            "Ctrl+Plus".parse::<KeyCombo>().unwrap().to_string(),
            "Ctrl+Plus"
        );
    }

    #[test]
    fn rejects_bad_combos() {
        assert!("Ctrl+Foo".parse::<KeyCombo>().is_err());
        assert!("A+B".parse::<KeyCombo>().is_err());
        assert!("Ctrl+".parse::<KeyCombo>().is_err());
        assert!("F25".parse::<KeyCombo>().is_err());
        assert!("".parse::<KeyCombo>().is_err());
    }
}
