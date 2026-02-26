//! core/playback/decoder.rs
//! Audio decoding utilities (Symphonia) -> rodio::Source.
//!
//! Robust seeking strategy:
//! 1) Try Symphonia demuxer seek (coarse, timestamp-based).
//! 2) If seek undershoots (or fails), decode-skip the remaining delta.

use std::fs::File;
use std::path::{Path, PathBuf};
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

/// Construct a new seekable rodio Source from `path`, starting at `start_ms`.
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

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
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

    // If requested, seek before decoding. If we undershoot, we’ll decode-skip.
    let mut skip_ms: u64 = 0;

    if start_ms > 0 {
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
                    track_id, // <-- symphonia 0.5.5 expects u32 here
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

                    // If coarse seek lands before requested time, decode-skip remainder.
                    skip_ms = start_ms.saturating_sub(actual_ms);

                    // Recreate decoder after seek for clean state.
                    decoder = symphonia::default::get_codecs()
                        .make(&codec_params, &DecoderOptions::default())
                        .map_err(|e| format!("Decoder re-init failed after seek: {e}"))?;
                }
                Err(e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[DECODER] seek failed, will decode-skip: {e}");
                    skip_ms = start_ms;
                }
            }
        } else {
            #[cfg(debug_assertions)]
            eprintln!("[DECODER] no time_base; trying time seek else decode-skip");

            match format.seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: requested_time,
                    track_id: Some(track_id), // <-- symphonia 0.5.5 expects Option<u32>
                },
            ) {
                Ok(_seeked) => {
                    decoder = symphonia::default::get_codecs()
                        .make(&codec_params, &DecoderOptions::default())
                        .map_err(|e| format!("Decoder re-init failed after seek: {e}"))?;
                    skip_ms = 0;
                }
                Err(e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[DECODER] time seek failed, decode-skip: {e}");
                    skip_ms = start_ms;
                }
            }
        }
    }

    let src = SymphoniaSource::new(path.to_path_buf(), format, decoder, track_id, skip_ms)?;
    Ok((src, duration_ms))
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

pub struct SymphoniaSource {
    _path: PathBuf,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,

    sample_rate: u32,
    channels: u16,

    out: Vec<f32>,
    out_pos: usize,

    // Decode-skip support (remaining interleaved samples to skip)
    skip_ms: u64,
    skip_samples_remaining: u64,
    skip_initialized: bool,

    ended: bool,
}

impl SymphoniaSource {
    fn new(
        path: PathBuf,
        format: Box<dyn FormatReader>,
        decoder: Box<dyn Decoder>,
        track_id: u32,
        skip_ms: u64,
    ) -> Result<Self, String> {
        let mut this = Self {
            _path: path,
            format,
            decoder,
            track_id,
            sample_rate: 44100,
            channels: 2,
            out: Vec::new(),
            out_pos: 0,
            skip_ms,
            skip_samples_remaining: 0,
            skip_initialized: false,
            ended: false,
        };

        // Prime once so sample_rate/channels become correct ASAP.
        let _ = this.fill_out_buffer();

        Ok(this)
    }

    fn ensure_skip_initialized(&mut self) {
        if self.skip_initialized {
            return;
        }
        self.skip_initialized = true;

        if self.skip_ms == 0 {
            self.skip_samples_remaining = 0;
            return;
        }

        let frames_to_skip =
            ((self.skip_ms as f64) * (self.sample_rate as f64) / 1000.0).ceil() as u64;
        self.skip_samples_remaining = frames_to_skip * self.channels as u64;

        #[cfg(debug_assertions)]
        eprintln!(
            "[DECODER] init decode-skip: skip_ms={} => frames={} => samples={}",
            self.skip_ms, frames_to_skip, self.skip_samples_remaining
        );
    }

    fn apply_skip_to_current_buffer(&mut self) {
        if self.skip_samples_remaining == 0 {
            return;
        }

        let available = self.out.len() as u64;
        let skip_now = self.skip_samples_remaining.min(available) as usize;

        self.out_pos = skip_now;
        self.skip_samples_remaining -= skip_now as u64;
    }

    fn fill_out_buffer(&mut self) -> Result<(), String> {
        if self.ended {
            return Ok(());
        }

        self.out.clear();
        self.out_pos = 0;

        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymphoniaError::IoError(_)) => {
                    self.ended = true;
                    return Ok(());
                }
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(format!("Decode read error: {e}")),
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            // ---- IMPORTANT: keep decoded + its borrows inside this block ----
            let (sr, ch, mut samples): (u32, u16, Vec<f32>) = {
                let decoded = match self.decoder.decode(&packet) {
                    Ok(d) => d,
                    Err(SymphoniaError::IoError(_)) => {
                        self.ended = true;
                        return Ok(());
                    }
                    Err(SymphoniaError::DecodeError(_)) => continue,
                    Err(SymphoniaError::ResetRequired) => {
                        self.decoder.reset();
                        continue;
                    }
                    Err(e) => return Err(format!("Decode error: {e}")),
                };

                match decoded {
                    AudioBufferRef::F32(buf) => {
                        let sr = buf.spec().rate;
                        let ch = buf.spec().channels.count() as u16;
                        let frames = buf.frames();
                        let chans = buf.spec().channels.count();

                        let mut out = Vec::with_capacity(frames * chans);
                        for f in 0..frames {
                            for c in 0..chans {
                                out.push(buf.chan(c)[f]);
                            }
                        }
                        (sr, ch, out)
                    }
                    other => {
                        let spec =
                            SignalSpec::new(other.spec().rate, other.spec().channels.clone());
                        let sr = spec.rate;
                        let ch = spec.channels.count() as u16;

                        let frames = other.frames();
                        let chans = spec.channels.count();

                        let mut sbuf = SampleBuffer::<f32>::new(frames as u64, spec);
                        sbuf.copy_interleaved_ref(other);

                        let mut out = Vec::with_capacity(frames * chans);
                        out.extend_from_slice(sbuf.samples());
                        (sr, ch, out)
                    }
                }
            };
            // ---- decoded dropped here; decoder borrow released ----

            // If decoder hit EOF-ish via IoError above, end cleanly.
            if samples.is_empty() {
                self.ended = true;
                return Ok(());
            }

            self.sample_rate = sr;
            self.channels = ch;

            self.out.append(&mut samples);
            self.out_pos = 0;

            self.ensure_skip_initialized();
            self.apply_skip_to_current_buffer();
            return Ok(());
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

        let s = self.out.get(self.out_pos).copied();
        self.out_pos += 1;
        s
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
