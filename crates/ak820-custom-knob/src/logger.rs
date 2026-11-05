//! A small file logger: `%LOCALAPPDATA%\ak820-custom-knob\logs\knob.log`.
//! The previous log is kept as `knob.old.log` once the current one grows
//! past 1 MB.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use log::{Level, Log, Metadata, Record};
use windows::Win32::System::SystemInformation::GetLocalTime;

const MAX_SIZE: u64 = 1024 * 1024;

struct FileLogger {
    file: Mutex<File>,
    level: Level,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let t = unsafe { GetLocalTime() };
        let line = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {:<5} {}",
            t.wYear,
            t.wMonth,
            t.wDay,
            t.wHour,
            t.wMinute,
            t.wSecond,
            t.wMilliseconds,
            record.level(),
            record.args()
        );
        if cfg!(debug_assertions) {
            eprintln!("{line}");
        }
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{line}");
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

pub fn init(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    let path = dir.join("knob.log");
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_SIZE) {
        let _ = fs::rename(&path, dir.join("knob.old.log"));
    }
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let level = if std::env::var_os("KNOB_TRACE").is_some() {
        Level::Trace
    } else if cfg!(debug_assertions) {
        Level::Debug
    } else {
        Level::Info
    };
    if log::set_boxed_logger(Box::new(FileLogger {
        file: Mutex::new(file),
        level,
    }))
    .is_ok()
    {
        log::set_max_level(level.to_level_filter());
    }
}
