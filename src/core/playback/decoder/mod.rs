//! core/playback/decoder/mod.rs
//!
//! Audio decoding utilities (Symphonia) -> rodio::Source.
//! Seek:
//! - Try Symphonia demuxer seek first.
//! - If seek undershoots or fails, decode-skip the remaining delta.

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

mod source;

pub use source::SymphoniaSource;

/// Construct a seekable rodio Source from 'path', starting at 'start_ms'.
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
        seek_and_compute_skip_ms(&mut *format, track_id, time_base, start_ms)?
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
            Err(_e) => {
                #[cfg(debug_assertions)]
                eprintln!("[DECODER] seek failed, will decode-skip: {_e}");
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
            Err(_e) => {
                #[cfg(debug_assertions)]
                eprintln!("[DECODER] time seek failed, decode-skip: {_e}");
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
