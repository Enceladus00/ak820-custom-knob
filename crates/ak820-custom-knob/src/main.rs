//! ak820-custom-knob: gives the AJAZZ AK820 MAX knob custom, per-app
//! functions, without changing any other key.
//!
//! Threads:
//! * input  (knob-win): keyboard hook, Raw Input, foreground app
//! * engine: knob-core's engine, timers, actions
//! * config-watch: reloads the config file when it is saved
//! * main (UI): tray icon and on-screen popup

// Release builds run without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod config_file;
mod engine_thread;
mod logger;
mod osd;
mod paths;
mod ui;

use std::sync::OnceLock;
use std::sync::mpsc::Sender;
use std::thread;

use knob_win::input::{self, InputEvent};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
use windows::core::w;

use crate::engine_thread::EngineMsg;
use crate::ui::UiEvent;

static ENGINE: OnceLock<Sender<EngineMsg>> = OnceLock::new();

fn main() {
    let Some(_instance) = single_instance() else {
        unsafe {
            MessageBoxW(
                None,
                w!("AK820 knob is already running (see the tray)."),
                w!("AK820 knob"),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        return;
    };

    logger::init(&paths::log_dir());
    log::info!("ak820-custom-knob {} starting", env!("CARGO_PKG_VERSION"));
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let (config, config_error) = config_file::load_or_create();
    log::info!("config: {}", paths::config_file().display());
    if let Some(e) = &config_error {
        log::warn!("config has errors, using the built-in default: {e}");
    }

    let engine = engine_thread::start(config);
    let _ = ENGINE.set(engine.clone());

    config_file::watch(move |result| match result {
        Ok(config) => {
            log::info!("config reloaded");
            let _ = engine.send(EngineMsg::Config(config));
            ui::notify(UiEvent::Osd("Settings reloaded".into()));
        }
        Err(e) => {
            log::warn!("config has errors, keeping the previous one: {e}");
            ui::notify(UiEvent::Warning {
                title: "Config file has an error".into(),
                text: e,
            });
        }
    });

    thread::Builder::new()
        .name("input".into())
        .spawn(|| {
            if let Err(e) = input::run(forward_input) {
                log::error!("input listeners failed: {e}");
                ui::notify(UiEvent::Warning {
                    title: "AK820 knob can't read the keyboard".into(),
                    text: format!("{e}"),
                });
            }
        })
        .expect("spawn input thread");

    let startup_warning = config_error.map(|e| {
        (
            "Config file has an error".to_string(),
            format!("Using the default settings. {e}"),
        )
    });
    if let Err(e) = ui::run(startup_warning) {
        log::error!("tray failed: {e}");
    }

    input::request_stop();
    log::info!("exiting");
    log::logger().flush();
}

/// Called on the input thread for every event: hand it to the engine.
fn forward_input(event: InputEvent) {
    let delivered = ENGINE
        .get()
        .is_some_and(|tx| tx.send(EngineMsg::Input(event)).is_ok());
    if !delivered {
        input::set_engine_alive(false);
    }
}

/// Holds a named mutex while running; `None` if another instance has it.
fn single_instance() -> Option<HANDLE> {
    unsafe {
        let handle = CreateMutexW(None, true, w!("Local\\ak820-custom-knob")).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }
        Some(handle)
    }
}
