//! Aurum audio: the playback engine abstraction. M0 has no real backend yet.

use std::path::Path;
use std::time::Duration;

/// Playback backend. Implementations decode and output audio.
pub trait AudioEngine {
    /// Starts or resumes playback of the loaded track.
    fn play(&mut self);
    fn pause(&mut self);
    /// Seeks within the current track.
    fn seek(&mut self, position: Duration);
    /// Sets the output volume, clamped to `0.0..=1.0`.
    fn set_volume(&mut self, volume: f32);
    /// Queues a file to start right after the current one ends (gapless).
    fn queue_next(&mut self, path: &Path);
}

/// Engine that does nothing. Used until a real backend exists and in tests.
#[derive(Debug, Default)]
pub struct NullEngine;

impl AudioEngine for NullEngine {
    fn play(&mut self) {}
    fn pause(&mut self) {}
    fn seek(&mut self, _position: Duration) {}
    fn set_volume(&mut self, _volume: f32) {}
    fn queue_next(&mut self, _path: &Path) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_engine_accepts_all_calls() {
        let mut engine: Box<dyn AudioEngine> = Box::new(NullEngine);
        engine.play();
        engine.seek(Duration::from_secs(3));
        engine.set_volume(0.5);
        engine.queue_next(Path::new("/music/a.flac"));
        engine.pause();
    }
}
