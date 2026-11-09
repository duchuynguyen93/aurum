//! Aurum core: data model, playback queue and music library. No UI, no audio device.

pub mod library;
mod queue;
mod track;

pub use queue::{Queue, Repeat};
pub use track::{ReplayGain, Track};
