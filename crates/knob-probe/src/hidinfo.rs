//! `knob-probe hid-info`: describes every HID interface of the keyboard so we
//! can plan how to talk to its vendor interfaces (RGB lighting).
//!
//! For each interface it prints the report descriptor and the reports it
//! declares. On vendor-defined interfaces it also reads every declared
//! feature report once (GetFeature). It never sends anything to the
//! keyboard: no output reports, no SetFeature. Input reports are not read
//! here; the normal probe logs them as they arrive.

use std::fmt::Write as _;
use std::path::Path;

use hidapi::{DeviceInfo, HidApi, HidDevice, MAX_REPORT_DESCRIPTOR_SIZE};

use crate::descriptor::{self, Kind};
use crate::names::{short_device_name, usage_page_name};

/// Sonix. The 2.4 GHz dongle is PID FEFC; wired and Bluetooth PIDs are not
/// known yet, so every PID of this vendor is included.
const VID_SONIX: u16 = 0x0C45;

pub fn run(log_dir: &Path, stamp: u64) {
    let mut out = String::new();
    report(&mut out);
    print!("{out}");

    let path = log_dir.join(format!("hid-info-{stamp}.txt"));
    match std::fs::write(&path, &out) {
        Ok(()) => println!("\nSaved to {}", path.display()),
        Err(e) => eprintln!("\ncannot write {}: {e}", path.display()),
    }
}

fn report(out: &mut String) {
    let api = match HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            let _ = writeln!(out, "hidapi init failed: {e}");
            return;
        }
    };

    let mut devices: Vec<&DeviceInfo> = api
        .device_list()
        .filter(|d| d.vendor_id() == VID_SONIX)
        .collect();
    devices.sort_by_key(|d| {
        (
            d.product_id(),
            d.interface_number(),
            d.usage_page(),
            d.usage(),
        )
    });

    if devices.is_empty() {
        let _ = writeln!(
            out,
            "No HID interface with VID_{VID_SONIX:04X} found. Is the keyboard connected?"
        );
        return;
    }
    let _ = writeln!(
        out,
        "{} HID interfaces with VID_{VID_SONIX:04X}:",
        devices.len()
    );

    for d in devices {
        let _ = writeln!(
            out,
            "\n=== VID_{:04X} PID_{:04X} if={} page=0x{:04X} usage=0x{:04X} {} '{} {}' release=0x{:04X}\n    {}",
            d.vendor_id(),
            d.product_id(),
            d.interface_number(),
            d.usage_page(),
            d.usage(),
            usage_page_name(d.usage_page(), d.usage()),
            d.manufacturer_string().unwrap_or(""),
            d.product_string().unwrap_or(""),
            d.release_number(),
            short_device_name(&d.path().to_string_lossy()),
        );
        match d.open_device(&api) {
            Ok(device) => describe(out, &device, d.usage_page() >= 0xFF00),
            Err(e) => {
                let _ = writeln!(out, "    cannot open: {e}");
            }
        }
    }
}

fn describe(out: &mut String, device: &HidDevice, vendor: bool) {
    let mut buf = vec![0u8; MAX_REPORT_DESCRIPTOR_SIZE];
    let desc = match device.get_report_descriptor(&mut buf) {
        Ok(n) => &buf[..n],
        Err(e) => {
            let _ = writeln!(out, "    report descriptor unavailable: {e}");
            return;
        }
    };
    let _ = writeln!(
        out,
        "    report descriptor ({} bytes, rebuilt by Windows):",
        desc.len()
    );
    for chunk in desc.chunks(16) {
        let _ = writeln!(out, "      {}", hex(chunk));
    }

    let reports = match descriptor::parse(desc) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(out, "    cannot parse descriptor: {e}");
            return;
        }
    };
    if reports.is_empty() {
        let _ = writeln!(out, "    declares no reports");
    }
    for r in &reports {
        let _ = writeln!(
            out,
            "    {:<7} report id 0x{:02X}: {} bytes on the wire",
            r.kind.name(),
            r.id,
            r.wire_len()
        );
    }

    if !vendor {
        return;
    }
    for r in reports.iter().filter(|r| r.kind == Kind::Feature) {
        let mut data = vec![0u8; r.wire_len()];
        data[0] = r.id;
        let _ = match device.get_feature_report(&mut data) {
            Ok(n) => writeln!(
                out,
                "    GetFeature 0x{:02X} -> {} bytes: {}",
                r.id,
                n,
                hex(&data[..n.min(data.len())])
            ),
            Err(e) => writeln!(out, "    GetFeature 0x{:02X} failed: {e}", r.id),
        };
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
