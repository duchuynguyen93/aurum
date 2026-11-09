use std::path::PathBuf;

/// ReplayGain values read from tags. Gains are in dB, peaks are linear (1.0 = full scale).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReplayGain {
    pub track_gain: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain: Option<f32>,
    pub album_peak: Option<f32>,
}

/// A single audio file known to the library.
///
/// `path` is stored in the database via `to_string_lossy`, so paths that are not valid
/// UTF-8 do not round-trip exactly (invalid sequences become U+FFFD).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Track {
    /// Database row id; 0 for a track that has not been stored yet.
    pub id: u64,
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    /// Falls back to `artist` at scan time when the file has no album artist tag.
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub channels: u8,
    /// Container/codec family, e.g. `flac`, `mp3`, `m4a`.
    pub codec: String,
    pub replay_gain: ReplayGain,
    /// Modification time in nanoseconds since the Unix epoch.
    pub mtime: i64,
    /// File size in bytes.
    pub size: u64,
}
