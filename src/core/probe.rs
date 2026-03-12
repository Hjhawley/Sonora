//! core/probe.rs
//! Probe read-only technical audio properties from a media file using Symphonia.
//!
//! These properties are derived from the actual stream/container and are
//! intentionally separate from editable tag metadata.

use std::fs::File;
use std::path::Path;

use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

/// Read-only technical properties derived from the actual media stream/container.
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioProperties {
    pub duration_ms: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
}

/// Probe a file for stream/container-derived audio properties.
///
/// This is best-effort:
/// - unsupported/unreadable files return 'Err'
/// - callers should usually tolerate failure and fall back to 'None' fields
pub fn probe_audio_properties(path: &Path) -> Result<AudioProperties, String> {
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

    let format = probed.format;

    let track = format
        .default_track()
        .ok_or_else(|| "No supported audio track found.".to_string())?;

    let params = &track.codec_params;

    let duration_ms = duration_from_params(params.time_base, params.n_frames);

    // Symphonia reports bitrate in bits per second when available.
    let bitrate_kbps = params
        .bitrate
        .and_then(|bps| u32::try_from(bps / 1000).ok())
        .filter(|kbps| *kbps > 0);

    let sample_rate_hz = params.sample_rate;

    let channels = params
        .channels
        .map(|chs| chs.count())
        .and_then(|count| u8::try_from(count).ok())
        .filter(|count| *count > 0);

    Ok(AudioProperties {
        duration_ms,
        bitrate_kbps,
        sample_rate_hz,
        channels,
    })
}

fn duration_from_params(time_base: Option<TimeBase>, n_frames: Option<u64>) -> Option<u32> {
    let tb = time_base?;
    let frames = n_frames?;
    let t = tb.calc_time(frames);
    let ms = time_to_ms(t);

    u32::try_from(ms).ok()
}

fn time_to_ms(t: Time) -> u64 {
    let ms = (t.seconds as f64 * 1000.0) + (t.frac * 1000.0);
    ms.round() as u64
}
