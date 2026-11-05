//! Injects keyboard and mouse input. Everything is marked with [`MARKER`],
//! so our own hook lets it through.

use knob_core::{HookKey, Key, KeyCombo, Modifiers, MouseButton};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSE_EVENT_FLAGS,
    MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
    MOUSEINPUT, SendInput, VIRTUAL_KEY, VkKeyScanExW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::MARKER;

const VK_CONTROL: u16 = 0x11;
const VK_SHIFT: u16 = 0x10;
const VK_MENU: u16 = 0x12;
const VK_LWIN: u16 = 0x5B;
const WHEEL_DELTA: i32 = 120;

/// Keys that need the "extended" flag to be recognised correctly.
fn is_extended(vk: u16) -> bool {
    matches!(vk, 0x21..=0x28 | 0x2D | 0x2E | 0x5B..=0x5D | 0x6F | 0x90 | 0xA6..=0xB7)
}

fn key_input(vk: u16, down: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if is_extended(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if !down {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MARKER,
            },
        },
    }
}

fn mouse_input(flags: MOUSE_EVENT_FLAGS, data: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: data as _,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MARKER,
            },
        },
    }
}

fn send(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs, size_of::<INPUT>() as i32);
    }
}

/// Sends one volume / mute key event (used to replay what the hook swallowed).
pub fn hook_key(key: HookKey, down: bool) {
    send(&[key_input(key.vk(), down)]);
}

/// Presses and releases a volume / mute key `times` times.
pub fn tap_hook_key(key: HookKey, times: u32) {
    let inputs: Vec<INPUT> = (0..times)
        .flat_map(|_| [key_input(key.vk(), true), key_input(key.vk(), false)])
        .collect();
    send(&inputs);
}

/// Presses a key combination: modifiers down, key down, key up, modifiers up.
///
/// A character key is looked up on the keyboard layout of the app in front,
/// adding Shift / AltGr when the character needs them. A character that no
/// key on that layout produces is typed as Unicode text instead (without the
/// modifiers, which can't apply to it).
pub fn combo(combo: KeyCombo) {
    match combo.key {
        Key::Vk(vk) => press(combo.mods, vk),
        Key::Char(c) => match key_for_char(c) {
            Some((vk, needed)) => {
                let mods = Modifiers {
                    ctrl: combo.mods.ctrl || needed.ctrl,
                    shift: combo.mods.shift || needed.shift,
                    alt: combo.mods.alt || needed.alt,
                    win: combo.mods.win,
                };
                press(mods, vk);
            }
            None => type_text(c),
        },
    }
}

fn press(mods: Modifiers, vk: u16) {
    let held: Vec<u16> = [
        (mods.ctrl, VK_CONTROL),
        (mods.shift, VK_SHIFT),
        (mods.alt, VK_MENU),
        (mods.win, VK_LWIN),
    ]
    .into_iter()
    .filter_map(|(on, vk)| on.then_some(vk))
    .collect();

    let mut inputs: Vec<INPUT> = held.iter().map(|&vk| key_input(vk, true)).collect();
    inputs.push(key_input(vk, true));
    inputs.push(key_input(vk, false));
    inputs.extend(held.iter().rev().map(|&vk| key_input(vk, false)));
    send(&inputs);
}

/// The key and the modifiers that type `c` on the foreground app's layout.
fn key_for_char(c: char) -> Option<(u16, Modifiers)> {
    let mut units = [0u16; 2];
    let [unit] = c.encode_utf16(&mut units) else {
        return None; // outside the Basic Multilingual Plane: no key types it
    };
    let result = unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        VkKeyScanExW(*unit, GetKeyboardLayout(thread))
    };
    if result == -1 {
        return None;
    }
    let vk = (result as u16) & 0xFF;
    let state = ((result as u16) >> 8) & 0xFF;
    // Bits 1/2/4 = Shift/Ctrl/Alt; 6 and 7 are layout-specific keys we can't press.
    if state & !0x07 != 0 {
        return None;
    }
    Some((
        vk,
        Modifiers {
            shift: state & 1 != 0,
            ctrl: state & 2 != 0,
            alt: state & 4 != 0,
            win: false,
        },
    ))
}

/// Types a character directly, independent of the keyboard layout.
fn type_text(c: char) {
    let mut units = [0u16; 2];
    let inputs: Vec<INPUT> = c
        .encode_utf16(&mut units)
        .iter()
        .flat_map(|&unit| [unicode_input(unit, true), unicode_input(unit, false)])
        .collect();
    send(&inputs);
}

fn unicode_input(unit: u16, down: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if !down {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MARKER,
            },
        },
    }
}

/// Scrolls `notches` wheel notches: positive = up (or right when horizontal).
pub fn wheel(notches: i32, horizontal: bool) {
    let flags = if horizontal {
        MOUSEEVENTF_HWHEEL
    } else {
        MOUSEEVENTF_WHEEL
    };
    send(&[mouse_input(flags, notches * WHEEL_DELTA)]);
}

/// Ctrl + wheel: positive = zoom in.
pub fn zoom(notches: i32) {
    send(&[
        key_input(VK_CONTROL, true),
        mouse_input(MOUSEEVENTF_WHEEL, notches * WHEEL_DELTA),
        key_input(VK_CONTROL, false),
    ]);
}

pub fn click(button: MouseButton) {
    let (down, up) = match button {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    };
    send(&[mouse_input(down, 0), mouse_input(up, 0)]);
}
