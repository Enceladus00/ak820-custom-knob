//! Opens apps, files, folders and web addresses the way double-clicking them
//! in Explorer would.

use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

/// Opens `target` with `args`, on a thread of its own so a slow start never
/// holds up the knob. `on_error` runs if Windows can't open it.
pub fn open(target: &str, args: &str, on_error: fn(&str)) {
    let target = target.trim().to_string();
    let args = args.trim().to_string();
    let spawned = std::thread::Builder::new()
        .name("open".into())
        .spawn(move || {
            if !open_now(&target, &args) {
                log::warn!("open: Windows couldn't open '{target}'");
                on_error(&target);
            }
        });
    if let Err(e) = spawned {
        log::warn!("open: {e}");
    }
}

/// Opens `target` and returns whether Windows accepted it. Windows shows no
/// message of its own when it can't; the caller reports it.
pub fn open_now(target: &str, args: &str) -> bool {
    let file = HSTRING::from(target);
    let params = HSTRING::from(args);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: if args.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(params.as_ptr())
        },
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe {
        // The shell may use COM to open some targets, and wants this apartment.
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let opened = ShellExecuteExW(&mut info).is_ok();
        if com.is_ok() {
            CoUninitialize();
        }
        opened
    }
}
