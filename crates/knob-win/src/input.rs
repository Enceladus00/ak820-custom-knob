//! The input thread: a low-level keyboard hook, Raw Input for the knob's
//! consumer-control reports, and foreground-app changes.
//!
//! The hook is deliberately dumb and fast. It swallows Volume Up/Down/Mute
//! (unless injected by us, paused, or the engine is down) and hands them to
//! the sink; everything else passes untouched. All decisions happen in the
//! engine, so Windows never removes the hook for answering too slowly.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};

use knob_core::remap::Remap;
use knob_core::router::{Decision, Hotkey, KeyInfo, Router, RouterEvent};
use knob_core::{HookKey, Input, Modifiers};
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RMENU,
    VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList, HRAWINPUT, RAWINPUT,
    RAWINPUTDEVICE, RAWINPUTDEVICELIST, RAWINPUTHEADER, RID_INPUT, RIDEV_DEVNOTIFY,
    RIDEV_INPUTSINK, RIDI_DEVICENAME, RIM_TYPEHID, RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    EVENT_SYSTEM_DESKTOPSWITCH, EVENT_SYSTEM_FOREGROUND, GIDC_REMOVAL, GetForegroundWindow,
    GetMessageW, HC_ACTION, HWND_MESSAGE, KBDLLHOOKSTRUCT, KillTimer, MSG, PostThreadMessageW,
    RegisterClassW, SetTimer, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WINDOW_EX_STYLE, WINDOW_STYLE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    WM_INPUT, WM_INPUT_DEVICE_CHANGE, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSW,
};
use windows::core::{Result, w};

use crate::{KNOWN_DEVICES, MARKER, clock, output, process};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    /// Knob-related input for the engine.
    Core(Input),
    /// The foreground window now belongs to this process.
    Foreground { pid: u32, exe: String },
    /// A supported keyboard (its receiver) was connected or removed.
    KeyboardPresent(bool),
    /// A hotkey, mouse-mode key or click-hint key.
    Extra(RouterEvent),
}

static SINK: OnceLock<fn(InputEvent)> = OnceLock::new();
static THREAD_ID: AtomicU32 = AtomicU32::new(0);
static PAUSED: AtomicBool = AtomicBool::new(false);
static ENGINE_ALIVE: AtomicBool = AtomicBool::new(true);
static KEYBOARD_PRESENT: AtomicBool = AtomicBool::new(false);
const SETTINGS_EXE: &str = "knob-settings.exe";
static ROUTER: LazyLock<Mutex<Router>> = LazyLock::new(Mutex::default);
/// Timers on the input window: a regular device rescan, and one shortly after
/// a device change.
const TIMER_RESCAN: usize = 1;
const TIMER_SETTLE: usize = 2;

/// Whether a supported keyboard is connected. Without one, nothing is taken
/// over: the volume keys and the knob work as without this program.
pub fn keyboard_present() -> bool {
    KEYBOARD_PRESENT.load(Ordering::SeqCst)
}

/// While paused, the knob behaves exactly as without this program.
pub fn set_paused(paused: bool) {
    PAUSED.store(paused, Ordering::SeqCst);
    if paused {
        release_remaps();
    }
}

pub fn is_paused() -> bool {
    PAUSED.load(Ordering::SeqCst)
}

/// Replaces the keys that start an optional feature.
pub fn set_hotkeys(hotkeys: Vec<Hotkey>) {
    if let Ok(mut router) = ROUTER.lock() {
        router.set_hotkeys(hotkeys);
    }
}

pub fn set_macro_running(running: bool) {
    if let Ok(mut router) = ROUTER.lock() {
        router.set_macro_running(running);
    }
}

pub fn set_mouse_active(active: bool) {
    if let Ok(mut router) = ROUTER.lock() {
        router.set_mouse_active(active);
    }
}

pub fn set_hints_active(active: bool) {
    if let Ok(mut router) = ROUTER.lock() {
        router.set_hints_active(active);
    }
}

/// Replaces the key remapping rules.
pub fn set_remaps(remaps: Vec<Remap>) {
    if let Ok(mut router) = ROUTER.lock() {
        router.set_remaps(remaps);
    }
}

/// Releases every key a remap is holding down, so none stays stuck when
/// remapping stops or a key's release can't be seen. Safe from any thread.
pub fn release_remaps() {
    let keys = match ROUTER.lock() {
        Ok(mut router) => router.take_held(),
        Err(_) => return,
    };
    for vk in keys {
        log::debug!("remap: released 0x{vk:02X}");
        output::remap_key(vk, false);
    }
}

/// Fail-safe: once the engine is gone, the hook stops swallowing keys.
pub fn set_engine_alive(alive: bool) {
    ENGINE_ALIVE.store(alive, Ordering::SeqCst);
    if !alive {
        release_remaps();
    }
}

/// Ends `run`'s message loop. Safe to call from any thread.
pub fn request_stop() {
    let tid = THREAD_ID.load(Ordering::SeqCst);
    if tid != 0 {
        unsafe {
            let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

fn emit(event: InputEvent) {
    if let Some(sink) = SINK.get() {
        sink(event);
    }
}

/// Installs the listeners and runs the message loop until `request_stop`.
/// `sink` is called on this thread for every event and must return quickly.
pub fn run(sink: fn(InputEvent)) -> Result<()> {
    let _ = SINK.set(sink);
    unsafe {
        THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);

        let instance = GetModuleHandleW(None)?;
        let class = w!("Ak820KnobInput");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("ak820-custom-knob input"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            None,
        )?;

        // Consumer control only: that's where the knob's reports arrive.
        let devices = [RAWINPUTDEVICE {
            usUsagePage: 0x0C,
            usUsage: 0x01,
            dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
            hwndTarget: hwnd,
        }];
        RegisterRawInputDevices(&devices, size_of::<RAWINPUTDEVICE>() as u32)?;
        log::debug!("input: raw input registered");
        scan_devices();
        // Device-change notices are the main signal; this catches any missed one.
        let _ = SetTimer(Some(hwnd), TIMER_RESCAN, 2000, None);

        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), Some(instance.into()), 0)?;
        log::debug!("input: keyboard hook installed");
        let win_event = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(foreground_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        log::debug!("input: foreground hook installed");
        // The lock screen and UAC prompts run on another desktop, where key
        // releases never reach the hook: let go of remapped keys first.
        let desktop_event = SetWinEventHook(
            EVENT_SYSTEM_DESKTOPSWITCH,
            EVENT_SYSTEM_DESKTOPSWITCH,
            None,
            Some(desktop_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        report_foreground(GetForegroundWindow());

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            log::trace!("input: msg 0x{:04X}", msg.message);
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWinEvent(win_event);
        let _ = UnhookWinEvent(desktop_event);
        let _ = UnhookWindowsHookEx(hook);
        release_remaps();
        let _ = DestroyWindow(hwnd);
    }
    Ok(())
}

const VK_LCONTROL_CODE: u16 = 0xA2;
const VK_RMENU_CODE: u16 = 0xA5;
/// Scan-code bit Windows sets on the Left Ctrl it adds to every AltGr press.
const ALTGR_CTRL_SCAN: u32 = 0x200;

/// What the optional features want done with this key. Does nothing at all
/// while none of them is in use.
fn route(vk: u16, scan: u32, down: bool) -> Decision {
    let Ok(mut router) = ROUTER.lock() else {
        return Decision::default();
    };
    // AltGr arrives as a made-up Left Ctrl plus Right Alt. When Right Alt is
    // remapped, the made-up Ctrl has to go too, or it would combine with
    // whatever Right Alt now sends.
    if vk == VK_LCONTROL_CODE && scan & ALTGR_CTRL_SCAN != 0 && router.is_remapped(VK_RMENU_CODE) {
        return Decision {
            swallow: true,
            ..Decision::default()
        };
    }
    if router.is_idle() {
        return Decision::default();
    }
    let held = |vk: u16| unsafe { GetAsyncKeyState(vk as i32) } < 0;
    let mut mods = current_modifiers();
    // AltGr reaches Windows as Left Ctrl + Right Alt. It types characters, so
    // it must not count as a Ctrl+Alt hotkey.
    if held(VK_LCONTROL.0) && held(VK_RMENU.0) {
        mods.ctrl = false;
        mods.alt = false;
    }
    router.handle(KeyInfo {
        vk,
        down,
        mods,
        other_modifier_held: held(VK_LCONTROL.0)
            || held(VK_LSHIFT.0)
            || held(VK_LMENU.0)
            || held(VK_RMENU.0)
            || held(VK_LWIN.0)
            || held(VK_RWIN.0),
    })
}

fn current_modifiers() -> Modifiers {
    let held = |vk: u16| unsafe { GetAsyncKeyState(vk as i32) } < 0;
    Modifiers {
        ctrl: held(VK_CONTROL.0),
        shift: held(VK_SHIFT.0),
        alt: held(VK_MENU.0),
        win: held(VK_LWIN.0) || held(VK_RWIN.0),
    }
}

extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    log::trace!("hook: code {code}");
    unsafe {
        if code == HC_ACTION as i32 {
            let k = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            if HookKey::from_vk(k.vkCode).is_none()
                && k.dwExtraInfo != MARKER
                && !PAUSED.load(Ordering::Relaxed)
                && ENGINE_ALIVE.load(Ordering::Relaxed)
            {
                let msg = wparam.0 as u32;
                let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let decision = route(k.vkCode as u16, k.scanCode, down);
                if let Some(event) = decision.event {
                    emit(InputEvent::Extra(event));
                }
                if let Some(send) = decision.send {
                    output::remap(send);
                }
                if decision.swallow {
                    return LRESULT(1);
                }
            }
            if let Some(key) = HookKey::from_vk(k.vkCode)
                && k.dwExtraInfo != MARKER
                && !PAUSED.load(Ordering::Relaxed)
                && ENGINE_ALIVE.load(Ordering::Relaxed)
            {
                let msg = wparam.0 as u32;
                emit(InputEvent::Core(Input::Key {
                    t: clock::now(),
                    key,
                    down: msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN,
                    mods: current_modifiers(),
                }));
                return LRESULT(1); // swallowed; the engine replays it if it isn't the knob
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }
}

extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    log::trace!("foreground event");
    report_foreground(hwnd);
}

fn report_foreground(hwnd: HWND) {
    if let Some((pid, exe)) = process::window_process(hwnd) {
        // A hotkey is being set up: pressing it must not also run it.
        if let Ok(mut router) = ROUTER.lock() {
            router.set_hotkeys_suspended(exe.eq_ignore_ascii_case(SETTINGS_EXE));
            router.set_foreground(&exe);
        }
        emit(InputEvent::Foreground { pid, exe });
    }
}

extern "system" fn desktop_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    log::debug!("input: desktop switch");
    release_remaps();
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        log::trace!("input wndproc: 0x{msg:04X}");
        match msg {
            WM_INPUT => handle_raw_input(lparam),
            // A device was plugged in or removed: forget what we knew about it
            // (handles can be reused) and look again at what is connected, now
            // and once more when Windows has finished updating its device list.
            WM_INPUT_DEVICE_CHANGE => {
                log::info!(
                    "input: device {} (0x{:X})",
                    if wparam.0 == GIDC_REMOVAL as usize {
                        "removed"
                    } else {
                        "connected"
                    },
                    lparam.0
                );
                DEVICE_CACHE.with(|c| {
                    c.borrow_mut().remove(&(lparam.0 as usize));
                });
                scan_devices();
                let _ = SetTimer(Some(hwnd), TIMER_SETTLE, 500, None);
            }
            WM_TIMER => {
                if wparam.0 == TIMER_SETTLE {
                    let _ = KillTimer(Some(hwnd), TIMER_SETTLE);
                }
                scan_devices();
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

thread_local! {
    /// Raw Input device handle → is it a supported keyboard?
    static DEVICE_CACHE: RefCell<HashMap<usize, bool>> = RefCell::new(HashMap::new());
}

unsafe fn handle_raw_input(lparam: LPARAM) {
    let t = clock::now();
    let handle = HRAWINPUT(lparam.0 as *mut c_void);
    let header_size = size_of::<RAWINPUTHEADER>() as u32;

    let mut size = 0u32;
    unsafe { GetRawInputData(handle, RID_INPUT, None, &mut size, header_size) };
    if size == 0 {
        return;
    }
    let mut buf = vec![0u64; (size as usize).div_ceil(8)];
    let read = unsafe {
        GetRawInputData(
            handle,
            RID_INPUT,
            Some(buf.as_mut_ptr().cast()),
            &mut size,
            header_size,
        )
    };
    if read == u32::MAX {
        return;
    }
    let raw = unsafe { &*(buf.as_ptr() as *const RAWINPUT) };
    if raw.header.dwType != RIM_TYPEHID.0 || !is_supported_device(raw.header.hDevice) {
        return;
    }

    let hid = unsafe { &raw.data.hid };
    let report_size = hid.dwSizeHid as usize;
    let len = report_size * hid.dwCount as usize;
    let data = unsafe { std::slice::from_raw_parts(hid.bRawData.as_ptr(), len) };
    // Consumer reports: [report id][usage lo][usage hi]...
    for report in data.chunks(report_size.max(1)) {
        if report.len() >= 3 {
            let usage = u16::from_le_bytes([report[1], report[2]]);
            emit(InputEvent::Core(Input::DeviceReport { t, usage }));
        }
    }
}

/// Checks the connected HID devices for a supported keyboard and reports a
/// change.
fn scan_devices() {
    let header = size_of::<RAWINPUTDEVICELIST>() as u32;
    let mut count = 0u32;
    let mut list = Vec::new();
    unsafe {
        if GetRawInputDeviceList(None, &mut count, header) == u32::MAX {
            return;
        }
        list.resize(count as usize, RAWINPUTDEVICELIST::default());
        let n = GetRawInputDeviceList(Some(list.as_mut_ptr()), &mut count, header);
        if n == u32::MAX {
            return; // the list changed in between; the next change notice rescans
        }
        list.truncate(n as usize);
    }
    let present = list
        .iter()
        .any(|d| d.dwType == RIM_TYPEHID && is_supported_device(d.hDevice));
    if KEYBOARD_PRESENT.swap(present, Ordering::SeqCst) != present {
        log::info!(
            "keyboard {}",
            if present {
                "connected"
            } else {
                "not connected"
            }
        );
        emit(InputEvent::KeyboardPresent(present));
    }
}

fn is_supported_device(handle: HANDLE) -> bool {
    let key = handle.0 as usize;
    if key == 0 {
        return false;
    }
    if let Some(known) = DEVICE_CACHE.with(|c| c.borrow().get(&key).copied()) {
        return known;
    }
    let name = unsafe {
        let mut len = 0u32;
        GetRawInputDeviceInfoW(Some(handle), RIDI_DEVICENAME, None, &mut len);
        let mut buf = vec![0u16; len as usize];
        GetRawInputDeviceInfoW(
            Some(handle),
            RIDI_DEVICENAME,
            Some(buf.as_mut_ptr().cast()),
            &mut len,
        );
        String::from_utf16_lossy(&buf).to_ascii_uppercase()
    };
    let supported = KNOWN_DEVICES.iter().any(|id| name.contains(id));
    DEVICE_CACHE.with(|c| c.borrow_mut().insert(key, supported));
    supported
}
