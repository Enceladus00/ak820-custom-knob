//! ak820-custom-knob: gives the AJAZZ AK820 MAX Ultra knob custom, per-app
//! functions, without changing any other key.
//!
//! Threads:
//! * input  (knob-win): keyboard hook, Raw Input, foreground app
//! * engine: knob-core's engine, timers, actions
//! * extras: macros, mouse mode and click hints
//! * config-watch: reloads the config file when it is saved
//! * main (UI): tray icon and on-screen popup

// Release builds run without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod config_file;
mod engine_thread;
mod extras;
mod hint_overlay;
mod i18n;
mod logger;
mod osd;
mod ui;

use std::sync::OnceLock;
use std::sync::mpsc::Sender;
use std::thread;

use knob_core::config::AUTO_LANGUAGE;
use knob_win::input::{self, InputEvent};
use knob_win::paths;
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
use windows::core::{HSTRING, w};

use crate::engine_thread::EngineMsg;
use crate::i18n::{Text, tr, trf};
use crate::ui::UiEvent;

static ENGINE: OnceLock<Sender<EngineMsg>> = OnceLock::new();

fn main() {
    i18n::set_language(AUTO_LANGUAGE); // until the config is read
    let Some(_instance) = single_instance() else {
        unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(tr(Text::AlreadyRunning)),
                &HSTRING::from(tr(Text::Tooltip)),
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
    i18n::set_language(&config.ui.language);
    if let Ok(exe) = std::env::current_exe() {
        knob_win::autostart::repair(&exe);
    }

    extras::start(config.clone());
    let engine = engine_thread::start(config);
    let _ = ENGINE.set(engine.clone());

    config_file::watch(move |result| match result {
        Ok(config) => {
            log::info!("config reloaded");
            i18n::set_language(&config.ui.language);
            extras::send(extras::ExtrasMsg::Config(Box::new(config.clone())));
            let _ = engine.send(EngineMsg::Config(config));
            ui::notify(UiEvent::LanguageChanged);
            ui::notify(UiEvent::Osd(tr(Text::OsdReloaded).into()));
        }
        Err(e) => {
            log::warn!("config has errors, keeping the previous one: {e}");
            ui::notify(UiEvent::Warning {
                title: tr(Text::ConfigErrorTitle).into(),
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
                    title: tr(Text::InputErrorTitle).into(),
                    text: format!("{e}"),
                });
            }
        })
        .expect("spawn input thread");

    let startup_warning = config_error.map(|e| {
        (
            tr(Text::ConfigErrorTitle).to_string(),
            trf(Text::ConfigErrorDefaults, &[("error", &e)]),
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
    if let InputEvent::KeyboardPresent(_) = event {
        ui::notify(ui::UiEvent::KeyboardChanged);
        return;
    }
    if let InputEvent::Extra(event) = event {
        extras::send(extras::ExtrasMsg::Router(event));
        return;
    }
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
