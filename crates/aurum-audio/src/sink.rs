//! Audio output side: the [`Sink`] trait and its two implementations.
//!
//! A sink consumes interleaved `f32` frames from an `rtrb` ring. The decoder thread is the only
//! producer. The sink side is real-time: no allocation, no locking, no logging while pulling.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream};
use rtrb::Consumer;

use crate::error::AudioError;

/// Output format a sink wants to receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

/// State shared between the decoder thread and the sink's pulling side. All atomics.
#[derive(Debug, Default)]
pub struct SinkControl {
    /// Bumped by the decoder to ask the sink to drop everything currently in the ring.
    pub(crate) generation: AtomicU64,
    /// Set by the sink to the generation it last drained for.
    pub(crate) flushed: AtomicU64,
    /// While true the sink outputs silence and leaves the ring untouched.
    pub(crate) paused: AtomicBool,
    /// True while the decoder has audio it should be delivering. Starvation only counts as an
    /// underrun then (not while idle, paused, or just after a load).
    pub(crate) expect_data: AtomicBool,
    /// Number of callbacks that had to pad with silence while data was expected.
    pub(crate) underruns: AtomicU64,
    /// Asks a non-callback sink (the WAV writer thread) to drain the ring and finish.
    pub(crate) stop: AtomicBool,
}

/// Consumer-side helper shared by all sinks: whole-frame reads plus the flush handshake.
pub(crate) struct Puller {
    consumer: Consumer<f32>,
    ctrl: Arc<SinkControl>,
    channels: usize,
    seen_generation: u64,
}

impl Puller {
    pub(crate) fn new(consumer: Consumer<f32>, ctrl: Arc<SinkControl>, channels: u16) -> Self {
        Self {
            consumer,
            ctrl,
            channels: usize::from(channels).max(1),
            seen_generation: 0,
        }
    }

    /// Drops everything in the ring if the decoder requested a flush, then acknowledges it.
    /// The decoder does not push between requesting and the acknowledgement, so everything
    /// drained here is stale.
    pub(crate) fn handle_flush(&mut self) {
        let generation = self.ctrl.generation.load(Ordering::Acquire);
        if generation != self.seen_generation {
            let n = self.consumer.slots();
            if let Ok(chunk) = self.consumer.read_chunk(n) {
                chunk.commit_all();
            }
            self.seen_generation = generation;
            self.ctrl.flushed.store(generation, Ordering::Release);
        }
    }

    pub(crate) fn is_paused(&self) -> bool {
        self.ctrl.paused.load(Ordering::Relaxed)
    }

    /// Number of whole frames available, capped at `max_samples / channels`.
    fn whole_samples(&self, max_samples: usize) -> usize {
        let n = self.consumer.slots().min(max_samples);
        n - n % self.channels
    }

    /// Fills `out` (a device buffer) from the ring, converting from `f32`. Pads with silence
    /// and counts an underrun when the ring runs dry. Real-time safe.
    pub(crate) fn fill<T: Sample + FromSample<f32>>(&mut self, out: &mut [T]) {
        self.handle_flush();
        if self.is_paused() {
            out.fill(T::EQUILIBRIUM);
            return;
        }
        let n = self.whole_samples(out.len());
        let mut written = 0;
        if n > 0
            && let Ok(chunk) = self.consumer.read_chunk(n)
        {
            let (a, b) = chunk.as_slices();
            for &s in a.iter().chain(b) {
                out[written] = T::from_sample(s);
                written += 1;
            }
            chunk.commit_all();
        }
        if written < out.len() {
            out[written..].fill(T::EQUILIBRIUM);
            if self.ctrl.expect_data.load(Ordering::Relaxed) {
                self.ctrl.underruns.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Moves up to `max_samples` whole frames into `dst`. Returns how many samples were moved.
    /// Not real-time safe (appends to a `Vec`); for offline sinks.
    fn take_into(&mut self, dst: &mut Vec<f32>, max_samples: usize) -> usize {
        let n = self.whole_samples(max_samples);
        if n == 0 {
            return 0;
        }
        let Ok(chunk) = self.consumer.read_chunk(n) else {
            return 0;
        };
        let (a, b) = chunk.as_slices();
        dst.extend_from_slice(a);
        dst.extend_from_slice(b);
        chunk.commit_all();
        n
    }
}

/// An audio output. The engine owns it for its whole life.
pub trait Sink {
    /// Format the decoder must produce. Fixed once the sink is constructed.
    fn spec(&self) -> SinkSpec;
    /// Starts pulling from `consumer`. Called once by the engine.
    fn start(&mut self, consumer: Consumer<f32>, ctrl: Arc<SinkControl>) -> Result<(), AudioError>;
    /// Stops pulling and releases the device or file. Idempotent.
    fn stop(&mut self);
}

// ---------------------------------------------------------------------------------------------
// CpalSink
// ---------------------------------------------------------------------------------------------

/// Plays through the default output device using the device's default config (the sample rate
/// is whatever the device prefers, never forced).
pub struct CpalSink {
    device: cpal::Device,
    config: cpal::StreamConfig,
    sample_format: SampleFormat,
    stream: Option<Stream>,
}

impl CpalSink {
    pub fn new() -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::Sink("no default output device".into()))?;
        let supported = device
            .default_output_config()
            .map_err(|e| AudioError::Sink(e.to_string()))?;
        let sample_format = supported.sample_format();
        Ok(Self {
            device,
            config: supported.into(),
            sample_format,
            stream: None,
        })
    }

    /// Sample format the device expects (the engine always produces `f32` and converts).
    pub fn sample_format(&self) -> SampleFormat {
        self.sample_format
    }

    fn build<T>(&self, mut puller: Puller) -> Result<Stream, AudioError>
    where
        T: SizedSample + FromSample<f32>,
    {
        self.device
            .build_output_stream(
                self.config,
                move |data: &mut [T], _| puller.fill(data),
                |err| log::error!("output stream error: {err}"),
                None,
            )
            .map_err(|e| AudioError::Sink(e.to_string()))
    }
}

impl Sink for CpalSink {
    fn spec(&self) -> SinkSpec {
        SinkSpec {
            sample_rate: self.config.sample_rate,
            channels: self.config.channels,
        }
    }

    fn start(&mut self, consumer: Consumer<f32>, ctrl: Arc<SinkControl>) -> Result<(), AudioError> {
        let puller = Puller::new(consumer, ctrl, self.config.channels);
        let stream = match self.sample_format {
            SampleFormat::F32 => self.build::<f32>(puller)?,
            SampleFormat::F64 => self.build::<f64>(puller)?,
            SampleFormat::I16 => self.build::<i16>(puller)?,
            SampleFormat::I32 => self.build::<i32>(puller)?,
            SampleFormat::U16 => self.build::<u16>(puller)?,
            other => {
                return Err(AudioError::Sink(format!(
                    "unsupported sample format {other}"
                )));
            }
        };
        stream.play().map_err(|e| AudioError::Sink(e.to_string()))?;
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) {
        self.stream = None;
    }
}

// ---------------------------------------------------------------------------------------------
// WavSink
// ---------------------------------------------------------------------------------------------

/// Writes everything it pulls to a 32-bit float WAV file, as fast as the decoder produces it
/// (no device clock). Never inserts silence when the ring is empty, so the file is exactly what
/// the decoder emitted. For tests and offline rendering.
pub struct WavSink {
    path: PathBuf,
    spec: SinkSpec,
    ctrl: Option<Arc<SinkControl>>,
    frames_written: Arc<AtomicU64>,
    thread: Option<JoinHandle<std::io::Result<()>>>,
}

impl WavSink {
    pub fn new(path: impl Into<PathBuf>, sample_rate: u32, channels: u16) -> Self {
        Self {
            path: path.into(),
            spec: SinkSpec {
                sample_rate,
                channels,
            },
            ctrl: None,
            frames_written: Arc::new(AtomicU64::new(0)),
            thread: None,
        }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Frames written to the file so far.
    pub fn frames_written(&self) -> u64 {
        self.frames_written.load(Ordering::Relaxed)
    }

    /// Stops, drains the ring, finalizes the file and reports I/O errors.
    pub fn finish(&mut self) -> std::io::Result<()> {
        if let Some(ctrl) = &self.ctrl {
            ctrl.stop.store(true, Ordering::Release);
        }
        match self.thread.take() {
            Some(handle) => handle
                .join()
                .unwrap_or_else(|_| Err(std::io::Error::other("wav writer thread panicked"))),
            None => Ok(()),
        }
    }
}

impl Sink for WavSink {
    fn spec(&self) -> SinkSpec {
        self.spec
    }

    fn start(&mut self, consumer: Consumer<f32>, ctrl: Arc<SinkControl>) -> Result<(), AudioError> {
        let file = File::create(&self.path)
            .map_err(|e| AudioError::Sink(format!("{}: {e}", self.path.display())))?;
        let mut writer = BufWriter::new(file);
        write_wav_header(&mut writer, self.spec, 0).map_err(|e| AudioError::Sink(e.to_string()))?;
        let spec = self.spec;
        let frames_written = Arc::clone(&self.frames_written);
        let mut puller = Puller::new(consumer, Arc::clone(&ctrl), spec.channels);
        self.ctrl = Some(ctrl.clone());
        self.thread = Some(std::thread::spawn(move || {
            let mut scratch: Vec<f32> = Vec::with_capacity(8192);
            let mut bytes: Vec<u8> = Vec::with_capacity(8192 * 4);
            let mut total_samples: u64 = 0;
            loop {
                puller.handle_flush();
                // Read the stop flag *before* trying to drain: if it was already set, everything
                // the decoder will ever push is in the ring, so an empty ring means done.
                let stopping = ctrl.stop.load(Ordering::Acquire);
                scratch.clear();
                let n = if puller.is_paused() {
                    0
                } else {
                    puller.take_into(&mut scratch, 8192)
                };
                if n > 0 {
                    bytes.clear();
                    for s in &scratch {
                        bytes.extend_from_slice(&s.to_le_bytes());
                    }
                    writer.write_all(&bytes)?;
                    total_samples += n as u64;
                    frames_written
                        .store(total_samples / u64::from(spec.channels), Ordering::Relaxed);
                } else if stopping {
                    break;
                } else {
                    std::thread::sleep(Duration::from_micros(200));
                }
            }
            // Patch the header now that the data length is known.
            writer.flush()?;
            let mut file = writer.into_inner().map_err(|e| e.into_error())?;
            file.seek(SeekFrom::Start(0))?;
            write_wav_header(&mut file, spec, total_samples * 4)?;
            file.flush()
        }));
        Ok(())
    }

    fn stop(&mut self) {
        if let Err(e) = self.finish() {
            log::error!("wav sink: {e}");
        }
    }
}

impl Drop for WavSink {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Writes a 44-byte IEEE-float WAV header for `data_len` bytes of sample data.
fn write_wav_header(w: &mut impl Write, spec: SinkSpec, data_len: u64) -> std::io::Result<()> {
    let channels = spec.channels;
    let block_align = channels * 4;
    let byte_rate = spec.sample_rate * u32::from(block_align);
    let data_len = u32::try_from(data_len).unwrap_or(u32::MAX - 36);
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data_len).to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&3u16.to_le_bytes())?; // WAVE_FORMAT_IEEE_FLOAT
    w.write_all(&channels.to_le_bytes())?;
    w.write_all(&spec.sample_rate.to_le_bytes())?;
    w.write_all(&byte_rate.to_le_bytes())?;
    w.write_all(&block_align.to_le_bytes())?;
    w.write_all(&32u16.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data_len.to_le_bytes())
}
