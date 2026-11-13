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
//!
//! The keyboard holds 8 effects; the config may have any number. Slots 1 to
//! 7 are a cache ([`SlotCache`]): an effect that isn't in one is uploaded
//! into a free or the least recently played slot just before it plays.
//! Slot 8 belongs to the Settings window's Play button.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use knob_core::config::EFFECT_CACHE_SLOTS;
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
    /// Talking to an engine; holds the frame counter seen then, to notice
    /// a keyboard restart (the counter starts again from zero, and its RAM,
    /// with the uploaded effects, is empty).
    connected: Option<u32>,
    /// Which effect each cache slot holds.
    cache: SlotCache,
    /// What this thread started, if anything ([`effect_key`]), and when.
    playing: Option<EffectKey>,
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
            connected: None,
            cache: SlotCache::default(),
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
            let mut wait = match self.connected {
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
            if self.connected.is_some() && Instant::now() >= self.next_check {
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
                // An edited effect no longer matches its cached copy, so it
                // is uploaded again when it next plays.
                self.effects = config.effects;
            }
            EffectsMsg::Foreground(exe) => self.app = exe,
            EffectsMsg::Mode { id, name } => {
                self.mode_id = id;
                self.mode_name = name;
            }
            EffectsMsg::KeyboardChanged => {
                self.forget_keyboard();
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

    /// The effect that should be playing now, and the game event that
    /// started it, if one did.
    fn wanted(&self) -> Option<(Effect, Option<Instant>)> {
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
            .filter(|p| effect_key(p.effect).is_some())
            .map(|p| (p.effect.clone(), p.event_age.map(|age| now - age)))
    }

    /// Periodic, while connected: notice a restarted keyboard (its RAM, and
    /// our effects, are gone) so they are uploaded again.
    fn check(&mut self) {
        self.next_check = Instant::now() + CHECK_EVERY;
        if !self.in_use() {
            return;
        }
        let Some(seen) = self.connected else { return };
        let result = Keyboard::open().and_then(|k| k.require_engine());
        match result {
            Ok(info) if info.frames >= seen => {}
            Ok(_) => {
                log::info!("effects: the keyboard restarted; uploading again");
                self.forget_keyboard();
            }
            Err(e) => {
                log::debug!("effects: keyboard check failed: {e}");
                self.forget_keyboard();
            }
        }
    }

    /// The keyboard went away or restarted: nothing on it can be trusted.
    fn forget_keyboard(&mut self) {
        self.connected = None;
        self.cache = SlotCache::default();
        self.playing = None;
    }

    /// Brings the keyboard in line with what should be playing.
    fn sync(&mut self) {
        let in_use = self.in_use();
        // Switched off: stop what this thread started, then leave the keyboard alone.
        if !in_use && self.playing.is_none() {
            return;
        }
        let (wanted, trigger) = match self.wanted().filter(|_| in_use) {
            Some((effect, trigger)) => (Some(effect), trigger),
            None => (None, None),
        };
        if self.connected.is_none() {
            if wanted.is_none() && self.playing.is_none() {
                return; // nothing to show yet: connect when it's needed
            }
            if Instant::now() < self.retry_at {
                return;
            }
            if let Err(e) = self.connect() {
                self.report(e);
                self.retry_at = Instant::now() + RETRY_AFTER;
                return;
            }
        }
        // The same event again restarts its effect (a second hit flashes too).
        let again = trigger.is_some_and(|t| self.played_at.is_none_or(|p| p < t));
        let wanted_key = wanted.as_ref().and_then(effect_key);
        if wanted_key == self.playing && !again {
            return;
        }
        let result = Keyboard::open().and_then(|k| match &wanted {
            Some(effect) => self.play(&k, effect).map(Some),
            None => k.stop().map(|()| None),
        });
        match result {
            Ok(slot) => {
                match (&wanted, slot) {
                    (Some(effect), Some(slot)) => {
                        log::info!("effects: playing '{}' (slot {slot})", effect.name)
                    }
                    _ => log::info!("effects: stopped"),
                }
                self.playing = wanted_key;
                self.played_at = Some(Instant::now());
            }
            Err(e) => {
                self.report(e);
                self.forget_keyboard();
                self.retry_at = Instant::now() + RETRY_AFTER;
            }
        }
    }

    /// Plays `effect`, uploading it first unless a cache slot holds it.
    /// Returns the slot it plays from.
    fn play(&mut self, keyboard: &Keyboard, effect: &Effect) -> Result<u8, EffectsError> {
        let key = effect_key(effect).ok_or_else(|| invalid(effect))?;
        let slot = match self.cache.find(&key) {
            Some(slot) => slot,
            None => {
                // Never push out the effect showing now: switching away from
                // it is what this upload is for, and it may be needed again.
                let showing = self.playing.and_then(|k| self.cache.find(&k));
                let slot = self.cache.room(showing);
                let def = effect.to_def(slot).ok_or_else(|| invalid(effect))?;
                // Unknown until the keyboard confirms the new contents.
                self.cache.clear(slot);
                keyboard.define(&def)?;
                self.cache.put(slot, key);
                log::debug!("effects: uploaded '{}' to slot {slot} (RAM only)", effect.name);
                slot
            }
        };
        keyboard.play(slot)?;
        self.cache.touch(slot);
        Ok(slot)
    }

    /// Checks for the engine, then uploads ahead the first effects that may
    /// start by themselves, so they start with a single command.
    fn connect(&mut self) -> Result<(), EffectsError> {
        let keyboard = Keyboard::open()?;
        let info = keyboard.require_engine()?;
        self.cache = SlotCache::default();
        self.playing = None;
        let mut count = 0;
        for effect in self.effects.iter().filter(|e| e.enabled && !e.when.is_empty()) {
            let Some(key) = effect_key(effect) else { continue };
            if self.cache.find(&key).is_some() {
                continue; // the same look under another name
            }
            let Some(slot) = self.cache.free() else { break };
            let def = effect.to_def(slot).ok_or_else(|| invalid(effect))?;
            keyboard.define(&def)?;
            self.cache.put(slot, key);
            count += 1;
        }
        log::info!(
            "effects: connected; uploaded {count} of {} effects ahead (RAM only)",
            self.effects.len()
        );
        self.connected = Some(info.frames);
        self.next_check = Instant::now() + CHECK_EVERY;
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

/// What identifies an effect on the keyboard: its definition without the
/// slot. Two effects that look the same share a cache slot.
type EffectKey = [u8; 18];

fn effect_key(effect: &Effect) -> Option<EffectKey> {
    let payload = effect.to_def(*EFFECT_CACHE_SLOTS.start())?.payload();
    payload[1..].try_into().ok()
}

fn invalid(effect: &Effect) -> EffectsError {
    EffectsError::Failed(format!("effect '{}' can't be sent", effect.name))
}

const CACHE_LEN: usize = 7;
const _: () = assert!(
    (*EFFECT_CACHE_SLOTS.end() - *EFFECT_CACHE_SLOTS.start()) as usize + 1 == CACHE_LEN
);

/// The effects this thread uploaded into slots 1 to 7, and when each last
/// played.
#[derive(Default)]
struct SlotCache {
    slots: [Option<(EffectKey, Option<Instant>)>; CACHE_LEN],
}

impl SlotCache {
    fn index(slot: u8) -> usize {
        (slot - EFFECT_CACHE_SLOTS.start()) as usize
    }

    fn slot(index: usize) -> u8 {
        EFFECT_CACHE_SLOTS.start() + index as u8
    }

    /// The slot holding `key`.
    fn find(&self, key: &EffectKey) -> Option<u8> {
        self.slots
            .iter()
            .position(|s| s.as_ref().is_some_and(|(k, _)| k == key))
            .map(Self::slot)
    }

    fn free(&self) -> Option<u8> {
        self.slots.iter().position(Option::is_none).map(Self::slot)
    }

    /// Where a new effect goes: a free slot, else the one played longest
    /// ago (never played counts as oldest), but never `keep`.
    fn room(&self, keep: Option<u8>) -> u8 {
        if let Some(slot) = self.free() {
            return slot;
        }
        (0..CACHE_LEN)
            .filter(|&i| Some(Self::slot(i)) != keep)
            .min_by_key(|&i| self.slots[i].as_ref().and_then(|(_, used)| *used))
            .map(Self::slot)
            .unwrap_or(*EFFECT_CACHE_SLOTS.start())
    }

    fn put(&mut self, slot: u8, key: EffectKey) {
        self.slots[Self::index(slot)] = Some((key, None));
    }

    fn clear(&mut self, slot: u8) {
        self.slots[Self::index(slot)] = None;
    }

    fn touch(&mut self, slot: u8) {
        if let Some((_, used)) = &mut self.slots[Self::index(slot)] {
            *used = Some(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knob_core::config::Pattern;

    fn key(n: u8) -> EffectKey {
        [n; 18]
    }

    #[test]
    fn fills_free_slots_first() {
        let mut cache = SlotCache::default();
        for n in 0..7 {
            let slot = cache.room(None);
            assert_eq!(slot, n + 1);
            cache.put(slot, key(n));
        }
        assert_eq!(cache.free(), None);
        assert_eq!(cache.find(&key(3)), Some(4));
        assert_eq!(cache.find(&key(9)), None);
    }

    #[test]
    fn makes_room_in_the_least_recently_played_slot() {
        let mut cache = SlotCache::default();
        for n in 0..7 {
            cache.put(n + 1, key(n));
        }
        // Every slot but 3 played, oldest first; 3 never played.
        for slot in [1, 2, 4, 5, 6, 7] {
            cache.touch(slot);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(cache.room(None), 3);
        cache.touch(3);
        // Now slot 1 is the oldest, unless it is the one showing.
        assert_eq!(cache.room(None), 1);
        assert_eq!(cache.room(Some(1)), 2);
    }

    #[test]
    fn same_look_shares_a_slot_and_any_change_does_not() {
        let fire = Effect {
            name: "Fire".into(),
            legacy_slot: None,
            pattern: Pattern::Flicker,
            colors: vec!["#FF2000".into()],
            speed: 6,
            brightness: 100,
            duration_ms: 0,
            enabled: true,
            when: vec![],
        };
        let renamed = Effect {
            name: "Burning".into(),
            ..fire.clone()
        };
        let faster = Effect {
            speed: 7,
            ..fire.clone()
        };
        assert_eq!(effect_key(&fire), effect_key(&renamed));
        assert_ne!(effect_key(&fire), effect_key(&faster));
        let broken = Effect {
            colors: vec![],
            ..fire
        };
        assert_eq!(effect_key(&broken), None);
    }
}
