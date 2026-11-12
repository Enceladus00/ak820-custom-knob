//! The effects thread: keeps the keyboard's effect engine (LIVE v2 firmware)
//! in step with the situation — which program is in front and which knob
//! mode is active — using the `when` lists of the configured effects.
//!
//! Everything it sends is RAM-only on the keyboard: uploading, playing and
//! stopping effects never writes the keyboard's flash. It talks to the
//! keyboard only while some effect has a situation, so nothing changes for
//! anyone who doesn't use them, and it runs on its own thread because a
//! sleeping keyboard takes over a second to time out.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use knob_core::{Config, Effect, effect_for};
use knob_win::effects::{EffectsError, Keyboard};

/// How often a connected keyboard is checked for a restart (which empties
/// its RAM), and how long to wait before retrying an absent one.
const CHECK_EVERY: Duration = Duration::from_secs(20);
/// After a plug or unplug, give Windows a moment to set the device up.
const AFTER_PLUG: Duration = Duration::from_secs(2);

pub enum EffectsMsg {
    Config(Box<Config>),
    Foreground(String),
    Mode { id: String, name: String },
    KeyboardChanged,
    Paused(bool),
}

static SENDER: OnceLock<Sender<EffectsMsg>> = OnceLock::new();

pub fn send(msg: EffectsMsg) {
    if let Some(tx) = SENDER.get() {
        let _ = tx.send(msg);
    }
}

pub fn start(config: Config) {
    let (tx, rx) = mpsc::channel();
    let _ = SENDER.set(tx);
    thread::Builder::new()
        .name("effects".into())
        .spawn(move || {
            if catch_unwind(AssertUnwindSafe(|| Effects::new(config).run(rx))).is_err() {
                log::error!("effects thread crashed; effects no longer follow situations");
            }
        })
        .expect("spawn effects thread");
}

struct Effects {
    effects: Vec<Effect>,
    app: String,
    mode_id: String,
    mode_name: String,
    paused: bool,
    /// Effects are uploaded; holds the frame counter seen then, to notice
    /// a keyboard restart (the counter starts again from zero).
    uploaded: Option<u32>,
    /// The slot this thread started, if any.
    playing: Option<u8>,
    next_check: Instant,
    /// Logged once, so a stock keyboard doesn't fill the log.
    told_no_engine: bool,
}

impl Effects {
    fn new(config: Config) -> Self {
        Self {
            effects: config.effects,
            app: String::new(),
            mode_id: String::new(),
            mode_name: String::new(),
            paused: false,
            uploaded: None,
            playing: None,
            next_check: Instant::now(),
            told_no_engine: false,
        }
    }

    fn run(mut self, rx: Receiver<EffectsMsg>) {
        loop {
            let wait = self.next_check.saturating_duration_since(Instant::now());
            match rx.recv_timeout(wait) {
                Ok(msg) => self.handle(msg),
                Err(RecvTimeoutError::Timeout) => self.check(),
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.sync();
        }
    }

    fn handle(&mut self, msg: EffectsMsg) {
        match msg {
            EffectsMsg::Config(config) => {
                if config.effects != self.effects {
                    self.effects = config.effects;
                    self.uploaded = None; // definitions may have changed
                }
            }
            EffectsMsg::Foreground(exe) => self.app = exe,
            EffectsMsg::Mode { id, name } => {
                self.mode_id = id;
                self.mode_name = name;
            }
            EffectsMsg::KeyboardChanged => {
                self.uploaded = None;
                self.playing = None;
                self.next_check = Instant::now() + AFTER_PLUG;
            }
            EffectsMsg::Paused(paused) => self.paused = paused,
        }
    }

    fn in_use(&self) -> bool {
        self.effects.iter().any(|e| !e.when.is_empty())
    }

    /// The slot that should be playing now.
    fn wanted(&self) -> Option<u8> {
        if self.paused {
            return None;
        }
        effect_for(&self.effects, &self.app, &self.mode_id, &self.mode_name)
            .filter(|e| e.to_def().is_some())
            .map(|e| e.slot)
    }

    /// Periodic: notice a restarted keyboard (its RAM, and our effects, are
    /// gone) or retry one that wasn't there.
    fn check(&mut self) {
        self.next_check = Instant::now() + CHECK_EVERY;
        if !self.in_use() {
            return;
        }
        let Some(seen) = self.uploaded else { return };
        let result = Keyboard::open().and_then(|k| k.require_engine());
        match result {
            Ok(info) if info.frames >= seen => {}
            Ok(_) => {
                log::info!("effects: the keyboard restarted; uploading again");
                self.uploaded = None;
                self.playing = None;
            }
            Err(e) => {
                log::debug!("effects: keyboard check failed: {e}");
                self.uploaded = None;
                self.playing = None;
            }
        }
    }

    /// Brings the keyboard in line with what should be playing.
    fn sync(&mut self) {
        if !self.in_use() {
            return;
        }
        let wanted = self.wanted();
        if self.uploaded.is_none() {
            if wanted.is_none() && self.playing.is_none() {
                return; // nothing to show yet: upload when it's needed
            }
            if Instant::now() < self.next_check {
                return;
            }
            if let Err(e) = self.upload() {
                self.report(e);
                self.next_check = Instant::now() + CHECK_EVERY;
                return;
            }
        }
        if wanted == self.playing {
            return;
        }
        let result = Keyboard::open().and_then(|k| match wanted {
            Some(slot) => k.play(slot),
            None => k.stop(),
        });
        match result {
            Ok(()) => {
                log::info!(
                    "effects: {}",
                    match wanted {
                        Some(slot) => format!("playing slot {slot}"),
                        None => "stopped".into(),
                    }
                );
                self.playing = wanted;
            }
            Err(e) => {
                self.report(e);
                self.uploaded = None;
                self.next_check = Instant::now() + CHECK_EVERY;
            }
        }
    }

    fn upload(&mut self) -> Result<(), EffectsError> {
        let keyboard = Keyboard::open()?;
        let info = keyboard.require_engine()?;
        let mut count = 0;
        for effect in &self.effects {
            if let Some(def) = effect.to_def() {
                keyboard.define(&def)?;
                count += 1;
            }
        }
        log::info!("effects: uploaded {count} to the keyboard (RAM only)");
        self.uploaded = Some(info.frames);
        self.playing = None;
        self.told_no_engine = false;
        Ok(())
    }

    fn report(&mut self, e: EffectsError) {
        match e {
            EffectsError::NoEngine if self.told_no_engine => {}
            EffectsError::NoEngine => {
                log::info!("effects: the keyboard's firmware has no effect engine (needs LIVE v2)");
                self.told_no_engine = true;
            }
            EffectsError::NoKeyboard | EffectsError::NoReply => log::debug!("effects: {e}"),
            e => log::warn!("effects: {e}"),
        }
    }
}
