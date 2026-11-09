//! `knob-probe rgb-read` and `knob-probe rgb-set`: talk to the keyboard's
//! lighting block through the `knob-rgb` protocol.
//!
//! `rgb-read` only asks for information. `rgb-set` prints what it would send
//! and changes nothing unless `--apply` is given. Both stop at the first thing
//! that doesn't look as expected.

use std::fmt::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};
use knob_rgb::{
    Command, DeviceInfo, FRAME_LEN, INFO_LEN, LED_LEN, LedState, PID_DONGLE, USAGE, USAGE_PAGE,
    VID, WIRE_LEN, parse_reply, read_request, to_wire,
};

const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);
const POLL_INTERVAL: Duration = Duration::from_secs(10);
const CHUNK: usize = FRAME_LEN - 8;

/// With `--wait <seconds>`, keeps asking until the keyboard answers, since a
/// sleeping keyboard stays silent until a key is pressed.
pub fn read(log_dir: &Path, stamp: u64, args: &[String]) {
    let wait = match args {
        [flag, secs] if flag == "--wait" => secs.parse().unwrap_or(0),
        _ => 0,
    };
    let deadline = Instant::now() + Duration::from_secs(wait);
    let mut out = String::new();
    let result = loop {
        out.clear();
        let result = report(&mut out);
        if result.is_ok() || Instant::now() >= deadline {
            break result;
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    if let Err(e) = result {
        let _ = writeln!(out, "stopped: {e}");
    }
    finish(log_dir, "rgb-read", stamp, &out);
}

pub fn set(log_dir: &Path, stamp: u64, args: &[String]) {
    let mut out = String::new();
    if let Err(e) = change(&mut out, args) {
        let _ = writeln!(out, "stopped: {e}");
    }
    finish(log_dir, "rgb-set", stamp, &out);
}

fn finish(log_dir: &Path, name: &str, stamp: u64, out: &str) {
    print!("{out}");
    let path = log_dir.join(format!("{name}-{stamp}.txt"));
    match std::fs::write(&path, out) {
        Ok(()) => println!("\nSaved to {}", path.display()),
        Err(e) => eprintln!("\ncannot write {}: {e}", path.display()),
    }
}

fn open() -> Result<HidDevice, String> {
    let api = HidApi::new().map_err(|e| format!("hidapi init failed: {e}"))?;
    let info = api
        .device_list()
        .find(|d| {
            d.vendor_id() == VID
                && d.product_id() == PID_DONGLE
                && d.usage_page() == USAGE_PAGE
                && d.usage() == USAGE
        })
        .ok_or("the 2.4 GHz receiver's lighting interface was not found")?;
    info.open_device(&api)
        .map_err(|e| format!("cannot open the receiver: {e}"))
}

/// Sends the frames of one transfer, waiting for each acknowledgement, and
/// returns the payload bytes the keyboard answered with.
fn transfer(
    device: &HidDevice,
    out: &mut String,
    command: Command,
    frames: &[[u8; FRAME_LEN]],
) -> Result<Vec<u8>, String> {
    let mut block = Vec::new();
    for frame in frames {
        let _ = writeln!(out, "  -> {}", hex(frame));
        device
            .write(&to_wire(frame))
            .map_err(|e| format!("write failed: {e}"))?;

        let deadline = Instant::now() + REPLY_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(format!(
                    "no reply to command 0x{:02X}; is the keyboard awake?",
                    command.code()
                ));
            }
            let mut buf = [0u8; WIRE_LEN];
            let n = device
                .read_timeout(&mut buf, left.as_millis() as i32)
                .map_err(|e| format!("read failed: {e}"))?;
            if n == 0 {
                continue;
            }
            let Some(reply) = parse_reply(&buf[..n]) else {
                let _ = writeln!(out, "  .. ignored {}", hex(&buf[..n]));
                continue;
            };
            if reply.command != command.code() {
                let _ = writeln!(out, "  .. ignored {}", hex(&buf[..n]));
                continue;
            }
            let _ = writeln!(out, "  <- {}", hex(&buf[..n]));
            block.extend_from_slice(&reply.payload[..reply.payload.len().min(CHUNK)]);
            break;
        }
    }
    Ok(block)
}

fn read_info(device: &HidDevice, out: &mut String) -> Result<DeviceInfo, String> {
    let frames = read_request(Command::ReadInfo, INFO_LEN);
    let block = transfer(device, out, Command::ReadInfo, &frames)?;
    let _ = writeln!(out, "info block: {}", hex(&block));
    DeviceInfo::parse(&block).ok_or_else(|| "the info block is too short".to_string())
}

fn read_led(device: &HidDevice, out: &mut String) -> Result<LedState, String> {
    let frames = read_request(Command::ReadLed, LED_LEN);
    let block = transfer(device, out, Command::ReadLed, &frames)?;
    let _ = writeln!(out, "led block:  {}", hex(&block));
    LedState::parse(&block).map_err(|e| format!("unexpected lighting block: {e:?}"))
}

fn describe(out: &mut String, led: &LedState) {
    let [r, g, b] = led.color();
    let _ = writeln!(
        out,
        "lighting: effect=0x{:02X} color=#{r:02X}{g:02X}{b:02X} colorful={} brightness={} speed={} direction={}",
        led.effect(),
        led.colorful(),
        led.brightness(),
        led.speed(),
        led.direction(),
    );
}

fn report(out: &mut String) -> Result<(), String> {
    let device = open()?;
    let info = read_info(&device, out)?;
    let _ = writeln!(
        out,
        "firmware: {}.{:02}  battery: {}",
        info.firmware / 100,
        info.firmware % 100,
        info.battery
            .map_or("no reading".to_string(), |b| format!("{b}%")),
    );
    let led = read_led(&device, out)?;
    describe(out, &led);
    Ok(())
}

/// The requested edits, parsed before the keyboard is touched.
#[derive(Default)]
struct Edits {
    effect: Option<u8>,
    color: Option<[u8; 3]>,
    brightness: Option<u8>,
    speed: Option<u8>,
    restore: Option<Vec<u8>>,
    apply: bool,
}

fn parse_args(args: &[String]) -> Result<Edits, String> {
    let mut edits = Edits::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--apply" {
            edits.apply = true;
            continue;
        }
        let value = it
            .next()
            .ok_or_else(|| format!("{flag} needs a value"))?
            .as_str();
        match flag.as_str() {
            "--effect" => edits.effect = Some(number(value)?),
            "--brightness" => edits.brightness = Some(number(value)?),
            "--speed" => edits.speed = Some(number(value)?),
            "--color" => {
                let bytes = parse_hex(value)?;
                edits.color = Some(
                    bytes
                        .try_into()
                        .map_err(|_| "--color needs RRGGBB".to_string())?,
                );
            }
            "--restore" => edits.restore = Some(parse_hex(value)?),
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(edits)
}

fn number(value: &str) -> Result<u8, String> {
    let parsed = match value.strip_prefix("0x") {
        Some(hex) => u8::from_str_radix(hex, 16),
        None => value.parse(),
    };
    parsed.map_err(|_| format!("bad number: {value}"))
}

fn parse_hex(value: &str) -> Result<Vec<u8>, String> {
    let digits = value.trim_start_matches('#');
    if !digits.len().is_multiple_of(2) {
        return Err(format!("bad hex: {value}"));
    }
    (0..digits.len())
        .step_by(2)
        .map(|i| {
            digits
                .get(i..i + 2)
                .and_then(|d| u8::from_str_radix(d, 16).ok())
                .ok_or_else(|| format!("bad hex: {value}"))
        })
        .collect()
}

fn change(out: &mut String, args: &[String]) -> Result<(), String> {
    let edits = parse_args(args)?;
    let device = open()?;
    let before = read_led(&device, out)?;
    let _ = writeln!(out, "backup (for --restore): {}", plain_hex(before.raw()));
    describe(out, &before);

    let mut after = match &edits.restore {
        Some(bytes) => {
            LedState::parse(bytes).map_err(|e| format!("cannot restore that block: {e:?}"))?
        }
        None => before.normalized(),
    };
    if let Some(effect) = edits.effect {
        after.set_effect(effect).map_err(|e| format!("{e:?}"))?;
    }
    if let Some(color) = edits.color {
        after.set_color(color);
    }
    if let Some(level) = edits.brightness {
        after.set_brightness(level).map_err(|e| format!("{e:?}"))?;
    }
    if let Some(level) = edits.speed {
        after.set_speed(level).map_err(|e| format!("{e:?}"))?;
    }

    let frames = after.write_request();
    if !edits.apply {
        let _ = writeln!(out, "dry run, nothing sent. Would write:");
        for frame in &frames {
            let _ = writeln!(out, "  -> {}", hex(frame));
        }
        return Ok(());
    }

    transfer(&device, out, Command::WriteLed, &frames)?;
    let check = read_led(&device, out)?;
    describe(out, &check);
    if check.same_settings(&after) {
        let _ = writeln!(out, "read back: matches");
        Ok(())
    } else {
        Err(format!(
            "read back differs; to undo, run rgb-set --restore {} --apply",
            plain_hex(before.raw())
        ))
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn plain_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}
