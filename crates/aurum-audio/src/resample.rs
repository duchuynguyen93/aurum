//! Streaming wrapper around rubato's FFT resampler for interleaved `f32`.

use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use crate::error::AudioError;

const CHUNK_FRAMES: usize = 1024;

/// Converts one track's audio from `rate_in` to `rate_out`.
///
/// rubato works in fixed chunks and has a start-up delay. This wrapper hides both: the output
/// begins at the first real sample and [`finish`](Self::finish) flushes exactly the frames the
/// input implies (`ceil(frames_in * ratio)`), so no silence is inserted at either end.
pub(crate) struct StreamResampler {
    inner: Fft<f32>,
    channels: usize,
    ratio: f64,
    /// Input not yet consumed (less than one chunk after `process`).
    pending: Vec<f32>,
    /// Scratch for one chunk of resampler output.
    out_chunk: Vec<f32>,
    delay_to_skip: usize,
    /// Frames written to `out_chunk` by the last `run_chunk`.
    produced: usize,
    frames_in: u64,
    frames_out: u64,
}

impl StreamResampler {
    pub(crate) fn new(rate_in: u32, rate_out: u32, channels: usize) -> Result<Self, AudioError> {
        let inner = Fft::<f32>::new(
            rate_in as usize,
            rate_out as usize,
            CHUNK_FRAMES,
            channels,
            FixedSync::Input,
        )
        .map_err(|e| AudioError::Resample(e.to_string()))?;
        let out_chunk = vec![0.0; inner.output_frames_max() * channels];
        let delay_to_skip = inner.output_delay();
        Ok(Self {
            inner,
            channels,
            ratio: f64::from(rate_out) / f64::from(rate_in),
            pending: Vec::new(),
            out_chunk,
            delay_to_skip,
            produced: 0,
            frames_in: 0,
            frames_out: 0,
        })
    }

    /// Feeds interleaved input, appending whatever output is ready to `out`.
    pub(crate) fn process(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<(), AudioError> {
        self.frames_in += (input.len() / self.channels) as u64;
        self.pending.extend_from_slice(input);
        loop {
            let need = self.inner.input_frames_next();
            if self.pending.len() < need * self.channels {
                return Ok(());
            }
            let nin = self.run_chunk(need, None)?;
            self.pending.drain(..nin * self.channels);
            self.emit(out);
        }
    }

    /// Flushes the tail. Call once at the end of the track; the resampler is unusable after.
    pub(crate) fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), AudioError> {
        let expected = (self.ratio * self.frames_in as f64).ceil() as u64;
        let remaining = self.pending.len() / self.channels;
        let mut partial = remaining;
        while self.frames_out < expected {
            let need = self.inner.input_frames_next();
            let idx = Indexing::new().partial_len(partial);
            let nin = self.run_chunk(need, Some(&idx))?;
            self.pending.drain(..(nin.min(partial)) * self.channels);
            partial = 0;
            self.emit(out);
        }
        // `emit` caps the total at `expected`, so nothing extra to trim here.
        Ok(())
    }

    /// Runs one resampler call over the front of `pending` (zero-padded by rubato when
    /// `indexing.partial_len` is set). Returns input frames consumed. Output lands in `out_chunk`.
    fn run_chunk(&mut self, need: usize, indexing: Option<&Indexing>) -> Result<usize, AudioError> {
        if self.pending.len() < need * self.channels {
            // Partial or empty final chunk: pad so the adapter has the frames it expects.
            self.pending.resize(need * self.channels, 0.0);
        }
        let out_frames = self.out_chunk.len() / self.channels;
        let input =
            InterleavedSlice::new(&self.pending[..need * self.channels], self.channels, need)
                .map_err(|e| AudioError::Resample(e.to_string()))?;
        let mut output = InterleavedSlice::new_mut(&mut self.out_chunk, self.channels, out_frames)
            .map_err(|e| AudioError::Resample(e.to_string()))?;
        let (nin, nout) = self
            .inner
            .process_into_buffer(&input, &mut output, indexing)
            .map_err(|e| AudioError::Resample(e.to_string()))?;
        self.produced = nout;
        Ok(nin)
    }

    fn emit(&mut self, out: &mut Vec<f32>) {
        let mut start = 0;
        let mut frames = self.produced;
        let skip = self.delay_to_skip.min(frames);
        self.delay_to_skip -= skip;
        start += skip;
        frames -= skip;
        let expected = (self.ratio * self.frames_in as f64).ceil() as u64;
        // Never emit past the length implied by the input seen so far (drops the filter tail
        // and the zero-padding of a partial chunk).
        let room = expected.saturating_sub(self.frames_out) as usize;
        let take = frames.min(room);
        out.extend_from_slice(
            &self.out_chunk[start * self.channels..(start + take) * self.channels],
        );
        self.frames_out += take as u64;
        self.produced = 0;
    }
}
