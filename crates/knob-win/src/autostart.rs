//! "Start with Windows": a per-user entry under
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. No admin rights
//! needed. The registry entry itself is the setting, so the tray menu and the
//! settings app always show the real state.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::core::{HSTRING, w};

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: windows::core::PCWSTR = w!("ak820-custom-knob");

/// File name of the background program.
pub const DAEMON_EXE: &str = "ak820-custom-knob.exe";

/// The background program's exe: this exe when it is the daemon, otherwise
/// the daemon next to it (the settings app lives in the same folder).
pub fn daemon_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    if exe
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case(DAEMON_EXE))
    {
        return Some(exe);
    }
    Some(exe.parent()?.join(DAEMON_EXE)).filter(|p| p.exists())
}

fn check(code: WIN32_ERROR) -> std::io::Result<()> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(code.0 as i32))
    }
}

/// The command line registered to run at sign-in, if any.
pub fn registered_command() -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut size = size_of_val(&buf) as u32;
    let code = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            VALUE,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if code != ERROR_SUCCESS {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

pub fn is_enabled() -> bool {
    registered_command().is_some()
}

fn command_for(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Turns starting `exe` at sign-in on or off.
pub fn set_enabled(enabled: bool, exe: &Path) -> std::io::Result<()> {
    if enabled {
        let command = HSTRING::from(command_for(exe));
        let bytes = (command.len() + 1) * 2; // with the terminating zero
        check(unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                VALUE,
                REG_SZ.0,
                Some(command.as_ptr().cast()),
                bytes as u32,
            )
        })
    } else {
        match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE) } {
            ERROR_FILE_NOT_FOUND => Ok(()),
            code => check(code),
        }
    }
}

/// When autostart is on but points at another exe (the folder was moved or
/// the app updated in a new place), points it at `exe` instead.
pub fn repair(exe: &Path) {
    if let Some(current) = registered_command()
        && current != command_for(exe)
    {
        match set_enabled(true, exe) {
            Ok(()) => log::info!("autostart: updated from {current} to {}", exe.display()),
            Err(e) => log::warn!("autostart: cannot update: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes the real Run entry, so it only runs when asked:
    /// `cargo test -p knob-win -- --ignored`. Puts back whatever was there.
    #[test]
    #[ignore]
    fn turns_on_and_off() {
        let before = registered_command();
        let exe = Path::new(r"C:\Program Files\AK820 Knob\ak820-custom-knob.exe");

        set_enabled(true, exe).unwrap();
        assert_eq!(registered_command().as_deref(), Some(&*command_for(exe)));
        assert!(is_enabled());

        let moved = Path::new(r"D:\Apps\knob\ak820-custom-knob.exe");
        repair(moved);
        assert_eq!(registered_command(), Some(command_for(moved)));

        set_enabled(false, exe).unwrap();
        assert!(!is_enabled());
        set_enabled(false, exe).unwrap(); // already off: still fine

        if let Some(command) = before {
            let path = command.trim_matches('"');
            set_enabled(true, Path::new(path)).unwrap();
        }
    }
}
