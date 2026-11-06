//! Translations for the tray menu, popups and notifications.
//!
//! Every visible text has a [`Text`] key. A language is a function mapping
//! keys to strings, listed in [`LANGUAGES`]. English is the reference and
//! must cover every key; other languages may leave keys out (they fall back
//! to English). See docs/translating.md.

use std::sync::RwLock;

use knob_core::config::AUTO_LANGUAGE;
use windows::Win32::Globalization::GetUserDefaultLocaleName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    /// Tooltip of the tray icon.
    Tooltip,
    /// Tooltip while paused.
    TooltipPaused,
    /// Tooltip with the active mode. `{mode}`
    TooltipMode,
    /// Tooltip when no supported keyboard is connected.
    TooltipNoKeyboard,
    MenuNoMode,
    /// First menu line when no supported keyboard is connected.
    MenuNoKeyboard,
    /// `{mode}`
    MenuMode,
    MenuPaused,
    MenuAutostart,
    MenuSettings,
    MenuEditConfig,
    MenuLogs,
    MenuQuit,
    OsdPaused,
    OsdResumed,
    OsdReloaded,
    OsdAutostartOn,
    OsdAutostartOff,
    AutostartErrorTitle,
    /// App volume popup. `{app}`, `{percent}`
    OsdAppVolume,
    /// `{app}`
    OsdNoSound,
    ConfigErrorTitle,
    /// `{error}`
    ConfigErrorDefaults,
    CrashTitle,
    CrashMessage,
    InputErrorTitle,
    AlreadyRunning,
}

type Language = fn(Text) -> Option<&'static str>;

/// (code, translation). Add new languages here.
const LANGUAGES: &[(&str, Language)] = &[("en", en)];

fn en(text: Text) -> Option<&'static str> {
    use Text::*;
    Some(match text {
        Tooltip => "AK820 knob",
        TooltipPaused => "AK820 knob - paused",
        TooltipMode => "AK820 knob - {mode}",
        TooltipNoKeyboard => "AK820 knob - keyboard not found",
        MenuNoMode => "No mode active",
        MenuNoKeyboard => "AK820 not found: plug in its 2.4 GHz receiver",
        MenuMode => "Mode: {mode}",
        MenuPaused => "Paused (knob works as normal)",
        MenuAutostart => "Start with Windows",
        MenuSettings => "Settings…",
        MenuEditConfig => "Edit config file",
        MenuLogs => "Open log folder",
        MenuQuit => "Quit",
        OsdPaused => "Knob: paused",
        OsdResumed => "Knob: custom functions on",
        OsdReloaded => "Settings reloaded",
        OsdAutostartOn => "Starts with Windows",
        OsdAutostartOff => "Doesn’t start with Windows",
        AutostartErrorTitle => "Can’t change Start with Windows",
        OsdAppVolume => "{app}  {percent}%",
        OsdNoSound => "{app}: no sound playing",
        ConfigErrorTitle => "Config file has an error",
        ConfigErrorDefaults => "Using the default settings. {error}",
        CrashTitle => "AK820 knob stopped",
        CrashMessage => {
            "Something went wrong. The knob works as a normal volume knob until you restart the program."
        }
        InputErrorTitle => "AK820 knob can't read the keyboard",
        AlreadyRunning => "AK820 knob is already running (see the tray).",
    })
}

static ACTIVE: RwLock<Language> = RwLock::new(en);

/// Picks the language: `"auto"` follows the Windows display language.
/// Unknown languages fall back to English.
pub fn set_language(preference: &str) {
    let wanted = if preference == AUTO_LANGUAGE {
        system_language()
    } else {
        preference.to_string()
    };
    let base = wanted
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let language = LANGUAGES
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(&wanted))
        .or_else(|| LANGUAGES.iter().find(|(code, _)| *code == base))
        .map(|&(_, language)| language)
        .unwrap_or(en);
    if let Ok(mut active) = ACTIVE.write() {
        *active = language;
    }
}

/// e.g. "tr-TR"
fn system_language() -> String {
    let mut buf = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH
    let len = unsafe { GetUserDefaultLocaleName(&mut buf) };
    if len <= 1 {
        return "en".into();
    }
    String::from_utf16_lossy(&buf[..len as usize - 1])
}

/// The text in the active language.
pub fn tr(text: Text) -> &'static str {
    let language = ACTIVE.read().map(|l| *l).unwrap_or(en);
    language(text).or_else(|| en(text)).unwrap_or("")
}

/// The text with `{name}` placeholders filled in.
pub fn trf(text: Text, params: &[(&str, &str)]) -> String {
    let mut out = tr(text).to_string();
    for (name, value) in params {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text saved with the wrong encoding turns "…" into "â€¦", which shows
    /// up as C1 control characters (U+0080–U+009F). Real text never has them.
    #[test]
    fn translation_files_are_valid_utf8_text() {
        for (name, source) in [
            ("i18n.rs", include_str!("i18n.rs")),
            (
                "locales/en.js",
                include_str!("../../knob-settings/ui/locales/en.js"),
            ),
        ] {
            assert!(
                !source.chars().any(|c| ('\u{80}'..='\u{9F}').contains(&c)),
                "{name} has garbled characters (double-encoded UTF-8)"
            );
        }
    }

    #[test]
    fn english_and_placeholders() {
        set_language("en");
        assert_eq!(tr(Text::MenuQuit), "Quit");
        assert_eq!(
            trf(Text::OsdAppVolume, &[("app", "Spotify"), ("percent", "40")]),
            "Spotify  40%"
        );
    }

    #[test]
    fn unknown_languages_fall_back_to_english() {
        set_language("xx-YY");
        assert_eq!(tr(Text::MenuSettings), "Settings…");
        set_language("en-GB");
        assert_eq!(tr(Text::MenuSettings), "Settings…");
    }
}
