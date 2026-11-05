//! Lists every HID interface and reads raw reports from each one Windows
//! lets us open. Keyboard and mouse collections are owned exclusively by
//! Windows, so those are listed but not read.

use std::time::{Duration, Instant};

use hidapi::{DeviceInfo, HidApi};

use crate::event::{Event, now_ms, send};
use crate::names::{short_device_name, usage_page_name};

/// A device sending more reports than this within `CHATTY_WINDOW` is muted,
/// so streaming devices (sensors, some headsets) don't flood the log.
const CHATTY_LIMIT: u32 = 100;
const CHATTY_WINDOW: Duration = Duration::from_secs(2);

pub fn enumerate_and_listen() {
    let api = match HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            send(Event::Info(format!("hidapi init failed: {e}")));
            return;
        }
    };

    let mut devices: Vec<DeviceInfo> = api.device_list().cloned().collect();
    devices.sort_by_key(|d| {
        (
            d.vendor_id(),
            d.product_id(),
            d.interface_number(),
            d.usage_page(),
            d.usage(),
        )
    });

    send(Event::Info(format!(
        "{} HID interfaces found:",
        devices.len()
    )));
    for d in &devices {
        send(Event::Info(format!(
            "  VID_{:04X} PID_{:04X} if={:<2} page=0x{:04X} usage=0x{:04X} {:<16} '{} {}'  {}",
            d.vendor_id(),
            d.product_id(),
            d.interface_number(),
            d.usage_page(),
            d.usage(),
            usage_page_name(d.usage_page(), d.usage()),
            d.manufacturer_string().unwrap_or(""),
            d.product_string().unwrap_or(""),
            short_device_name(&d.path().to_string_lossy()),
        )));
    }

    let mut opened = 0;
    for d in devices {
        // Windows owns keyboard/mouse collections exclusively; reads would fail.
        if d.usage_page() == 0x01 && matches!(d.usage(), 0x02 | 0x06) {
            continue;
        }
        let name = short_device_name(&d.path().to_string_lossy());
        let label = format!("{name} page=0x{:04X}", d.usage_page());
        match d.open_device(&api) {
            Ok(device) => {
                opened += 1;
                std::thread::spawn(move || read_loop(device, label));
            }
            Err(e) => send(Event::Info(format!("cannot open {label}: {e}"))),
        }
    }
    send(Event::Info(format!(
        "reading raw reports from {opened} HID interfaces"
    )));
}

fn read_loop(device: hidapi::HidDevice, label: String) {
    let mut buf = [0u8; 512];
    let mut window_start = Instant::now();
    let mut count = 0u32;
    loop {
        match device.read_timeout(&mut buf, 500) {
            Ok(0) => continue,
            Ok(n) => {
                if window_start.elapsed() > CHATTY_WINDOW {
                    window_start = Instant::now();
                    count = 0;
                }
                count += 1;
                if count > CHATTY_LIMIT {
                    send(Event::Info(format!(
                        "muted {label}: too many reports (streaming device)"
                    )));
                    return;
                }
                send(Event::HidReport {
                    t: now_ms(),
                    device: label.clone(),
                    data: buf[..n].to_vec(),
                });
            }
            Err(e) => {
                send(Event::Info(format!("stopped reading {label}: {e}")));
                return;
            }
        }
    }
}
