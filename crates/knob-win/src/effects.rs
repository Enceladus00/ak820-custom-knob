//! Talks to the effect engine of the patched keyboard firmware (LIVE v2, see
//! docs/custom-firmware.md and docs/effects-format.md).
//!
//! Every command here only touches the keyboard's RAM: nothing is stored in
//! its flash, so effects can be uploaded, played and stopped as often as
//! needed. Stock firmware ignores these commands; [`Keyboard::firmware`]
//! tells the two apart before anything else is sent.

use std::fmt;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};
use knob_rgb::{
    Command, EffectDef, FRAME_LEN, LIVE_STATUS_INDEX, LiveInfo, PID_DONGLE, PID_WIRED, USAGE,
    USAGE_PAGE, VID, WIRED_FRAME_LEN, WIRED_USAGE, WIRED_USAGE_PAGE, frames, parse_reply,
};

const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);

/// What runs on the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Firmware {
    /// AJAZZ's own firmware: no effects.
    Stock,
    /// The live-lighting patch.
    Live(LiveInfo),
}

impl Firmware {
    pub fn has_effects(&self) -> bool {
        matches!(self, Firmware::Live(info) if info.has_effects())
    }
}

/// Why an effect command failed. `Display` gives codes the Settings UI
/// translates, or the detail of an unexpected failure.
#[derive(Debug, PartialEq, Eq)]
pub enum EffectsError {
    /// Neither the wired keyboard nor the 2.4 GHz receiver is plugged in.
    NoKeyboard,
    /// The keyboard is asleep or off.
    NoReply,
    /// The keyboard runs firmware without the effect engine.
    NoEngine,
    /// The keyboard refused a command (status code from the firmware).
    Refused(u8),
    Failed(String),
}

impl fmt::Display for EffectsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EffectsError::NoKeyboard => f.write_str("no_keyboard"),
            EffectsError::NoReply => f.write_str("no_reply"),
            EffectsError::NoEngine => f.write_str("no_engine"),
            EffectsError::Refused(code) => write!(f, "refused ({code})"),
            EffectsError::Failed(detail) => f.write_str(detail),
        }
    }
}

fn failed(what: &str, detail: impl fmt::Display) -> EffectsError {
    EffectsError::Failed(format!("{what}: {detail}"))
}

/// An open link to the keyboard: over the cable when it is in cable mode,
/// otherwise through the 2.4 GHz receiver. Both carry the same frames; only
/// the report size differs.
pub struct Keyboard {
    device: HidDevice,
    wired: bool,
}

impl Keyboard {
    pub fn open() -> Result<Self, EffectsError> {
        let api = HidApi::new().map_err(|e| failed("HID init", e))?;
        let find = |pid, page, usage| {
            api.device_list().find(|d| {
                d.vendor_id() == VID
                    && d.product_id() == pid
                    && d.usage_page() == page
                    && d.usage() == usage
            })
        };
        let (info, wired) = match find(PID_WIRED, WIRED_USAGE_PAGE, WIRED_USAGE) {
            Some(info) => (info, true),
            None => (
                find(PID_DONGLE, USAGE_PAGE, USAGE).ok_or(EffectsError::NoKeyboard)?,
                false,
            ),
        };
        let device = info.open_device(&api).map_err(|e| failed("open", e))?;
        Ok(Self { device, wired })
    }

    pub fn firmware(&self) -> Result<Firmware, EffectsError> {
        let reply = self.exchange(Command::Identify, &frames(Command::Identify, &[]))?;
        Ok(LiveInfo::parse(&reply).map_or(Firmware::Stock, Firmware::Live))
    }

    /// Fails with [`EffectsError::NoEngine`] unless the effect engine is there.
    pub fn require_engine(&self) -> Result<LiveInfo, EffectsError> {
        match self.firmware()? {
            Firmware::Live(info) if info.has_effects() => Ok(info),
            _ => Err(EffectsError::NoEngine),
        }
    }

    pub fn define(&self, effect: &EffectDef) -> Result<(), EffectsError> {
        self.checked(Command::DefineEffect, &effect.define_request())
    }

    pub fn play(&self, slot: u8) -> Result<(), EffectsError> {
        self.checked(Command::PlayEffect, &knob_rgb::play_request(slot))
    }

    /// Stops any effect, then re-applies the saved lighting block, so the
    /// keyboard shows its normal lighting again. Both are RAM-only.
    pub fn stop(&self) -> Result<(), EffectsError> {
        self.checked(Command::StopEffect, &knob_rgb::stop_request())?;
        self.checked(Command::LiveRestore, &frames(Command::LiveRestore, &[]))
    }

    pub fn set_frame_period(&self, micros: u32) -> Result<(), EffectsError> {
        self.checked(
            Command::SetFramePeriod,
            &knob_rgb::frame_period_request(micros),
        )
    }

    fn checked(&self, command: Command, frames: &[[u8; FRAME_LEN]]) -> Result<(), EffectsError> {
        let reply = self.exchange(command, frames)?;
        match reply.get(LIVE_STATUS_INDEX) {
            Some(0) => Ok(()),
            Some(&code) => Err(EffectsError::Refused(code)),
            None => Err(failed("reply", "too short")),
        }
    }

    fn exchange(
        &self,
        command: Command,
        frames: &[[u8; FRAME_LEN]],
    ) -> Result<Vec<u8>, EffectsError> {
        let mut block = Vec::new();
        for frame in frames {
            let mut wire = vec![
                0u8;
                if self.wired {
                    WIRED_FRAME_LEN
                } else {
                    FRAME_LEN
                } + 1
            ];
            wire[1..=FRAME_LEN].copy_from_slice(frame);
            self.device.write(&wire).map_err(|e| failed("write", e))?;
            block = self.await_reply(command)?;
        }
        Ok(block)
    }

    fn await_reply(&self, command: Command) -> Result<Vec<u8>, EffectsError> {
        let deadline = Instant::now() + REPLY_TIMEOUT;
        let mut buf = [0u8; WIRED_FRAME_LEN + 1];
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(EffectsError::NoReply);
            }
            let n = self
                .device
                .read_timeout(&mut buf, left.as_millis() as i32)
                .map_err(|e| failed("read", e))?;
            if let Some(reply) = parse_reply(&buf[..n]).filter(|r| r.command == command.code()) {
                return Ok(reply.payload.to_vec());
            }
        }
    }
}

/// Uploads `effect` and plays it at once: what the Settings preview button
/// does. RAM-only.
pub fn play_now(effect: &EffectDef) -> Result<(), EffectsError> {
    let keyboard = Keyboard::open()?;
    keyboard.require_engine()?;
    keyboard.define(effect)?;
    keyboard.play(effect.slot())
}

/// Stops whatever effect is playing. RAM-only.
pub fn stop_now() -> Result<(), EffectsError> {
    let keyboard = Keyboard::open()?;
    keyboard.require_engine()?;
    keyboard.stop()
}

/// What the keyboard runs, for the Settings page.
pub fn firmware_now() -> Result<Firmware, EffectsError> {
    Keyboard::open()?.firmware()
}
