//! Aurum core: data model and playback queue. No UI, no audio device.

mod queue;
mod track;

pub use queue::{Queue, Repeat};
pub use track::Track;
