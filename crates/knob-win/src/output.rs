//! Injects keyboard and mouse input. Everything is marked with [`MARKER`],
//! so our own hook lets it through.

use knob_core::{HookKey, KeyCombo, MouseButton};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MOUSE_EVENT_FLAGS, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
    VIRTUAL_KEY,
};

use crate::MARKER;

const VK_CONTROL: u16 = 0x11;
const VK_SHIFT: u16 = 0x10;
const VK_MENU: u16 = 0x12;
const VK_LWIN: u16 = 0x5B;
const WHEEL_DELTA: i32 = 120;

/// Keys that need the "extended" flag to be recognised correctly.
fn is_extended(vk: u16) -> bool {
    matches!(vk, 0x21..=0x28 | 0x2D | 0x2E | 0x5B | 0x5C | 0x6F | 0xA6..=0xB7)
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
pub fn combo(combo: KeyCombo) {
    let mods: Vec<u16> = [
        (combo.mods.ctrl, VK_CONTROL),
        (combo.mods.shift, VK_SHIFT),
        (combo.mods.alt, VK_MENU),
        (combo.mods.win, VK_LWIN),
    ]
    .into_iter()
    .filter_map(|(held, vk)| held.then_some(vk))
    .collect();

    let mut inputs: Vec<INPUT> = mods.iter().map(|&vk| key_input(vk, true)).collect();
    inputs.push(key_input(combo.vk, true));
    inputs.push(key_input(combo.vk, false));
    inputs.extend(mods.iter().rev().map(|&vk| key_input(vk, false)));
    send(&inputs);
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
