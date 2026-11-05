//! Per-app volume through Windows audio sessions (the same thing the Volume
//! Mixer changes).

use windows::Win32::Media::Audio::{
    IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume,
    MMDeviceEnumerator, eMultimedia, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::core::{Interface, Result};

use crate::process;

pub struct Audio {
    enumerator: IMMDeviceEnumerator,
}

impl Audio {
    /// Initialises COM on the calling thread; use the value on that thread only.
    pub fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let enumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            Ok(Self { enumerator })
        }
    }

    /// Changes the volume of every audio session belonging to `exe` (apps like
    /// browsers play audio from several processes) by `delta` (-1.0 ..= 1.0).
    /// Returns the new level, or `None` if the app has no audio session.
    pub fn change_app_volume(&self, exe: &str, delta: f32) -> Result<Option<f32>> {
        unsafe {
            let device = self
                .enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)?;
            let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            let sessions = manager.GetSessionEnumerator()?;

            let mut level = None;
            for i in 0..sessions.GetCount()? {
                let control = sessions.GetSession(i)?;
                let Ok(control2) = control.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let Ok(pid) = control2.GetProcessId() else {
                    continue;
                };
                if pid == 0
                    || !process::exe_name(pid).is_some_and(|name| name.eq_ignore_ascii_case(exe))
                {
                    continue;
                }
                let volume: ISimpleAudioVolume = control.cast()?;
                // All of the app's sessions follow the first one, so they stay in step.
                let new_level = match level {
                    Some(l) => l,
                    None => (volume.GetMasterVolume()? + delta).clamp(0.0, 1.0),
                };
                volume.SetMasterVolume(new_level, std::ptr::null())?;
                level = Some(new_level);
            }
            Ok(level)
        }
    }
}
