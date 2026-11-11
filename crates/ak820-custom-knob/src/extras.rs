//! Runs the optional features: macros, mouse mode and click hints. They have
//! their own thread so that a long macro or a slow app never delays the knob.

use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use knob_core::extras::{MacroDef, MacroStep};
use knob_core::pointer::{PointerMotion, hint_labels};
use knob_core::remap::Remap;
use knob_core::router::{Hotkey, HotkeyId, MouseControl, RouterEvent};
use knob_core::{Config, Key, KeyCombo, MouseButton};
use knob_win::hints::{HintFinder, Target};
use knob_win::{input, output};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use crate::i18n::{Text, tr};
use crate::ui::{self, UiEvent};

pub enum ExtrasMsg {
    Config(Box<Config>),
    Router(RouterEvent),
    RunMacro(String),
    ToggleMouse,
    ShowHints,
    /// The Pause menu item was toggled.
    Paused(bool),
    /// Another window came to the front.
    Foreground,
}

static SENDER: OnceLock<Sender<ExtrasMsg>> = OnceLock::new();

pub fn send(msg: ExtrasMsg) {
    if let Some(tx) = SENDER.get() {
        let _ = tx.send(msg);
    }
}

pub fn start(config: Config) {
    let (tx, rx) = mpsc::channel();
    let _ = SENDER.set(tx);
    thread::Builder::new()
        .name("extras".into())
        .spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| Extras::new(config).run(rx)));
            if result.is_err() {
                log::error!("extras crashed; arrow keys and hotkeys are back to normal");
            }
            release_keys();
        })
        .expect("spawn extras thread");
}

/// Whatever happens to this thread, no key stays taken over.
fn release_keys() {
    input::set_mouse_active(false);
    input::set_hints_active(false);
    input::set_macro_running(false);
    input::set_hotkeys(Vec::new());
    input::set_remaps(Vec::new());
    input::release_remaps();
    MOUSE_ON.store(false, Ordering::SeqCst);
    ui::notify(UiEvent::HintsHide);
}

const TICK: Duration = Duration::from_millis(8);
/// Forgetting mouse mode is on would leave the arrow keys taken over.
const MOUSE_IDLE_OFF: Duration = Duration::from_secs(10 * 60);
const HINTS_TIMEOUT: Duration = Duration::from_secs(15);
const STEP_GAP: Duration = Duration::from_millis(15);
const MODIFIER_RELEASE_WAIT: Duration = Duration::from_millis(1500);

static MACRO_RUNNING: AtomicBool = AtomicBool::new(false);
static MOUSE_ON: AtomicBool = AtomicBool::new(false);

/// For the tray tooltip.
pub fn mouse_on() -> bool {
    MOUSE_ON.load(Ordering::SeqCst)
}

static MACRO_CANCEL: AtomicBool = AtomicBool::new(false);

struct HintSession {
    labels: Vec<String>,
    targets: Vec<Target>,
    typed: String,
    deadline: Instant,
}

struct Extras {
    config: Config,
    finder: Option<HintFinder>,
    mouse_on: bool,
    motion: PointerMotion,
    buttons_held: HashSet<MouseButtonKey>,
    last_tick: Instant,
    hints: Option<HintSession>,
    mouse_last_use: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum MouseButtonKey {
    Left,
    Right,
    Middle,
}

impl MouseButtonKey {
    fn button(self) -> MouseButton {
        match self {
            Self::Left => MouseButton::Left,
            Self::Right => MouseButton::Right,
            Self::Middle => MouseButton::Middle,
        }
    }
}

impl Extras {
    fn new(config: Config) -> Self {
        let finder = HintFinder::new()
            .inspect_err(|e| log::warn!("click hints unavailable: {e}"))
            .ok();
        let extras = Self {
            config,
            finder,
            mouse_on: false,
            motion: PointerMotion::default(),
            buttons_held: HashSet::new(),
            last_tick: Instant::now(),
            hints: None,
            mouse_last_use: Instant::now(),
        };
        extras.install_hotkeys();
        extras
    }

    fn run(mut self, rx: Receiver<ExtrasMsg>) {
        loop {
            let msg = match self.wait_time() {
                Some(wait) => match rx.recv_timeout(wait) {
                    Ok(msg) => Some(msg),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                },
                None => match rx.recv() {
                    Ok(msg) => Some(msg),
                    Err(_) => return,
                },
            };
            if let Some(msg) = msg {
                self.handle(msg);
            }
            self.tick();
        }
    }

    /// How long to sleep. `None` means nothing is going on.
    fn wait_time(&self) -> Option<Duration> {
        if self.motion.is_active() {
            return Some(TICK);
        }
        let now = Instant::now();
        let hints = self
            .hints
            .as_ref()
            .map(|h| h.deadline.saturating_duration_since(now));
        let idle = self
            .mouse_on
            .then(|| (self.mouse_last_use + MOUSE_IDLE_OFF).saturating_duration_since(now));
        match (hints, idle) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn tick(&mut self) {
        if self.mouse_on && self.mouse_last_use.elapsed() >= MOUSE_IDLE_OFF {
            self.set_mouse(false);
        }
        if self
            .hints
            .as_ref()
            .is_some_and(|h| Instant::now() >= h.deadline)
        {
            self.end_hints();
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32().min(0.05);
        self.last_tick = now;
        if self.motion.is_active() {
            let step = self.motion.tick(dt, self.config.mouse.speed as f32);
            if step.dx != 0 || step.dy != 0 {
                output::move_pointer(step.dx, step.dy);
            }
            if step.scroll != 0 {
                output::wheel(step.scroll, false);
            }
        }
    }

    fn handle(&mut self, msg: ExtrasMsg) {
        match msg {
            ExtrasMsg::Config(config) => {
                self.config = *config;
                if self.mouse_on && !self.config.mouse.enabled {
                    self.set_mouse(false);
                }
                self.install_hotkeys();
            }
            ExtrasMsg::RunMacro(name) => match self.config.macros.iter().find(|m| m.name == name) {
                Some(item) => start_macro(item.clone()),
                None => log::warn!("macro '{name}' not found"),
            },
            ExtrasMsg::ToggleMouse => self.set_mouse(!self.mouse_on),
            ExtrasMsg::Paused(paused) => {
                if paused {
                    self.set_mouse(false);
                    self.end_hints();
                    MACRO_CANCEL.store(true, Ordering::SeqCst);
                }
            }
            ExtrasMsg::Foreground => self.end_hints(),
            ExtrasMsg::ShowHints => self.show_hints(),
            ExtrasMsg::Router(event) => self.router_event(event),
        }
    }

    fn router_event(&mut self, event: RouterEvent) {
        match event {
            RouterEvent::Hotkey(id) => {
                output::mask_menu();
                match id {
                    HotkeyId::MouseToggle => self.set_mouse(!self.mouse_on),
                    HotkeyId::Hints => self.show_hints(),
                    HotkeyId::Macro(index) => {
                        if let Some(item) = self.config.macros.get(index) {
                            start_macro(item.clone());
                        }
                    }
                }
            }
            RouterEvent::Mouse { control, down } => self.mouse_control(control, down),
            RouterEvent::MouseExit => self.set_mouse(false),
            RouterEvent::HintChar {
                letter,
                right_click,
            } => self.hint_char(letter, right_click),
            RouterEvent::MacroStop => MACRO_CANCEL.store(true, Ordering::SeqCst),
            RouterEvent::HintBack => {
                if let Some(session) = &mut self.hints {
                    session.typed.pop();
                    ui::notify(UiEvent::HintsTyped(session.typed.clone()));
                }
            }
            RouterEvent::HintCancel => self.end_hints(),
        }
    }

    /// Hands the hotkeys and key remaps to the keyboard hook.
    fn install_hotkeys(&self) {
        let config = &self.config;
        let remaps: Vec<Remap> = config
            .remaps
            .iter()
            .filter_map(|def| {
                let vk = match def.from.key {
                    Key::Vk(vk) => vk,
                    // Looked up once, on the layout in use now.
                    Key::Char(c) => match output::plain_vk(c) {
                        Some(vk) => vk,
                        None => {
                            log::warn!(
                                "remap {}: no key types it without Shift or AltGr; skipped",
                                def.from
                            );
                            return None;
                        }
                    },
                };
                Some(Remap::new(vk, def))
            })
            .collect();
        if !remaps.is_empty() {
            log::info!("remap: {} rule(s)", remaps.len());
        }
        input::set_remaps(remaps);
        let mut wanted: Vec<(KeyCombo, HotkeyId)> = config
            .macros
            .iter()
            .enumerate()
            .filter_map(|(i, m)| m.hotkey.map(|k| (k, HotkeyId::Macro(i))))
            .collect();
        if config.mouse.enabled
            && let Some(combo) = config.mouse.toggle
        {
            wanted.push((combo, HotkeyId::MouseToggle));
        }
        if config.hints.enabled
            && let Some(combo) = config.hints.hotkey
        {
            wanted.push((combo, HotkeyId::Hints));
        }
        let hotkeys = wanted
            .into_iter()
            .filter_map(|(combo, id)| {
                let resolved = output::with_layout_vk(combo)?;
                Hotkey::from_combo(resolved, id)
            })
            .collect();
        input::set_hotkeys(hotkeys);
    }

    fn set_mouse(&mut self, on: bool) {
        if on == self.mouse_on {
            return;
        }
        self.mouse_on = on;
        input::set_mouse_active(on);
        MOUSE_ON.store(on, Ordering::SeqCst);
        self.mouse_last_use = Instant::now();
        ui::notify(UiEvent::KeyboardChanged); // refreshes the tray tooltip
        if !on {
            self.motion.clear();
            for key in self.buttons_held.drain() {
                output::button(key.button(), false);
            }
        }
        self.last_tick = Instant::now();
        ui::notify(UiEvent::Osd(
            tr(if on {
                Text::OsdMouseOn
            } else {
                Text::OsdMouseOff
            })
            .into(),
        ));
    }

    fn mouse_control(&mut self, control: MouseControl, down: bool) {
        self.mouse_last_use = Instant::now();
        if self.motion.set(control, down) {
            if !self.mouse_on {
                self.motion.clear();
            }
            return;
        }
        let key = match control {
            MouseControl::ClickLeft => MouseButtonKey::Left,
            MouseControl::ClickRight => MouseButtonKey::Right,
            _ => MouseButtonKey::Middle,
        };
        if down && self.mouse_on {
            if self.buttons_held.insert(key) {
                output::button(key.button(), true);
            }
        } else if !down && self.buttons_held.remove(&key) {
            output::button(key.button(), false);
        }
    }

    fn show_hints(&mut self) {
        if self.hints.is_some() {
            return;
        }
        let Some(finder) = &self.finder else { return };
        let window = unsafe { GetForegroundWindow() };
        let targets = finder
            .find(window)
            .inspect_err(|e| log::warn!("hints: {e}"))
            .unwrap_or_default();
        if targets.is_empty() {
            ui::notify(UiEvent::Osd(tr(Text::OsdNoHints).into()));
            return;
        }
        let labels = hint_labels(targets.len());
        input::set_hints_active(true);
        ui::notify(UiEvent::HintsShow(
            labels
                .iter()
                .cloned()
                .zip(targets.iter().copied())
                .collect(),
            self.config.hints.style.clone(),
        ));
        self.hints = Some(HintSession {
            labels,
            targets,
            typed: String::new(),
            deadline: Instant::now() + HINTS_TIMEOUT,
        });
    }

    fn hint_char(&mut self, c: char, right_click: bool) {
        let Some(session) = &mut self.hints else {
            return;
        };
        let mut typed = session.typed.clone();
        typed.push(c);
        if !session.labels.iter().any(|l| l.starts_with(&typed)) {
            return; // no label starts this way: ignore the typo
        }
        if let Some(index) = session.labels.iter().position(|l| *l == typed) {
            let (x, y) = session.targets[index].center();
            self.end_hints();
            if right_click {
                output::right_click_at(x, y);
            } else {
                output::click_at(x, y);
            }
            return;
        }
        session.typed = typed.clone();
        ui::notify(UiEvent::HintsTyped(typed));
    }

    fn end_hints(&mut self) {
        if self.hints.take().is_some() {
            input::set_hints_active(false);
            ui::notify(UiEvent::HintsHide);
        }
    }
}

/// Runs a macro on its own thread. Starting a macro while one is running
/// stops the running one instead, so a long or repeating macro can be ended.
fn start_macro(item: MacroDef) {
    if MACRO_RUNNING.swap(true, Ordering::SeqCst) {
        MACRO_CANCEL.store(true, Ordering::SeqCst);
        return;
    }
    MACRO_CANCEL.store(false, Ordering::SeqCst);
    input::set_macro_running(true);
    let spawned = thread::Builder::new().name("macro".into()).spawn(move || {
        // The hotkey's own Ctrl or Alt must not combine with what the macro presses.
        output::wait_modifiers_released(MODIFIER_RELEASE_WAIT);
        if catch_unwind(AssertUnwindSafe(|| run_macro(&item))).is_err() {
            log::error!("macro '{}' crashed", item.name);
        }
        finish_macro();
    });
    if let Err(e) = spawned {
        log::warn!("macro: {e}");
        finish_macro();
    }
}

fn finish_macro() {
    input::set_macro_running(false);
    MACRO_RUNNING.store(false, Ordering::SeqCst);
}

fn cancelled() -> bool {
    MACRO_CANCEL.load(Ordering::SeqCst)
}

fn run_macro(item: &MacroDef) {
    log::info!("macro: {}", item.name);
    let rounds = if item.until_stopped {
        u32::MAX
    } else {
        item.repeat
    };
    for _ in 0..rounds {
        for step in &item.steps {
            if cancelled() {
                return;
            }
            match step {
                MacroStep::Keys { keys } => output::combo(*keys),
                MacroStep::Text { text } => output::text(text),
                MacroStep::Click { button } => output::click(*button),
                MacroStep::Wait { ms } => sleep_cancellable(*ms),
            }
            thread::sleep(STEP_GAP);
        }
    }
}

fn sleep_cancellable(ms: u32) {
    let end = Instant::now() + Duration::from_millis(ms.into());
    while Instant::now() < end && !cancelled() {
        thread::sleep(Duration::from_millis(10).min(end - Instant::now()));
    }
}
