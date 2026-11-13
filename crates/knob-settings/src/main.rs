//! Settings app: edits the config file that the daemon reloads on save.
//! The UI lives in `ui/` (plain HTML/CSS/JS, no build step); this file
//! exposes the commands it calls.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::Path;

use knob_core::{Config, Effect, LightingLayout};
use knob_win::{autostart, paths};
use serde::Serialize;
use tauri::Manager;

const PROJECT_URL: &str = "https://github.com/Enceladus00/ak820-max-ultra-toolbox";

#[derive(Serialize)]
struct Loaded {
    config: Config,
    path: String,
    /// Set when the file exists but can't be used; `config` is then the default.
    error: Option<String>,
}

#[derive(Serialize)]
struct App {
    exe: String,
    title: String,
}

const HEADER: &str = "\
# ak820-custom-knob configuration, written by the Settings app.
# You can still edit it by hand; comments are not kept when Settings saves.
# The built-in default with comments explaining every option:
# https://github.com/Enceladus00/ak820-max-ultra-toolbox/blob/main/crates/knob-core/src/default-config.toml

";

#[tauri::command]
fn load_config() -> Loaded {
    let path = paths::config_file();
    let (config, error) = match fs::read_to_string(&path) {
        Ok(text) => match Config::from_toml(&text) {
            Ok(config) => (config, None),
            Err(e) => (Config::default_config(), Some(e.to_string())),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Config::default_config(), None),
        Err(e) => (
            Config::default_config(),
            Some(format!("cannot read the file: {e}")),
        ),
    };
    Loaded {
        config,
        path: path.display().to_string(),
        error,
    }
}

#[tauri::command]
fn default_config() -> Config {
    Config::default_config()
}

/// Problems with the config, in plain words. Empty when it's valid.
#[tauri::command]
fn validate_config(config: Config) -> Vec<String> {
    match config.validate() {
        Ok(()) => Vec::new(),
        Err(knob_core::ConfigError::Invalid(errors)) => errors,
        Err(e) => vec![e.to_string()],
    }
}

#[tauri::command]
fn save_config(config: Config) -> Result<(), String> {
    config.validate().map_err(|e| e.to_string())?;
    let text = toml::to_string(&config).map_err(|e| e.to_string())?;
    write_atomic(&paths::config_file(), &format!("{HEADER}{text}")).map_err(|e| e.to_string())
}

/// Writes to a temporary file first, so the daemon never reads half a file.
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    // One step of undo, in case a save is regretted or the file was hand-edited.
    if path.exists() {
        let _ = fs::copy(path, path.with_extension("toml.bak"));
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

#[tauri::command]
fn running_apps() -> Vec<App> {
    let own = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_default();
    knob_win::process::running_apps()
        .into_iter()
        .filter(|a| !a.exe.eq_ignore_ascii_case(&own))
        .map(|a| App {
            exe: a.exe,
            title: a.title,
        })
        .collect()
}

/// Opens the config file, its folder, or the log folder in Explorer / the default app.
#[tauri::command]
fn open_location(which: String) -> Result<(), String> {
    let config = paths::config_file();
    let (program, arg) = match which.as_str() {
        "config" => ("notepad.exe", config),
        "config-folder" => (
            "explorer.exe",
            config.parent().map(Path::to_path_buf).unwrap_or_default(),
        ),
        "logs" => ("explorer.exe", paths::log_dir()),
        "project" => ("explorer.exe", PROJECT_URL.into()),
        "releases" => ("explorer.exe", format!("{PROJECT_URL}/releases").into()),
        other => return Err(format!("unknown location '{other}'")),
    };
    std::process::Command::new(program)
        .arg(arg)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Whether the background program is running, and whether it can be started.
#[derive(Serialize)]
struct DaemonState {
    running: bool,
    available: bool,
}

#[tauri::command]
fn daemon_state() -> DaemonState {
    DaemonState {
        running: knob_win::process::tray_running(),
        available: autostart::daemon_exe().is_some(),
    }
}

#[tauri::command]
fn start_daemon() -> Result<(), String> {
    let exe = autostart::daemon_exe().ok_or("the knob program was not found")?;
    std::process::Command::new(exe)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Sends a layout to the keyboard. On its own thread, because a sleeping
/// keyboard takes over a second to time out.
#[tauri::command(async)]
fn apply_lighting(layout: LightingLayout) -> Result<(), String> {
    let mut errors = Vec::new();
    layout.check(&mut errors);
    if let Some(problem) = errors.into_iter().next() {
        return Err(problem);
    }
    knob_win::lighting::apply(&layout).map_err(|e| e.to_string())
}

/// What the keyboard runs, for the Effects page.
#[derive(Serialize)]
struct EffectFirmware {
    /// "stock", "live" or "unknown" (keyboard missing or asleep).
    kind: &'static str,
    /// LIVE version, when `kind` is "live".
    version: Option<u8>,
    /// Whether effects can play (LIVE v2 or newer, RAM self-test passed).
    effects: bool,
    /// Why it is "unknown": the codes the UI translates.
    error: Option<String>,
}

/// Asks the keyboard which firmware it runs. RAM-only.
#[tauri::command(async)]
fn effect_firmware() -> EffectFirmware {
    use knob_win::effects::{Firmware, firmware_now};
    match firmware_now() {
        Ok(Firmware::Stock) => EffectFirmware {
            kind: "stock",
            version: None,
            effects: false,
            error: None,
        },
        Ok(fw @ Firmware::Live(info)) => EffectFirmware {
            kind: "live",
            version: Some(info.version),
            effects: fw.has_effects(),
            error: None,
        },
        Err(e) => EffectFirmware {
            kind: "unknown",
            version: None,
            effects: false,
            error: Some(e.to_string()),
        },
    }
}

/// Uploads the effect being edited and plays it on the keyboard. RAM-only:
/// nothing is stored in the keyboard's flash.
#[tauri::command(async)]
fn effect_play(effect: Effect) -> Result<(), String> {
    let mut errors = Vec::new();
    effect.check(&mut errors);
    if let Some(problem) = errors.into_iter().next() {
        return Err(problem);
    }
    let def = effect.to_def().ok_or("the effect can't be sent")?;
    knob_win::effects::play_now(&def).map_err(|e| e.to_string())
}

/// Stops the effect playing on the keyboard. RAM-only.
#[tauri::command(async)]
fn effect_stop() -> Result<(), String> {
    knob_win::effects::stop_now().map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct Autostart {
    /// False when the background program isn't next to this exe.
    available: bool,
    enabled: bool,
}

fn autostart_state() -> Autostart {
    Autostart {
        available: autostart::daemon_exe().is_some(),
        enabled: autostart::is_enabled(),
    }
}

/// Whether the knob program starts at sign-in. Not part of the config file:
/// the registry entry is the setting, shared with the tray menu.
#[tauri::command]
fn get_autostart() -> Autostart {
    autostart_state()
}

#[tauri::command]
fn set_autostart(enabled: bool) -> Result<Autostart, String> {
    let exe = autostart::daemon_exe().ok_or_else(|| {
        format!(
            "{} not found next to the Settings app",
            autostart::DAEMON_EXE
        )
    })?;
    autostart::set_enabled(enabled, &exe).map_err(|e| e.to_string())?;
    Ok(autostart_state())
}

// ---- text snippets: kept apart from config.toml, encrypted for this user

#[tauri::command]
fn snippets_load() -> Result<knob_core::snippets::SnippetStore, String> {
    knob_win::snippet_store::load()
}

/// Problems with a list, in plain words; empty when it can be saved.
#[tauri::command]
fn snippets_check(store: knob_core::snippets::SnippetStore) -> Vec<String> {
    store.problems()
}

/// Encrypts and saves the whole list; the knob program picks it up at once.
#[tauri::command]
fn snippets_save(store: knob_core::snippets::SnippetStore) -> Result<(), String> {
    knob_win::snippet_store::save(&store)
}

fn main() {
    tauri::Builder::default()
        // Opening Settings again (tray, Start menu) focuses the open window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            load_config,
            default_config,
            validate_config,
            save_config,
            running_apps,
            open_location,
            apply_lighting,
            effect_firmware,
            effect_play,
            effect_stop,
            daemon_state,
            start_daemon,
            app_version,
            get_autostart,
            set_autostart,
            snippets_load,
            snippets_check,
            snippets_save
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the settings window");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_keeps_the_previous_file_as_a_backup() {
        let dir = std::env::temp_dir().join(format!("knob-backup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("config.toml");

        write_atomic(&path, "first").unwrap();
        assert!(!path.with_extension("toml.bak").exists());
        write_atomic(&path, "second").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        assert_eq!(
            fs::read_to_string(path.with_extension("toml.bak")).unwrap(),
            "first"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
