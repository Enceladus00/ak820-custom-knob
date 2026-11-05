//! Tells knob events apart from ordinary volume keys.
//!
//! The knob and Fn+F2/F3 send identical HID reports. The difference is
//! timing: a knob detent is released after ~5 ms, a finger press after
//! 75 ms or more (see docs/probe-findings.md). The device is known from the
//! consumer report, which arrives just before the hook's key event.

use std::collections::VecDeque;

use crate::signal::{Dir, HookKey, Input, MS, Micros, Modifiers};

/// A volume key from the AK820 released within this time is a knob detent.
pub const STEP_MAX_HOLD: Micros = 30 * MS;
/// How far apart a device report and a hook event may be and still match.
pub const REPORT_WINDOW: Micros = 20 * MS;
/// Reports older than this are dropped.
const REPORT_KEEP: Micros = 200 * MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classified {
    /// One knob detent.
    Step {
        t: Micros,
        dir: Dir,
        mods: Modifiers,
    },
    /// Knob pressed down.
    Press { t: Micros, mods: Modifiers },
    /// Knob released.
    Release { t: Micros },
    /// Not the knob: send this key event on to Windows unchanged.
    Replay { key: HookKey, down: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyState {
    Idle,
    /// Down, not decided yet.
    Pending {
        t_down: Micros,
        mods: Modifiers,
        from_knob: bool,
    },
    /// Decided "not the knob" and the down was replayed; replay the rest.
    Forwarding,
    /// Knob press in progress.
    KnobHeld,
}

#[derive(Debug)]
pub struct Classifier {
    keys: [KeyState; 3],
    /// Recent unmatched device reports: (time, usage).
    reports: VecDeque<(Micros, u16)>,
}

impl Default for Classifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Classifier {
    pub fn new() -> Self {
        Self {
            keys: [KeyState::Idle; 3],
            reports: VecDeque::new(),
        }
    }

    pub fn handle(&mut self, input: Input, out: &mut Vec<Classified>) {
        match input {
            Input::DeviceReport { t, usage } => {
                while let Some(&(t0, _)) = self.reports.front() {
                    if t.saturating_sub(t0) > REPORT_KEEP {
                        self.reports.pop_front();
                    } else {
                        break;
                    }
                }
                if usage != 0 {
                    self.reports.push_back((t, usage));
                }
            }
            Input::Key {
                t,
                key,
                down: true,
                mods,
            } => self.key_down(t, key, mods, out),
            Input::Key {
                t,
                key,
                down: false,
                ..
            } => self.key_up(t, key, out),
        }
    }

    /// Earliest time at which `poll` has something to decide.
    pub fn next_deadline(&self) -> Option<Micros> {
        HookKey::ALL
            .iter()
            .filter_map(|&key| match self.keys[key as usize] {
                KeyState::Pending { t_down, .. } => Some(t_down + deadline(key)),
                _ => None,
            })
            .min()
    }

    /// Decides keys whose deadline has passed at `now`.
    pub fn poll(&mut self, now: Micros, out: &mut Vec<Classified>) {
        for key in HookKey::ALL {
            let i = key as usize;
            if let KeyState::Pending {
                t_down,
                mods,
                from_knob,
            } = self.keys[i]
            {
                if now < t_down + deadline(key) {
                    continue;
                }
                let from_knob = from_knob || self.take_report(key, t_down);
                if key == HookKey::Mute && from_knob {
                    self.keys[i] = KeyState::KnobHeld;
                    out.push(Classified::Press { t: t_down, mods });
                } else {
                    // Still held after STEP_MAX_HOLD (Fn+F2/F3), or another device.
                    self.keys[i] = KeyState::Forwarding;
                    out.push(Classified::Replay { key, down: true });
                }
            }
        }
    }

    fn key_down(&mut self, t: Micros, key: HookKey, mods: Modifiers, out: &mut Vec<Classified>) {
        let i = key as usize;
        match self.keys[i] {
            // Auto-repeat of a forwarded key: forward it too.
            KeyState::Forwarding => out.push(Classified::Replay { key, down: true }),
            // Auto-repeat while undecided or while the knob is held: ignore.
            KeyState::Pending { .. } | KeyState::KnobHeld => {}
            KeyState::Idle => {
                let from_knob = self.take_report(key, t);
                if key == HookKey::Mute && from_knob {
                    self.keys[i] = KeyState::KnobHeld;
                    out.push(Classified::Press { t, mods });
                } else {
                    self.keys[i] = KeyState::Pending {
                        t_down: t,
                        mods,
                        from_knob,
                    };
                }
            }
        }
    }

    fn key_up(&mut self, t: Micros, key: HookKey, out: &mut Vec<Classified>) {
        let i = key as usize;
        match self.keys[i] {
            KeyState::Idle | KeyState::Forwarding => {
                out.push(Classified::Replay { key, down: false })
            }
            KeyState::KnobHeld => out.push(Classified::Release { t }),
            KeyState::Pending {
                t_down,
                mods,
                from_knob,
            } => {
                let from_knob = from_knob || self.take_report(key, t_down);
                let quick = t.saturating_sub(t_down) < STEP_MAX_HOLD;
                match (key, from_knob) {
                    (HookKey::Mute, true) => {
                        out.push(Classified::Press { t: t_down, mods });
                        out.push(Classified::Release { t });
                    }
                    (HookKey::VolumeUp, true) if quick => {
                        out.push(Classified::Step {
                            t: t_down,
                            dir: Dir::Cw,
                            mods,
                        });
                    }
                    (HookKey::VolumeDown, true) if quick => {
                        out.push(Classified::Step {
                            t: t_down,
                            dir: Dir::Ccw,
                            mods,
                        });
                    }
                    _ => {
                        out.push(Classified::Replay { key, down: true });
                        out.push(Classified::Replay { key, down: false });
                    }
                }
            }
        }
        self.keys[i] = KeyState::Idle;
    }

    /// Finds and consumes a device report for `key` close to time `t`.
    fn take_report(&mut self, key: HookKey, t: Micros) -> bool {
        let usage = key.consumer_usage();
        let found = self
            .reports
            .iter()
            .position(|&(tr, u)| u == usage && tr.abs_diff(t) <= REPORT_WINDOW);
        if let Some(i) = found {
            self.reports.remove(i);
        }
        found.is_some()
    }
}

/// How long a key may stay undecided.
fn deadline(key: HookKey) -> Micros {
    match key {
        // Mute: only the device is unknown, so wait for a late report at most.
        HookKey::Mute => REPORT_WINDOW,
        HookKey::VolumeUp | HookKey::VolumeDown => STEP_MAX_HOLD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AK_UP: u16 = 0xE9;
    const AK_MUTE: u16 = 0xE2;

    fn report(t_ms: u64, usage: u16) -> Input {
        Input::DeviceReport {
            t: t_ms * MS,
            usage,
        }
    }
    fn key(t_ms: u64, key: HookKey, down: bool) -> Input {
        Input::Key {
            t: t_ms * MS,
            key,
            down,
            mods: Modifiers::default(),
        }
    }

    /// Feeds inputs in order, polling deadlines in between like the engine does.
    fn run(inputs: &[Input]) -> Vec<Classified> {
        let mut c = Classifier::new();
        let mut out = Vec::new();
        for &input in inputs {
            while let Some(d) = c.next_deadline().filter(|&d| d <= input.time()) {
                c.poll(d, &mut out);
            }
            c.handle(input, &mut out);
        }
        c.poll(u64::MAX / 2, &mut out);
        out
    }

    #[test]
    fn knob_detent_is_a_step() {
        let out = run(&[
            report(100, AK_UP),
            key(100, HookKey::VolumeUp, true),
            report(104, 0),
            key(104, HookKey::VolumeUp, false),
        ]);
        assert_eq!(
            out,
            vec![Classified::Step {
                t: 100 * MS,
                dir: Dir::Cw,
                mods: Modifiers::default()
            }]
        );
    }

    #[test]
    fn fn_f3_is_replayed_after_the_deadline() {
        let out = run(&[
            report(100, AK_UP),
            key(100, HookKey::VolumeUp, true),
            key(190, HookKey::VolumeUp, false),
        ]);
        assert_eq!(
            out,
            vec![
                Classified::Replay {
                    key: HookKey::VolumeUp,
                    down: true
                },
                Classified::Replay {
                    key: HookKey::VolumeUp,
                    down: false
                },
            ]
        );
    }

    #[test]
    fn other_device_quick_tap_is_replayed() {
        // No AK820 report: some other keyboard's volume key.
        let out = run(&[
            key(100, HookKey::VolumeUp, true),
            key(104, HookKey::VolumeUp, false),
        ]);
        assert_eq!(
            out,
            vec![
                Classified::Replay {
                    key: HookKey::VolumeUp,
                    down: true
                },
                Classified::Replay {
                    key: HookKey::VolumeUp,
                    down: false
                },
            ]
        );
    }

    #[test]
    fn knob_click_is_press_and_release() {
        let out = run(&[
            report(100, AK_MUTE),
            key(100, HookKey::Mute, true),
            key(270, HookKey::Mute, false),
        ]);
        assert_eq!(
            out,
            vec![
                Classified::Press {
                    t: 100 * MS,
                    mods: Modifiers::default()
                },
                Classified::Release { t: 270 * MS },
            ]
        );
    }

    #[test]
    fn other_device_mute_is_replayed() {
        let out = run(&[
            key(100, HookKey::Mute, true),
            key(250, HookKey::Mute, false),
        ]);
        assert_eq!(
            out,
            vec![
                Classified::Replay {
                    key: HookKey::Mute,
                    down: true
                },
                Classified::Replay {
                    key: HookKey::Mute,
                    down: false
                },
            ]
        );
    }

    #[test]
    fn report_is_used_only_once() {
        // One AK820 report, then two key presses: only the first is the knob.
        let out = run(&[
            report(100, AK_UP),
            key(100, HookKey::VolumeUp, true),
            key(104, HookKey::VolumeUp, false),
            key(110, HookKey::VolumeUp, true),
            key(114, HookKey::VolumeUp, false),
        ]);
        assert_eq!(out.len(), 3);
        assert!(matches!(out[0], Classified::Step { .. }));
        assert!(matches!(out[1], Classified::Replay { down: true, .. }));
    }
}
