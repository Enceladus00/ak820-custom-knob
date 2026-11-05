//! Picks the profile for the foreground app and tracks each profile's
//! current mode.

use crate::config::{Config, Mode, Profile};

#[derive(Debug, Clone)]
pub struct Resolver {
    /// Index of the profile matching the foreground app, if any.
    active: Option<usize>,
    /// Current position in each profile's mode list.
    positions: Vec<usize>,
}

impl Resolver {
    pub fn new(config: &Config) -> Self {
        Self {
            active: None,
            positions: vec![0; config.profiles.len()],
        }
    }

    /// Returns true when the active profile changed.
    pub fn set_app(&mut self, config: &Config, exe: &str) -> bool {
        let new = config.profiles.iter().position(|p| p.matcher.matches(exe));
        if new == self.active {
            return false;
        }
        if let Some(i) = new
            && !config.profiles[i].remember_last_mode
        {
            self.positions[i] = 0;
        }
        self.active = new;
        true
    }

    pub fn profile<'a>(&self, config: &'a Config) -> Option<&'a Profile> {
        self.active.map(|i| &config.profiles[i])
    }

    pub fn mode<'a>(&self, config: &'a Config) -> Option<&'a Mode> {
        let i = self.active?;
        let profile = &config.profiles[i];
        let id = profile.modes.get(self.positions[i] % profile.modes.len())?;
        config.mode(id)
    }

    /// Switches the active profile to its next mode. Returns true if it changed.
    pub fn cycle(&mut self, config: &Config) -> bool {
        let Some(i) = self.active else { return false };
        let count = config.profiles[i].modes.len();
        if count < 2 {
            return false;
        }
        self.positions[i] = (self.positions[i] + 1) % count;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"
        version = 1
        [[mode]]
        id = "a"
        name = "A"
        [[mode]]
        id = "b"
        name = "B"
        [[profile]]
        name = "Editor"
        match = { exe = "resolve.exe" }
        modes = ["b", "a"]
        [[profile]]
        name = "Forgetful"
        match = { exe = "game.exe" }
        modes = ["a", "b"]
        remember_last_mode = false
        [[profile]]
        name = "Default"
        match = { exe = "*" }
        modes = ["a", "b"]
    "#;

    fn mode_id(r: &Resolver, c: &Config) -> String {
        r.mode(c).unwrap().id.clone()
    }

    #[test]
    fn first_matching_profile_wins_and_modes_are_remembered() {
        let c = Config::from_toml(CONFIG).unwrap();
        let mut r = Resolver::new(&c);
        assert!(r.mode(&c).is_none());

        assert!(r.set_app(&c, "explorer.exe"));
        assert_eq!(mode_id(&r, &c), "a");
        assert!(r.cycle(&c));
        assert_eq!(mode_id(&r, &c), "b");

        assert!(r.set_app(&c, "Resolve.exe"));
        assert_eq!(mode_id(&r, &c), "b");

        // Back to the default profile: still on "b".
        assert!(r.set_app(&c, "notepad.exe"));
        assert_eq!(mode_id(&r, &c), "b");
        // Same profile again: no change.
        assert!(!r.set_app(&c, "explorer.exe"));
    }

    #[test]
    fn forgetful_profile_restarts_at_first_mode() {
        let c = Config::from_toml(CONFIG).unwrap();
        let mut r = Resolver::new(&c);
        r.set_app(&c, "game.exe");
        r.cycle(&c);
        assert_eq!(mode_id(&r, &c), "b");
        r.set_app(&c, "explorer.exe");
        r.set_app(&c, "game.exe");
        assert_eq!(mode_id(&r, &c), "a");
    }
}
