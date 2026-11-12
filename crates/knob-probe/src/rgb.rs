//! `knob-probe rgb-read`, `rgb-set` and `rgb-bench`: talk to the keyboard's
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
    Command, DeviceInfo, EffectDef, FRAME_LEN, INFO_LEN, LED_LEN, LIVE_STATUS_INDEX, LedState,
    LiveInfo, PID_DONGLE, Pattern, ReadBlock, USAGE, USAGE_PAGE, VID, WIRE_LEN, frames,
    parse_reply, read_request, to_wire,
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

/// Default and largest number of writes `rgb-bench` sends. Small on purpose:
/// until it's known whether a write reaches flash, every write may count.
const BENCH_WRITES: usize = 30;
const BENCH_MAX: usize = 200;

/// Times colour-only writes, to see how often sound-reactive lighting could
/// update the keyboard, then restores the original block.
pub fn bench(log_dir: &Path, stamp: u64, args: &[String]) {
    let mut out = String::new();
    if let Err(e) = run_bench(&mut out, args) {
        let _ = writeln!(out, "stopped: {e}");
    }
    finish(log_dir, "rgb-bench", stamp, &out);
}

fn run_bench(out: &mut String, args: &[String]) -> Result<(), String> {
    if let [flag] = args
        && flag == "--reads"
    {
        return bench_reads(out);
    }
    let count = match args {
        [] => BENCH_WRITES,
        [flag, n] if flag == "--count" => n
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=BENCH_MAX).contains(n))
            .ok_or_else(|| format!("--count needs 1 to {BENCH_MAX}"))?,
        _ => return Err("usage: rgb-bench [--count N]".into()),
    };
    let device = open()?;
    let before = read_led(&device, out)?;
    let _ = writeln!(
        out,
        "backup (for rgb-set --restore): {}",
        plain_hex(before.raw())
    );
    describe(out, &before);

    const COLORS: [[u8; 3]; 3] = [[255, 0, 0], [255, 160, 0], [255, 255, 0]];
    let mut times = Vec::with_capacity(count);
    let mut log = String::new();
    for i in 0..count {
        let mut led = before.normalized();
        led.set_color(COLORS[i % COLORS.len()]);
        let started = Instant::now();
        log.clear();
        transfer(&device, &mut log, Command::WriteLed, &led.write_request())?;
        times.push(started.elapsed());
    }

    let restore = transfer(
        &device,
        out,
        Command::WriteLed,
        &before.normalized().write_request(),
    );
    let check = read_led(&device, out);

    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let total: Duration = times.iter().sum();
    let avg = total / count as u32;
    let _ = writeln!(
        out,
        "{count} writes: min {:.1} ms, median {:.1} ms, avg {:.1} ms, max {:.1} ms (about {:.0} writes/s)",
        ms(times[0]),
        ms(times[count / 2]),
        ms(avg),
        ms(times[count - 1]),
        1000.0 / ms(avg).max(0.001),
    );
    restore?;
    let check = check?;
    if check.same_settings(&before.normalized()) {
        let _ = writeln!(out, "restored: matches the backup");
    } else {
        let _ = writeln!(
            out,
            "restore differs; run rgb-set --restore {} --apply",
            plain_hex(before.raw())
        );
    }
    let _ = writeln!(
        out,
        "
Persistence test (does a write survive power-off?):
           1. rgb-set --brightness <a level other than {}> --apply
           2. switch the keyboard off and on, press a key
           3. rgb-read --wait 60
           If the brightness went back to {}, writes live in RAM and streaming is safe.
           If the new level stayed, writes are likely stored in flash.
           4. rgb-set --restore {} --apply",
        before.brightness(),
        before.brightness(),
        plain_hex(before.raw())
    );
    Ok(())
}

/// Default and longest run of `rgb-stream`, in seconds.
const STREAM_SECONDS: u64 = 5;
const STREAM_MAX_SECONDS: u64 = 30;
/// Time between frames, as in the vendor driver's music mode.
const STREAM_FRAME_INTERVAL: Duration = Duration::from_millis(50);
/// How long to keep listening after the last frame.
const STREAM_LISTEN_AFTER: Duration = Duration::from_secs(3);

/// Streams a moving rainbow with the live per-key command (`0x35`), the way
/// the vendor driver's music mode does, logs whatever the keyboard sends
/// back, and checks the stored lighting block afterwards.
pub fn stream(log_dir: &Path, stamp: u64, args: &[String]) {
    let mut out = String::new();
    if let Err(e) = run_stream(&mut out, args) {
        let _ = writeln!(out, "stopped: {e}");
    }
    finish(log_dir, "rgb-stream", stamp, &out);
}

fn run_stream(out: &mut String, args: &[String]) -> Result<(), String> {
    if let [flag, rest @ ..] = args
        && flag == "--wired"
    {
        return run_wired_stream(out, rest);
    }
    let seconds = match args {
        [] => STREAM_SECONDS,
        [flag, n] if flag == "--seconds" => n
            .parse::<u64>()
            .ok()
            .filter(|n| (1..=STREAM_MAX_SECONDS).contains(n))
            .ok_or_else(|| format!("--seconds needs 1 to {STREAM_MAX_SECONDS}"))?,
        _ => return Err("usage: rgb-stream [--seconds N]".into()),
    };
    let device = open()?;
    let before = read_led(&device, out)?;
    describe(out, &before);

    type Seen = std::collections::BTreeMap<u8, (usize, String)>;
    let mut replies = Seen::new();
    let drain = |device: &HidDevice, replies: &mut Seen| {
        let mut buf = [0u8; WIRE_LEN];
        while let Ok(n) = device.read_timeout(&mut buf, 0) {
            if n == 0 {
                break;
            }
            let start = usize::from(buf[0] == 0 && n > 1);
            let key = buf.get(start + 1).copied().unwrap_or(0);
            let entry = replies.entry(key).or_insert((0, hex(&buf[..n])));
            entry.0 += 1;
        }
    };

    let started = Instant::now();
    let deadline = started + Duration::from_secs(seconds);
    let mut writes = Vec::new();
    let mut frames = 0u32;
    let mut next = started;
    while Instant::now() < deadline {
        let shift = started.elapsed().as_secs_f32() * 180.0;
        let colors = std::array::from_fn(|i| {
            let (_, _, col) = knob_rgb::KEYS[i];
            hue(f32::from(col) * 24.0 + shift)
        });
        for (i, packet) in knob_rgb::stream_packets(&colors).iter().enumerate() {
            if i > 0 {
                std::thread::sleep(Duration::from_millis(knob_rgb::STREAM_PACKET_GAP_MS));
            }
            let t = Instant::now();
            device
                .write(&to_wire(packet))
                .map_err(|e| format!("write failed: {e}"))?;
            writes.push(t.elapsed());
            drain(&device, &mut replies);
        }
        frames += 1;
        next += STREAM_FRAME_INTERVAL;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
    let streamed = started.elapsed();
    let listen_until = Instant::now() + STREAM_LISTEN_AFTER;
    while Instant::now() < listen_until {
        drain(&device, &mut replies);
        std::thread::sleep(Duration::from_millis(20));
    }

    writes.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let _ = writeln!(
        out,
        "{frames} frames in {:.1} s ({:.1} per second), {} packets; one packet write: median {:.2} ms, max {:.2} ms",
        streamed.as_secs_f64(),
        f64::from(frames) / streamed.as_secs_f64(),
        writes.len(),
        ms(writes[writes.len() / 2]),
        ms(writes[writes.len() - 1]),
    );
    if replies.is_empty() {
        let _ = writeln!(out, "the keyboard sent nothing back");
    }
    for (cmd, (count, first)) in &replies {
        let _ = writeln!(
            out,
            "received {count} x command 0x{cmd:02X}, first: {first}"
        );
    }

    let after = read_led(&device, out)?;
    describe(out, &after);
    if after.same_settings(&before) {
        let _ = writeln!(out, "stored lighting block unchanged");
    } else {
        let _ = writeln!(
            out,
            "stored lighting block CHANGED; restore with rgb-set --restore {} --apply",
            plain_hex(before.raw())
        );
    }
    Ok(())
}

/// How long a wired stream packet waits for its reply.
const WIRED_REPLY_TIMEOUT: Duration = Duration::from_millis(300);

/// `rgb-stream --wired`: the same rainbow over the USB cable, using the
/// driver's 64-byte `0x32` stream. Only streams; it reads and writes nothing
/// else.
fn run_wired_stream(out: &mut String, args: &[String]) -> Result<(), String> {
    let seconds = match args {
        [] => STREAM_SECONDS,
        [flag, n] if flag == "--seconds" => n
            .parse::<u64>()
            .ok()
            .filter(|n| (1..=STREAM_MAX_SECONDS).contains(n))
            .ok_or_else(|| format!("--seconds needs 1 to {STREAM_MAX_SECONDS}"))?,
        _ => return Err("usage: rgb-stream --wired [--seconds N]".into()),
    };
    let api = HidApi::new().map_err(|e| format!("hidapi init failed: {e}"))?;
    let device = api
        .device_list()
        .find(|d| {
            d.vendor_id() == VID
                && d.product_id() == knob_rgb::PID_WIRED
                && d.usage_page() == knob_rgb::WIRED_USAGE_PAGE
                && d.usage() == knob_rgb::WIRED_USAGE
        })
        .ok_or("the wired keyboard's vendor interface was not found; is the cable in and the keyboard in cable mode?")?
        .open_device(&api)
        .map_err(|e| format!("cannot open the keyboard: {e}"))?;

    let mut replies: std::collections::BTreeMap<u8, (usize, String)> = Default::default();
    let mut timeouts = 0usize;
    let mut round_trips = Vec::new();
    let started = Instant::now();
    let deadline = started + Duration::from_secs(seconds);
    let mut frames = 0u32;
    let mut next = started;
    while Instant::now() < deadline {
        let shift = started.elapsed().as_secs_f32() * 180.0;
        let colors = std::array::from_fn(|i| {
            let (_, _, col) = knob_rgb::KEYS[i];
            hue(f32::from(col) * 24.0 + shift)
        });
        for packet in knob_rgb::wired_stream_packets(&colors) {
            if frames == 0 && round_trips.is_empty() {
                let _ = writeln!(out, "  -> {}", hex(&packet));
            }
            let mut wire = [0u8; knob_rgb::WIRED_FRAME_LEN + 1];
            wire[1..].copy_from_slice(&packet);
            let sent = Instant::now();
            device
                .write(&wire)
                .map_err(|e| format!("write failed: {e}"))?;
            let mut buf = [0u8; knob_rgb::WIRED_FRAME_LEN + 1];
            loop {
                let left = WIRED_REPLY_TIMEOUT.saturating_sub(sent.elapsed());
                if left.is_zero() {
                    timeouts += 1;
                    break;
                }
                let n = device
                    .read_timeout(&mut buf, left.as_millis() as i32)
                    .map_err(|e| format!("read failed: {e}"))?;
                if n == 0 {
                    continue;
                }
                let start = usize::from(buf[0] == 0 && n > 1);
                let cmd = buf.get(start + 1).copied().unwrap_or(0);
                replies.entry(cmd).or_insert((0, hex(&buf[..n]))).0 += 1;
                if cmd == 0x37 {
                    continue;
                }
                round_trips.push(sent.elapsed());
                break;
            }
        }
        frames += 1;
        next += STREAM_FRAME_INTERVAL;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
    let streamed = started.elapsed();
    round_trips.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let _ = writeln!(
        out,
        "{frames} frames in {:.1} s ({:.1} per second), {timeouts} packets without a reply",
        streamed.as_secs_f64(),
        f64::from(frames) / streamed.as_secs_f64(),
    );
    if let (Some(first), Some(last)) = (round_trips.first(), round_trips.last()) {
        let _ = writeln!(
            out,
            "packet round trip: min {:.2} ms, median {:.2} ms, max {:.2} ms",
            ms(*first),
            ms(round_trips[round_trips.len() / 2]),
            ms(*last),
        );
    }
    for (cmd, (count, first)) in &replies {
        let _ = writeln!(
            out,
            "received {count} x command 0x{cmd:02X}, first: {first}"
        );
    }
    Ok(())
}

/// Fully saturated colour at `degrees` around the colour wheel.
fn hue(degrees: f32) -> [u8; 3] {
    let h = degrees.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u8 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    [r, g, b].map(|c: f32| (c * 255.0).round() as u8)
}

/// Times lighting-block reads. A read stores nothing, so its round trip is
/// the cost of the radio link alone, to compare with a write's.
fn bench_reads(out: &mut String) -> Result<(), String> {
    let device = open()?;
    let mut times = Vec::with_capacity(BENCH_WRITES);
    let mut scratch = String::new();
    for _ in 0..BENCH_WRITES {
        scratch.clear();
        let started = Instant::now();
        read_led(&device, &mut scratch)?;
        times.push(started.elapsed());
    }
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let _ = writeln!(
        out,
        "{} reads: min {:.1} ms, median {:.1} ms, max {:.1} ms",
        times.len(),
        ms(times[0]),
        ms(times[times.len() / 2]),
        ms(times[times.len() - 1]),
    );
    Ok(())
}

/// `rgb-identify`: asks whether the keyboard runs the live-lighting patch.
/// Stock firmware just echoes the request.
pub fn identify(log_dir: &Path, stamp: u64) {
    let mut out = String::new();
    let result = open().and_then(|device| probe_live(&device, &mut out));
    match result {
        Ok(Some(info)) => {
            let _ = writeln!(out, "live-lighting patch v{} is running", info.version);
        }
        Ok(None) => {
            let _ = writeln!(out, "stock firmware (no live-lighting patch)");
        }
        Err(e) => {
            let _ = writeln!(out, "stopped: {e}");
        }
    }
    finish(log_dir, "rgb-identify", stamp, &out);
}

fn probe_live(device: &HidDevice, out: &mut String) -> Result<Option<LiveInfo>, String> {
    let reply = transfer(
        device,
        out,
        Command::Identify,
        &frames(Command::Identify, &[]),
    )?;
    Ok(LiveInfo::parse(&reply))
}

/// `rgb-live`: shows lighting without storing it, on patched firmware only.
/// Takes the same --effect/--color/--brightness/--speed options as rgb-set,
/// starting from the current state, or --saved to go back to the stored
/// lighting. Nothing is written to flash either way.
pub fn live(log_dir: &Path, stamp: u64, args: &[String]) {
    let mut out = String::new();
    if let Err(e) = run_live(&mut out, args) {
        let _ = writeln!(out, "stopped: {e}");
    }
    finish(log_dir, "rgb-live", stamp, &out);
}

/// `rgb-live --bench N`: times N live commands against N plain reads on the
/// same link. A stored write costs ~31 ms of flash work; a RAM-only command
/// should cost about the same as a read.
fn live_bench(out: &mut String, device: &HidDevice, count: usize) -> Result<(), String> {
    let state = read_led(device, &mut String::new())?.normalized();
    let frames = state.live_request();
    let reads = read_request(Command::ReadLed, LED_LEN);
    let mut scratch = String::new();
    let mut time =
        |command: Command, frames: &[[u8; FRAME_LEN]]| -> Result<Vec<Duration>, String> {
            let mut times = Vec::with_capacity(count);
            for _ in 0..count {
                scratch.clear();
                let started = Instant::now();
                transfer(device, &mut scratch, command, frames)?;
                times.push(started.elapsed());
            }
            times.sort();
            Ok(times)
        };
    let live = time(Command::LiveLed, &frames)?;
    let read = time(Command::ReadLed, &reads)?;
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    for (name, t) in [("0x30 live", &live), ("0x13 read", &read)] {
        let _ = writeln!(
            out,
            "{name}: {} round trips, min {:.2} ms, median {:.2} ms, max {:.2} ms",
            t.len(),
            ms(t[0]),
            ms(t[t.len() / 2]),
            ms(t[t.len() - 1])
        );
    }
    Ok(())
}

fn run_live(out: &mut String, args: &[String]) -> Result<(), String> {
    if let [flag, n] = args
        && flag == "--bench"
    {
        let count: usize = n
            .parse()
            .ok()
            .filter(|n| (1..=500).contains(n))
            .ok_or("--bench needs 1 to 500")?;
        let device = open()?;
        if probe_live(&device, out)?.is_none() {
            return Err("this keyboard runs stock firmware; rgb-live needs the patch".into());
        }
        return live_bench(out, &device, count);
    }
    let saved = args.iter().any(|a| a == "--saved");
    let rest: Vec<String> = args.iter().filter(|a| *a != "--saved").cloned().collect();
    let edits = parse_args(&rest)?;
    let device = open()?;
    let Some(info) = probe_live(&device, out)? else {
        return Err("this keyboard runs stock firmware; rgb-live needs the patch".into());
    };
    let _ = writeln!(out, "live-lighting patch v{}", info.version);

    if saved {
        transfer(
            &device,
            out,
            Command::LiveRestore,
            &frames(Command::LiveRestore, &[]),
        )?;
    } else {
        // A live block needs no valid starting point: after an "off" state
        // the read-back can carry speed 0, which the parser rejects.
        let mut state = match read_led(&device, out) {
            Ok(led) => led.normalized(),
            Err(e) => {
                let _ = writeln!(out, "starting from a default block ({e})");
                LedState::parse(&[1, 255, 255, 255, 255, 0, 0, 0, 0, 5, 3, 0, 0, 0, 0xAA, 0x55])
                    .map_err(|e| format!("{e:?}"))?
            }
        };
        if let Some(effect) = edits.effect {
            state.set_effect(effect).map_err(|e| format!("{e:?}"))?;
        }
        if let Some(color) = edits.color {
            state.set_color(color);
        }
        if let Some(level) = edits.brightness {
            state.set_brightness(level).map_err(|e| format!("{e:?}"))?;
        }
        if let Some(level) = edits.speed {
            state.set_speed(level).map_err(|e| format!("{e:?}"))?;
        }
        let reply = transfer(&device, out, Command::LiveLed, &state.live_request())?;
        match reply.get(LIVE_STATUS_INDEX) {
            Some(0) => {}
            Some(code) => return Err(format!("the keyboard refused the block (status {code})")),
            None => return Err("short reply".into()),
        }
    }
    let now = read_led(&device, out)?;
    describe(out, &now);
    Ok(())
}

/// `rgb-effect`: the LIVE v2 effect engine. Everything here is RAM-only on
/// the keyboard.
///
///   rgb-effect measure
///   rgb-effect define <slot> <pattern> <speed> <brightness> <#RRGGBB,...> [duration_ms]
///   rgb-effect play <slot> | stop
///   rgb-effect demo
pub fn effect(log_dir: &Path, stamp: u64, args: &[String]) {
    let mut out = String::new();
    if let Err(e) = run_effect(&mut out, args) {
        let _ = writeln!(out, "stopped: {e}");
    }
    finish(log_dir, "rgb-effect", stamp, &out);
}

fn engine(device: &HidDevice, out: &mut String) -> Result<LiveInfo, String> {
    let info = probe_live(device, out)?
        .ok_or("this keyboard runs stock firmware; effects need LIVE v2")?;
    if !info.has_effects() {
        return Err(format!(
            "this keyboard runs LIVE v{} (RAM self-test {}); effects need v2",
            info.version,
            if info.ram_ok { "ok" } else { "failed" }
        ));
    }
    Ok(info)
}

fn send_checked(
    device: &HidDevice,
    out: &mut String,
    command: Command,
    frames: &[[u8; FRAME_LEN]],
) -> Result<(), String> {
    let reply = transfer(device, out, command, frames)?;
    match reply.get(LIVE_STATUS_INDEX) {
        Some(0) => Ok(()),
        Some(code) => Err(format!(
            "command 0x{:02X} refused, status {code}",
            command.code()
        )),
        None => Err("short reply".into()),
    }
}

/// Counts LED frames over a few seconds and gives the engine the real period.
fn measure(device: &HidDevice, out: &mut String) -> Result<u32, String> {
    let mut scratch = String::new();
    let a = engine(device, &mut scratch)?;
    let started = Instant::now();
    std::thread::sleep(Duration::from_secs(3));
    let b = engine(device, &mut scratch)?;
    let elapsed = started.elapsed();
    let frames = b.frames.wrapping_sub(a.frames);
    if frames == 0 {
        return Err("the frame counter did not move".into());
    }
    let micros = (elapsed.as_micros() / u128::from(frames)) as u32;
    let _ = writeln!(
        out,
        "{frames} LED frames in {:.2} s: {:.2} ms per frame ({:.1} fps)",
        elapsed.as_secs_f64(),
        f64::from(micros) / 1000.0,
        1e6 / f64::from(micros)
    );
    send_checked(
        device,
        &mut scratch,
        Command::SetFramePeriod,
        &knob_rgb::frame_period_request(micros),
    )?;
    Ok(micros)
}

fn parse_colors(list: &str) -> Result<Vec<[u8; 3]>, String> {
    list.split(',')
        .map(|c| {
            parse_hex(c)?
                .try_into()
                .map_err(|_| format!("bad colour {c}, use #RRGGBB"))
        })
        .collect()
}

fn run_effect(out: &mut String, args: &[String]) -> Result<(), String> {
    let device = open()?;
    let info = engine(&device, out)?;
    let _ = writeln!(
        out,
        "LIVE v{}, RAM ok, {} frames so far",
        info.version, info.frames
    );
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["measure"] => {
            measure(&device, out)?;
        }
        [
            "define",
            slot,
            pattern,
            speed,
            brightness,
            colors,
            rest @ ..,
        ] => {
            let def = EffectDef::new(
                number(slot)?,
                Pattern::parse(pattern).ok_or_else(|| format!("unknown pattern {pattern}"))?,
                number(speed)?,
                number(brightness)?,
                &parse_colors(colors)?,
                match rest {
                    [] => 0,
                    [ms] => ms.parse().map_err(|_| format!("bad duration {ms}"))?,
                    _ => return Err("too many arguments".into()),
                },
            )
            .map_err(|e| format!("invalid effect: {e:?}"))?;
            send_checked(&device, out, Command::DefineEffect, &def.define_request())?;
            let _ = writeln!(out, "defined slot {}", def.slot());
        }
        ["play", slot] => {
            send_checked(
                &device,
                out,
                Command::PlayEffect,
                &knob_rgb::play_request(number(slot)?),
            )?;
        }
        ["stop"] => {
            send_checked(&device, out, Command::StopEffect, &knob_rgb::stop_request())?;
        }
        ["demo"] => {
            measure(&device, out)?;
            let demo: [(&str, Pattern, u8, &[[u8; 3]]); 5] = [
                (
                    "Fire",
                    Pattern::Flicker,
                    6,
                    &[[0xFF, 0x20, 0], [0xFF, 0x70, 0], [0xFF, 0xC0, 0x40]],
                ),
                (
                    "Frost",
                    Pattern::Wave,
                    3,
                    &[[0xA0, 0xE8, 0xFF], [0x20, 0x60, 0xFF], [0xFF, 0xFF, 0xFF]],
                ),
                (
                    "Poison",
                    Pattern::Pulse,
                    5,
                    &[[0x40, 0xFF, 0x20], [0, 0x30, 0]],
                ),
                (
                    "Low health",
                    Pattern::Pulse,
                    8,
                    &[[0xFF, 0, 0], [0x20, 0, 0]],
                ),
                (
                    "Shock",
                    Pattern::Sparkle,
                    9,
                    &[[0x10, 0x10, 0x30], [0xFF, 0xFF, 0xFF], [0x80, 0xC0, 0xFF]],
                ),
            ];
            for (i, (name, pattern, speed, colors)) in demo.iter().enumerate() {
                let slot = i as u8 + 1;
                let def = EffectDef::new(slot, *pattern, *speed, 100, colors, 0)
                    .map_err(|e| format!("{name}: {e:?}"))?;
                send_checked(
                    &device,
                    &mut String::new(),
                    Command::DefineEffect,
                    &def.define_request(),
                )?;
            }
            for (i, (name, ..)) in demo.iter().enumerate() {
                let _ = writeln!(out, "playing {name} (slot {})", i + 1);
                println!("playing {name}...");
                send_checked(
                    &device,
                    &mut String::new(),
                    Command::PlayEffect,
                    &knob_rgb::play_request(i as u8 + 1),
                )?;
                std::thread::sleep(Duration::from_secs(4));
            }
            send_checked(&device, out, Command::StopEffect, &knob_rgb::stop_request())?;
            let _ = writeln!(out, "stopped; back to the normal lighting");
        }
        _ => {
            return Err(
                "usage: rgb-effect measure | define <slot> <pattern> <speed> <brightness> \
                        <#RRGGBB,...> [duration_ms] | play <slot> | stop | demo"
                    .into(),
            );
        }
    }
    Ok(())
}

/// `rgb-dump`: saves every stored settings block the keyboard lets us read,
/// as a backup before any firmware work. Reads only.
pub fn dump(log_dir: &Path, stamp: u64) {
    let mut out = String::new();
    let dir = log_dir.join(format!("dump-{stamp}"));
    if let Err(e) = run_dump(&mut out, &dir) {
        let _ = writeln!(out, "stopped: {e}");
    }
    finish(log_dir, "rgb-dump", stamp, &out);
}

fn run_dump(out: &mut String, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let device = open()?;
    let mut scratch = String::new();
    let led = read_led(&device, &mut scratch)?;
    describe(out, &led);
    save(dir, "block_13_lighting.bin", led.raw())?;
    let _ = writeln!(out, "0x13 lighting: {}", hex(led.raw()));

    for block in ReadBlock::ALL {
        let command = Command::ReadBlock(block);
        scratch.clear();
        let mut data = transfer(
            &device,
            &mut scratch,
            command,
            &read_request(command, ReadBlock::LEN),
        )?;
        // The short last frame still carries a full chunk; only its first
        // bytes belong to the block.
        if data.len() < ReadBlock::LEN {
            return Err(format!(
                "block 0x{:02X}: got {} bytes",
                command.code(),
                data.len()
            ));
        }
        data.truncate(ReadBlock::LEN);
        let name = format!("block_{:02x}.bin", command.code());
        save(dir, &name, &data)?;
        let used = data.iter().filter(|&&b| b != 0 && b != 0xFF).count();
        let _ = writeln!(
            out,
            "0x{:02X}: {} bytes, {used} not 00/FF, first 16: {}",
            command.code(),
            data.len(),
            hex(&data[..16])
        );
    }
    let _ = writeln!(out, "saved to {}", dir.display());
    Ok(())
}

fn save(dir: &Path, name: &str, data: &[u8]) -> Result<(), String> {
    let path = dir.join(name);
    std::fs::write(&path, data).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn finish(log_dir: &Path, name: &str, stamp: u64, out: &str) {
    print!("{out}");
    let path = log_dir.join(format!("{name}-{stamp}.txt"));
    match std::fs::write(&path, out) {
        Ok(()) => println!("\nSaved to {}", path.display()),
        Err(e) => eprintln!("\ncannot write {}: {e}", path.display()),
    }
}

/// Set by [`open`]: whether the keyboard is reached over the USB cable
/// (64-byte reports) rather than the 2.4 GHz receiver (32-byte reports).
static WIRED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Opens the keyboard's vendor interface: over the cable when the keyboard
/// is in cable mode, otherwise through the 2.4 GHz receiver. Both speak the
/// same `AA`/`55` frames; only the report size differs.
fn open() -> Result<HidDevice, String> {
    use std::sync::atomic::Ordering;
    let api = HidApi::new().map_err(|e| format!("hidapi init failed: {e}"))?;
    let wired = api.device_list().find(|d| {
        d.vendor_id() == VID
            && d.product_id() == knob_rgb::PID_WIRED
            && d.usage_page() == knob_rgb::WIRED_USAGE_PAGE
            && d.usage() == knob_rgb::WIRED_USAGE
    });
    if let Some(info) = wired {
        WIRED.store(true, Ordering::Relaxed);
        return info
            .open_device(&api)
            .map_err(|e| format!("cannot open the keyboard over the cable: {e}"));
    }
    WIRED.store(false, Ordering::Relaxed);
    let info = api
        .device_list()
        .find(|d| {
            d.vendor_id() == VID
                && d.product_id() == PID_DONGLE
                && d.usage_page() == USAGE_PAGE
                && d.usage() == USAGE
        })
        .ok_or("neither the wired keyboard nor the 2.4 GHz receiver was found")?;
    info.open_device(&api)
        .map_err(|e| format!("cannot open the receiver: {e}"))
}

/// A frame as written to whichever link [`open`] chose.
fn wire_frame(frame: &[u8; FRAME_LEN]) -> Vec<u8> {
    if WIRED.load(std::sync::atomic::Ordering::Relaxed) {
        let mut wire = vec![0u8; knob_rgb::WIRED_FRAME_LEN + 1];
        wire[1..=FRAME_LEN].copy_from_slice(frame);
        wire
    } else {
        to_wire(frame).to_vec()
    }
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
            .write(&wire_frame(frame))
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
            let mut buf = [0u8; knob_rgb::WIRED_FRAME_LEN + 1];
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
