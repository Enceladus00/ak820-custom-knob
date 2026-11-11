//! Injects keyboard and mouse input. Everything is marked with [`MARKER`],
//! so our own hook lets it through.

use knob_core::router::RemapSend;
use knob_core::{HookKey, Key, KeyCombo, Modifiers, MouseButton};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE,
    KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    MAPVK_VK_TO_VSC, MOUSE_EVENT_FLAGS, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, MapVirtualKeyW, SendInput, VIRTUAL_KEY,
    VkKeyScanExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetWindowThreadProcessId, SetCursorPos,
};

use crate::MARKER;
use std::time::{Duration, Instant};

const VK_CONTROL: u16 = 0x11;
const VK_SHIFT: u16 = 0x10;
const VK_MENU: u16 = 0x12;
const VK_LWIN: u16 = 0x5B;
const VK_RWIN: u16 = 0x5C;
/// A key code no keyboard sends. Tapping it stops Windows from treating a
/// released Alt or Win as "open the menu".
const VK_UNASSIGNED: u16 = 0xE8;
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

/// Sends what a remapped key stands for.
pub fn remap(send: RemapSend) {
    match send {
        RemapSend::Down(vk) => remap_key(vk, true),
        RemapSend::Up(vk) => remap_key(vk, false),
        RemapSend::Tap(keys) => combo(keys),
    }
}

/// Presses or releases a key in place of a remapped one. It carries the key's
/// scan code too, because games that read scan codes ignore a key without one.
pub fn remap_key(vk: u16, down: bool) {
    let mut input = key_input(vk, down);
    let scan = unsafe { MapVirtualKeyW(vk.into(), MAPVK_VK_TO_VSC) };
    // Right Ctrl and Right Alt share their scan codes with the left keys.
    let right_modifier = matches!(vk, 0xA3 | 0xA5);
    unsafe {
        let ki = &mut input.Anonymous.ki;
        ki.wScan = scan as u16;
        if right_modifier {
            ki.dwFlags |= KEYEVENTF_EXTENDEDKEY;
        }
    }
    send(&[input]);
}

/// The key that types `c` on the current layout without Shift or AltGr.
pub fn plain_vk(c: char) -> Option<u16> {
    key_for_char(c)
        .filter(|(_, needed)| needed.is_empty())
        .map(|(vk, _)| vk)
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

fn button_flags(button: MouseButton) -> (MOUSE_EVENT_FLAGS, MOUSE_EVENT_FLAGS) {
    match button {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    }
}

pub fn click(button: MouseButton) {
    let (down, up) = button_flags(button);
    send(&[mouse_input(down, 0), mouse_input(up, 0)]);
}

/// Presses or releases a mouse button, for dragging.
pub fn button(button: MouseButton, down: bool) {
    let (press, release) = button_flags(button);
    send(&[mouse_input(if down { press } else { release }, 0)]);
}

/// Moves the pointer by exactly this many pixels. Setting the position
/// directly keeps Windows' pointer acceleration out of the speed setting.
pub fn move_pointer(dx: i32, dy: i32) {
    let mut at = POINT::default();
    unsafe {
        if GetCursorPos(&mut at).is_ok() {
            let _ = SetCursorPos(at.x + dx, at.y + dy);
        }
    }
}

/// Moves the pointer to a screen position and clicks there.
pub fn click_at(x: i32, y: i32) {
    unsafe {
        let _ = SetCursorPos(x, y);
    }
    click(MouseButton::Left);
}

/// Moves the pointer to a screen position and right-clicks there.
pub fn right_click_at(x: i32, y: i32) {
    unsafe {
        let _ = SetCursorPos(x, y);
    }
    click(MouseButton::Right);
}

/// Types text directly, whatever the keyboard layout is.
pub fn text(text: &str) {
    let inputs: Vec<INPUT> = text
        .encode_utf16()
        .flat_map(|unit| [unicode_input(unit, true), unicode_input(unit, false)])
        .collect();
    send(&inputs);
}

/// See [`VK_UNASSIGNED`].
pub fn mask_menu() {
    send(&[
        key_input(VK_UNASSIGNED, true),
        key_input(VK_UNASSIGNED, false),
    ]);
}

/// Waits until Ctrl, Shift, Alt and Win are up, or the timeout passes.
pub fn wait_modifiers_released(timeout: Duration) {
    let end = Instant::now() + timeout;
    let held = |vk: u16| unsafe { GetAsyncKeyState(vk as i32) } < 0;
    while Instant::now() < end
        && [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
            .into_iter()
            .any(held)
    {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The combination with a character key replaced by its key on the current
/// layout. `None` if no key on the layout types that character.
pub fn with_layout_vk(combo: KeyCombo) -> Option<KeyCombo> {
    match combo.key {
        Key::Vk(_) => Some(combo),
        Key::Char(c) => {
            let (vk, _) = key_for_char(c)?;
            Some(KeyCombo {
                key: Key::Vk(vk),
                ..combo
            })
        }
    }
}
