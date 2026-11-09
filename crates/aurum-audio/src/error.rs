//! Error type shared by the audio modules.

use std::path::PathBuf;

/// Everything that can go wrong while opening, decoding or outputting audio.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("cannot open {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("decode error: {0}")]
    Symphonia(#[from] symphonia::core::errors::Error),
    #[error("no decodable audio track")]
    NoAudioTrack,
    #[error("resampler error: {0}")]
    Resample(String),
    #[error("audio output error: {0}")]
    Sink(String),
}
