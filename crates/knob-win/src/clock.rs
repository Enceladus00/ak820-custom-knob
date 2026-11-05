//! One monotonic clock shared by all threads, in knob-core's microseconds.

use std::sync::OnceLock;
use std::time::Instant;

use knob_core::Micros;

static START: OnceLock<Instant> = OnceLock::new();

pub fn now() -> Micros {
    START.get_or_init(Instant::now).elapsed().as_micros() as Micros
}
