//! core/playback/decoder.rs
//! Audio decoding utilities (Symphonia) -> rodio::Source.

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

    // Clone codec params so we can seek (mutable borrow of format) without borrow conflicts.
    let codec_params = track.codec_params.clone();

    // Duration: prefer time_base + n_frames if available.
    let duration_ms = duration_from_params(codec_params.time_base, codec_params.n_frames);

    // Build decoder (may be recreated after seek).
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Decoder init failed: {e}"))?;

    // If requested, seek before we start decoding.
    if start_ms > 0 {
        let time = Time::from(Duration::from_millis(start_ms));
        let seek_to = SeekTo::Time {
            time,
            track_id: Some(track_id),
        };

        format
            .seek(SeekMode::Accurate, seek_to)
            .map_err(|e| format!("Seek failed: {e}"))?;

        // After seek, safest is to reset decoder state by recreating it.
        decoder = symphonia::default::get_codecs()
            .make(&codec_params, &DecoderOptions::default())
            .map_err(|e| format!("Decoder re-init failed after seek: {e}"))?;
    }

    let src = SymphoniaSource::new(path.to_path_buf(), format, decoder, track_id)?;
    Ok((src, duration_ms))
}

fn duration_from_params(time_base: Option<TimeBase>, n_frames: Option<u64>) -> Option<u64> {
    let tb = time_base?;
    let frames = n_frames?;

    let t = tb.calc_time(frames);
    // Time is { seconds: u64, frac: f64 } in symphonia 0.5.x.
    let ms = (t.seconds as f64 * 1000.0) + (t.frac * 1000.0);
    Some(ms.round() as u64)
}

/// A streaming rodio Source backed by Symphonia.
pub struct SymphoniaSource {
    _path: PathBuf, // kept for debugging
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,

    // Output format for rodio
    sample_rate: u32,
    channels: u16,

    // Interleaved f32 samples ready to be yielded
    out: Vec<f32>,
    out_pos: usize,

    ended: bool,
}

impl SymphoniaSource {
    fn new(
        path: PathBuf,
        format: Box<dyn FormatReader>,
        decoder: Box<dyn Decoder>,
        track_id: u32,
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
            ended: false,
        };

        // Prime once so sample_rate/channels become correct ASAP.
        let _ = this.fill_out_buffer();

        Ok(this)
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

            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                Err(SymphoniaError::IoError(_)) => {
                    self.ended = true;
                    return Ok(());
                }
                Err(SymphoniaError::DecodeError(_)) => {
                    // Corrupt packet; skip.
                    continue;
                }
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(format!("Decode error: {e}")),
            };

            match decoded {
                AudioBufferRef::F32(buf) => {
                    // NOTE: buf is Cow<AudioBuffer<f32>>; methods are from Signal trait.
                    self.sample_rate = buf.spec().rate;
                    self.channels = buf.spec().channels.count() as u16;

                    let frames = buf.frames();
                    let chans = buf.spec().channels.count();

                    self.out.reserve(frames * chans);
                    for f in 0..frames {
                        for c in 0..chans {
                            self.out.push(buf.chan(c)[f]);
                        }
                    }
                    return Ok(());
                }
                other => {
                    let spec = SignalSpec::new(other.spec().rate, other.spec().channels.clone());
                    self.sample_rate = spec.rate;
                    self.channels = spec.channels.count() as u16;

                    let frames = other.frames();
                    let chans = spec.channels.count();

                    let mut sbuf = SampleBuffer::<f32>::new(frames as u64, spec);
                    sbuf.copy_interleaved_ref(other);

                    self.out.reserve(frames * chans);
                    self.out.extend_from_slice(sbuf.samples());
                    return Ok(());
                }
            }
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
            if self.out.is_empty() && self.ended {
                return None;
            }
        }

        let s = self.out.get(self.out_pos).copied();
        self.out_pos += 1;
        s
    }
}

impl Source for SymphoniaSource {
    // rodio 0.21 uses current_span_len (not current_frame_len).
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
