//! core/playback/decoder.rs
//! Audio decoding utilities (Symphonia) -> rodio::Source.
//!
//! Seeking strategy:
//! - Try Symphonia demuxer seek first (coarse, timestamp-based).
//! - If seek undershoots or fails, decode-skip the remaining delta.
//!
//! Gapless note:
//! - Symphonia does not enable gapless handling by default.
//! - We explicitly enable it so encoder delay / end padding can be removed
//!   when container and codec metadata support it.

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use rodio::Source;

use symphonia::core::audio::{AudioBufferRef, SampleBuffer, Signal, SignalSpec};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

/// Construct a seekable rodio Source from `path`, starting at `start_ms`.
pub fn open_source_at_ms(
    path: &Path,
    start_ms: u64,
) -> Result<(SymphoniaSource, Option<u64>), String> {
    let file = File::open(path).map_err(|e| format!("Open failed: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut format_opts = FormatOptions::default();
    format_opts.enable_gapless = true;

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &MetadataOptions::default())
        .map_err(|e| format!("Format probe failed: {e}"))?;

    let mut format = probed.format;

    let track = format
        .default_track()
        .ok_or_else(|| "No supported audio track found.".to_string())?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let time_base = codec_params.time_base;
    let duration_ms = duration_from_params(codec_params.time_base, codec_params.n_frames);

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Decoder init failed: {e}"))?;

    let skip_ms = if start_ms > 0 {
        seek_and_compute_skip_ms(&mut *format, track_id, time_base, start_ms, &codec_params)?
    } else {
        0
    };

    if start_ms > 0 {
        decoder = symphonia::default::get_codecs()
            .make(&codec_params, &DecoderOptions::default())
            .map_err(|e| format!("Decoder re-init failed after seek: {e}"))?;
    }

    let src = SymphoniaSource::new(format, decoder, track_id, skip_ms)?;
    Ok((src, duration_ms))
}

fn seek_and_compute_skip_ms(
    format: &mut dyn FormatReader,
    track_id: u32,
    time_base: Option<TimeBase>,
    start_ms: u64,
    codec_params: &symphonia::core::codecs::CodecParameters,
) -> Result<u64, String> {
    let requested_time = Time::from(Duration::from_millis(start_ms));

    if let Some(tb) = time_base {
        let required_ts = tb.calc_timestamp(requested_time);

        #[cfg(debug_assertions)]
        eprintln!(
            "[DECODER] request start_ms={} => required_ts={} (time_base={:?})",
            start_ms, required_ts, tb
        );

        match format.seek(
            SeekMode::Coarse,
            SeekTo::TimeStamp {
                ts: required_ts,
                track_id,
            },
        ) {
            Ok(seeked) => {
                let actual_time = tb.calc_time(seeked.actual_ts);
                let actual_ms = time_to_ms(actual_time);

                #[cfg(debug_assertions)]
                eprintln!(
                    "[DECODER] seek ok: required_ts={} actual_ts={} => actual_ms={} (requested_ms={})",
                    seeked.required_ts, seeked.actual_ts, actual_ms, start_ms
                );

                Ok(start_ms.saturating_sub(actual_ms))
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                eprintln!("[DECODER] seek failed, will decode-skip: {e}");

                let _ = codec_params;
                Ok(start_ms)
            }
        }
    } else {
        #[cfg(debug_assertions)]
        eprintln!("[DECODER] no time_base; trying time seek else decode-skip");

        match format.seek(
            SeekMode::Coarse,
            SeekTo::Time {
                time: requested_time,
                track_id: Some(track_id),
            },
        ) {
            Ok(_) => Ok(0),
            Err(e) => {
                #[cfg(debug_assertions)]
                eprintln!("[DECODER] time seek failed, decode-skip: {e}");
                Ok(start_ms)
            }
        }
    }
}

fn duration_from_params(time_base: Option<TimeBase>, n_frames: Option<u64>) -> Option<u64> {
    let tb = time_base?;
    let frames = n_frames?;
    let t = tb.calc_time(frames);
    Some(time_to_ms(t))
}

fn time_to_ms(t: Time) -> u64 {
    let ms = (t.seconds as f64 * 1000.0) + (t.frac * 1000.0);
    ms.round() as u64
}

#[derive(Debug, Default)]
struct InitialSkip {
    requested_ms: u64,
    remaining_samples: u64,
    initialized: bool,
}

pub struct SymphoniaSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,

    sample_rate: u32,
    channels: u16,

    out: Vec<f32>,
    out_pos: usize,

    initial_skip: InitialSkip,
    ended: bool,
}

impl SymphoniaSource {
    fn new(
        format: Box<dyn FormatReader>,
        decoder: Box<dyn Decoder>,
        track_id: u32,
        skip_ms: u64,
    ) -> Result<Self, String> {
        let mut this = Self {
            format,
            decoder,
            track_id,
            sample_rate: 44_100,
            channels: 2,
            out: Vec::new(),
            out_pos: 0,
            initial_skip: InitialSkip {
                requested_ms: skip_ms,
                remaining_samples: 0,
                initialized: false,
            },
            ended: false,
        };

        // Prime once so sample rate / channel count become correct as early as possible.
        let _ = this.fill_out_buffer();

        Ok(this)
    }

    fn ensure_initial_skip_initialized(&mut self) {
        if self.initial_skip.initialized {
            return;
        }

        self.initial_skip.initialized = true;

        if self.initial_skip.requested_ms == 0 {
            self.initial_skip.remaining_samples = 0;
            return;
        }

        let frames_to_skip = ((self.initial_skip.requested_ms as f64) * (self.sample_rate as f64)
            / 1000.0)
            .ceil() as u64;

        self.initial_skip.remaining_samples = frames_to_skip * self.channels as u64;

        #[cfg(debug_assertions)]
        eprintln!(
            "[DECODER] init decode-skip: skip_ms={} => frames={} => samples={}",
            self.initial_skip.requested_ms, frames_to_skip, self.initial_skip.remaining_samples
        );
    }

    fn apply_initial_skip_to_current_buffer(&mut self) {
        if self.initial_skip.remaining_samples == 0 {
            return;
        }

        let available = self.out.len() as u64;
        let skip_now = self.initial_skip.remaining_samples.min(available) as usize;

        self.out_pos = skip_now;
        self.initial_skip.remaining_samples -= skip_now as u64;
    }

    fn fill_out_buffer(&mut self) -> Result<(), String> {
        if self.ended {
            return Ok(());
        }

        self.out.clear();
        self.out_pos = 0;

        loop {
            let packet = match self.next_target_packet()? {
                Some(packet) => packet,
                None => {
                    self.ended = true;
                    return Ok(());
                }
            };

            let (sample_rate, channels, mut samples) =
                match self.decode_packet_to_interleaved(packet) {
                    Ok(Some(decoded)) => decoded,
                    Ok(None) => continue,
                    Err(e) => return Err(e),
                };

            if samples.is_empty() {
                self.ended = true;
                return Ok(());
            }

            self.sample_rate = sample_rate;
            self.channels = channels;

            self.out.append(&mut samples);
            self.out_pos = 0;

            self.ensure_initial_skip_initialized();
            self.apply_initial_skip_to_current_buffer();
            return Ok(());
        }
    }

    fn next_target_packet(&mut self) -> Result<Option<symphonia::core::formats::Packet>, String> {
        loop {
            // Symphonia signals end-of-stream here as an IoError.
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(_)) => return Ok(None),
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(format!("Decode read error: {e}")),
            };

            if packet.track_id() == self.track_id {
                return Ok(Some(packet));
            }
        }
    }

    fn decode_packet_to_interleaved(
        &mut self,
        packet: symphonia::core::formats::Packet,
    ) -> Result<Option<(u32, u16, Vec<f32>)>, String> {
        let decoded = match self.decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::IoError(_)) => return Ok(None),
            Err(SymphoniaError::DecodeError(_)) => return Ok(None),
            Err(SymphoniaError::ResetRequired) => {
                self.decoder.reset();
                return Ok(None);
            }
            Err(e) => return Err(format!("Decode error: {e}")),
        };

        Ok(Some(audio_buffer_ref_to_interleaved_f32(decoded)))
    }
}

fn audio_buffer_ref_to_interleaved_f32(decoded: AudioBufferRef<'_>) -> (u32, u16, Vec<f32>) {
    match decoded {
        AudioBufferRef::F32(buf) => {
            let sample_rate = buf.spec().rate;
            let channels = buf.spec().channels.count() as u16;
            let frames = buf.frames();
            let channel_count = buf.spec().channels.count();

            let mut out = Vec::with_capacity(frames * channel_count);
            for frame in 0..frames {
                for channel in 0..channel_count {
                    out.push(buf.chan(channel)[frame]);
                }
            }

            (sample_rate, channels, out)
        }
        other => {
            let spec = SignalSpec::new(other.spec().rate, other.spec().channels.clone());
            let sample_rate = spec.rate;
            let channels = spec.channels.count() as u16;
            let frames = other.frames();
            let channel_count = spec.channels.count();

            let mut sbuf = SampleBuffer::<f32>::new(frames as u64, spec);
            sbuf.copy_interleaved_ref(other);

            let mut out = Vec::with_capacity(frames * channel_count);
            out.extend_from_slice(sbuf.samples());

            (sample_rate, channels, out)
        }
    }
}

impl Iterator for SymphoniaSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.out_pos >= self.out.len() {
            if self.ended {
                return None;
            }

            if self.fill_out_buffer().is_err() {
                self.ended = true;
                return None;
            }

            if self.out_pos >= self.out.len() && self.ended {
                return None;
            }
        }

        let sample = self.out.get(self.out_pos).copied();
        self.out_pos += 1;
        sample
    }
}

impl Source for SymphoniaSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
