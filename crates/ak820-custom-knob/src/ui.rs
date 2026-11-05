//! The UI thread: tray icon, its menu, and the on-screen popup.
//! Other threads talk to it through [`notify`].

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, IDI_APPLICATION, LoadIconW, MF_CHECKED, MF_GRAYED,
    MF_SEPARATOR, MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, SW_SHOWNORMAL, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_APP, WM_CONTEXTMENU,
    WM_DESTROY, WM_LBUTTONDBLCLK, WM_NULL, WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{HSTRING, PCWSTR, Result, w};

use crate::i18n::{Text, tr, trf};
use crate::{osd, paths};

const WM_TRAY: u32 = WM_APP + 1;
const WM_UI_EVENT: u32 = WM_APP + 2;

const ID_MODE: usize = 1;
const ID_PAUSE: usize = 2;
const ID_CONFIG: usize = 3;
const ID_LOGS: usize = 4;
const ID_SETTINGS: usize = 6;
const ID_QUIT: usize = 5;

pub enum UiEvent {
    /// The active mode changed. `announce` also shows it on screen.
    Mode { name: String, announce: bool },
    /// Show a short message on screen.
    Osd(String),
    /// Show a tray notification.
    Warning { title: String, text: String },
    /// The language changed: refresh texts that stay on screen.
    LanguageChanged,
}

static HWND_VALUE: AtomicIsize = AtomicIsize::new(0);
static QUEUE: Mutex<VecDeque<UiEvent>> = Mutex::new(VecDeque::new());

/// Sends an event to the UI thread. Safe to call from any thread.
pub fn notify(event: UiEvent) {
    if let Ok(mut queue) = QUEUE.lock() {
        queue.push_back(event);
    }
    let hwnd = HWND_VALUE.load(Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(hwnd as *mut c_void)),
                WM_UI_EVENT,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

struct Tray {
    hwnd: HWND,
    mode: String,
    /// Explorer broadcasts this after restarting; the icon must be added again.
    taskbar_created: u32,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

/// Runs the tray until the user quits.
pub fn run(startup_warning: Option<(String, String)>) -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("Ak820KnobTray");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        // A real (never shown) window, not message-only: message-only windows
        // don't receive the TaskbarCreated broadcast.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("ak820-custom-knob"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let taskbar_created = RegisterWindowMessageW(w!("TaskbarCreated"));
        TRAY.with(|t| {
            *t.borrow_mut() = Some(Tray {
                hwnd,
                mode: String::new(),
                taskbar_created,
            })
        });
        log::debug!("ui: window created");
        add_icon(hwnd);
        log::debug!("ui: tray icon added");
        osd::init(instance.into())?;
        log::debug!("ui: osd ready");

        HWND_VALUE.store(hwnd.0 as isize, Ordering::SeqCst);
        drain_events(); // events sent before the window existed
        log::debug!("ui: running");
        if let Some((title, text)) = startup_warning {
            balloon(&title, &text);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        HWND_VALUE.store(0, Ordering::SeqCst);
    }
    Ok(())
}

fn copy_wide(dst: &mut [u16], text: &str) {
    let max = dst.len() - 1;
    let mut n = 0;
    for unit in text.encode_utf16().take(max) {
        dst[n] = unit;
        n += 1;
    }
    dst[n] = 0;
}

fn icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    }
}

fn tooltip() -> String {
    let mode = TRAY.with(|t| {
        t.borrow()
            .as_ref()
            .map(|t| t.mode.clone())
            .unwrap_or_default()
    });
    if knob_win::input::is_paused() {
        tr(Text::TooltipPaused).into()
    } else if mode.is_empty() {
        tr(Text::Tooltip).into()
    } else {
        trf(Text::TooltipMode, &[("mode", &mode)])
    }
}

fn add_icon(hwnd: HWND) {
    let mut data = icon_data(hwnd);
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap_or_default();
    copy_wide(&mut data.szTip, &tooltip());
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &data);
    }
}

fn update_tooltip() {
    let Some(hwnd) = TRAY.with(|t| t.borrow().as_ref().map(|t| t.hwnd)) else {
        return;
    };
    let mut data = icon_data(hwnd);
    data.uFlags = NIF_TIP;
    copy_wide(&mut data.szTip, &tooltip());
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
    }
}

fn balloon(title: &str, text: &str) {
    let Some(hwnd) = TRAY.with(|t| t.borrow().as_ref().map(|t| t.hwnd)) else {
        return;
    };
    let mut data = icon_data(hwnd);
    data.uFlags = NIF_INFO;
    data.dwInfoFlags = NIIF_WARNING;
    copy_wide(&mut data.szInfoTitle, title);
    copy_wide(&mut data.szInfo, text);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
    }
}

fn drain_events() {
    // The lock guard is consumed inside `and_then`, so it's released before handling.
    while let Some(event) = QUEUE.lock().ok().and_then(|mut q| q.pop_front()) {
        match event {
            UiEvent::Mode { name, announce } => {
                if announce {
                    osd::show(&name);
                }
                TRAY.with(|t| {
                    if let Some(t) = t.borrow_mut().as_mut() {
                        t.mode = name;
                    }
                });
                update_tooltip();
            }
            UiEvent::Osd(text) => osd::show(&text),
            UiEvent::Warning { title, text } => balloon(&title, &text),
            UiEvent::LanguageChanged => update_tooltip(),
        }
    }
}

/// Opens the settings app (installed next to this exe), or the config file
/// itself when the settings app isn't there.
fn open_settings() {
    let settings = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("knob-settings.exe")))
        .filter(|path| path.exists());
    match settings {
        Some(path) => {
            if let Err(e) = std::process::Command::new(&path).spawn() {
                log::warn!("cannot start {}: {e}", path.display());
                open(&paths::config_file());
            }
        }
        None => open(&paths::config_file()),
    }
}

fn open(target: &std::path::Path) {
    let file = HSTRING::from(target.as_os_str());
    unsafe {
        let result = ShellExecuteW(
            None,
            w!("open"),
            &file,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        // No app associated with .toml: fall back to Notepad.
        if result.0 as isize <= 32 {
            let _ = ShellExecuteW(
                None,
                w!("open"),
                w!("notepad.exe"),
                &file,
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }
}

fn show_menu(hwnd: HWND) {
    let mode = TRAY.with(|t| {
        t.borrow()
            .as_ref()
            .map(|t| t.mode.clone())
            .unwrap_or_default()
    });
    let paused = knob_win::input::is_paused();
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let mode_label = HSTRING::from(if mode.is_empty() {
            tr(Text::MenuNoMode).to_string()
        } else {
            trf(Text::MenuMode, &[("mode", &mode)])
        });
        let item = |text: Text| HSTRING::from(tr(text));
        let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, ID_MODE, &mode_label);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let pause_flags = if paused {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        let _ = AppendMenuW(menu, pause_flags, ID_PAUSE, &item(Text::MenuPaused));
        let _ = AppendMenuW(menu, MF_STRING, ID_SETTINGS, &item(Text::MenuSettings));
        let _ = AppendMenuW(menu, MF_STRING, ID_CONFIG, &item(Text::MenuEditConfig));
        let _ = AppendMenuW(menu, MF_STRING, ID_LOGS, &item(Text::MenuLogs));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, ID_QUIT, &item(Text::MenuQuit));

        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        // Required so the menu closes when clicking elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            point.x,
            point.y,
            None,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);

        match command.0 as usize {
            ID_PAUSE => {
                knob_win::input::set_paused(!paused);
                log::info!("{}", if paused { "resumed" } else { "paused" });
                osd::show(tr(if paused {
                    Text::OsdResumed
                } else {
                    Text::OsdPaused
                }));
                update_tooltip();
            }
            ID_SETTINGS => open_settings(),
            ID_CONFIG => open(&paths::config_file()),
            ID_LOGS => open(&paths::log_dir()),
            ID_QUIT => {
                let _ = DestroyWindow(hwnd);
            }
            _ => {}
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    log::trace!("ui wndproc: 0x{msg:04X}");
    let taskbar_created =
        TRAY.with(|t| t.borrow().as_ref().map(|t| t.taskbar_created).unwrap_or(0));
    unsafe {
        match msg {
            WM_TRAY => {
                match (lparam.0 as u32) & 0xFFFF {
                    WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                    WM_LBUTTONDBLCLK => open_settings(),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_UI_EVENT => {
                drain_events();
                LRESULT(0)
            }
            WM_DESTROY => {
                let data = icon_data(hwnd);
                let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                PostQuitMessage(0);
                LRESULT(0)
            }
            m if m != 0 && m == taskbar_created => {
                add_icon(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
