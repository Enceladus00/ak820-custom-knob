//! Turns held arrow keys into pointer movement, and generates the short
//! letter labels used by click hints.

use crate::router::MouseControl;

const START_SPEED: f32 = 250.0;
const SLOW_SPEED: f32 = 110.0;
const RAMP_SECS: f32 = 0.55;
const SCROLL_DELAY_SECS: f32 = 0.30;
const SCROLL_REPEAT_SECS: f32 = 0.09;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PointerStep {
    pub dx: i32,
    pub dy: i32,
    /// Positive is up.
    pub scroll: i32,
}

#[derive(Debug, Default)]
pub struct PointerMotion {
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    slow: bool,
    scroll: i32,
    held_secs: f32,
    remainder: (f32, f32),
    scroll_timer: f32,
    pending_scroll: i32,
}

impl PointerMotion {
    /// Records a movement key. Returns false for keys handled elsewhere.
    pub fn set(&mut self, control: MouseControl, down: bool) -> bool {
        if !self.is_moving() {
            self.held_secs = 0.0;
            self.remainder = (0.0, 0.0);
        }
        match control {
            MouseControl::Left => self.left = down,
            MouseControl::Right => self.right = down,
            MouseControl::Up => self.up = down,
            MouseControl::Down => self.down = down,
            MouseControl::Slow => self.slow = down,
            MouseControl::ScrollUp => self.set_scroll(1, down),
            MouseControl::ScrollDown => self.set_scroll(-1, down),
            MouseControl::ClickLeft | MouseControl::ClickRight | MouseControl::ClickMiddle => {
                return false;
            }
        }
        true
    }

    fn set_scroll(&mut self, direction: i32, down: bool) {
        if down {
            self.scroll = direction;
            self.pending_scroll = direction; // one notch right away
            self.scroll_timer = SCROLL_DELAY_SECS;
        } else if self.scroll == direction {
            self.scroll = 0;
        }
    }

    fn is_moving(&self) -> bool {
        self.left || self.right || self.up || self.down
    }

    /// Whether `tick` still has work to do.
    pub fn is_active(&self) -> bool {
        self.left
            || self.right
            || self.up
            || self.down
            || self.scroll != 0
            || self.pending_scroll != 0
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Advances by `dt` seconds with a top speed in pixels per second.
    pub fn tick(&mut self, dt: f32, top_speed: f32) -> PointerStep {
        let x = i32::from(self.right) - i32::from(self.left);
        let y = i32::from(self.down) - i32::from(self.up);
        let mut step = PointerStep {
            scroll: self.next_scroll(dt),
            ..Default::default()
        };
        if x == 0 && y == 0 {
            self.held_secs = 0.0;
            self.remainder = (0.0, 0.0);
            return step;
        }
        self.held_secs += dt;
        let speed = if self.slow {
            SLOW_SPEED
        } else {
            let ramp = (self.held_secs / RAMP_SECS).min(1.0);
            START_SPEED + (top_speed - START_SPEED).max(0.0) * ramp * ramp
        };
        let scale = if x != 0 && y != 0 {
            std::f32::consts::FRAC_1_SQRT_2
        } else {
            1.0
        };
        let dx = x as f32 * speed * scale * dt + self.remainder.0;
        let dy = y as f32 * speed * scale * dt + self.remainder.1;
        step.dx = dx.trunc() as i32;
        step.dy = dy.trunc() as i32;
        self.remainder = (dx.fract(), dy.fract());
        step
    }

    fn next_scroll(&mut self, dt: f32) -> i32 {
        if self.pending_scroll != 0 {
            return std::mem::take(&mut self.pending_scroll);
        }
        if self.scroll == 0 {
            return 0;
        }
        self.scroll_timer -= dt;
        if self.scroll_timer > 0.0 {
            return 0;
        }
        self.scroll_timer += SCROLL_REPEAT_SECS;
        self.scroll
    }
}

/// Home row first, so that the most common labels are the easiest to reach.
const HINT_LETTERS: &[u8] = b"ASDFGHJKLQWERTYUIOPZXCVBNM";

/// `count` labels of equal length, so that no label is the start of another.
pub fn hint_labels(count: usize) -> Vec<String> {
    let base = HINT_LETTERS.len();
    let mut length = 1;
    let mut capacity = base;
    while capacity < count {
        length += 1;
        capacity *= base;
    }
    (0..count)
        .map(|index| {
            let mut value = index;
            let mut letters = vec![0u8; length];
            for slot in letters.iter_mut().rev() {
                *slot = HINT_LETTERS[value % base];
                value /= base;
            }
            String::from_utf8_lossy(&letters).into_owned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: f32 = 0.008;

    fn run(motion: &mut PointerMotion, ticks: usize) -> PointerStep {
        let mut total = PointerStep::default();
        for _ in 0..ticks {
            let step = motion.tick(TICK, 1400.0);
            total.dx += step.dx;
            total.dy += step.dy;
            total.scroll += step.scroll;
        }
        total
    }

    #[test]
    fn it_starts_gently_and_speeds_up() {
        let mut motion = PointerMotion::default();
        motion.set(MouseControl::Right, true);
        let first = run(&mut motion, 12).dx;
        run(&mut motion, 80);
        let later = run(&mut motion, 12).dx;
        assert!(first > 0);
        assert!(later > first * 3, "{first} then {later}");
    }

    #[test]
    fn slow_key_gives_fine_control() {
        let mut fast = PointerMotion::default();
        fast.set(MouseControl::Down, true);
        run(&mut fast, 100);
        let mut slow = PointerMotion::default();
        slow.set(MouseControl::Down, true);
        slow.set(MouseControl::Slow, true);
        run(&mut slow, 100);
        assert!(run(&mut slow, 25).dy < run(&mut fast, 25).dy / 3);
    }

    #[test]
    fn opposite_keys_cancel() {
        let mut motion = PointerMotion::default();
        motion.set(MouseControl::Left, true);
        motion.set(MouseControl::Right, true);
        assert_eq!(run(&mut motion, 50).dx, 0);
    }

    #[test]
    fn diagonals_are_not_faster() {
        let mut straight = PointerMotion::default();
        straight.set(MouseControl::Right, true);
        let along = run(&mut straight, 100).dx as f32;
        let mut diagonal = PointerMotion::default();
        diagonal.set(MouseControl::Right, true);
        diagonal.set(MouseControl::Down, true);
        let step = run(&mut diagonal, 100);
        let distance = ((step.dx * step.dx + step.dy * step.dy) as f32).sqrt();
        assert!((distance - along).abs() < along * 0.03);
    }

    #[test]
    fn small_movements_are_not_lost_to_rounding() {
        let mut motion = PointerMotion::default();
        motion.set(MouseControl::Right, true);
        motion.set(MouseControl::Slow, true);
        let moved = run(&mut motion, 125).dx;
        assert!((108..=112).contains(&moved), "{moved}");
    }

    #[test]
    fn releasing_stops_movement() {
        let mut motion = PointerMotion::default();
        motion.set(MouseControl::Left, true);
        run(&mut motion, 10);
        motion.set(MouseControl::Left, false);
        assert!(!motion.is_active());
        assert_eq!(run(&mut motion, 10).dx, 0);
    }

    #[test]
    fn every_new_press_starts_gently_again() {
        let mut motion = PointerMotion::default();
        motion.set(MouseControl::Right, true);
        let first = run(&mut motion, 12).dx;
        run(&mut motion, 80);
        motion.set(MouseControl::Right, false);
        motion.set(MouseControl::Right, true);
        assert_eq!(run(&mut motion, 12).dx, first);
    }

    #[test]
    fn one_scroll_press_is_one_notch_then_it_repeats() {
        let mut motion = PointerMotion::default();
        motion.set(MouseControl::ScrollUp, true);
        assert_eq!(run(&mut motion, 30).scroll, 1);
        assert!(run(&mut motion, 60).scroll >= 4);
        motion.set(MouseControl::ScrollUp, false);
        assert_eq!(run(&mut motion, 30).scroll, 0);
    }

    #[test]
    fn click_keys_are_not_movement() {
        let mut motion = PointerMotion::default();
        assert!(!motion.set(MouseControl::ClickLeft, true));
    }

    #[test]
    fn labels_are_unique_and_the_same_length() {
        for count in [1, 26, 27, 300, 676, 677] {
            let labels = hint_labels(count);
            assert_eq!(labels.len(), count);
            let length = labels[0].len();
            assert!(labels.iter().all(|l| l.len() == length));
            let mut sorted = labels.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), count);
        }
    }

    #[test]
    fn a_few_labels_are_single_home_row_letters() {
        assert_eq!(hint_labels(3), ["A", "S", "D"]);
        assert_eq!(hint_labels(27)[0], "AA");
    }

    #[test]
    fn no_labels_for_nothing() {
        assert!(hint_labels(0).is_empty());
    }
}
