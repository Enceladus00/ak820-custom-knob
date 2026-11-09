//! Finding out which app a window or process belongs to.

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetForegroundWindow, GetWindowLongW, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PWSTR};

/// An app with a visible window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningApp {
    pub exe: String,
    pub title: String,
}

/// Apps that currently have a visible, titled window, one entry per exe,
/// sorted by exe name.
pub fn running_apps() -> Vec<RunningApp> {
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let apps = &mut *(lparam.0 as *mut Vec<RunningApp>);
            let tool_window = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0;
            if IsWindowVisible(hwnd).as_bool() && !tool_window && GetWindowTextLengthW(hwnd) > 0 {
                let mut buf = [0u16; 256];
                let len = GetWindowTextW(hwnd, &mut buf).max(0) as usize;
                if let Some((_, exe)) = window_process(hwnd) {
                    apps.push(RunningApp {
                        exe,
                        title: String::from_utf16_lossy(&buf[..len]),
                    });
                }
            }
            BOOL(1)
        }
    }

    let mut apps: Vec<RunningApp> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(collect),
            LPARAM(&mut apps as *mut Vec<RunningApp> as isize),
        );
    }
    apps.sort_by_key(|a| a.exe.to_lowercase());
    apps.dedup_by(|a, b| a.exe.eq_ignore_ascii_case(&b.exe));
    apps
}

/// Exe file name of a process, e.g. `"Spotify.exe"`.
pub fn exe_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().map(str::to_string)
    }
}

/// Process id and exe name of the process owning a window.
pub fn window_process(hwnd: HWND) -> Option<(u32, String)> {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    exe_name(pid).map(|exe| (pid, exe))
}

pub fn foreground() -> Option<(u32, String)> {
    window_process(unsafe { GetForegroundWindow() })
}

/// Whether the background program is running (its tray window exists).
pub fn tray_running() -> bool {
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::FindWindowW(
            windows::core::w!("Ak820KnobTray"),
            windows::core::PCWSTR::null(),
        )
        .is_ok_and(|window| !window.is_invalid())
    }
}
