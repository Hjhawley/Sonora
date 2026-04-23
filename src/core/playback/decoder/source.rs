//! core/playback/decoder/source.rs
//!
//! Streaming Symphonia decoder exposed as a rodio::Source.

use std::time::Duration;

use rodio::Source;

use symphonia::core::audio::{AudioBufferRef, SampleBuffer, Signal, SignalSpec};
use symphonia::core::codecs::Decoder;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatReader, Packet};

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
    pub(super) fn new(
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

    fn next_target_packet(&mut self) -> Result<Option<Packet>, String> {
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
        packet: Packet,
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
