//! Text expansion in the keyboard hook: turns typed keys into characters
//! (with the layout of the app in front), feeds them to the
//! [`Matcher`](knob_core::snippets::Matcher) and, on a trigger, swallows its
//! last key, erases the rest and types the snippet.
//!
//! Safety rules:
//! * Nothing is remembered or expanded while the focused field is a password
//!   box. A separate thread asks UI Automation on every focus change; until
//!   it has answered, the field counts as a password box.
//! * At most the last 32 characters are remembered, and they are wiped when
//!   focus or the app in front changes.
//! * Triggers and texts are never logged, and text is typed, not pasted, so
//!   it never passes through the clipboard (or its history).

use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use knob_core::snippets::{LocalTime, Matcher, SnippetStore, expand};
use windows::Win32::Globalization::{GetLocaleInfoEx, LOCALE_SSHORTDATE, LOCALE_SSHORTTIME};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardLayout, ToUnicodeEx,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ES_PASSWORD, GUITHREADINFO, GWL_STYLE, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongW,
    GetWindowThreadProcessId,
};

use crate::output;

struct Live {
    store: SnippetStore,
    matcher: Matcher,
    /// The app in front, lower case.
    exe: String,
    /// Settings is in front: never expand there, so triggers can be typed.
    suspended: bool,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

const FIELD_OK: u8 = 0;
const FIELD_PASSWORD: u8 = 1;
const FIELD_UNKNOWN: u8 = 2;
static FIELD: AtomicU8 = AtomicU8::new(FIELD_UNKNOWN);
static GUARD: OnceLock<Sender<()>> = OnceLock::new();

/// The key whose press finished a trigger: its release is swallowed too.
/// Holds vk + 1, 0 for none.
static SWALLOW_UP: AtomicU32 = AtomicU32::new(0);

/// Sets the snippet list (from the encrypted file). `None` turns expansion off.
pub fn set_store(store: Option<SnippetStore>) {
    let Ok(mut live) = LIVE.lock() else { return };
    let (exe, suspended) = live
        .as_ref()
        .map(|l| (l.exe.clone(), l.suspended))
        .unwrap_or_default();
    *live = store
        .filter(|s| s.enabled && !s.snippets.is_empty())
        .map(|store| Live {
            matcher: Matcher::new(&store),
            store,
            exe,
            suspended,
        });
    let count = live.as_ref().map_or(0, |l| l.store.snippets.len());
    log::info!("snippets: {count} active");
    if live.is_some() {
        start_guard();
        check_focus();
    }
}

/// The app in front changed. Forgets what was typed.
pub fn set_foreground(exe: &str, settings: bool) {
    if let Ok(mut live) = LIVE.lock()
        && let Some(live) = live.as_mut()
    {
        live.exe = exe.to_lowercase();
        live.suspended = settings;
        live.matcher.clear();
    }
}

/// Keyboard focus moved (a window event). Forgets what was typed and holds
/// expansion until the new field is known not to be a password box.
pub fn focus_changed() {
    if let Ok(mut live) = LIVE.lock()
        && let Some(live) = live.as_mut()
    {
        live.matcher.clear();
        check_focus();
    }
}

fn check_focus() {
    FIELD.store(FIELD_UNKNOWN, Ordering::SeqCst);
    if let Some(tx) = GUARD.get() {
        let _ = tx.send(());
    }
}

/// From the keyboard hook, for every real (not injected) key. Returns true
/// to swallow the key.
pub fn on_key(vk: u16, scan: u32, down: bool) -> bool {
    if !down {
        return SWALLOW_UP
            .compare_exchange(vk as u32 + 1, 0, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();
    }
    let Ok(mut guard) = LIVE.lock() else {
        return false;
    };
    let Some(live) = guard.as_mut() else {
        return false;
    };
    if live.suspended || FIELD.load(Ordering::SeqCst) != FIELD_OK {
        live.matcher.clear();
        return false;
    }
    if live
        .store
        .exclude
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&live.exe))
    {
        return false;
    }
    match vk {
        0x08 => {
            live.matcher.backspace();
            return false;
        }
        // Shift, Ctrl, Alt (either side) and Caps Lock change the next
        // character; they are not characters themselves.
        0x10..=0x12 | 0x14 | 0xA0..=0xA5 => return false,
        _ => {}
    }
    let held = |vk: i32| unsafe { GetAsyncKeyState(vk) } < 0;
    if held(0x5B) || held(0x5C) {
        live.matcher.clear();
        return false;
    }
    let Some(c) = to_char(vk, scan) else {
        live.matcher.clear();
        return false;
    };
    let Live {
        store,
        matcher,
        exe,
        ..
    } = live;
    let Some((index, trigger_len)) = matcher.push(c, |i| store.allowed(&store.snippets[i], exe))
    else {
        return false;
    };
    let expansion = expand(&store.snippets[index].text, &local_time());
    drop(guard);
    SWALLOW_UP.store(vk as u32 + 1, Ordering::SeqCst);
    // The last key of the trigger is swallowed; erase the ones before it.
    output::replace_typed(trigger_len - 1, &expansion.text, expansion.back);
    log::info!(
        "snippets: expanded ({} characters)",
        expansion.text.chars().count()
    );
    true
}

/// The character this key types in the app in front, if it types one.
fn to_char(vk: u16, scan: u32) -> Option<char> {
    let mut state = [0u8; 256];
    for key in [0x10, 0x11, 0x12, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5] {
        if unsafe { GetAsyncKeyState(key) } < 0 {
            state[key as usize] = 0x80;
        }
    }
    if unsafe { GetKeyState(0x14) } & 1 != 0 {
        state[0x14] = 0x01;
    }
    let layout = unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        GetKeyboardLayout(thread)
    };
    let mut out = [0u16; 4];
    // Flag 4: leave the keyboard state alone, so dead keys (^, ¨) still
    // work in the app.
    let n = unsafe { ToUnicodeEx(vk as u32, scan, &state, &mut out, 4, Some(layout)) };
    if n != 1 {
        return None;
    }
    char::from_u32(out[0] as u32).filter(|c| !c.is_control())
}

fn local_time() -> LocalTime {
    let t = unsafe { GetLocalTime() };
    LocalTime {
        year: t.wYear,
        month: t.wMonth,
        day: t.wDay,
        hour: t.wHour,
        minute: t.wMinute,
        second: t.wSecond,
        date_format: locale_info(LOCALE_SSHORTDATE).unwrap_or_else(|| "yyyy-MM-dd".into()),
        time_format: locale_info(LOCALE_SSHORTTIME).unwrap_or_else(|| "HH:mm".into()),
    }
}

fn locale_info(kind: u32) -> Option<String> {
    let mut buf = [0u16; 80];
    let n = unsafe { GetLocaleInfoEx(None, kind, Some(&mut buf)) };
    (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
}

/// Starts the thread that tells password boxes apart, once.
fn start_guard() {
    if GUARD.get().is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel::<()>();
    if GUARD.set(tx).is_err() {
        return;
    }
    thread::Builder::new()
        .name("snippet-guard".into())
        .spawn(move || {
            let automation: Option<IUIAutomation> = unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
            };
            if automation.is_none() {
                log::warn!("snippets: UI Automation unavailable; only classic password boxes are recognised");
            }
            while rx.recv().is_ok() {
                // Focus often moves several times in a row; look once it settles.
                thread::sleep(Duration::from_millis(15));
                while rx.try_recv().is_ok() {}
                let password = is_password(automation.as_ref());
                FIELD.store(
                    if password { FIELD_PASSWORD } else { FIELD_OK },
                    Ordering::SeqCst,
                );
            }
        })
        .expect("spawn snippet guard");
}

/// Whether keyboard focus is in a password box: asked of UI Automation
/// (browsers, modern apps), and of the classic edit control's style.
fn is_password(automation: Option<&IUIAutomation>) -> bool {
    if let Some(a) = automation
        && let Ok(element) = unsafe { a.GetFocusedElement() }
        && let Ok(password) = unsafe { element.CurrentIsPassword() }
        && password.as_bool()
    {
        return true;
    }
    unsafe {
        let mut info = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        if GetGUIThreadInfo(thread, &mut info).is_ok() && !info.hwndFocus.is_invalid() {
            let style = GetWindowLongW(info.hwndFocus, GWL_STYLE) as u32;
            return style & ES_PASSWORD as u32 != 0;
        }
    }
    false
}
