//! Opening files with symphonia and turning them into interleaved `f32` at the sink's format.

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase};

use crate::error::AudioError;
use crate::resample::StreamResampler;
use crate::sink::SinkSpec;

/// Static facts about an opened file, reported in `Event::Loaded`.
#[derive(Debug, Clone)]
pub(crate) struct TrackInfo {
    pub duration: Duration,
    pub sample_rate: u32,
    pub channels: u16,
    pub codec: String,
}

/// An opened file: demuxer plus decoder, nothing decoded yet.
pub(crate) struct Decoded {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    pub info: TrackInfo,
}

impl Decoded {
    pub(crate) fn open(path: &Path) -> Result<Self, AudioError> {
        let file = File::open(path).map_err(|source| AudioError::Open {
            path: path.to_path_buf(),
            source,
        })?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe().probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or(AudioError::NoAudioTrack)?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or(AudioError::NoAudioTrack)?;

        // gapless: decoders drop encoder delay/padding frames they know about (MP3 via the
        // LAME/Xing header). On by default in 0.6; stated here because the engine relies on it.
        let options = AudioDecoderOptions::default().gapless(true);
        let decoder = symphonia::default::get_codecs().make_audio_decoder(params, &options)?;

        let sample_rate = params.sample_rate.ok_or(AudioError::NoAudioTrack)?;
        let channels = params.channels.as_ref().map(|c| c.count()).unwrap_or(0) as u16;
        let codec = symphonia::default::get_codecs()
            .get_audio_decoder(params.codec)
            .map(|d| d.codec.info.short_name.to_string())
            .unwrap_or_else(|| "unknown".into());
        let duration = match (track.num_frames, track.time_base, track.duration) {
            (Some(frames), _, _) => Duration::from_secs_f64(frames as f64 / f64::from(sample_rate)),
            (None, Some(tb), Some(dur)) => tb
                .calc_duration(dur)
                .map(|t| Duration::from_secs_f64(t.as_secs_f64().max(0.0)))
                .unwrap_or_default(),
            _ => Duration::ZERO,
        };
        Ok(Self {
            track_id: track.id,
            time_base: track.time_base,
            info: TrackInfo {
                duration,
                sample_rate,
                channels,
                codec,
            },
            format,
            decoder,
        })
    }

    /// Decodes the next packet of our track into `out` as interleaved `f32` (source rate and
    /// channel count). Returns the channel count of the decoded buffer, `None` at end of stream.
    /// Corrupt packets are skipped.
    fn next_samples(&mut self, out: &mut Vec<f32>) -> Result<Option<usize>, AudioError> {
        loop {
            let Some(packet) = self.format.next_packet()? else {
                return Ok(None);
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buf) => {
                    buf.copy_to_vec_interleaved::<f32>(out);
                    return Ok(Some(buf.spec().channels().count().max(1)));
                }
                Err(SymError::DecodeError(msg)) => {
                    log::warn!("skipping corrupt packet: {msg}");
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Seeks to `to` (coarse, then exact via a skip count). Returns the position the next decoded
    /// sample will have once `skip` source frames are dropped, and that `skip`.
    fn seek(&mut self, to: Duration) -> Result<(Duration, u64), AudioError> {
        let time = Time::try_from_secs_f64(to.as_secs_f64()).ok_or(AudioError::Symphonia(
            SymError::Unsupported("seek position out of range"),
        ))?;
        let seeked = self.format.seek(
            SeekMode::Coarse,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        )?;
        self.decoder.reset();
        let (Some(tb), rate) = (self.time_base, self.info.sample_rate) else {
            return Ok((to, 0));
        };
        let actual = tb.calc_time_saturating(seeked.actual_ts).as_secs_f64();
        let required = tb.calc_time_saturating(seeked.required_ts).as_secs_f64();
        if actual <= required {
            // Landed before the target: decode and drop the difference.
            let skip = ((required - actual) * f64::from(rate)).round() as u64;
            Ok((Duration::from_secs_f64(required.max(0.0)), skip))
        } else {
            Ok((Duration::from_secs_f64(actual.max(0.0)), 0))
        }
    }
}

/// One track converted to the sink's format: decode, remap channels, resample.
/// Gain is applied by the caller (it depends on the live volume).
pub(crate) struct TrackStream {
    src: Decoded,
    sink: SinkSpec,
    resampler: Option<StreamResampler>,
    /// Source frames still to drop after a seek.
    skip_frames: u64,
    ended: bool,
    interleaved: Vec<f32>,
    remapped: Vec<f32>,
}

impl TrackStream {
    pub(crate) fn new(src: Decoded, sink: SinkSpec) -> Result<Self, AudioError> {
        let mut s = Self {
            src,
            sink,
            resampler: None,
            skip_frames: 0,
            ended: false,
            interleaved: Vec::new(),
            remapped: Vec::new(),
        };
        s.reset_resampler()?;
        Ok(s)
    }

    pub(crate) fn info(&self) -> &TrackInfo {
        &self.src.info
    }

    fn reset_resampler(&mut self) -> Result<(), AudioError> {
        let rate = self.src.info.sample_rate;
        self.resampler = if rate == self.sink.sample_rate {
            None
        } else {
            Some(StreamResampler::new(
                rate,
                self.sink.sample_rate,
                usize::from(self.sink.channels),
            )?)
        };
        Ok(())
    }

    /// Seeks; returns the position the next sample corresponds to.
    pub(crate) fn seek(&mut self, to: Duration) -> Result<Duration, AudioError> {
        let (pos, skip) = self.src.seek(to)?;
        self.skip_frames = skip;
        self.ended = false;
        self.reset_resampler()?;
        Ok(pos)
    }

    /// Fills `out` (cleared first) with the next block at the sink's format. `Ok(false)` once the
    /// track is fully delivered, including the resampler tail.
    pub(crate) fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        out.clear();
        while out.is_empty() {
            if self.ended {
                return Ok(false);
            }
            let Some(src_ch) = self.src.next_samples(&mut self.interleaved)? else {
                self.ended = true;
                if let Some(r) = self.resampler.as_mut() {
                    r.finish(out)?;
                }
                continue;
            };
            if self.skip_frames > 0 {
                let frames = self.interleaved.len() / src_ch;
                let drop = (self.skip_frames as usize).min(frames);
                self.interleaved.drain(..drop * src_ch);
                self.skip_frames -= drop as u64;
            }
            if self.interleaved.is_empty() {
                continue;
            }
            remap_channels(
                &self.interleaved,
                src_ch,
                usize::from(self.sink.channels),
                &mut self.remapped,
            );
            match self.resampler.as_mut() {
                Some(r) => r.process(&self.remapped, out)?,
                None => out.extend_from_slice(&self.remapped),
            }
        }
        Ok(true)
    }
}

/// Converts interleaved audio between channel counts: duplicate mono, average to mono, otherwise
/// copy matching channels and zero-fill extras.
fn remap_channels(src: &[f32], src_ch: usize, dst_ch: usize, dst: &mut Vec<f32>) {
    dst.clear();
    if src_ch == dst_ch {
        dst.extend_from_slice(src);
    } else if src_ch == 1 {
        for &s in src {
            dst.extend(std::iter::repeat_n(s, dst_ch));
        }
    } else if dst_ch == 1 {
        for frame in src.chunks_exact(src_ch) {
            dst.push(frame.iter().sum::<f32>() / src_ch as f32);
        }
    } else {
        for frame in src.chunks_exact(src_ch) {
            for c in 0..dst_ch {
                dst.push(frame.get(c).copied().unwrap_or(0.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_is_duplicated() {
        let mut out = Vec::new();
        remap_channels(&[1.0, 2.0], 1, 2, &mut out);
        assert_eq!(out, [1.0, 1.0, 2.0, 2.0]);
    }

    #[test]
    fn stereo_is_averaged_to_mono() {
        let mut out = Vec::new();
        remap_channels(&[1.0, 3.0, 2.0, 4.0], 2, 1, &mut out);
        assert_eq!(out, [2.0, 3.0]);
    }

    #[test]
    fn extra_channels_are_zero_filled() {
        let mut out = Vec::new();
        remap_channels(&[1.0, 2.0], 2, 4, &mut out);
        assert_eq!(out, [1.0, 2.0, 0.0, 0.0]);
    }
}
