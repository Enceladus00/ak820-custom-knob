//! Platform-independent logic for the AK820 knob.
//!
//! Nothing here touches Windows: inputs come in as plain values with
//! timestamps, outputs go out as plain values. That keeps the whole pipeline
//! deterministic and testable against recorded probe logs.
//!
//! ```text
//! Input ─► Classifier ─► GestureRecognizer ─► Engine (config, modes) ─► Output
//! ```

pub mod classify;
pub mod config;
pub mod engine;
pub mod extras;
pub mod game;
pub mod gesture;
pub mod hint_style;
pub mod keys;
pub mod pointer;
pub mod remap;
pub mod resolver;
pub mod router;
pub mod signal;

pub use config::{
    Action, Config, ConfigError, Effect, LightingLayout, Mode, MouseButton, Now, Pick, Profile,
    Situation, Theme, UiPrefs, effect_for,
};
pub use engine::{Engine, Output};
pub use gesture::{Gesture, GestureKind, accel_factor};
pub use keys::{Key, KeyCombo};
pub use signal::{Dir, HookKey, Input, MS, Micros, Modifiers};
