//! Carries out the engine's outputs.

use knob_core::{Action, Dir, Gesture, HookKey, Output, accel_factor};
use knob_win::audio::Audio;
use knob_win::{launch, output};

use crate::extras::{self, ExtrasMsg};
use crate::i18n::{Text, tr, trf};
use crate::ui::{self, UiEvent};

pub struct Actions {
    audio: Option<Audio>,
    foreground_exe: String,
}

impl Actions {
    /// Must be created on the thread that uses it (COM).
    pub fn new() -> Self {
        let audio = Audio::new()
            .inspect_err(|e| log::warn!("audio sessions unavailable, app volume won't work: {e}"))
            .ok();
        Self {
            audio,
            foreground_exe: String::new(),
        }
    }

    pub fn set_foreground(&mut self, exe: &str) {
        self.foreground_exe = exe.to_string();
    }

    pub fn execute(&mut self, output: Output, announce: bool) {
        log::debug!("{output:?}");
        match output {
            Output::Replay { key, down } => output::hook_key(key, down),
            Output::ModeChanged {
                mode_name, profile, ..
            } => {
                log::info!("mode: {mode_name} (profile {profile})");
                ui::notify(UiEvent::Mode {
                    name: mode_name,
                    announce,
                });
            }
            Output::Run { action, gesture } => self.run(&action, &gesture),
        }
    }

    fn run(&mut self, action: &Action, gesture: &Gesture) {
        // Direction (+1 clockwise, -1 counter-clockwise) and spin acceleration.
        let (sign, speed_factor) = match *gesture {
            Gesture::Rotate {
                dir: Dir::Cw,
                speed,
                ..
            } => (1, accel_factor(speed)),
            Gesture::Rotate {
                dir: Dir::Ccw,
                speed,
                ..
            } => (-1, accel_factor(speed)),
            _ => (0, 1.0),
        };
        let factor = |accelerate: bool| if accelerate { speed_factor } else { 1.0 };

        match action {
            Action::Volume { step, accelerate } => {
                // A volume key press is 2 % in Windows, and shows its volume flyout.
                let presses = (*step as f32 * factor(*accelerate) / 2.0).round().max(1.0) as u32;
                let key = if sign > 0 {
                    HookKey::VolumeUp
                } else {
                    HookKey::VolumeDown
                };
                output::tap_hook_key(key, presses);
            }
            Action::AppVolume { step, accelerate } => {
                let delta = sign as f32 * *step as f32 * factor(*accelerate) / 100.0;
                self.app_volume(delta);
            }
            Action::Mute => output::tap_hook_key(HookKey::Mute, 1),
            Action::MicVolume { step, accelerate } => {
                let delta = sign as f32 * *step as f32 * factor(*accelerate) / 100.0;
                self.mic_volume(delta);
            }
            Action::MicMute => self.mic_mute(),
            Action::AudioOutput => self.switch_output(if sign < 0 { -1 } else { 1 }),
            Action::Open { target, args } => launch::open(target, args, |target| {
                ui::notify(UiEvent::Osd(trf(
                    Text::OsdOpenFailed,
                    &[("target", target)],
                )))
            }),
            Action::Scroll {
                lines,
                horizontal,
                accelerate,
            } => {
                let notches = (*lines as f32 * factor(*accelerate)).round().max(1.0) as i32;
                output::wheel(sign * notches, *horizontal);
            }
            Action::Zoom => output::zoom(sign),
            Action::Keys { keys, cw, ccw } => {
                let combo = match gesture {
                    Gesture::Rotate { dir: Dir::Cw, .. } => cw,
                    Gesture::Rotate { dir: Dir::Ccw, .. } => ccw,
                    _ => keys,
                };
                if let Some(combo) = combo {
                    output::combo(*combo);
                }
            }
            Action::MouseClick { button } => output::click(*button),
            Action::ToggleMouseMode => extras::send(ExtrasMsg::ToggleMouse),
            Action::ClickHints => extras::send(ExtrasMsg::ShowHints),
            Action::RunMacro { name } => extras::send(ExtrasMsg::RunMacro(name.clone())),
            // Handled inside the engine.
            Action::CycleMode | Action::Passthrough | Action::Nothing => {}
        }
    }

    fn mic_volume(&mut self, delta: f32) {
        let Some(audio) = &self.audio else { return };
        match audio.change_mic_volume(delta) {
            Ok(Some(level)) => {
                let percent = (level * 100.0).round().to_string();
                ui::notify(UiEvent::Osd(trf(
                    Text::OsdMicVolume,
                    &[("percent", &percent)],
                )))
            }
            Ok(None) => ui::notify(UiEvent::Osd(tr(Text::OsdNoMic).into())),
            Err(e) => self.sound_failed("microphone volume", &e),
        }
    }

    fn mic_mute(&mut self) {
        let Some(audio) = &self.audio else { return };
        match audio.toggle_mic_mute() {
            Ok(Some(muted)) => {
                log::info!("microphone {}", if muted { "muted" } else { "on" });
                let text = if muted {
                    Text::OsdMicMuted
                } else {
                    Text::OsdMicOn
                };
                ui::notify(UiEvent::Osd(tr(text).into()))
            }
            Ok(None) => ui::notify(UiEvent::Osd(tr(Text::OsdNoMic).into())),
            Err(e) => self.sound_failed("microphone mute", &e),
        }
    }

    fn switch_output(&mut self, step: i32) {
        let Some(audio) = &self.audio else { return };
        match audio.switch_output(step) {
            Ok(Some(device)) => {
                log::info!("sound output: {device}");
                ui::notify(UiEvent::Osd(trf(Text::OsdOutput, &[("device", &device)])))
            }
            Ok(None) => ui::notify(UiEvent::Osd(tr(Text::OsdOneOutput).into())),
            Err(e) => self.sound_failed("sound output", &e),
        }
    }

    fn sound_failed(&self, what: &str, error: &windows::core::Error) {
        log::warn!("{what}: {error}");
        ui::notify(UiEvent::Osd(tr(Text::OsdSoundFailed).into()));
    }

    fn app_volume(&mut self, delta: f32) {
        let exe = &self.foreground_exe;
        let app = exe
            .strip_suffix(".exe")
            .or_else(|| exe.strip_suffix(".EXE"))
            .unwrap_or(exe);
        let Some(audio) = &self.audio else { return };
        match audio.change_app_volume(exe, delta) {
            Ok(Some(level)) => {
                let percent = (level * 100.0).round().to_string();
                ui::notify(UiEvent::Osd(trf(
                    Text::OsdAppVolume,
                    &[("app", app), ("percent", &percent)],
                )))
            }
            Ok(None) => ui::notify(UiEvent::Osd(trf(Text::OsdNoSound, &[("app", app)]))),
            Err(e) => log::warn!("app volume for {exe}: {e}"),
        }
    }
}
