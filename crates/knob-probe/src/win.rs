//! Windows-side listeners: a low-level keyboard hook and Raw Input, both on
//! the main thread, which runs a message loop. Neither blocks any input.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RID_INPUT, RIDEV_INPUTSINK, RIDI_DEVICENAME, RIM_TYPEHID, RIM_TYPEKEYBOARD,
    RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    HC_ACTION, HWND_MESSAGE, KBDLLHOOKSTRUCT, MSG, PostThreadMessageW, RegisterClassW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_INPUT, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN, WNDCLASSW,
};
use windows::core::{BOOL, Result, w};

use crate::event::{Event, now_ms, send};
use crate::names::short_device_name;

static MAIN_THREAD: AtomicU32 = AtomicU32::new(0);

/// Ends the message loop. Safe to call from any thread.
pub fn request_quit() {
    let tid = MAIN_THREAD.load(Ordering::SeqCst);
    if tid != 0 {
        unsafe {
            let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn run() -> Result<()> {
    unsafe {
        MAIN_THREAD.store(GetCurrentThreadId(), Ordering::SeqCst);
        SetConsoleCtrlHandler(Some(ctrl_handler), true)?;

        let instance = GetModuleHandleW(None)?;
        let class = w!("KnobProbeWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        // Message-only window: invisible, exists just to receive WM_INPUT.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("knob-probe"),
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

        // INPUTSINK: receive input even when our (invisible) window isn't focused.
        let devices = [
            RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x06,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
            RAWINPUTDEVICE {
                usUsagePage: 0x0C,
                usUsage: 0x01,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
        ];
        RegisterRawInputDevices(&devices, size_of::<RAWINPUTDEVICE>() as u32)?;

        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), Some(instance.into()), 0)?;
        send(Event::Info(
            "keyboard hook and Raw Input listeners ready".into(),
        ));
        send(Event::Begin);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWindowsHookEx(hook);
        let _ = DestroyWindow(hwnd);
    }
    Ok(())
}

extern "system" fn ctrl_handler(_ctrl_type: u32) -> BOOL {
    request_quit();
    BOOL(1) // handled: let the message loop exit and the log flush
}

extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code == HC_ACTION as i32 {
            let k = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let msg = wparam.0 as u32;
            send(Event::Hook {
                t: now_ms(),
                vk: k.vkCode,
                scan: k.scanCode,
                flags: k.flags.0,
                extra: k.dwExtraInfo,
                down: msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN,
            });
        }
        CallNextHookEx(None, code, wparam, lparam)
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_INPUT {
            handle_raw_input(lparam);
        }
        // Must be called for WM_INPUT too, so Windows can clean up.
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe fn handle_raw_input(lparam: LPARAM) {
    let t = now_ms();
    let handle = HRAWINPUT(lparam.0 as *mut c_void);
    let header_size = size_of::<RAWINPUTHEADER>() as u32;

    let mut size = 0u32;
    unsafe { GetRawInputData(handle, RID_INPUT, None, &mut size, header_size) };
    if size == 0 {
        return;
    }
    // u64 buffer keeps RAWINPUT correctly aligned.
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
    let device = device_name(raw.header.hDevice);

    if raw.header.dwType == RIM_TYPEKEYBOARD.0 {
        let k = unsafe { raw.data.keyboard };
        send(Event::RawKey {
            t,
            device,
            make: k.MakeCode,
            flags: k.Flags,
            vkey: k.VKey,
            message: k.Message,
            extra: k.ExtraInformation,
        });
    } else if raw.header.dwType == RIM_TYPEHID.0 {
        let hid = unsafe { &raw.data.hid };
        let len = (hid.dwSizeHid * hid.dwCount) as usize;
        let data = unsafe { std::slice::from_raw_parts(hid.bRawData.as_ptr(), len) }.to_vec();
        send(Event::RawHid {
            t,
            device,
            data,
            report_size: hid.dwSizeHid,
        });
    }
}

/// Short device name for a Raw Input handle, cached. The first time a device
/// is seen, its full path goes into the log.
fn device_name(handle: HANDLE) -> String {
    thread_local! {
        static CACHE: RefCell<HashMap<usize, String>> = RefCell::new(HashMap::new());
    }
    let key = handle.0 as usize;
    if key == 0 {
        return "(none/injected)".into();
    }
    if let Some(name) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return name;
    }

    let mut len = 0u32;
    let full = unsafe {
        GetRawInputDeviceInfoW(Some(handle), RIDI_DEVICENAME, None, &mut len);
        let mut buf = vec![0u16; len as usize];
        GetRawInputDeviceInfoW(
            Some(handle),
            RIDI_DEVICENAME,
            Some(buf.as_mut_ptr().cast()),
            &mut len,
        );
        String::from_utf16_lossy(&buf)
            .trim_end_matches('\0')
            .to_string()
    };
    let short = short_device_name(&full);
    send(Event::Info(format!("new Raw Input device {short}: {full}")));
    CACHE.with(|c| c.borrow_mut().insert(key, short.clone()));
    short
}
