//! Inputs to the engine. Times are microseconds on any monotonic clock.

pub type Micros = u64;
pub const MS: Micros = 1_000;

/// The three keys the knob produces, as seen by the keyboard hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookKey {
    VolumeUp = 0,
    VolumeDown = 1,
    Mute = 2,
}

impl HookKey {
    pub const ALL: [HookKey; 3] = [Self::VolumeUp, Self::VolumeDown, Self::Mute];

    /// From a Windows virtual-key code.
    pub fn from_vk(vk: u32) -> Option<Self> {
        match vk {
            0xAF => Some(Self::VolumeUp),
            0xAE => Some(Self::VolumeDown),
            0xAD => Some(Self::Mute),
            _ => None,
        }
    }

    /// Windows virtual-key code.
    pub fn vk(self) -> u16 {
        match self {
            Self::VolumeUp => 0xAF,
            Self::VolumeDown => 0xAE,
            Self::Mute => 0xAD,
        }
    }

    /// HID Consumer page usage the keyboard sends for this key.
    pub fn consumer_usage(self) -> u16 {
        match self {
            Self::VolumeUp => 0xE9,
            Self::VolumeDown => 0xEA,
            Self::Mute => 0xE2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}

impl Modifiers {
    pub fn is_empty(&self) -> bool {
        !(self.ctrl || self.shift || self.alt || self.win)
    }
}

/// Knob rotation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    /// Clockwise.
    Cw,
    /// Counter-clockwise.
    Ccw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// A consumer-control report from the AK820 (via Raw Input).
    /// `usage` is the pressed usage, 0 when everything is released.
    DeviceReport { t: Micros, usage: u16 },
    /// A volume/mute key event from the hook. The hook has already
    /// swallowed it; the engine decides whether to replay it.
    Key {
        t: Micros,
        key: HookKey,
        down: bool,
        mods: Modifiers,
    },
}

impl Input {
    pub fn time(&self) -> Micros {
        match *self {
            Input::DeviceReport { t, .. } | Input::Key { t, .. } => t,
        }
    }
}
