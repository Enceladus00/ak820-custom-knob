//! Key combinations written as text in the config, like `"Ctrl+Shift+Right"`,
//! `"."` or `"Ctrl+ç"`.
//!
//! A key is either
//! * a **named key** (`Right`, `F5`, `A`, `Num5`, `MediaNext`, or `VK_BF` for
//!   any Windows virtual-key code), or
//! * a **character** (`.`, `*`, `ç`, `+`). Characters are matched to a key on
//!   the keyboard layout that is active when the action runs, so `"."` works
//!   whether the full stop is next to the comma (US) or somewhere else
//!   (Turkish Q). Shift or AltGr needed to type it are added automatically.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::signal::Modifiers;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A Windows virtual-key code.
    Vk(u16),
    /// A character, typed with whatever key produces it on the active layout.
    Char(char),
}

/// A key plus the modifiers held with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct KeyCombo {
    pub mods: Modifiers,
    pub key: Key,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid key combination '{text}': {reason}")]
pub struct KeyComboError {
    pub text: String,
    pub reason: String,
}

/// Named keys. The first name for a code is the canonical one.
/// Letters, digits, F1–F24 and Num0–Num9 are handled separately.
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
    ("Pause", 0x13),
    ("CapsLock", 0x14),
    ("NumLock", 0x90),
    ("ScrollLock", 0x91),
    ("Menu", 0x5D),
    ("NumMultiply", 0x6A),
    ("NumAdd", 0x6B),
    ("NumSubtract", 0x6D),
    ("NumDecimal", 0x6E),
    ("NumDivide", 0x6F),
    // Punctuation keys by their position on a US keyboard. Prefer writing the
    // character itself ("." instead of "Period"): it works on every layout.
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
    ("BrowserStop", 0xA9),
    ("BrowserSearch", 0xAA),
    ("BrowserFavorites", 0xAB),
    ("BrowserHome", 0xAC),
    ("LaunchMail", 0xB4),
    ("LaunchMedia", 0xB5),
    ("LaunchApp1", 0xB6),
    ("LaunchApp2", 0xB7),
];

fn named_key(name: &str) -> Option<u16> {
    let upper = name.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    // Single ASCII letter or digit: its virtual-key code.
    if bytes.len() == 1 && bytes[0].is_ascii_alphanumeric() {
        return Some(bytes[0] as u16);
    }
    // F1..F24
    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u16>().ok())
        && (1..=24).contains(&n)
    {
        return Some(0x6F + n);
    }
    // Num0..Num9 (numeric keypad)
    if let Some(n) = upper
        .strip_prefix("NUM")
        .and_then(|n| n.parse::<u16>().ok())
        && n <= 9
        && upper.len() == 4
    {
        return Some(0x60 + n);
    }
    // VK_XX: any virtual-key code, in hex.
    if let Some(hex) = upper.strip_prefix("VK_") {
        return u16::from_str_radix(hex, 16)
            .ok()
            .filter(|vk| (1..=0xFE).contains(vk));
    }
    NAMED_KEYS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|&(_, vk)| vk)
}

fn parse_key(text: &str) -> Option<Key> {
    if let Some(vk) = named_key(text) {
        return Some(Key::Vk(vk));
    }
    // Exactly one character that isn't a letter/digit handled above and isn't
    // whitespace or a control character: a typed character.
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_whitespace() && !c.is_control() => Some(Key::Char(c)),
        _ => None,
    }
}

fn key_name(key: Key) -> String {
    match key {
        Key::Char(c) => c.to_string(),
        Key::Vk(vk) => match vk {
            0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
            0x60..=0x69 => format!("Num{}", vk - 0x60),
            0x70..=0x87 => format!("F{}", vk - 0x6F),
            _ => NAMED_KEYS
                .iter()
                .find(|&&(_, code)| code == vk)
                .map(|&(n, _)| n.to_string())
                .unwrap_or_else(|| format!("VK_{vk:02X}")),
        },
    }
}

impl FromStr for KeyCombo {
    type Err = KeyComboError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let err = |reason: &str| KeyComboError {
            text: text.to_string(),
            reason: reason.to_string(),
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(err("empty"));
        }
        // The "+" character itself: "+", "Ctrl++".
        let (mods_text, key_text) = if trimmed == "+" {
            ("", "+")
        } else if let Some(rest) = trimmed.strip_suffix("++") {
            (rest, "+")
        } else {
            match trimmed.rsplit_once('+') {
                Some((mods, key)) => (mods, key.trim()),
                None => ("", trimmed),
            }
        };
        if key_text.is_empty() {
            return Err(err("missing key"));
        }

        let mut mods = Modifiers::default();
        for name in mods_text
            .split('+')
            .map(str::trim)
            .filter(|n| !n.is_empty())
        {
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
        let key = parse_key(key_text).ok_or_else(|| err(&format!("unknown key '{key_text}'")))?;
        Ok(KeyCombo { mods, key })
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
        f.write_str(&key_name(self.key))
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

    fn parse(text: &str) -> KeyCombo {
        text.parse().unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn parses_named_keys() {
        let c = parse("ctrl + shift+Right");
        assert!(c.mods.ctrl && c.mods.shift && !c.mods.alt);
        assert_eq!(c.key, Key::Vk(0x27));
        assert_eq!(c.to_string(), "Ctrl+Shift+Right");

        assert_eq!(parse("F13").key, Key::Vk(0x7C));
        assert_eq!(parse("a").key, Key::Vk(b'A' as u16));
        assert_eq!(parse("Num5").key, Key::Vk(0x65));
        assert_eq!(parse("MediaPlayPause").to_string(), "MediaPlayPause");
        assert_eq!(parse("Ctrl+Plus").to_string(), "Ctrl+Plus");
    }

    #[test]
    fn parses_characters() {
        // The YouTube "next frame" key, on any layout.
        assert_eq!(parse(".").key, Key::Char('.'));
        assert_eq!(parse("Shift+.").to_string(), "Shift+.");
        assert_eq!(parse("ç").key, Key::Char('ç'));
        assert_eq!(parse("Ctrl+*").key, Key::Char('*'));
        // "+" is also the separator.
        assert_eq!(parse("+").key, Key::Char('+'));
        let plus = parse("Ctrl++");
        assert!(plus.mods.ctrl);
        assert_eq!(plus.key, Key::Char('+'));
        assert_eq!(plus.to_string(), "Ctrl++");
    }

    #[test]
    fn parses_any_virtual_key_code() {
        assert_eq!(parse("VK_BF").key, Key::Vk(0xBF));
        assert_eq!(parse("Alt+vk_e2").to_string(), "Alt+VK_E2");
        // Codes with a name are shown by name.
        assert_eq!(parse("VK_25").to_string(), "Left");
    }

    #[test]
    fn display_round_trips() {
        for text in [
            "Ctrl+Shift+Right",
            ".",
            "Ctrl++",
            "Win+ö",
            "VK_E2",
            "Num0",
            "Alt+F4",
            "Shift+,",
        ] {
            assert_eq!(parse(text).to_string(), text);
            assert_eq!(parse(&parse(text).to_string()), parse(text));
        }
    }

    /// Every form the Settings key field records (checked in the UI with a
    /// Turkish-Q layout) must be accepted here.
    #[test]
    fn accepts_what_settings_records() {
        for text in [
            ".",
            ":",
            "Ctrl+.",
            "ç",
            "@",
            "Ctrl+Shift+Z",
            "Ctrl++",
            "Num5",
            "CapsLock",
            "VK_C0",
            "Space",
            "Menu",
            "Pause",
            "NumDecimal",
            "Shift+Win+S",
        ] {
            assert_eq!(parse(text).to_string(), text);
        }
    }

    #[test]
    fn rejects_bad_combos() {
        for text in [
            "Ctrl+Foo", "A+B", "Ctrl+", "F25", "", "  ", "VK_00", "VK_ZZ", "Ctrl+ab",
        ] {
            assert!(
                text.parse::<KeyCombo>().is_err(),
                "{text:?} should be rejected"
            );
        }
    }
}
