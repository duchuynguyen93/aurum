//! The playback engine: a decoder thread feeding an `rtrb` ring that a [`Sink`] drains.
//!
//! ```text
//! caller ──Command──▶ decoder thread ──f32 interleaved @ sink format──▶ ring ──▶ Sink
//!        ◀───Event───┘   (symphonia → remap → resample → gain)
//! ```
//!
//! Gapless: when the current track is fully decoded the thread immediately starts the preloaded
//! `next` track and keeps pushing into the same ring. Nothing is flushed, so there is no gap.
//! `TrackEnded`/`TrackStarted` fire when the *playhead* crosses the boundary (what is audible),
//! not when decoding does; the ring holds a few hundred ms.
//!
//! Seeking flushes the ring with a generation handshake: the decoder bumps
//! [`SinkControl::generation`] and stops pushing; the sink drains the ring on its next pull and
//! stores the generation in [`SinkControl::flushed`]; only then does the decoder push again.
//! This needs no second ring and keeps the producer/consumer split of `rtrb` intact.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use rtrb::{Producer, RingBuffer};

use crate::AudioEngine;
use crate::decoder::{Decoded, TrackStream};
use crate::error::AudioError;
use crate::gain::{GainRamp, ReplayGain, total_gain};
use crate::sink::{CpalSink, Sink, SinkControl, SinkSpec};

/// Ring length in milliseconds of audio at the sink's format.
const RING_MS: usize = 300;
/// Buffered audio required before starvation counts as an underrun.
const PRIME_MS: usize = 100;
const POSITION_INTERVAL: Duration = Duration::from_millis(100);
const FLUSH_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub enum Command {
    Load {
        path: PathBuf,
        gain: Option<ReplayGain>,
    },
    Play,
    Pause,
    Seek(Duration),
    SetVolume(f32),
    /// Preloads the track to start right after the current one. `None` clears the queue.
    QueueNext(Option<(PathBuf, Option<ReplayGain>)>),
    Stop,
    /// Ends the decoder thread. Sent by [`Engine::shutdown`] and on drop.
    Shutdown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Loaded {
        path: PathBuf,
        duration: Duration,
        sample_rate: u32,
        channels: u16,
        codec: String,
    },
    /// Playback position within the current track, about every 100 ms while playing.
    Position(Duration),
    TrackEnded {
        path: PathBuf,
    },
    /// The queued track became audible without a `Load`.
    TrackStarted {
        path: PathBuf,
    },
    Error(String),
    /// The output ran dry while audio was expected.
    Underrun,
}

/// Playback engine generic over its output.
pub struct Engine<S: Sink> {
    commands: Sender<Command>,
    events: Receiver<Event>,
    thread: Option<JoinHandle<()>>,
    sink: Option<S>,
}

/// The real-audio engine: default output device through cpal.
pub type CpalEngine = Engine<CpalSink>;

impl CpalEngine {
    pub fn new() -> Result<Self, AudioError> {
        Engine::with_sink(CpalSink::new()?)
    }
}

impl<S: Sink> Engine<S> {
    /// Starts the sink and the decoder thread. The engine begins paused.
    pub fn with_sink(mut sink: S) -> Result<Self, AudioError> {
        let spec = sink.spec();
        let samples = (spec.sample_rate as usize * usize::from(spec.channels) * RING_MS / 1000)
            .max(usize::from(spec.channels) * 64);
        let (producer, consumer) = RingBuffer::<f32>::new(samples);
        let ctrl = Arc::new(SinkControl::default());
        ctrl.paused.store(true, Ordering::Relaxed);
        sink.start(consumer, Arc::clone(&ctrl))?;

        let (commands, cmd_rx) = unbounded();
        let (evt_tx, events) = unbounded();
        let worker = Worker::new(spec, producer, ctrl, cmd_rx, evt_tx);
        let thread = std::thread::Builder::new()
            .name("aurum-decoder".into())
            .spawn(move || worker.run())
            .map_err(|e| AudioError::Sink(e.to_string()))?;
        Ok(Self {
            commands,
            events,
            thread: Some(thread),
            sink: Some(sink),
        })
    }

    pub fn send(&self, command: Command) {
        // The thread only disappears after shutdown; nothing useful to do with the error.
        let _ = self.commands.send(command);
    }

    /// A handle to the event stream. All handles share one queue (each event goes to one reader).
    pub fn events(&self) -> Receiver<Event> {
        self.events.clone()
    }

    /// Stops the decoder thread, stops the sink and returns it (e.g. to finalize a WAV file).
    pub fn shutdown(mut self) -> S {
        self.stop_threads();
        // `sink` is only taken here and in `Drop`, which cannot have run yet.
        self.sink.take().expect("sink present until shutdown")
    }

    fn stop_threads(&mut self) {
        self.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        if let Some(sink) = self.sink.as_mut() {
            sink.stop();
        }
    }
}

impl<S: Sink> Drop for Engine<S> {
    fn drop(&mut self) {
        self.stop_threads();
    }
}

impl<S: Sink> AudioEngine for Engine<S> {
    fn load(&mut self, path: &Path, gain: Option<ReplayGain>) {
        self.send(Command::Load {
            path: path.to_path_buf(),
            gain,
        });
    }
    fn play(&mut self) {
        self.send(Command::Play);
    }
    fn pause(&mut self) {
        self.send(Command::Pause);
    }
    fn stop(&mut self) {
        self.send(Command::Stop);
    }
    fn seek(&mut self, position: Duration) {
        self.send(Command::Seek(position));
    }
    fn set_volume(&mut self, volume: f32) {
        self.send(Command::SetVolume(volume));
    }
    fn queue_next(&mut self, path: &Path) {
        self.queue_next_with_gain(path, None);
    }
    fn queue_next_with_gain(&mut self, path: &Path, gain: Option<ReplayGain>) {
        self.send(Command::QueueNext(Some((path.to_path_buf(), gain))));
    }
    fn events(&self) -> Receiver<Event> {
        Engine::events(self)
    }
}

// ---------------------------------------------------------------------------------------------
// Decoder thread
// ---------------------------------------------------------------------------------------------

/// A track being decoded or preloaded.
struct Active {
    stream: TrackStream,
    path: PathBuf,
    gain: Option<ReplayGain>,
}

/// A stretch of the output stream belonging to one track, in global sink-frame indices.
struct Segment {
    path: PathBuf,
    start_frame: u64,
    /// Known once the track is fully decoded.
    end_frame: Option<u64>,
    /// Position within the track at `start_frame` (non-zero after a seek).
    base: Duration,
}

struct Worker {
    spec: SinkSpec,
    producer: Producer<f32>,
    ctrl: Arc<SinkControl>,
    cmd_rx: Receiver<Command>,
    evt_tx: Sender<Event>,
    ring_samples: usize,

    current: Option<Active>,
    next: Option<Active>,
    segments: VecDeque<Segment>,

    /// Decoded block not yet fully in the ring.
    pending: Vec<f32>,
    pending_offset: usize,
    /// Frames ever handed to the ring pipeline (pushed + pending).
    total_decoded: u64,
    /// Frames ever pushed into the ring.
    total_pushed: u64,

    volume: f32,
    ramp: GainRamp,
    last_position: Instant,
    underruns_seen: u64,
}

impl Worker {
    fn new(
        spec: SinkSpec,
        producer: Producer<f32>,
        ctrl: Arc<SinkControl>,
        cmd_rx: Receiver<Command>,
        evt_tx: Sender<Event>,
    ) -> Self {
        let ring_samples = producer.buffer().capacity();
        Self {
            spec,
            producer,
            ctrl,
            cmd_rx,
            evt_tx,
            ring_samples,
            current: None,
            next: None,
            segments: VecDeque::new(),
            pending: Vec::new(),
            pending_offset: 0,
            total_decoded: 0,
            total_pushed: 0,
            volume: 1.0,
            ramp: GainRamp::new(1.0),
            last_position: Instant::now(),
            underruns_seen: 0,
        }
    }

    fn channels(&self) -> usize {
        usize::from(self.spec.channels)
    }

    fn emit(&self, event: Event) {
        // The receiver may be gone during shutdown.
        let _ = self.evt_tx.send(event);
    }

    fn run(mut self) {
        let mut block: Vec<f32> = Vec::new();
        loop {
            // Commands first so Seek/Load feel immediate.
            loop {
                match self.cmd_rx.try_recv() {
                    Ok(Command::Shutdown) => return,
                    Ok(cmd) => self.handle(cmd),
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => return,
                }
            }

            let mut progressed = self.push_pending();
            if self.pending_offset >= self.pending.len() && self.current.is_some() {
                progressed |= self.decode_block(&mut block);
                progressed |= self.push_pending();
            }

            self.update_expect_data();
            self.report();

            if !progressed {
                let idle = self.current.is_none() && self.pending_offset >= self.pending.len();
                let wait = Duration::from_millis(if idle { 10 } else { 2 });
                match self.cmd_rx.recv_timeout(wait) {
                    Ok(Command::Shutdown) => return,
                    Ok(cmd) => self.handle(cmd),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
    }

    // --- commands ---

    fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Load { path, gain } => self.load(path, gain),
            Command::Play => self.ctrl.paused.store(false, Ordering::Relaxed),
            Command::Pause => self.ctrl.paused.store(true, Ordering::Relaxed),
            Command::Seek(to) => self.seek(to),
            Command::SetVolume(v) => self.volume = v.clamp(0.0, 1.0),
            Command::QueueNext(item) => self.queue_next(item),
            Command::Stop => {
                self.reset_output();
                self.current = None;
                self.next = None;
                self.segments.clear();
            }
            Command::Shutdown => {}
        }
    }

    fn open(&self, path: &Path, gain: Option<ReplayGain>) -> Result<Active, AudioError> {
        let stream = TrackStream::new(Decoded::open(path)?, self.spec)?;
        Ok(Active {
            stream,
            path: path.to_path_buf(),
            gain,
        })
    }

    fn load(&mut self, path: PathBuf, gain: Option<ReplayGain>) {
        let active = match self.open(&path, gain) {
            Ok(a) => a,
            Err(e) => return self.emit(Event::Error(format!("{}: {e}", path.display()))),
        };
        self.reset_output();
        self.next = None;
        self.segments.clear();
        let info = active.stream.info().clone();
        self.begin_track(active, Duration::ZERO);
        self.emit(Event::Loaded {
            path,
            duration: info.duration,
            sample_rate: info.sample_rate,
            channels: info.channels,
            codec: info.codec,
        });
    }

    fn queue_next(&mut self, item: Option<(PathBuf, Option<ReplayGain>)>) {
        let Some((path, gain)) = item else {
            self.next = None;
            return;
        };
        match self.open(&path, gain) {
            Ok(active) => {
                if self.current.is_none() && !self.segments.is_empty() {
                    // Everything is already decoded and draining: continue seamlessly.
                    self.begin_track(active, Duration::ZERO);
                } else {
                    self.next = Some(active);
                }
            }
            Err(e) => self.emit(Event::Error(format!("{}: {e}", path.display()))),
        }
    }

    fn seek(&mut self, to: Duration) {
        let Some(current) = self.current.as_mut() else {
            return self.emit(Event::Error("seek: nothing is loaded".into()));
        };
        let actual = match current.stream.seek(to) {
            Ok(pos) => pos,
            Err(e) => return self.emit(Event::Error(format!("seek: {e}"))),
        };
        let path = current.path.clone();
        let announced = self.segments.front().map(|s| s.path.clone());
        self.reset_output();
        self.segments.clear();
        self.segments.push_back(Segment {
            path: path.clone(),
            start_frame: self.total_decoded,
            end_frame: None,
            base: actual,
        });
        // The decoder had already moved on to the next track while the old one was still audible.
        if let Some(old) = announced.filter(|p| *p != path) {
            self.emit(Event::TrackEnded { path: old });
            self.emit(Event::TrackStarted { path });
        }
    }

    /// Makes `active` the current track starting at the end of everything decoded so far.
    fn begin_track(&mut self, active: Active, base: Duration) {
        self.ramp.snap(total_gain(self.volume, active.gain));
        self.segments.push_back(Segment {
            path: active.path.clone(),
            start_frame: self.total_decoded,
            end_frame: None,
            base,
        });
        self.current = Some(active);
    }

    /// Drops all buffered audio: pending block and everything in the ring.
    fn reset_output(&mut self) {
        self.pending.clear();
        self.pending_offset = 0;
        self.ctrl.expect_data.store(false, Ordering::Relaxed);
        let generation = self.ctrl.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let deadline = Instant::now() + FLUSH_TIMEOUT;
        while self.ctrl.flushed.load(Ordering::Acquire) != generation {
            if Instant::now() >= deadline {
                log::warn!("sink did not acknowledge flush in {FLUSH_TIMEOUT:?}");
                break;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
        // The ring is empty now, so everything decoded so far counts as played.
        self.total_pushed = self.total_decoded;
    }

    // --- decoding and pushing ---

    /// Decodes one block of the current track into `pending`. Returns true if it did any work.
    fn decode_block(&mut self, block: &mut Vec<f32>) -> bool {
        let Some(current) = self.current.as_mut() else {
            return false;
        };
        match current.stream.read(block) {
            Ok(true) => {
                let target = total_gain(self.volume, current.gain);
                self.ramp
                    .apply(block, usize::from(self.spec.channels), target);
                self.total_decoded += (block.len() / usize::from(self.spec.channels)) as u64;
                std::mem::swap(&mut self.pending, block);
                self.pending_offset = 0;
            }
            Ok(false) => self.finish_current(),
            Err(e) => {
                let path = current.path.clone();
                self.emit(Event::Error(format!("{}: {e}", path.display())));
                self.finish_current();
            }
        }
        true
    }

    /// The current track is fully decoded: close its segment and roll into `next` (gapless).
    fn finish_current(&mut self) {
        self.current = None;
        if let Some(seg) = self.segments.back_mut() {
            seg.end_frame = Some(self.total_decoded);
        }
        if let Some(next) = self.next.take() {
            self.begin_track(next, Duration::ZERO);
        }
    }

    /// Pushes as much of `pending` as fits, whole frames only. Returns true if anything moved.
    fn push_pending(&mut self) -> bool {
        let ch = self.channels();
        let remaining = self.pending.len() - self.pending_offset;
        if remaining == 0 {
            return false;
        }
        let room = self.producer.slots();
        let n = remaining.min(room);
        let n = n - n % ch;
        if n == 0 {
            return false;
        }
        let Ok(chunk) = self.producer.write_chunk_uninit(n) else {
            return false;
        };
        let src = &self.pending[self.pending_offset..self.pending_offset + n];
        chunk.fill_from_iter(src.iter().copied());
        self.pending_offset += n;
        self.total_pushed += (n / ch) as u64;
        true
    }

    // --- bookkeeping ---

    fn buffered_frames(&self) -> u64 {
        ((self.ring_samples - self.producer.slots()) / self.channels()) as u64
    }

    fn update_expect_data(&self) {
        let has_source = self.current.is_some() || self.pending_offset < self.pending.len();
        let expect = self.ctrl.expect_data.load(Ordering::Relaxed);
        if !has_source {
            if expect {
                self.ctrl.expect_data.store(false, Ordering::Relaxed);
            }
        } else if !expect {
            let prime = self.spec.sample_rate as usize * self.channels() * PRIME_MS / 1000;
            if self.ring_samples - self.producer.slots() >= prime.min(self.ring_samples / 2) {
                self.ctrl.expect_data.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Emits boundary events, `Position` and `Underrun`.
    fn report(&mut self) {
        let playhead = self.total_pushed.saturating_sub(self.buffered_frames());

        while let Some(end) = self.segments.front().and_then(|s| s.end_frame) {
            if playhead < end {
                break;
            }
            if let Some(done) = self.segments.pop_front() {
                self.emit(Event::TrackEnded { path: done.path });
            }
            if let Some(started) = self.segments.front() {
                self.emit(Event::TrackStarted {
                    path: started.path.clone(),
                });
            }
        }

        let underruns = self.ctrl.underruns.load(Ordering::Relaxed);
        if underruns != self.underruns_seen {
            self.underruns_seen = underruns;
            self.emit(Event::Underrun);
        }

        if !self.ctrl.paused.load(Ordering::Relaxed)
            && self.last_position.elapsed() >= POSITION_INTERVAL
            && let Some(seg) = self.segments.front()
        {
            let played = playhead.saturating_sub(seg.start_frame);
            let secs = played as f64 / f64::from(self.spec.sample_rate);
            self.last_position = Instant::now();
            self.emit(Event::Position(seg.base + Duration::from_secs_f64(secs)));
        }
    }
}
