//! Human-readable names for virtual-key codes, HID usages and device paths.

pub fn vk_name(vk: u32) -> String {
    let name = match vk {
        0x08 => "BACKSPACE",
        0x09 => "TAB",
        0x0D => "ENTER",
        0x1B => "ESC",
        0x20 => "SPACE",
        0x10 => "SHIFT",
        0x11 => "CTRL",
        0x12 => "ALT",
        0x5B => "LWIN",
        0x5C => "RWIN",
        0xA0 => "LSHIFT",
        0xA1 => "RSHIFT",
        0xA2 => "LCTRL",
        0xA3 => "RCTRL",
        0xA4 => "LALT",
        0xA5 => "RALT",
        0xAD => "VOLUME_MUTE",
        0xAE => "VOLUME_DOWN",
        0xAF => "VOLUME_UP",
        0xB0 => "MEDIA_NEXT",
        0xB1 => "MEDIA_PREV",
        0xB2 => "MEDIA_STOP",
        0xB3 => "MEDIA_PLAY_PAUSE",
        0xB4 => "LAUNCH_MAIL",
        0xB5 => "LAUNCH_MEDIA",
        0xB6 => "LAUNCH_APP1",
        0xB7 => "LAUNCH_APP2",
        0xA6 => "BROWSER_BACK",
        0xA7 => "BROWSER_FORWARD",
        0xA8 => "BROWSER_REFRESH",
        0xAC => "BROWSER_HOME",
        0x70..=0x87 => return format!("F{}", vk - 0x6F),
        0x30..=0x39 | 0x41..=0x5A => return (vk as u8 as char).to_string(),
        _ => "?",
    };
    name.to_string()
}

/// Names for the HID Consumer page (0x0C) usages these keyboards commonly send.
pub fn consumer_usage_name(usage: u16) -> &'static str {
    match usage {
        0x00 => "(release)",
        0x6F => "BRIGHTNESS_UP",
        0x70 => "BRIGHTNESS_DOWN",
        0xB5 => "SCAN_NEXT",
        0xB6 => "SCAN_PREV",
        0xB7 => "STOP",
        0xCD => "PLAY_PAUSE",
        0xE2 => "MUTE",
        0xE9 => "VOLUME_UP",
        0xEA => "VOLUME_DOWN",
        0x183 => "MEDIA_SELECT",
        0x18A => "MAIL",
        0x192 => "CALCULATOR",
        0x194 => "MY_COMPUTER",
        0x221 => "WWW_SEARCH",
        0x223 => "WWW_HOME",
        0x224 => "WWW_BACK",
        0x225 => "WWW_FORWARD",
        0x227 => "WWW_REFRESH",
        0x22A => "WWW_FAVORITES",
        _ => "?",
    }
}

pub fn usage_page_name(page: u16, usage: u16) -> &'static str {
    match (page, usage) {
        (0x01, 0x02) => "Mouse",
        (0x01, 0x06) => "Keyboard",
        (0x01, 0x80) => "System Control",
        (0x0C, 0x01) => "Consumer Control",
        (p, _) if p >= 0xFF00 => "Vendor-defined",
        _ => "",
    }
}

/// Turns `\\?\HID#VID_3151&PID_4015&MI_02&Col01#8&abc#{guid}` into
/// `VID_3151&PID_4015&MI_02&Col01`. Falls back to the full path.
pub fn short_device_name(path: &str) -> String {
    path.split('#')
        .find(|seg| seg.to_ascii_uppercase().contains("VID_"))
        .map(|seg| seg.to_ascii_uppercase())
        .unwrap_or_else(|| path.to_string())
}

pub fn hex_bytes(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
