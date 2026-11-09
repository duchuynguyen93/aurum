use std::io;
use std::path::{Path, PathBuf};

use aurum_core::Repeat;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl From<RepeatMode> for Repeat {
    fn from(m: RepeatMode) -> Self {
        match m {
            RepeatMode::Off => Repeat::Off,
            RepeatMode::All => Repeat::All,
            RepeatMode::One => Repeat::One,
        }
    }
}

impl From<Repeat> for RepeatMode {
    fn from(r: Repeat) -> Self {
        match r {
            Repeat::Off => RepeatMode::Off,
            Repeat::All => RepeatMode::All,
            Repeat::One => RepeatMode::One,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub roots: Vec<PathBuf>,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            volume: 0.8,
            shuffle: false,
            repeat: RepeatMode::Off,
        }
    }
}

impl Settings {
    /// Reads the file; a missing or unreadable file yields defaults (logged, never a panic).
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(mut s) => {
                    s.volume = if s.volume.is_finite() {
                        s.volume.clamp(0.0, 1.0)
                    } else {
                        Settings::default().volume
                    };
                    s
                }
                Err(e) => {
                    log::warn!(
                        "settings {} are corrupt, using defaults: {e}",
                        path.display()
                    );
                    Settings::default()
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Settings::default(),
            Err(e) => {
                log::warn!("cannot read settings {}: {e}", path.display());
                Settings::default()
            }
        }
    }

    /// Writes a temp file next to `path`, then renames it over the target.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_replaces_atomically() {
        let dir = std::env::temp_dir().join(format!("aurum-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        let s = Settings {
            roots: vec!["/music".into()],
            volume: 0.4,
            shuffle: true,
            repeat: RepeatMode::All,
        };
        s.save(&path).unwrap();
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn corrupt_or_missing_file_gives_defaults() {
        let dir = std::env::temp_dir().join(format!("aurum-settings-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        std::fs::write(&path, r#"{"volume": 7.0}"#).unwrap();
        assert_eq!(Settings::load(&path).volume, 1.0);
        std::fs::remove_dir_all(dir).ok();
    }
}
