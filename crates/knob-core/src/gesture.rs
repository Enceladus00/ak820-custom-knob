//! Turns knob steps and presses into gestures: rotation (with speed),
//! click, double-click and long press.

use std::collections::VecDeque;

use crate::classify::Classified;
use crate::signal::{Dir, MS, Micros, Modifiers};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesture {
    /// One detent. `speed` is detents per second over the last moment.
    Rotate {
        dir: Dir,
        mods: Modifiers,
        speed: f32,
    },
    Click {
        mods: Modifiers,
    },
    DoubleClick {
        mods: Modifiers,
    },
    LongPress {
        mods: Modifiers,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureKind {
    Rotate,
    Click,
    DoubleClick,
    LongPress,
}

impl Gesture {
    pub fn kind(&self) -> GestureKind {
        match self {
            Gesture::Rotate { .. } => GestureKind::Rotate,
            Gesture::Click { .. } => GestureKind::Click,
            Gesture::DoubleClick { .. } => GestureKind::DoubleClick,
            Gesture::LongPress { .. } => GestureKind::LongPress,
        }
    }

    pub fn mods(&self) -> Modifiers {
        match *self {
            Gesture::Rotate { mods, .. }
            | Gesture::Click { mods }
            | Gesture::DoubleClick { mods }
            | Gesture::LongPress { mods } => mods,
        }
    }
}

/// Multiplier for rotation actions: 1× for normal turning, up to 4× for a
/// fast spin. Measured: slow turns are well under 15 detents/s, a fast spin
/// reaches 60–100.
pub fn accel_factor(speed: f32) -> f32 {
    (speed / 15.0).clamp(1.0, 4.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GestureTiming {
    pub long_press: Micros,
    pub double_click: Micros,
}

impl Default for GestureTiming {
    fn default() -> Self {
        Self {
            long_press: 500 * MS,
            double_click: 250 * MS,
        }
    }
}

/// Speed is measured over this window.
const SPEED_WINDOW: Micros = 150 * MS;

#[derive(Debug, Clone, Copy)]
struct Press {
    t: Micros,
    mods: Modifiers,
    /// Second press of a double-click.
    second: bool,
    /// Already used up (long press fired, or the knob was turned while held).
    consumed: bool,
}

#[derive(Debug)]
pub struct GestureRecognizer {
    timing: GestureTiming,
    /// Only when the current mode binds double-click do single clicks wait.
    double_click_enabled: bool,
    press: Option<Press>,
    /// Released once; waiting to see whether a second press follows.
    awaiting_second: Option<(Micros, Modifiers)>,
    steps: VecDeque<(Micros, Dir)>,
}

impl GestureRecognizer {
    pub fn new(timing: GestureTiming) -> Self {
        Self {
            timing,
            double_click_enabled: false,
            press: None,
            awaiting_second: None,
            steps: VecDeque::new(),
        }
    }

    pub fn set_timing(&mut self, timing: GestureTiming) {
        self.timing = timing;
    }

    pub fn set_double_click_enabled(&mut self, enabled: bool) {
        self.double_click_enabled = enabled;
    }

    pub fn handle(&mut self, event: Classified, out: &mut Vec<Gesture>) {
        match event {
            Classified::Step { t, dir, mods } => {
                self.flush_awaiting(out);
                if let Some(press) = &mut self.press {
                    press.consumed = true;
                }
                if self.steps.back().is_some_and(|&(_, d)| d != dir) {
                    self.steps.clear();
                }
                while self
                    .steps
                    .front()
                    .is_some_and(|&(t0, _)| t.saturating_sub(t0) > SPEED_WINDOW)
                {
                    self.steps.pop_front();
                }
                self.steps.push_back((t, dir));
                let speed = self.steps.len() as f32 / (SPEED_WINDOW as f32 / 1_000_000.0);
                out.push(Gesture::Rotate { dir, mods, speed });
            }
            Classified::Press { t, mods } => {
                let second = match self.awaiting_second.take() {
                    Some((released, _))
                        if t.saturating_sub(released) <= self.timing.double_click =>
                    {
                        true
                    }
                    Some((_, first_mods)) => {
                        out.push(Gesture::Click { mods: first_mods });
                        false
                    }
                    None => false,
                };
                self.press = Some(Press {
                    t,
                    mods,
                    second,
                    consumed: false,
                });
            }
            Classified::Release { t } => {
                let Some(press) = self.press.take() else {
                    return;
                };
                if press.consumed {
                    return;
                }
                if press.second {
                    out.push(Gesture::DoubleClick { mods: press.mods });
                } else if self.double_click_enabled {
                    self.awaiting_second = Some((t, press.mods));
                } else {
                    out.push(Gesture::Click { mods: press.mods });
                }
            }
            Classified::Replay { .. } => {}
        }
    }

    pub fn next_deadline(&self) -> Option<Micros> {
        let long = self
            .press
            .filter(|p| !p.consumed)
            .map(|p| p.t + self.timing.long_press);
        let double = self
            .awaiting_second
            .map(|(t, _)| t + self.timing.double_click);
        match (long, double) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    pub fn poll(&mut self, now: Micros, out: &mut Vec<Gesture>) {
        if let Some(press) = &mut self.press
            && !press.consumed
            && now >= press.t + self.timing.long_press
        {
            press.consumed = true;
            out.push(Gesture::LongPress { mods: press.mods });
        }
        if let Some((t, mods)) = self.awaiting_second
            && now >= t + self.timing.double_click
        {
            self.awaiting_second = None;
            out.push(Gesture::Click { mods });
        }
    }

    fn flush_awaiting(&mut self, out: &mut Vec<Gesture>) {
        if let Some((_, mods)) = self.awaiting_second.take() {
            out.push(Gesture::Click { mods });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO: Modifiers = Modifiers {
        ctrl: false,
        shift: false,
        alt: false,
        win: false,
    };

    fn press(t_ms: u64) -> Classified {
        Classified::Press {
            t: t_ms * MS,
            mods: NO,
        }
    }
    fn release(t_ms: u64) -> Classified {
        Classified::Release { t: t_ms * MS }
    }
    fn step(t_ms: u64, dir: Dir) -> Classified {
        Classified::Step {
            t: t_ms * MS,
            dir,
            mods: NO,
        }
    }

    fn run(double: bool, events: &[(u64, Classified)]) -> Vec<Gesture> {
        let mut g = GestureRecognizer::new(GestureTiming::default());
        g.set_double_click_enabled(double);
        let mut out = Vec::new();
        for &(t_ms, event) in events {
            while let Some(d) = g.next_deadline().filter(|&d| d <= t_ms * MS) {
                g.poll(d, &mut out);
            }
            g.handle(event, &mut out);
        }
        g.poll(u64::MAX / 2, &mut out);
        out
    }

    #[test]
    fn click_is_immediate_without_double_click() {
        let out = run(false, &[(0, press(0)), (150, release(150))]);
        assert_eq!(out, vec![Gesture::Click { mods: NO }]);
    }

    #[test]
    fn long_press_fires_while_held_and_release_is_ignored() {
        let out = run(false, &[(0, press(0)), (900, release(900))]);
        assert_eq!(out, vec![Gesture::LongPress { mods: NO }]);
    }

    #[test]
    fn double_click() {
        let out = run(
            true,
            &[
                (0, press(0)),
                (120, release(120)),
                (250, press(250)),
                (370, release(370)),
            ],
        );
        assert_eq!(out, vec![Gesture::DoubleClick { mods: NO }]);
    }

    #[test]
    fn slow_second_press_is_two_clicks() {
        let out = run(
            true,
            &[
                (0, press(0)),
                (120, release(120)),
                (600, press(600)),
                (700, release(700)),
            ],
        );
        assert_eq!(
            out,
            vec![Gesture::Click { mods: NO }, Gesture::Click { mods: NO }]
        );
    }

    #[test]
    fn turning_while_pressed_cancels_the_click() {
        let out = run(
            false,
            &[
                (0, press(0)),
                (100, step(100, Dir::Cw)),
                (200, release(200)),
            ],
        );
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], Gesture::Rotate { dir: Dir::Cw, .. }));
    }

    #[test]
    fn fast_spin_is_faster_than_slow_turning() {
        let fast: Vec<_> = (0..10).map(|i| (i * 15, step(i * 15, Dir::Cw))).collect();
        let slow: Vec<_> = (0..3).map(|i| (i * 700, step(i * 700, Dir::Cw))).collect();
        let speed = |g: &Gesture| match g {
            Gesture::Rotate { speed, .. } => *speed,
            _ => unreachable!(),
        };
        let fast_speed = speed(run(false, &fast).last().unwrap());
        let slow_speed = speed(run(false, &slow).last().unwrap());
        assert!(accel_factor(fast_speed) > 3.0, "fast: {fast_speed}");
        assert_eq!(accel_factor(slow_speed), 1.0, "slow: {slow_speed}");
    }

    #[test]
    fn direction_change_resets_speed() {
        let out = run(
            false,
            &[
                (0, step(0, Dir::Cw)),
                (10, step(10, Dir::Cw)),
                (20, step(20, Dir::Ccw)),
            ],
        );
        let Gesture::Rotate { speed, .. } = out[2] else {
            panic!()
        };
        assert!(speed < 7.0);
    }
}
