//! Loading the config file, creating it on first start, and noticing edits.

use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime};

use knob_core::Config;
use knob_core::config::DEFAULT_CONFIG;

use crate::paths;

/// Loads the config, writing the default file first if there is none.
/// An invalid file falls back to the built-in default, with the error.
pub fn load_or_create() -> (Config, Option<String>) {
    let path = paths::config_file();
    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        match fs::write(&path, DEFAULT_CONFIG) {
            Ok(()) => log::info!("created default config at {}", path.display()),
            Err(e) => log::warn!("cannot create {}: {e}", path.display()),
        }
    }
    match read(&path) {
        Ok(config) => (config, None),
        Err(e) => (Config::default_config(), Some(e)),
    }
}

pub fn read(path: &Path) -> Result<Config, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Config::from_toml(&text).map_err(|e| e.to_string())
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Loads the encrypted snippet list into the keyboard hook, now and every
/// time Settings saves it. A file that can't be read turns expansion off.
pub fn load_and_watch_snippets() {
    use knob_win::{snippet_store, snippets};
    let load = || match snippet_store::load() {
        Ok(store) => snippets::set_store(Some(store)),
        Err(e) => {
            log::warn!("snippets: {e}; expansion is off");
            snippets::set_store(None);
        }
    };
    load();
    let path = snippet_store::path();
    thread::Builder::new()
        .name("snippets-watch".into())
        .spawn(move || {
            let mut last = modified(&path);
            loop {
                thread::sleep(Duration::from_millis(750));
                let now = modified(&path);
                if now != last {
                    last = now;
                    thread::sleep(Duration::from_millis(150));
                    load();
                }
            }
        })
        .expect("spawn snippets watcher");
}

/// Calls `on_change` with the new config whenever the file is saved.
pub fn watch(on_change: impl Fn(Result<Config, String>) + Send + 'static) {
    let path = paths::config_file();
    thread::Builder::new()
        .name("config-watch".into())
        .spawn(move || {
            let mut last = modified(&path);
            loop {
                thread::sleep(Duration::from_millis(750));
                let now = modified(&path);
                if now != last && now.is_some() {
                    last = now;
                    // Editors sometimes write in several steps; let them finish.
                    thread::sleep(Duration::from_millis(150));
                    on_change(read(&path));
                }
            }
        })
        .expect("spawn config watcher");
}
