//! Ties classifier, gesture recognizer and config together. The platform
//! layer feeds it inputs and timer ticks and carries out its outputs.

use crate::classify::{Classified, Classifier};
use crate::config::{Action, Config};
use crate::gesture::{Gesture, GestureRecognizer};
use crate::resolver::Resolver;
use crate::signal::{Dir, HookKey, Input, Micros};

#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// Send this key event on to Windows (marked as ours, so the hook lets it pass).
    Replay { key: HookKey, down: bool },
    /// Perform an action for a gesture.
    Run { action: Action, gesture: Gesture },
    /// The active mode changed (show it to the user).
    ModeChanged {
        profile: String,
        mode_id: String,
        mode_name: String,
        icon: Option<String>,
    },
}

#[derive(Debug)]
pub struct Engine {
    config: Config,
    classifier: Classifier,
    gestures: GestureRecognizer,
    resolver: Resolver,
    app: String,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        let gestures = GestureRecognizer::new(config.timing.gesture_timing());
        let resolver = Resolver::new(&config);
        Self {
            config,
            classifier: Classifier::new(),
            gestures,
            resolver,
            app: String::new(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Replaces the config (e.g. after the file was edited).
    pub fn set_config(&mut self, config: Config) -> Vec<Output> {
        self.gestures.set_timing(config.timing.gesture_timing());
        self.resolver = Resolver::new(&config);
        self.config = config;
        let app = std::mem::take(&mut self.app);
        let mut out = self.set_foreground_app(&app);
        if out.is_empty() {
            self.mode_changed(&mut out);
        }
        out
    }

    /// Call when the foreground app changes. `exe` is the file name, e.g. "Resolve.exe".
    pub fn set_foreground_app(&mut self, exe: &str) -> Vec<Output> {
        self.app = exe.to_string();
        let mut out = Vec::new();
        if self.resolver.set_app(&self.config, exe) {
            self.mode_changed(&mut out);
        }
        out
    }

    pub fn handle(&mut self, input: Input) -> Vec<Output> {
        let mut out = Vec::new();
        self.poll_into(input.time(), &mut out);
        let mut classified = Vec::new();
        self.classifier.handle(input, &mut classified);
        self.process(classified, &mut out);
        out
    }

    /// Call when `next_deadline` is reached.
    pub fn poll(&mut self, now: Micros) -> Vec<Output> {
        let mut out = Vec::new();
        self.poll_into(now, &mut out);
        out
    }

    /// When the engine next needs `poll` to be called, if ever.
    pub fn next_deadline(&self) -> Option<Micros> {
        match (
            self.classifier.next_deadline(),
            self.gestures.next_deadline(),
        ) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    pub fn current_mode_name(&self) -> Option<&str> {
        self.resolver.mode(&self.config).map(|m| m.name.as_str())
    }

    /// Fires every deadline up to `now`, oldest first.
    fn poll_into(&mut self, now: Micros, out: &mut Vec<Output>) {
        while let Some(deadline) = self.next_deadline().filter(|&d| d <= now) {
            let mut classified = Vec::new();
            self.classifier.poll(deadline, &mut classified);
            self.process(classified, out);
            let mut gestures = Vec::new();
            self.gestures.poll(deadline, &mut gestures);
            self.dispatch(gestures, out);
        }
    }

    fn process(&mut self, classified: Vec<Classified>, out: &mut Vec<Output>) {
        for event in classified {
            if let Classified::Replay { key, down } = event {
                out.push(Output::Replay { key, down });
                continue;
            }
            let mut gestures = Vec::new();
            self.gestures.handle(event, &mut gestures);
            self.dispatch(gestures, out);
        }
    }

    fn dispatch(&mut self, gestures: Vec<Gesture>, out: &mut Vec<Output>) {
        for gesture in gestures {
            let action = match self.resolver.mode(&self.config) {
                Some(mode) => mode.binding(gesture.kind(), gesture.mods()).cloned(),
                // No profile matches this app: the knob keeps its stock behaviour.
                None => Some(Action::Passthrough),
            };
            match action {
                None | Some(Action::Nothing) => {}
                Some(Action::Passthrough) => replay_gesture(gesture, out),
                Some(Action::CycleMode) => {
                    if self.resolver.cycle(&self.config) {
                        self.mode_changed(out);
                    }
                }
                Some(action) => out.push(Output::Run { action, gesture }),
            }
        }
    }

    fn mode_changed(&mut self, out: &mut Vec<Output>) {
        let mode = self.resolver.mode(&self.config);
        self.gestures
            .set_double_click_enabled(mode.is_some_and(|m| m.uses_double_click()));
        if let (Some(profile), Some(mode)) = (self.resolver.profile(&self.config), mode) {
            out.push(Output::ModeChanged {
                profile: profile.name.clone(),
                mode_id: mode.id.clone(),
                mode_name: mode.name.clone(),
                icon: mode.icon.clone(),
            });
        }
    }
}

/// Sends the key the knob would have produced without this program.
fn replay_gesture(gesture: Gesture, out: &mut Vec<Output>) {
    let key = match gesture {
        Gesture::Rotate { dir: Dir::Cw, .. } => HookKey::VolumeUp,
        Gesture::Rotate { dir: Dir::Ccw, .. } => HookKey::VolumeDown,
        Gesture::Click { .. } | Gesture::DoubleClick { .. } | Gesture::LongPress { .. } => {
            HookKey::Mute
        }
    };
    out.push(Output::Replay { key, down: true });
    out.push(Output::Replay { key, down: false });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::{MS, Modifiers};

    fn knob_step(engine: &mut Engine, t_ms: u64, dir: Dir) -> Vec<Output> {
        let (key, usage) = match dir {
            Dir::Cw => (HookKey::VolumeUp, 0xE9),
            Dir::Ccw => (HookKey::VolumeDown, 0xEA),
        };
        let t = t_ms * MS;
        let mods = Modifiers::default();
        let mut out = engine.handle(Input::DeviceReport { t, usage });
        out.extend(engine.handle(Input::Key {
            t,
            key,
            down: true,
            mods,
        }));
        out.extend(engine.handle(Input::Key {
            t: t + 4 * MS,
            key,
            down: false,
            mods,
        }));
        out
    }

    fn knob_press(engine: &mut Engine, t_ms: u64, hold_ms: u64) -> Vec<Output> {
        let t = t_ms * MS;
        let mods = Modifiers::default();
        let mut out = engine.handle(Input::DeviceReport { t, usage: 0xE2 });
        out.extend(engine.handle(Input::Key {
            t,
            key: HookKey::Mute,
            down: true,
            mods,
        }));
        out.extend(engine.poll(t + hold_ms * MS));
        out.extend(engine.handle(Input::Key {
            t: t + hold_ms * MS,
            key: HookKey::Mute,
            down: false,
            mods,
        }));
        out
    }

    #[test]
    fn without_a_profile_the_knob_is_stock() {
        let mut engine = Engine::new(Config::default_config());
        let out = knob_step(&mut engine, 0, Dir::Cw);
        assert_eq!(
            out,
            vec![
                Output::Replay {
                    key: HookKey::VolumeUp,
                    down: true
                },
                Output::Replay {
                    key: HookKey::VolumeUp,
                    down: false
                },
            ]
        );
    }

    #[test]
    fn rotation_runs_the_mode_action_and_long_press_cycles() {
        let mut engine = Engine::new(Config::default_config());
        let out = engine.set_foreground_app("explorer.exe");
        assert!(matches!(&out[..], [Output::ModeChanged { mode_id, .. }] if mode_id == "volume"));

        let out = knob_step(&mut engine, 0, Dir::Cw);
        assert!(matches!(
            &out[..],
            [Output::Run {
                action: Action::Volume { step: 2, .. },
                ..
            }]
        ));

        let out = knob_press(&mut engine, 1000, 700);
        assert!(matches!(&out[..], [Output::ModeChanged { mode_id, .. }] if mode_id == "scroll"));

        let out = knob_step(&mut engine, 3000, Dir::Ccw);
        assert!(matches!(
            &out[..],
            [Output::Run {
                action: Action::Scroll { .. },
                gesture: Gesture::Rotate { dir: Dir::Ccw, .. }
            }]
        ));
    }

    #[test]
    fn short_press_is_a_click() {
        let mut engine = Engine::new(Config::default_config());
        engine.set_foreground_app("explorer.exe");
        let out = knob_press(&mut engine, 0, 150);
        assert!(matches!(
            &out[..],
            [Output::Run {
                action: Action::Mute,
                ..
            }]
        ));
    }
}
