use std::path::PathBuf;

/// A single audio file known to the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: u64,
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: u64,
}
