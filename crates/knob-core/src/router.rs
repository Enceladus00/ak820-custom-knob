//! Decides what happens to each key press once an optional feature is on.
//!
//! With no hotkeys set and neither mouse mode nor click hints active, every
//! key passes straight through.

use crate::keys::{Key, KeyCombo};
use crate::signal::Modifiers;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyId {
    Macro(usize),
    MouseToggle,
    Hints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub vk: u16,
    pub mods: Modifiers,
    pub id: HotkeyId,
}

impl Hotkey {
    /// `None` for a character key, whose virtual key depends on the keyboard
    /// layout and is looked up by the platform layer.
    pub fn from_combo(combo: KeyCombo, id: HotkeyId) -> Option<Self> {
        match combo.key {
            Key::Vk(vk) => Some(Self {
                vk,
                mods: combo.mods,
                id,
            }),
            Key::Char(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseControl {
    Left,
    Right,
    Up,
    Down,
    ClickLeft,
    ClickRight,
    ClickMiddle,
    ScrollUp,
    ScrollDown,
    Slow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterEvent {
    Hotkey(HotkeyId),
    Mouse { control: MouseControl, down: bool },
    MouseExit,
    HintChar(char),
    HintBack,
    HintCancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInfo {
    pub vk: u16,
    pub down: bool,
    pub mods: Modifiers,
    /// A modifier other than the two used as mouse buttons is held, so the
    /// arrow keys should keep their normal meaning (Ctrl+Left and so on).
    pub other_modifier_held: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Decision {
    pub swallow: bool,
    pub event: Option<RouterEvent>,
}

const PASS: Decision = Decision {
    swallow: false,
    event: None,
};
const VK_ESC: u16 = 0x1B;
const VK_BACKSPACE: u16 = 0x08;

fn is_modifier(vk: u16) -> bool {
    matches!(vk, 0x10..=0x12 | 0xA0..=0xA5 | 0x5B | 0x5C)
}

/// The arrow cluster, and the keys right next to it.
fn mouse_control(vk: u16) -> Option<MouseControl> {
    Some(match vk {
        0x25 => MouseControl::Left,
        0x27 => MouseControl::Right,
        0x26 => MouseControl::Up,
        0x28 => MouseControl::Down,
        0xA3 => MouseControl::ClickLeft,   // Right Ctrl
        0xA1 => MouseControl::ClickRight,  // Right Shift
        0x23 => MouseControl::ClickMiddle, // End
        0x21 => MouseControl::ScrollUp,    // Page Up
        0x22 => MouseControl::ScrollDown,  // Page Down
        0x24 => MouseControl::Slow,        // Home
        _ => return None,
    })
}

#[derive(Debug, Default)]
pub struct Router {
    hotkeys: Vec<Hotkey>,
    mouse_active: bool,
    hints_active: bool,
    /// Keys whose press was swallowed, so the release is swallowed too.
    swallowed: Vec<u16>,
}

impl Router {
    pub fn set_hotkeys(&mut self, hotkeys: Vec<Hotkey>) {
        self.hotkeys = hotkeys;
    }

    pub fn set_mouse_active(&mut self, active: bool) {
        self.mouse_active = active;
    }

    pub fn set_hints_active(&mut self, active: bool) {
        self.hints_active = active;
    }

    /// True when no key needs looking at.
    pub fn is_idle(&self) -> bool {
        self.hotkeys.is_empty()
            && !self.mouse_active
            && !self.hints_active
            && self.swallowed.is_empty()
    }

    pub fn mouse_active(&self) -> bool {
        self.mouse_active
    }

    pub fn hints_active(&self) -> bool {
        self.hints_active
    }

    pub fn handle(&mut self, key: KeyInfo) -> Decision {
        if !key.down {
            return self.release(key.vk);
        }
        if self.swallowed.contains(&key.vk) {
            return Decision {
                swallow: true,
                event: None, // auto-repeat of a key we already took
            };
        }
        let decision = self.press(key);
        if decision.swallow {
            self.swallowed.push(key.vk);
        }
        decision
    }

    fn release(&mut self, vk: u16) -> Decision {
        let Some(at) = self.swallowed.iter().position(|&k| k == vk) else {
            return PASS;
        };
        self.swallowed.swap_remove(at);
        Decision {
            swallow: true,
            event: mouse_control(vk).map(|control| RouterEvent::Mouse {
                control,
                down: false,
            }),
        }
    }

    fn press(&self, key: KeyInfo) -> Decision {
        if self.hints_active {
            return Self::hint_key(key.vk);
        }
        if let Some(hotkey) = self
            .hotkeys
            .iter()
            .find(|h| h.vk == key.vk && h.mods == key.mods)
        {
            return Decision {
                swallow: true,
                event: Some(RouterEvent::Hotkey(hotkey.id)),
            };
        }
        if !self.mouse_active || key.other_modifier_held {
            return PASS;
        }
        if key.vk == VK_ESC {
            return Decision {
                swallow: true,
                event: Some(RouterEvent::MouseExit),
            };
        }
        match mouse_control(key.vk) {
            Some(control) => Decision {
                swallow: true,
                event: Some(RouterEvent::Mouse {
                    control,
                    down: true,
                }),
            },
            None => PASS,
        }
    }

    /// While labels are on screen, typing picks a label and nothing reaches
    /// the app. Modifiers pass so that they can't get stuck.
    fn hint_key(vk: u16) -> Decision {
        let event = match vk {
            VK_ESC => Some(RouterEvent::HintCancel),
            VK_BACKSPACE => Some(RouterEvent::HintBack),
            0x41..=0x5A => Some(RouterEvent::HintChar(vk as u8 as char)),
            _ if is_modifier(vk) => return PASS,
            _ => None,
        };
        Decision {
            swallow: true,
            event,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(vk: u16) -> KeyInfo {
        KeyInfo {
            vk,
            down: true,
            mods: Modifiers::default(),
            other_modifier_held: false,
        }
    }

    fn release(vk: u16) -> KeyInfo {
        KeyInfo {
            down: false,
            ..press(vk)
        }
    }

    fn ctrl_alt(vk: u16) -> KeyInfo {
        KeyInfo {
            mods: Modifiers {
                ctrl: true,
                alt: true,
                ..Default::default()
            },
            other_modifier_held: true,
            ..press(vk)
        }
    }

    fn with_hotkey() -> Router {
        let mut router = Router::default();
        let combo: KeyCombo = "Ctrl+Alt+M".parse().unwrap();
        router.set_hotkeys(vec![
            Hotkey::from_combo(combo, HotkeyId::MouseToggle).unwrap(),
        ]);
        router
    }

    #[test]
    fn with_nothing_on_every_key_passes() {
        let mut router = Router::default();
        for vk in [0x41, 0x25, 0xA3, 0x1B, 0x70] {
            assert_eq!(router.handle(press(vk)), PASS);
            assert_eq!(router.handle(release(vk)), PASS);
        }
    }

    #[test]
    fn a_hotkey_fires_once_and_swallows_press_repeat_and_release() {
        let mut router = with_hotkey();
        let first = router.handle(ctrl_alt(0x4D));
        assert!(first.swallow);
        assert_eq!(
            first.event,
            Some(RouterEvent::Hotkey(HotkeyId::MouseToggle))
        );
        assert_eq!(
            router.handle(ctrl_alt(0x4D)),
            Decision {
                swallow: true,
                event: None
            }
        );
        let up = router.handle(release(0x4D));
        assert!(up.swallow);
        assert_eq!(up.event, None);
    }

    #[test]
    fn the_same_key_without_the_modifiers_is_left_alone() {
        let mut router = with_hotkey();
        assert_eq!(router.handle(press(0x4D)), PASS);
        assert_eq!(router.handle(release(0x4D)), PASS);
    }

    #[test]
    fn arrows_are_normal_until_mouse_mode_is_on() {
        let mut router = Router::default();
        assert_eq!(router.handle(press(0x25)), PASS);
        router.handle(release(0x25));

        router.set_mouse_active(true);
        let down = router.handle(press(0x25));
        assert!(down.swallow);
        assert_eq!(
            down.event,
            Some(RouterEvent::Mouse {
                control: MouseControl::Left,
                down: true
            })
        );
        assert_eq!(
            router.handle(release(0x25)).event,
            Some(RouterEvent::Mouse {
                control: MouseControl::Left,
                down: false
            })
        );
    }

    #[test]
    fn typing_still_works_in_mouse_mode() {
        let mut router = Router::default();
        router.set_mouse_active(true);
        assert_eq!(router.handle(press(0x41)), PASS);
    }

    #[test]
    fn right_ctrl_clicks_and_holding_it_allows_dragging() {
        let mut router = Router::default();
        router.set_mouse_active(true);
        let held = KeyInfo {
            mods: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            ..press(0x28)
        };
        router.handle(press(0xA3));
        assert_eq!(
            router.handle(held).event,
            Some(RouterEvent::Mouse {
                control: MouseControl::Down,
                down: true
            })
        );
    }

    #[test]
    fn ctrl_arrow_keeps_its_normal_meaning() {
        let mut router = Router::default();
        router.set_mouse_active(true);
        assert_eq!(router.handle(ctrl_alt(0x25)), PASS);
    }

    #[test]
    fn escape_leaves_mouse_mode() {
        let mut router = Router::default();
        router.set_mouse_active(true);
        assert_eq!(
            router.handle(press(VK_ESC)).event,
            Some(RouterEvent::MouseExit)
        );
    }

    #[test]
    fn a_key_held_when_the_mode_ends_is_still_released_cleanly() {
        let mut router = Router::default();
        router.set_mouse_active(true);
        router.handle(press(0x25));
        router.set_mouse_active(false);
        assert!(router.handle(release(0x25)).swallow);
        assert_eq!(router.handle(press(0x25)), PASS);
    }

    #[test]
    fn hints_take_letters_and_block_other_keys() {
        let mut router = Router::default();
        router.set_hints_active(true);
        assert_eq!(
            router.handle(press(0x41)).event,
            Some(RouterEvent::HintChar('A'))
        );
        assert_eq!(
            router.handle(press(VK_BACKSPACE)).event,
            Some(RouterEvent::HintBack)
        );
        assert_eq!(
            router.handle(press(VK_ESC)).event,
            Some(RouterEvent::HintCancel)
        );
        let digit = router.handle(press(0x31));
        assert!(digit.swallow);
        assert_eq!(digit.event, None);
        assert_eq!(router.handle(press(0xA2)), PASS);
    }

    #[test]
    fn character_hotkeys_need_the_layout_lookup() {
        let combo: KeyCombo = "Ctrl+.".parse().unwrap();
        assert_eq!(Hotkey::from_combo(combo, HotkeyId::Hints), None);
    }
}
