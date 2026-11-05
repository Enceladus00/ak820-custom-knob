//! Every input source pushes timestamped events into one channel; a single
//! logger thread formats them, writes them to the console and the log file,
//! and drives the guided test. Keeping all I/O off the hook thread matters:
//! Windows silently removes a low-level hook that answers too slowly.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::Instant;

use crate::names::{consumer_usage_name, hex_bytes, vk_name};

pub static START: OnceLock<Instant> = OnceLock::new();
static TX: OnceLock<Sender<Event>> = OnceLock::new();

/// Milliseconds since the probe started.
pub fn now_ms() -> f64 {
    START
        .get()
        .map(|s| s.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

pub fn send(event: Event) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(event);
    }
}

pub enum Event {
    /// Low-level keyboard hook (WH_KEYBOARD_LL).
    Hook {
        t: f64,
        vk: u32,
        scan: u32,
        flags: u32,
        extra: usize,
        down: bool,
    },
    /// Raw Input, keyboard device.
    RawKey {
        t: f64,
        device: String,
        make: u16,
        flags: u16,
        vkey: u16,
        message: u32,
        extra: u32,
    },
    /// Raw Input, generic HID device (consumer control).
    RawHid {
        t: f64,
        device: String,
        data: Vec<u8>,
        report_size: u32,
    },
    /// A report read directly from a HID interface via hidapi.
    HidReport {
        t: f64,
        device: String,
        data: Vec<u8>,
    },
    Info(String),
    /// Input listeners are ready; show the first test step.
    Begin,
    Shutdown,
}

const VK_SPACE: u32 = 0x20;
const LLKHF_INJECTED: u32 = 0x10;

const STEPS: &[&str] = &[
    "Turn the knob CLOCKWISE slowly, 5 clicks.",
    "Turn the knob COUNTER-CLOCKWISE slowly, 5 clicks.",
    "Spin the knob CLOCKWISE FAST, one quick spin.",
    "Press (click) the knob 3 times, with pauses.",
    "Press Fn+F3 3 times (the key combo that also raises volume).",
    "Press Fn+F2 3 times (the key combo that also lowers volume).",
    "Press the Fn combo that mutes (often Fn+F1) 3 times. No such combo? Just skip.",
    "HOLD Fn and turn the knob clockwise 3 clicks, then counter-clockwise 3 clicks.",
    "HOLD Fn and press the knob once.",
    "HOLD Ctrl and turn the knob clockwise 3 clicks.",
    "Free play: try anything else you are curious about (other Fn combos, long knob press...).",
];

struct Logger {
    file: BufWriter<File>,
    /// Press time of keys currently held, to report how long each press lasted.
    held: HashMap<String, f64>,
    /// 0 = not started, 1..=STEPS.len() = current step.
    step: usize,
    started: bool,
}

impl Logger {
    fn line(&mut self, text: &str) {
        println!("{text}");
        let _ = writeln!(self.file, "{text}");
    }

    fn banner(&mut self, text: &str) {
        self.line("");
        self.line(&format!("==================== {text}"));
    }

    fn show_step(&mut self) {
        if self.step == 0 {
            self.banner("READY");
            self.line("Your volume WILL change during the test; turn speakers down.");
            self.line("Each step: do the action, then press SPACE for the next step.");
            self.line("Ctrl+C quits at any time; the log is saved either way.");
            self.line("Press SPACE to start.");
        } else if self.step <= STEPS.len() {
            let text = STEPS[self.step - 1];
            self.banner(&format!("STEP {}/{}: {text}", self.step, STEPS.len()));
        }
    }

    /// Records press/release and returns a suffix like "  held 4.1 ms".
    fn track(&mut self, key: String, t: f64, down: bool) -> String {
        if down {
            // Keep the first press time, so auto-repeat doesn't shorten "held".
            if let std::collections::hash_map::Entry::Vacant(e) = self.held.entry(key) {
                e.insert(t);
                String::new()
            } else {
                "  (repeat)".to_string()
            }
        } else {
            match self.held.remove(&key) {
                Some(t0) => format!("  held {:.1} ms", t - t0),
                None => String::new(),
            }
        }
    }

    fn handle(&mut self, event: Event, on_finished: &mut Option<Box<dyn FnOnce() + Send>>) {
        match event {
            Event::Begin => {
                self.started = true;
                self.show_step();
            }
            Event::Hook {
                t,
                vk,
                scan,
                flags,
                extra,
                down,
            } => {
                // SPACE (typed by a person, not injected) advances the guided test.
                if vk == VK_SPACE && flags & LLKHF_INJECTED == 0 && self.started {
                    if down && !self.held.contains_key("space") {
                        self.held.insert("space".into(), t);
                        self.step += 1;
                        if self.step > STEPS.len() {
                            self.banner("DONE - all steps finished, quitting");
                            if let Some(f) = on_finished.take() {
                                f();
                            }
                        } else {
                            self.show_step();
                        }
                    } else if !down {
                        self.held.remove("space");
                    }
                    return;
                }
                let suffix = self.track(format!("hook:{vk}"), t, down);
                self.line(&format!(
                    "[{t:10.3}] HOOK    {:4} vk=0x{vk:02X} {:<14} sc=0x{scan:04X} flags=0x{flags:02X}{} extra=0x{extra:X}{suffix}",
                    if down { "down" } else { "up" },
                    vk_name(vk),
                    hook_flag_names(flags),
                ));
            }
            Event::RawKey {
                t,
                device,
                make,
                flags,
                vkey,
                message,
                extra,
            } => {
                if vkey as u32 == VK_SPACE {
                    return;
                }
                let down = flags & 0x01 == 0; // RI_KEY_BREAK
                let suffix = self.track(format!("raw:{device}:{vkey}"), t, down);
                self.line(&format!(
                    "[{t:10.3}] RAWKEY  {:4} vk=0x{vkey:02X} {:<14} make=0x{make:04X} flags=0x{flags:X}{} msg=0x{message:04X} extra=0x{extra:X} dev={device}{suffix}",
                    if down { "down" } else { "up" },
                    vk_name(vkey as u32),
                    raw_flag_names(flags),
                ));
            }
            Event::RawHid {
                t,
                device,
                data,
                report_size,
            } => {
                self.line(&format!(
                    "[{t:10.3}] RAWHID  dev={device} size={report_size} bytes=[{}]{}",
                    hex_bytes(&data),
                    consumer_guess(&data),
                ));
            }
            Event::HidReport { t, device, data } => {
                self.line(&format!(
                    "[{t:10.3}] HIDAPI  dev={device} bytes=[{}]{}",
                    hex_bytes(&data),
                    consumer_guess(&data),
                ));
            }
            Event::Info(text) => {
                let t = now_ms();
                self.line(&format!("[{t:10.3}] INFO    {text}"));
            }
            Event::Shutdown => {}
        }
        let _ = self.file.flush();
    }
}

fn hook_flag_names(flags: u32) -> String {
    let mut names = Vec::new();
    if flags & 0x01 != 0 {
        names.push("EXT");
    }
    if flags & 0x02 != 0 {
        names.push("LOWER_IL_INJ");
    }
    if flags & 0x10 != 0 {
        names.push("INJECTED");
    }
    if flags & 0x20 != 0 {
        names.push("ALT");
    }
    if flags & 0x80 != 0 {
        names.push("UP");
    }
    if names.is_empty() {
        String::new()
    } else {
        format!("[{}]", names.join(","))
    }
}

fn raw_flag_names(flags: u16) -> String {
    let mut names = Vec::new();
    if flags & 0x02 != 0 {
        names.push("E0");
    }
    if flags & 0x04 != 0 {
        names.push("E1");
    }
    if names.is_empty() {
        String::new()
    } else {
        format!("[{}]", names.join(","))
    }
}

/// Consumer-control reports are usually `[report id][usage lo][usage hi]...`.
/// This is only a hint for reading the log, not a parser.
fn consumer_guess(data: &[u8]) -> String {
    if data.len() >= 3 {
        let usage = u16::from_le_bytes([data[1], data[2]]);
        let name = consumer_usage_name(usage);
        if name != "?" {
            return format!("  ~ consumer {name}?");
        }
    }
    String::new()
}

pub fn start_logger(
    path: &Path,
    on_finished: impl FnOnce() + Send + 'static,
) -> std::io::Result<JoinHandle<()>> {
    let file = File::create(path)?;
    let (tx, rx) = mpsc::channel();
    TX.set(tx).ok();
    let mut on_finished: Option<Box<dyn FnOnce() + Send>> = Some(Box::new(on_finished));
    let handle = std::thread::spawn(move || {
        let mut logger = Logger {
            file: BufWriter::new(file),
            held: HashMap::new(),
            step: 0,
            started: false,
        };
        logger.line(&format!(
            "knob-probe {} - columns: [ms since start] SOURCE details",
            env!("CARGO_PKG_VERSION")
        ));
        for event in rx {
            let shutdown = matches!(event, Event::Shutdown);
            logger.handle(event, &mut on_finished);
            if shutdown {
                break;
            }
        }
        let _ = logger.file.flush();
    });
    Ok(handle)
}
