//! knob-probe: a diagnostic tool that logs everything the AK820's knob sends,
//! at three levels at once:
//!
//! * the low-level keyboard hook (what AutoHotkey sees),
//! * Raw Input (which physical device sent it, plus raw make codes / HID bytes),
//! * hidapi (raw reports from every HID interface Windows lets us open).
//!
//! A guided test walks through knob turns, knob clicks and the Fn+F-row keys,
//! so the log shows whether the knob can be told apart from those keys.
//! `knob-probe mute` runs a short test of knob presses against the Fn mute key.
//! Nothing is blocked or changed: every event passes through to Windows.

mod event;
mod hid;
mod names;
mod win;

use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn main() {
    event::START.set(Instant::now()).ok();
    if std::env::args().nth(1).as_deref() == Some("mute") {
        event::use_mute_test();
    }

    let dir = std::env::current_dir()
        .unwrap_or_default()
        .join("probe-logs");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("cannot create {}: {e}", dir.display());
        return;
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let log_path = dir.join(format!("probe-{secs}.log"));

    let logger = match event::start_logger(&log_path, win::request_quit) {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("cannot create log file {}: {e}", log_path.display());
            return;
        }
    };

    hid::enumerate_and_listen();

    if let Err(e) = win::run() {
        event::send(event::Event::Info(format!(
            "Windows input setup failed: {e}"
        )));
    }

    event::send(event::Event::Shutdown);
    let _ = logger.join();
    println!("\nLog saved to {}", log_path.display());
}
