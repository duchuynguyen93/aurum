//! Shared helpers for the offline (WavSink) engine tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use aurum_audio::{Engine, Event, WavSink};

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub struct Render {
    pub samples: Vec<f32>,
    pub channels: usize,
    pub rate: u32,
    pub events: Vec<Event>,
}

impl Render {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels
    }

    pub fn track_ended(&self) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(e, Event::TrackEnded { .. }))
            .count()
    }
}

/// Runs an engine on a mono/`channels` WavSink at `rate`, lets `script` issue commands, waits for
/// `ends` `TrackEnded` events and returns the rendered samples.
pub fn render(
    rate: u32,
    channels: u16,
    ends: usize,
    script: impl FnOnce(&mut Engine<WavSink>),
) -> Render {
    use aurum_audio::AudioEngine;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let wav = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "render-{}-{}.wav",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut engine = Engine::with_sink(WavSink::new(&wav, rate, channels)).expect("engine");
    let events = engine.events();
    script(&mut engine);
    engine.play();

    let mut seen = Vec::new();
    let mut ended = 0;
    let deadline = Instant::now() + Duration::from_secs(30);
    while ended < ends {
        let left = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(left) {
            Ok(e) => {
                if matches!(e, Event::TrackEnded { .. }) {
                    ended += 1;
                }
                seen.push(e);
            }
            Err(_) => panic!("timed out waiting for TrackEnded #{ends}; events: {seen:?}"),
        }
    }
    let mut sink = engine.shutdown();
    sink.finish().expect("finalize wav");
    // Events emitted between the last TrackEnded and shutdown.
    seen.extend(events.try_iter());

    let mut reader = hound::WavReader::open(&wav).expect("read wav");
    let spec = reader.spec();
    assert_eq!(spec.sample_format, hound::SampleFormat::Float);
    let samples: Vec<f32> = reader
        .samples::<f32>()
        .map(|s| s.expect("sample"))
        .collect();
    let _ = std::fs::remove_file(&wav);
    Render {
        samples,
        channels: usize::from(channels),
        rate,
        events: seen,
    }
}

pub fn rms(samples: &[f32]) -> f64 {
    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    (sum / samples.len() as f64).sqrt()
}

/// Samples with |x| below this would be exactly 0 once written as 16-bit PCM (half an LSB).
pub const SILENCE: f32 = 0.5 / 32768.0;

/// Longest run of consecutive near-silent frames within `frames[from..to]`.
pub fn longest_silence(samples: &[f32], from: usize, to: usize) -> usize {
    let to = to.min(samples.len());
    let mut best = 0;
    let mut run = 0;
    for &s in &samples[from..to] {
        if s.abs() < SILENCE {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}
