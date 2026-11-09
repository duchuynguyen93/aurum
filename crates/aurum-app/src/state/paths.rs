//! Where settings and the library database live. `AURUM_HOME` overrides both (tests, demos).

use std::path::PathBuf;

fn override_dir() -> Option<PathBuf> {
    std::env::var_os("AURUM_HOME").map(PathBuf::from)
}

/// `<config dir>/aurum`: `%APPDATA%\aurum` on Windows, `~/Library/Application Support/aurum` on macOS.
pub fn config_dir() -> PathBuf {
    override_dir().unwrap_or_else(|| {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("aurum")
    })
}

/// `<data dir>/aurum`; same location as the config dir on Windows and macOS.
pub fn data_dir() -> PathBuf {
    override_dir().unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("aurum")
    })
}

pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn db_path() -> PathBuf {
    data_dir().join("library.db")
}
