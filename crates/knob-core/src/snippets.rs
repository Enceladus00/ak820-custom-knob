//! Text expansion: typing a trigger such as `;mail` replaces it with a saved
//! text. This module holds the platform-independent parts: the snippet list
//! and its rules, the matcher that watches typed characters, and the
//! placeholders (`{date}`, `{time}`, `{cursor}`).
//!
//! The list is stored apart from config.toml, encrypted for the Windows user
//! (see `knob_win::snippet_store`), because snippet text is often personal.

use serde::{Deserialize, Serialize};

/// The longest trigger, and so the most typed characters ever remembered.
pub const MAX_TRIGGER: usize = 32;
pub const MIN_TRIGGER: usize = 2;
pub const MAX_TEXT: usize = 4000;

/// Password managers: their windows are full of secrets, so expansion is
/// off there unless the user removes them from the list.
pub const DEFAULT_EXCLUDE: &[&str] = &[
    "keepass.exe",
    "keepassxc.exe",
    "1password.exe",
    "bitwarden.exe",
    "dashlane.exe",
    "nordpass.exe",
    "enpass.exe",
    "proton pass.exe",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnippetStore {
    /// Turns all expansion on or off.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Lower-case exe names where nothing expands.
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snippet {
    /// What to type, e.g. `;mail`.
    pub trigger: String,
    /// What it becomes. May hold placeholders, see [`expand`].
    pub text: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Hide the text in Settings until shown on purpose.
    #[serde(default)]
    pub secret: bool,
    /// Lower-case exe names it works in. Empty means every app.
    #[serde(default)]
    pub apps: Vec<String>,
}

fn yes() -> bool {
    true
}

fn default_exclude() -> Vec<String> {
    DEFAULT_EXCLUDE.iter().map(|s| s.to_string()).collect()
}

impl Default for SnippetStore {
    fn default() -> Self {
        Self {
            enabled: true,
            exclude: default_exclude(),
            snippets: Vec::new(),
        }
    }
}

impl SnippetStore {
    /// Everything wrong with the list, in plain words. Empty when it's fine.
    pub fn problems(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for s in &self.snippets {
            let n = s.trigger.chars().count();
            let ctx = format!("snippet '{}'", s.trigger);
            if !(MIN_TRIGGER..=MAX_TRIGGER).contains(&n) {
                errors.push(format!(
                    "{ctx}: the trigger needs {MIN_TRIGGER} to {MAX_TRIGGER} characters"
                ));
            }
            if s.trigger
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
            {
                errors.push(format!("{ctx}: the trigger can't contain spaces"));
            }
            if s.text.is_empty() {
                errors.push(format!("{ctx}: the text is empty"));
            }
            if s.text.chars().count() > MAX_TEXT {
                errors.push(format!(
                    "{ctx}: the text is longer than {MAX_TEXT} characters"
                ));
            }
            if let Err(e) = parse(&s.text) {
                errors.push(format!("{ctx}: {e}"));
            }
        }
        for (i, a) in self.snippets.iter().enumerate() {
            for b in &self.snippets[i + 1..] {
                if a.trigger == b.trigger {
                    errors.push(format!("two snippets use the trigger '{}'", a.trigger));
                }
            }
        }
        for a in &self.snippets {
            for b in &self.snippets {
                if a.trigger != b.trigger && blocks(&a.trigger, &b.trigger) {
                    errors.push(format!(
                        "'{}' would expand while typing '{}': pick triggers that don't contain each other",
                        a.trigger, b.trigger
                    ));
                }
            }
        }
        errors
    }

    /// Whether `snippet` may expand in `exe` (lower case).
    pub fn allowed(&self, snippet: &Snippet, exe: &str) -> bool {
        self.enabled
            && snippet.enabled
            && !self.exclude.iter().any(|e| e.eq_ignore_ascii_case(exe))
            && (snippet.apps.is_empty() || snippet.apps.iter().any(|a| a.eq_ignore_ascii_case(exe)))
    }
}

/// Whether typing `b` would trigger `a` before `b` is complete: `a` appears
/// in `b` without its last character. (`a` at the very end of `b` is fine:
/// the longer trigger wins then.)
fn blocks(a: &str, b: &str) -> bool {
    let mut chars = b.chars();
    chars.next_back();
    chars.as_str().contains(a)
}

/// Watches typed characters for triggers. It remembers at most
/// [`MAX_TRIGGER`] characters and wipes them whenever it is cleared.
pub struct Matcher {
    /// Triggers, longest first, with their index in the store.
    triggers: Vec<(Vec<char>, usize)>,
    buf: [char; MAX_TRIGGER],
    len: usize,
}

impl Matcher {
    pub fn new(store: &SnippetStore) -> Self {
        let mut triggers: Vec<_> = store
            .snippets
            .iter()
            .enumerate()
            .filter(|(_, s)| s.enabled && !s.trigger.is_empty())
            .map(|(i, s)| (s.trigger.chars().collect::<Vec<_>>(), i))
            .collect();
        triggers.sort_by_key(|(t, _)| std::cmp::Reverse(t.len()));
        Self {
            triggers,
            buf: ['\0'; MAX_TRIGGER],
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.triggers.is_empty()
    }

    /// Adds a typed character. Returns the snippet whose trigger was just
    /// completed and that `allowed` accepts, with its trigger length, and
    /// forgets what was typed.
    pub fn push(&mut self, c: char, allowed: impl Fn(usize) -> bool) -> Option<(usize, usize)> {
        if self.len == MAX_TRIGGER {
            self.buf.copy_within(1.., 0);
            self.len -= 1;
        }
        self.buf[self.len] = c;
        self.len += 1;
        let typed = &self.buf[..self.len];
        let hit = self
            .triggers
            .iter()
            .find(|(t, i)| typed.ends_with(t) && allowed(*i))
            .map(|(t, i)| (*i, t.len()));
        if hit.is_some() {
            self.clear();
        }
        hit
    }

    pub fn backspace(&mut self) {
        if self.len > 0 {
            self.len -= 1;
            self.buf[self.len] = '\0';
        }
    }

    /// Forgets everything typed, overwriting it.
    pub fn clear(&mut self) {
        self.buf = ['\0'; MAX_TRIGGER];
        self.len = 0;
    }
}

impl Drop for Matcher {
    fn drop(&mut self) {
        self.clear();
    }
}

/// The local date and time, and the system's date and time formats
/// (Windows style: `dd.MM.yyyy`, `HH:mm`).
#[derive(Debug, Clone)]
pub struct LocalTime {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub date_format: String,
    pub time_format: String,
}

/// A piece of snippet text once placeholders are read.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Date(Option<String>),
    Time(Option<String>),
    Cursor,
}

/// Placeholders: `{date}` and `{time}` in the system's format, or with a
/// format of their own: `{date:yyyy-MM-dd}`, `{time:HH:mm:ss}`. `{cursor}`
/// leaves the cursor there. `{{` and `}}` type a brace.
fn parse(text: &str) -> Result<Vec<Part>, String> {
    let mut parts = Vec::new();
    let mut plain = String::new();
    let mut cursors = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                plain.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                plain.push('}');
            }
            '}' => return Err("a lone '}' (type }} for a brace)".into()),
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => name.push(c),
                        None => return Err("a '{' is never closed (type {{ for a brace)".into()),
                    }
                }
                let (kind, format) = match name.split_once(':') {
                    Some((k, f)) if !f.is_empty() => (k, Some(f.to_string())),
                    _ => (name.as_str(), None),
                };
                let part = match kind {
                    "date" => Part::Date(format),
                    "time" => Part::Time(format),
                    "cursor" if format.is_none() => {
                        cursors += 1;
                        Part::Cursor
                    }
                    _ => return Err(format!("unknown placeholder {{{name}}}")),
                };
                if !plain.is_empty() {
                    parts.push(Part::Text(std::mem::take(&mut plain)));
                }
                parts.push(part);
            }
            c => plain.push(c),
        }
    }
    if cursors > 1 {
        return Err("{cursor} can be used once".into());
    }
    if !plain.is_empty() {
        parts.push(Part::Text(plain));
    }
    Ok(parts)
}

/// Text ready to type: where to leave the cursor is given as how many
/// characters to step back from the end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    pub text: String,
    pub back: usize,
}

/// Fills in the placeholders. Invalid text (rejected by
/// [`SnippetStore::problems`]) is typed as written.
pub fn expand(text: &str, now: &LocalTime) -> Expansion {
    let Ok(parts) = parse(text) else {
        return Expansion {
            text: text.to_string(),
            back: 0,
        };
    };
    let mut out = String::new();
    let mut cursor = None;
    for part in parts {
        match part {
            Part::Text(t) => out.push_str(&t),
            Part::Date(f) => {
                out.push_str(&format_time(f.as_deref().unwrap_or(&now.date_format), now))
            }
            Part::Time(f) => {
                out.push_str(&format_time(f.as_deref().unwrap_or(&now.time_format), now))
            }
            Part::Cursor => cursor = Some(out.chars().count()),
        }
    }
    let total = out.chars().count();
    Expansion {
        back: cursor.map_or(0, |at| total - at),
        text: out,
    }
}

/// Formats with Windows-style codes: `yyyy yy MM M dd d HH H hh h mm m ss s
/// tt`; text in single quotes is typed as is.
fn format_time(format: &str, t: &LocalTime) -> String {
    let chars: Vec<char> = format.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                out.push(chars[i]);
                i += 1;
            }
            i += 1;
            continue;
        }
        let mut run = 1;
        while i + run < chars.len() && chars[i + run] == c {
            run += 1;
        }
        let two = |v: u16| format!("{v:02}");
        let hour12 = match t.hour % 12 {
            0 => 12,
            h => h,
        };
        let piece = match (c, run) {
            ('y', 1 | 2) => two(t.year % 100),
            ('y', _) => t.year.to_string(),
            ('M', 1) => t.month.to_string(),
            ('M', _) => two(t.month),
            ('d', 1) => t.day.to_string(),
            ('d', _) => two(t.day),
            ('H', 1) => t.hour.to_string(),
            ('H', _) => two(t.hour),
            ('h', 1) => hour12.to_string(),
            ('h', _) => two(hour12),
            ('m', 1) => t.minute.to_string(),
            ('m', _) => two(t.minute),
            ('s', 1) => t.second.to_string(),
            ('s', _) => two(t.second),
            ('t', 1) => (if t.hour < 12 { "A" } else { "P" }).to_string(),
            ('t', _) => (if t.hour < 12 { "AM" } else { "PM" }).to_string(),
            _ => std::iter::repeat_n(c, run).collect(),
        };
        out.push_str(&piece);
        i += run;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snippet(trigger: &str, text: &str) -> Snippet {
        Snippet {
            trigger: trigger.into(),
            text: text.into(),
            enabled: true,
            secret: false,
            apps: vec![],
        }
    }

    fn store(snippets: Vec<Snippet>) -> SnippetStore {
        SnippetStore {
            snippets,
            ..SnippetStore::default()
        }
    }

    fn now() -> LocalTime {
        LocalTime {
            year: 2026,
            month: 3,
            day: 7,
            hour: 14,
            minute: 5,
            second: 9,
            date_format: "dd.MM.yyyy".into(),
            time_format: "HH:mm".into(),
        }
    }

    fn typed(m: &mut Matcher, s: &str) -> Option<usize> {
        let mut hit = None;
        for c in s.chars() {
            hit = m.push(c, |_| true).map(|(i, _)| i);
        }
        hit
    }

    #[test]
    fn triggers_fire_on_their_last_character() {
        let s = store(vec![
            snippet(";mail", "me@example.com"),
            snippet(";date", "{date}"),
        ]);
        let mut m = Matcher::new(&s);
        assert_eq!(typed(&mut m, "hello ;mai"), None);
        assert_eq!(m.push('l', |_| true), Some((0, 5)));
        // Forgotten after a hit: typing on doesn't fire again.
        assert_eq!(typed(&mut m, "l"), None);
        assert_eq!(typed(&mut m, ";dat"), None);
        m.backspace();
        assert_eq!(typed(&mut m, "te"), Some(1));
        m.clear();
        assert_eq!(typed(&mut m, "e"), None);
    }

    #[test]
    fn the_longer_trigger_wins_and_apps_are_respected() {
        let s = store(vec![snippet("il", "short"), snippet(";mail", "long")]);
        let mut m = Matcher::new(&s);
        assert_eq!(typed(&mut m, ";mail"), Some(1));
        assert_eq!(
            ";mail".chars().fold(None, |_, c| m.push(c, |i| i == 0)),
            Some((0, 2))
        );
        let mut only_code = snippet(";x", "x");
        only_code.apps = vec!["code.exe".into()];
        let s = SnippetStore {
            exclude: vec!["keepass.exe".into()],
            ..store(vec![only_code.clone(), snippet(";y", "y")])
        };
        assert!(s.allowed(&only_code, "Code.exe"));
        assert!(!s.allowed(&only_code, "notepad.exe"));
        assert!(!s.allowed(&s.snippets[1], "KeePass.exe"));
        assert!(s.allowed(&s.snippets[1], "notepad.exe"));
        let off = SnippetStore {
            enabled: false,
            ..s.clone()
        };
        assert!(!off.allowed(&off.snippets[1], "notepad.exe"));
    }

    #[test]
    fn the_matcher_remembers_only_the_last_characters() {
        let s = store(vec![snippet(";mail", "x")]);
        let mut m = Matcher::new(&s);
        typed(&mut m, &"a".repeat(500));
        assert_eq!(m.len, MAX_TRIGGER);
        assert_eq!(typed(&mut m, ";mail"), Some(0));
        assert!(m.buf.iter().all(|&c| c == '\0'));
    }

    #[test]
    fn problems_are_found() {
        assert!(
            store(vec![snippet(";mail", "x"), snippet(";date", "{date}")])
                .problems()
                .is_empty()
        );
        let p = |s: Vec<Snippet>| store(s).problems().len();
        assert_eq!(p(vec![snippet(";", "x")]), 1);
        assert_eq!(p(vec![snippet("; m", "x")]), 1);
        assert_eq!(p(vec![snippet(";m", "")]), 1);
        assert_eq!(p(vec![snippet(";m", &"x".repeat(MAX_TEXT + 1))]), 1);
        assert_eq!(p(vec![snippet(";m", "a"), snippet(";m", "b")]), 1);
        // ";ma" fires before ";mail" can be finished.
        assert_eq!(p(vec![snippet(";ma", "a"), snippet(";mail", "b")]), 1);
        // A trigger at the end of another is fine: the longer one wins.
        assert_eq!(p(vec![snippet("il", "a"), snippet(";mail", "b")]), 0);
        assert_eq!(p(vec![snippet(";m", "{nope}")]), 1);
        assert_eq!(p(vec![snippet(";m", "{date")]), 1);
        assert_eq!(p(vec![snippet(";m", "a}b")]), 1);
        assert_eq!(p(vec![snippet(";m", "{cursor}{cursor}")]), 1);
    }

    #[test]
    fn placeholders_expand() {
        let e = |text| expand(text, &now());
        assert_eq!(e("{date}").text, "07.03.2026");
        assert_eq!(e("{time}").text, "14:05");
        assert_eq!(
            e("{date:yyyy-MM-dd} {time:h:mm tt}").text,
            "2026-03-07 2:05 PM"
        );
        assert_eq!(e("{date:d 'of' M, yy}").text, "7 of 3, 26");
        assert_eq!(e("{{x}}").text, "{x}");
        assert_eq!(
            e("Hi {cursor}, thanks!"),
            Expansion {
                text: "Hi , thanks!".into(),
                back: 9
            }
        );
        assert_eq!(e("ğüş {cursor}ç").back, 1);
        assert_eq!(e("plain").back, 0);
    }

    #[test]
    fn the_store_round_trips_with_defaults() {
        let s: SnippetStore = toml::from_str("").unwrap();
        assert!(s.enabled);
        assert!(s.exclude.contains(&"keepass.exe".to_string()));
        let full = store(vec![snippet(";mail", "me@example.com\nsecond line")]);
        let back: SnippetStore = toml::from_str(&toml::to_string(&full).unwrap()).unwrap();
        assert_eq!(back, full);
    }
}
