//! Where the config file and logs live. Shared by the daemon and the
//! settings app.

use std::path::PathBuf;

const APP_DIR: &str = "ak820-custom-knob";

fn env_dir(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_DIR)
}

/// `%APPDATA%\ak820-custom-knob\config.toml`
pub fn config_file() -> PathBuf {
    env_dir("APPDATA").join("config.toml")
}

/// `%LOCALAPPDATA%\ak820-custom-knob\logs`
pub fn log_dir() -> PathBuf {
    env_dir("LOCALAPPDATA").join("logs")
}
