//! What a game reports about the player, for `game:` effect situations.
//!
//! A game plugin (see `integrations/valheim/`) sends one UDP datagram of text
//! per line to 127.0.0.1:[`GAME_PORT`]:
//!
//! ```text
//! knob-game 1 valheim health=0.85 dead=0 effects=burning,resting biome=swamp night=1 weather=ThunderStorm boss=0
//! knob-game 1 valheim event=hurt amount=0.30
//! ```
//!
//! State lines come whenever the state changes and at least every 0.5 s;
//! event lines come once, the moment something happens. Keys and values are
//! compared without regard to case.

use std::collections::HashMap;

/// The UDP port game plugins send to (localhost only).
pub const GAME_PORT: u16 = 50820;

/// One line from a game plugin.
#[derive(Debug, Clone, PartialEq)]
pub enum GameLine {
    /// The player's current state.
    State(GameState),
    /// Something just happened: its name and how big it was (0 to 1).
    Event {
        game: String,
        name: String,
        amount: f32,
    },
}

/// The player's state as last reported: the game's name plus `key=value`
/// fields, all lower case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameState {
    pub game: String,
    pub fields: HashMap<String, String>,
}

impl GameState {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

/// Reads one line, or `None` if it isn't from a game plugin (version 1).
pub fn parse_line(line: &str) -> Option<GameLine> {
    let mut words = line.split_whitespace();
    if words.next()? != "knob-game" || words.next()? != "1" {
        return None;
    }
    let game = words.next()?.to_lowercase();
    let mut fields = HashMap::new();
    for word in words {
        if let Some((k, v)) = word.split_once('=') {
            fields.insert(k.to_lowercase(), v.to_lowercase());
        }
    }
    if let Some(name) = fields.remove("event") {
        if name.is_empty() {
            return None;
        }
        let amount = fields
            .get("amount")
            .and_then(|a| a.parse::<f32>().ok())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        return Some(GameLine::Event { game, name, amount });
    }
    Some(GameLine::State(GameState { game, fields }))
}

/// A test on the game's state, the part after `game:` in a situation.
///
/// | written | holds when |
/// |---|---|
/// | `game:valheim` | that game is reporting, or field `valheim` is set (not empty or `0`) |
/// | `game:boss` | field `boss` is set |
/// | `game:biome=swamp` | `biome` is `swamp`, or a comma list containing it (`effects=burning,wet`) |
/// | `game:weather~thunder` | `weather` contains `thunder` |
/// | `game:health<0.25` | `health` is a number below 0.25 (also `>`) |
/// | `game:event=hurt` | a `hurt` event came in a moment ago |
#[derive(Debug, Clone, PartialEq)]
pub enum GameTest {
    Set(String),
    Is(String, String),
    Contains(String, String),
    Below(String, f64),
    Above(String, f64),
    Event(String),
}

impl GameTest {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().to_lowercase();
        let ok_key = |k: &str| !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || c == '_');
        let split = |op: char| {
            text.split_once(op)
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                .filter(|(k, v)| ok_key(k) && !v.is_empty() && !v.contains(char::is_whitespace))
        };
        if text.contains('=') {
            let (k, v) = split('=')?;
            return Some(if k == "event" {
                GameTest::Event(v)
            } else {
                GameTest::Is(k, v)
            });
        }
        if text.contains('~') {
            let (k, v) = split('~')?;
            return Some(GameTest::Contains(k, v));
        }
        for (op, make) in [
            ('<', GameTest::Below as fn(String, f64) -> GameTest),
            ('>', GameTest::Above),
        ] {
            if text.contains(op) {
                let (k, v) = split(op)?;
                return Some(make(k, v.parse().ok().filter(|n: &f64| n.is_finite())?));
            }
        }
        let set = ok_key(&text);
        set.then_some(GameTest::Set(text))
    }

    /// The event this test waits for, if it is an event test.
    pub fn event(&self) -> Option<&str> {
        match self {
            GameTest::Event(name) => Some(name),
            _ => None,
        }
    }

    /// Whether this (non-event) test holds for `state`. Event tests never
    /// hold here: the caller tracks recent events.
    pub fn holds(&self, state: &GameState) -> bool {
        let number = |k: &str| state.get(k).and_then(|v| v.parse::<f64>().ok());
        match self {
            GameTest::Set(k) => {
                state.game == *k || state.get(k).is_some_and(|v| !v.is_empty() && v != "0")
            }
            GameTest::Is(k, want) => state
                .get(k)
                .is_some_and(|v| v == want || v.split(',').any(|part| part == want)),
            GameTest::Contains(k, part) => state.get(k).is_some_and(|v| v.contains(part.as_str())),
            GameTest::Below(k, n) => number(k).is_some_and(|v| v < *n),
            GameTest::Above(k, n) => number(k).is_some_and(|v| v > *n),
            GameTest::Event(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(line: &str) -> GameState {
        match parse_line(line) {
            Some(GameLine::State(s)) => s,
            other => panic!("not a state: {other:?}"),
        }
    }

    #[test]
    fn reads_the_valheim_plugin_lines() {
        let s = state(
            "knob-game 1 valheim health=0.85 dead=0 effects=burning,resting biome=swamp night=1 weather=ThunderStorm boss=0",
        );
        assert_eq!(s.game, "valheim");
        assert_eq!(s.get("weather"), Some("thunderstorm"));
        assert_eq!(s.get("effects"), Some("burning,resting"));
        // Main menu: no player, no fields.
        assert!(state("knob-game 1 valheim").fields.is_empty());
        assert_eq!(
            parse_line("knob-game 1 valheim event=hurt amount=0.30"),
            Some(GameLine::Event {
                game: "valheim".into(),
                name: "hurt".into(),
                amount: 0.3
            })
        );
        assert_eq!(parse_line("knob-game 2 valheim"), None);
        assert_eq!(parse_line("hello"), None);
        assert_eq!(parse_line("knob-game 1 valheim event="), None);
    }

    #[test]
    fn tests_parse() {
        use GameTest::*;
        assert_eq!(GameTest::parse("boss"), Some(Set("boss".into())));
        assert_eq!(
            GameTest::parse("Effects = Burning"),
            Some(Is("effects".into(), "burning".into()))
        );
        assert_eq!(
            GameTest::parse("weather~thunder"),
            Some(Contains("weather".into(), "thunder".into()))
        );
        assert_eq!(
            GameTest::parse("health<0.25"),
            Some(Below("health".into(), 0.25))
        );
        assert_eq!(
            GameTest::parse("health>0.9"),
            Some(Above("health".into(), 0.9))
        );
        assert_eq!(GameTest::parse("event=parry"), Some(Event("parry".into())));
        for bad in [
            "",
            "=x",
            "biome=",
            "health<lots",
            "health<nan",
            "two words",
            "a=b<c d",
        ] {
            assert_eq!(GameTest::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn tests_hold() {
        let s = state(
            "knob-game 1 valheim health=0.20 dead=0 effects=burning,wet biome=swamp weather=ThunderStorm boss=0",
        );
        let holds = |t: &str| GameTest::parse(t).unwrap().holds(&s);
        assert!(holds("valheim"));
        assert!(!holds("cs2"));
        assert!(holds("effects=burning"));
        assert!(holds("effects=wet"));
        assert!(!holds("effects=poison"));
        assert!(!holds("effects=burn"));
        assert!(holds("biome=swamp"));
        assert!(holds("weather~thunder"));
        assert!(holds("health<0.25"));
        assert!(!holds("health>0.25"));
        assert!(!holds("boss"));
        assert!(!holds("dead"));
        assert!(!holds("night"));
        assert!(!holds("event=hurt"));
    }
}
