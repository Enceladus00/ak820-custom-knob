//! Sound: per-app volume through Windows audio sessions (the same thing the
//! Volume Mixer changes), the microphone, and the default sound output.

use std::ffi::c_void;

use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, ERole, IAudioSessionControl2, IAudioSessionManager2, IMMDevice,
    IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eCapture, eCommunications,
    eConsole, eMultimedia, eRender,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ,
};
use windows::core::{GUID, HSTRING, IUnknown, Interface, PCWSTR, Result};

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

    /// The default recording devices: the one for everything else first, then
    /// the one for calls if it is a different device.
    fn microphones(&self) -> Result<Vec<IAudioEndpointVolume>> {
        let mut seen = Vec::new();
        let mut volumes = Vec::new();
        for role in [eConsole, eCommunications] {
            let Ok(device) = (unsafe { self.enumerator.GetDefaultAudioEndpoint(eCapture, role) })
            else {
                continue;
            };
            let id = device_id(&device)?;
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            volumes.push(unsafe { device.Activate(CLSCTX_ALL, None)? });
        }
        Ok(volumes)
    }

    /// Changes the microphone volume by `delta` (-1.0 ..= 1.0). Returns the new
    /// level, or `None` when there is no microphone.
    pub fn change_mic_volume(&self, delta: f32) -> Result<Option<f32>> {
        let mics = self.microphones()?;
        let Some(first) = mics.first() else {
            return Ok(None);
        };
        unsafe {
            let level = (first.GetMasterVolumeLevelScalar()? + delta).clamp(0.0, 1.0);
            for mic in &mics {
                mic.SetMasterVolumeLevelScalar(level, std::ptr::null())?;
            }
            Ok(Some(level))
        }
    }

    /// Sets the microphone volume (0.0 ..= 1.0).
    pub fn set_mic_volume(&self, level: f32) -> Result<()> {
        for mic in self.microphones()? {
            unsafe { mic.SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), std::ptr::null())? };
        }
        Ok(())
    }

    /// The microphone's volume and whether it is muted, or `None` without one.
    pub fn mic_state(&self) -> Result<Option<(f32, bool)>> {
        let mics = self.microphones()?;
        let Some(first) = mics.first() else {
            return Ok(None);
        };
        unsafe {
            Ok(Some((
                first.GetMasterVolumeLevelScalar()?,
                first.GetMute()?.as_bool(),
            )))
        }
    }

    /// Mutes the microphone, or unmutes it if it was muted. The device for
    /// calls follows, so Teams, Discord and the like hear the same. Returns
    /// whether it is now muted, or `None` when there is no microphone.
    pub fn toggle_mic_mute(&self) -> Result<Option<bool>> {
        let mics = self.microphones()?;
        let Some(first) = mics.first() else {
            return Ok(None);
        };
        unsafe {
            let muted = !first.GetMute()?.as_bool();
            for mic in &mics {
                mic.SetMute(muted, std::ptr::null())?;
            }
            Ok(Some(muted))
        }
    }

    /// The name of the default sound output.
    pub fn output_name(&self) -> Result<String> {
        unsafe {
            let device = self.enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            device_name(&device)
        }
    }

    /// Makes the next (`step` 1) or previous (-1) sound output the default, for
    /// music, games and calls alike. Returns its name, or `None` when there is
    /// only one output to choose from.
    pub fn switch_output(&self, step: i32) -> Result<Option<String>> {
        unsafe {
            let outputs = self
                .enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            let count = outputs.GetCount()?;
            if count < 2 {
                return Ok(None);
            }
            let current = self
                .enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .and_then(|d| device_id(&d))
                .unwrap_or_default();
            let mut ids = Vec::with_capacity(count as usize);
            for i in 0..count {
                ids.push(device_id(&outputs.Item(i)?)?);
            }
            let at = ids.iter().position(|id| *id == current).unwrap_or(0) as i32;
            let next = (at + step).rem_euclid(count as i32) as u32;
            set_default_output(&ids[next as usize])?;
            Ok(Some(device_name(&outputs.Item(next)?)?))
        }
    }

    /// Makes the output with this name the default. For restoring a choice.
    pub fn set_output_by_name(&self, name: &str) -> Result<bool> {
        unsafe {
            let outputs = self
                .enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            for i in 0..outputs.GetCount()? {
                let device = outputs.Item(i)?;
                if device_name(&device)? == name {
                    set_default_output(&device_id(&device)?)?;
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

fn device_id(device: &IMMDevice) -> Result<String> {
    unsafe {
        let raw = device.GetId()?;
        let id = raw.to_string().unwrap_or_default();
        CoTaskMemFree(Some(raw.0 as *const _));
        Ok(id)
    }
}

/// `PKEY_Device_FriendlyName`, such as "Speakers (Realtek Audio)".
const FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

fn device_name(device: &IMMDevice) -> Result<String> {
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ)?;
        let value = store.GetValue(&FRIENDLY_NAME)?;
        let raw = PropVariantToStringAlloc(&value)?;
        let name = raw.to_string().unwrap_or_default();
        CoTaskMemFree(Some(raw.0 as *const _));
        Ok(name)
    }
}

/// Windows has no public call to change the default sound device. This is the
/// interface the Sound control panel uses, as SoundSwitch and EarTrumpet do:
/// `IPolicyConfig`, whose `SetDefaultEndpoint` is the 14th entry in its table.
const POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
const SET_DEFAULT_ENDPOINT_SLOT: usize = 13;

fn set_default_output(id: &str) -> Result<()> {
    type SetDefaultEndpoint =
        unsafe extern "system" fn(*mut c_void, PCWSTR, ERole) -> windows::core::HRESULT;
    let wide = HSTRING::from(id);
    unsafe {
        let client: IUnknown = CoCreateInstance(&POLICY_CONFIG_CLIENT, None, CLSCTX_ALL)?;
        let mut raw = std::ptr::null_mut();
        client.query(&IID_POLICY_CONFIG, &mut raw).ok()?;
        // Owns the reference `query` added, and releases it when dropped.
        let policy = IUnknown::from_raw(raw);
        let table = *(raw as *const *const usize);
        let set: SetDefaultEndpoint = std::mem::transmute(*table.add(SET_DEFAULT_ENDPOINT_SLOT));
        for role in [eConsole, eMultimedia, eCommunications] {
            set(raw, PCWSTR(wide.as_ptr()), role).ok()?;
        }
        drop(policy);
    }
    Ok(())
}
