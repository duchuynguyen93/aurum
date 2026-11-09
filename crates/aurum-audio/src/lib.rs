//! Aurum audio: decoder thread, ring buffer and output sinks.

mod decoder;
mod engine;
mod error;
mod gain;
mod resample;
mod sink;

use std::path::Path;
use std::time::Duration;

pub use crossbeam_channel::Receiver;
pub use engine::{Command, CpalEngine, Engine, Event};
pub use error::AudioError;
pub use gain::ReplayGain;
pub use sink::{CpalSink, Sink, SinkControl, SinkSpec, WavSink};

/// Playback backend. Implementations decode and output audio.
pub trait AudioEngine {
    /// Replaces the current track (and clears the queue). `gain` is the track's ReplayGain.
    /// Errors are reported as [`Event::Error`], not returned.
    fn load(&mut self, path: &Path, gain: Option<ReplayGain>);
    /// Starts or resumes playback of the loaded track.
    fn play(&mut self);
    fn pause(&mut self);
    /// Stops playback and drops the current and queued tracks.
    fn stop(&mut self);
    /// Seeks within the current track.
    fn seek(&mut self, position: Duration);
    /// Sets the output volume, clamped to `0.0..=1.0`.
    fn set_volume(&mut self, volume: f32);
    /// Queues a file to start right after the current one ends (gapless).
    fn queue_next(&mut self, path: &Path);
    /// Like [`queue_next`](Self::queue_next) with the queued track's ReplayGain.
    fn queue_next_with_gain(&mut self, path: &Path, gain: Option<ReplayGain>);
    /// Stream of playback events. Clones share one queue.
    fn events(&self) -> Receiver<Event>;
}

/// Engine that does nothing. Used until a real backend exists and in tests.
#[derive(Debug, Default)]
pub struct NullEngine;

impl AudioEngine for NullEngine {
    fn load(&mut self, _path: &Path, _gain: Option<ReplayGain>) {}
    fn play(&mut self) {}
    fn pause(&mut self) {}
    fn stop(&mut self) {}
    fn seek(&mut self, _position: Duration) {}
    fn set_volume(&mut self, _volume: f32) {}
    fn queue_next(&mut self, _path: &Path) {}
    fn queue_next_with_gain(&mut self, _path: &Path, _gain: Option<ReplayGain>) {}
    fn events(&self) -> Receiver<Event> {
        // The sender is dropped, so the receiver reports "disconnected" and never yields events.
        crossbeam_channel::unbounded().1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_engine_accepts_all_calls() {
        let mut engine: Box<dyn AudioEngine> = Box::new(NullEngine);
        engine.load(Path::new("/music/a.flac"), None);
        engine.play();
        engine.seek(Duration::from_secs(3));
        engine.set_volume(0.5);
        engine.queue_next(Path::new("/music/a.flac"));
        engine.stop();
        engine.pause();
        assert!(engine.events().try_recv().is_err());
    }
}
