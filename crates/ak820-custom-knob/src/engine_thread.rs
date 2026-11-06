//! The engine thread: feeds input into `knob_core::Engine`, fires its timers
//! and carries out what it decides.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use knob_core::{Config, Engine};
use knob_win::clock;
use knob_win::input::{self, InputEvent};

use crate::actions::Actions;
use crate::i18n::{Text, tr};
use crate::ui::{self, UiEvent};

pub enum EngineMsg {
    Input(InputEvent),
    Config(Config),
}

pub fn start(config: Config) -> Sender<EngineMsg> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("engine".into())
        .spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| run(config, rx)));
            // Whatever happened, stop swallowing keys: the knob falls back to stock.
            input::set_engine_alive(false);
            if result.is_err() {
                log::error!("engine crashed; the knob is back to its normal behaviour");
                ui::notify(UiEvent::Warning {
                    title: tr(Text::CrashTitle).into(),
                    text: tr(Text::CrashMessage).into(),
                });
            }
        })
        .expect("spawn engine thread");
    tx
}

fn run(config: Config, rx: Receiver<EngineMsg>) {
    let mut engine = Engine::new(config);
    let mut actions = Actions::new();

    loop {
        let msg = match engine.next_deadline() {
            Some(deadline) => {
                let wait = Duration::from_micros(deadline.saturating_sub(clock::now()));
                match rx.recv_timeout(wait) {
                    Ok(msg) => Some(msg),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            None => match rx.recv() {
                Ok(msg) => Some(msg),
                Err(_) => return,
            },
        };

        // Mode changes caused by the knob are announced on screen; ones caused
        // by switching apps or reloading the config only update the tray.
        let (outputs, announce) = match msg {
            None => (engine.poll(clock::now()), true),
            Some(EngineMsg::Input(InputEvent::Core(input))) => (engine.handle(input), true),
            Some(EngineMsg::Input(InputEvent::Foreground { exe, .. })) => {
                log::debug!("foreground: {exe}");
                actions.set_foreground(&exe);
                (engine.set_foreground_app(&exe), false)
            }
            Some(EngineMsg::Config(config)) => (engine.set_config(config), false),
            Some(EngineMsg::Input(InputEvent::KeyboardPresent(_))) => continue, // handled by the UI
        };
        for output in outputs {
            actions.execute(output, announce);
        }
    }
}
