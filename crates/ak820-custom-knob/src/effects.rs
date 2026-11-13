//! The effects thread: keeps the keyboard's effect engine (LIVE v2 firmware)
//! in step with the situation — which program is in front, which knob mode
//! is active and what a game plugin reports (see `game.rs`) — using the
//! `when` lists of the configured effects.
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

use knob_core::game::{GameLine, GameState};
use knob_core::{Config, Effect, Now, effect_for};
use knob_win::effects::{EffectsError, Keyboard};

/// How often a connected keyboard is checked for a restart (which empties
/// its RAM).
const CHECK_EVERY: Duration = Duration::from_secs(20);
/// How long to wait before trying an absent or sleeping keyboard again.
const RETRY_AFTER: Duration = Duration::from_secs(5);
/// After a plug or unplug, give Windows a moment to set the device up.
const AFTER_PLUG: Duration = Duration::from_secs(2);
/// A game that hasn't reported for this long has quit (it reports at least
/// every 0.5 s).
const GAME_GONE: Duration = Duration::from_secs(3);
/// While a game reports, how often to look again (event effects run out).
const GAME_TICK: Duration = Duration::from_millis(50);
/// Events older than this can't matter (the longest effect is 60 s).
const EVENT_MEMORY: Duration = Duration::from_secs(61);

pub enum EffectsMsg {
    Config(Box<Config>),
    Foreground(String),
    Mode { id: String, name: String },
    KeyboardChanged,
    Paused(bool),
    Game(GameLine),
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
    /// The master switch: effects follow situations at all.
    automatic: bool,
    app: String,
    mode_id: String,
    mode_name: String,
    paused: bool,
    /// Effects are uploaded; holds the frame counter seen then, to notice
    /// a keyboard restart (the counter starts again from zero).
    uploaded: Option<u32>,
    /// The slot this thread started, if any, and when.
    playing: Option<u8>,
    played_at: Option<Instant>,
    /// The game's last reported state, and when it came.
    game: Option<(GameState, Instant)>,
    /// The latest time each game event came in.
    events: Vec<(String, Instant)>,
    /// When to next check an uploaded keyboard for a restart.
    next_check: Instant,
    /// The earliest time to try uploading again after a failure.
    retry_at: Instant,
    /// Logged once, so a stock keyboard doesn't fill the log.
    told_no_engine: bool,
}

impl Effects {
    fn new(config: Config) -> Self {
        Self {
            effects: config.effects,
            automatic: config.effects_automatic,
            app: String::new(),
            mode_id: String::new(),
            mode_name: String::new(),
            paused: false,
            uploaded: None,
            playing: None,
            played_at: None,
            game: None,
            events: Vec::new(),
            next_check: Instant::now(),
            retry_at: Instant::now(),
            told_no_engine: false,
        }
    }

    fn run(mut self, rx: Receiver<EffectsMsg>) {
        loop {
            let now = Instant::now();
            // Uploaded: wake for the restart check. Not yet: wake to retry
            // once a failed upload may be tried again. Otherwise nothing
            // was wanted, and a message will wake the thread.
            let mut wait = match self.uploaded {
                Some(_) => self.next_check.saturating_duration_since(now),
                None if self.retry_at > now => self.retry_at - now,
                None => CHECK_EVERY,
            };
            if self.game.is_some() || !self.events.is_empty() {
                wait = wait.min(GAME_TICK);
            }
            match rx.recv_timeout(wait) {
                Ok(msg) => self.handle(msg),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            if self.uploaded.is_some() && Instant::now() >= self.next_check {
                self.check();
            }
            self.forget_old();
            self.sync();
        }
    }

    fn handle(&mut self, msg: EffectsMsg) {
        match msg {
            EffectsMsg::Config(config) => {
                self.automatic = config.effects_automatic;
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
                self.retry_at = Instant::now() + AFTER_PLUG;
            }
            EffectsMsg::Paused(paused) => self.paused = paused,
            EffectsMsg::Game(GameLine::State(state)) => self.game = Some((state, Instant::now())),
            EffectsMsg::Game(GameLine::Event { name, .. }) => {
                let now = Instant::now();
                match self.events.iter_mut().find(|(n, _)| *n == name) {
                    Some(seen) => seen.1 = now,
                    None => self.events.push((name, now)),
                }
            }
        }
    }

    /// Drops a game that stopped reporting and events too old to matter.
    fn forget_old(&mut self) {
        if self
            .game
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > GAME_GONE)
        {
            log::info!("effects: the game stopped reporting");
            self.game = None;
        }
        self.events.retain(|(_, at)| at.elapsed() < EVENT_MEMORY);
    }

    /// Some effect may start by itself.
    fn in_use(&self) -> bool {
        self.automatic && self.effects.iter().any(|e| e.enabled && !e.when.is_empty())
    }

    /// The slot that should be playing now, and the game event that started
    /// it, if one did.
    fn wanted(&self) -> Option<(u8, Option<Instant>)> {
        if self.paused {
            return None;
        }
        let now = Instant::now();
        let events: Vec<_> = self
            .events
            .iter()
            .map(|(n, at)| (n.clone(), now.duration_since(*at)))
            .collect();
        let situation = Now {
            app: &self.app,
            mode_id: &self.mode_id,
            mode_name: &self.mode_name,
            game: self.game.as_ref().map(|(g, _)| g),
            events: &events,
        };
        effect_for(&self.effects, &situation)
            .filter(|p| p.effect.to_def().is_some())
            .map(|p| (p.effect.slot, p.event_age.map(|age| now - age)))
    }

    /// Periodic, while effects are uploaded: notice a restarted keyboard
    /// (its RAM, and our effects, are gone) so they are uploaded again.
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
        let in_use = self.in_use();
        // Switched off: stop what this thread started, then leave the keyboard alone.
        if !in_use && self.playing.is_none() {
            return;
        }
        let (wanted, trigger) = match self.wanted().filter(|_| in_use) {
            Some((slot, trigger)) => (Some(slot), trigger),
            None => (None, None),
        };
        if self.uploaded.is_none() {
            if wanted.is_none() && self.playing.is_none() {
                return; // nothing to show yet: upload when it's needed
            }
            if Instant::now() < self.retry_at {
                return;
            }
            if let Err(e) = self.upload() {
                self.report(e);
                self.retry_at = Instant::now() + RETRY_AFTER;
                return;
            }
        }
        // The same event again restarts its effect (a second hit flashes too).
        let again = trigger.is_some_and(|t| self.played_at.is_none_or(|p| p < t));
        if wanted == self.playing && !again {
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
                self.played_at = Some(Instant::now());
            }
            Err(e) => {
                self.report(e);
                self.uploaded = None;
                self.retry_at = Instant::now() + RETRY_AFTER;
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
        self.next_check = Instant::now() + CHECK_EVERY;
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
