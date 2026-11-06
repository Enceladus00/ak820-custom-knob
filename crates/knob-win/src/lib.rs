//! Windows platform layer for the AK820 knob.
//!
//! * [`input`]: keyboard hook, Raw Input and foreground-app tracking, on a
//!   dedicated thread with its own message loop.
//! * [`output`]: injects keys, wheel and clicks with `SendInput`.
//! * [`audio`]: per-app volume through Windows audio sessions.
//! * [`process`]: exe names for windows and process ids.
//! * [`autostart`]: the "Start with Windows" registry entry.

pub mod audio;
pub mod autostart;
pub mod clock;
pub mod input;
pub mod output;
pub mod paths;
pub mod process;

/// Marks input we inject ourselves (`dwExtraInfo`), so our hook lets it pass.
pub const MARKER: usize = 0x4B4E_4F42; // "KNOB"

/// Device-path fragments of supported keyboards (Raw Input device names).
pub const KNOWN_DEVICES: &[&str] = &[
    "VID_0C45&PID_FEFC", // AK820 MAX, 2.4 GHz dongle
];
