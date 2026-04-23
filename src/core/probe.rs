//! core/probe.rs
//!
//! Probe for read-only technical audio metadata:
//! - Duration
//! - Average bitrate
//! - Sample rate
//! - Channels

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

const MP3_SCAN_LIMIT: usize = 64 * 1024;
const FRAME_READ_LIMIT: usize = 256;

/// Read-only technical properties derived from the actual media stream/container.
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioProperties {
    pub duration_ms: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
}

/// Probe a file for stream/container-derived audio properties.
/// - MP3 uses a fast custom header probe first.
/// - Non-MP3 falls back to Symphonia.
/// - On failure, callers should tolerate 'Err' and degrade to 'None' fields.
pub fn probe_audio_properties(path: &Path) -> Result<AudioProperties, String> {
    if is_mp3_path(path) {
        if let Ok(props) = probe_mp3_properties(path) {
            return Ok(props);
        }
    }
    probe_generic_audio_properties(path)
}

/// Return the estimated number of audio bytes after excluding common ID3 tags.
/// This is useful for fallback Avg. Bitrate calculation when duration is known
/// from some other source (TLEN) but no direct bitrate was probeable.
pub fn mp3_audio_bytes_excluding_id3(path: &Path) -> Option<u64> {
    if !is_mp3_path(path) {
        return None;
    }

    let mut f = File::open(path).ok()?;
    let total = f.metadata().ok()?.len();

    let start_skip = id3v2_total_size(&mut f).unwrap_or(0);
    let end_skip = id3v1_size(&mut f, total).unwrap_or(0);

    total.checked_sub(start_skip + end_skip)
}

/// Average bitrate in kbps from audio bytes and duration.
/// kbps = (bytes * 8) / ms
pub fn average_bitrate_kbps_from_audio_bytes(audio_bytes: u64, duration_ms: u32) -> Option<u32> {
    if audio_bytes == 0 || duration_ms == 0 {
        return None;
    }

    let bits = u128::from(audio_bytes) * 8;
    let kbps = bits / u128::from(duration_ms);

    u32::try_from(kbps).ok().filter(|v| *v > 0)
}

fn is_mp3_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("mp3"))
        .unwrap_or(false)
}

fn probe_mp3_properties(path: &Path) -> Result<AudioProperties, String> {
    let mut f = File::open(path).map_err(|e| format!("Open failed: {e}"))?;
    let total_size = f
        .metadata()
        .map_err(|e| format!("Metadata failed: {e}"))?
        .len();

    let audio_start = id3v2_total_size(&mut f).unwrap_or(0);

    f.seek(SeekFrom::Start(audio_start))
        .map_err(|e| format!("Seek failed: {e}"))?;

    let mut scan_buf = vec![0u8; MP3_SCAN_LIMIT];
    let scan_len = f
        .read(&mut scan_buf)
        .map_err(|e| format!("Read failed: {e}"))?;
    scan_buf.truncate(scan_len);

    let frame_rel =
        find_first_mpeg_frame(&scan_buf).ok_or_else(|| "No MPEG frame found.".to_string())?;
    let frame_abs = audio_start + frame_rel as u64;

    f.seek(SeekFrom::Start(frame_abs))
        .map_err(|e| format!("Seek to frame failed: {e}"))?;

    let mut frame_buf = vec![0u8; FRAME_READ_LIMIT];
    let frame_len = f
        .read(&mut frame_buf)
        .map_err(|e| format!("Frame read failed: {e}"))?;
    frame_buf.truncate(frame_len);

    if frame_buf.len() < 4 {
        return Err("Frame too short.".into());
    }

    let header = Mp3FrameHeader::parse([frame_buf[0], frame_buf[1], frame_buf[2], frame_buf[3]])
        .ok_or_else(|| "Invalid MPEG frame header.".to_string())?;

    let mut props = AudioProperties {
        duration_ms: None,
        bitrate_kbps: Some(header.bitrate_kbps),
        sample_rate_hz: Some(header.sample_rate_hz),
        channels: Some(header.channels()),
    };

    if let Some(xing) = parse_xing_or_info(&frame_buf, &header) {
        let duration_ms = duration_ms_from_frame_count(
            xing.frames,
            header.samples_per_frame(),
            header.sample_rate_hz,
        );
        let bitrate_kbps = xing.bytes.and_then(|bytes| {
            duration_ms.and_then(|ms| average_bitrate_kbps_from_audio_bytes(bytes.into(), ms))
        });

        props.duration_ms = duration_ms;
        props.bitrate_kbps = bitrate_kbps.or(props.bitrate_kbps);
        return Ok(props);
    }

    if let Some(vbri) = parse_vbri(&frame_buf, &header) {
        let duration_ms = duration_ms_from_frame_count(
            Some(vbri.frames),
            header.samples_per_frame(),
            header.sample_rate_hz,
        );
        let bitrate_kbps =
            duration_ms.and_then(|ms| average_bitrate_kbps_from_audio_bytes(vbri.bytes.into(), ms));

        props.duration_ms = duration_ms;
        props.bitrate_kbps = bitrate_kbps.or(props.bitrate_kbps);
        return Ok(props);
    }

    // No Xing/VBRI: keep exact first-frame bitrate for MP3.
    // Duration is left None here and may be filled by TLEN or other fallback logic.
    let _ = total_size; // reserved for future heuristics
    Ok(props)
}

fn probe_generic_audio_properties(path: &Path) -> Result<AudioProperties, String> {
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

    let sample_rate_hz = params.sample_rate;

    let channels = params
        .channels
        .map(|chs| chs.count())
        .and_then(|count| u8::try_from(count).ok())
        .filter(|count| *count > 0);

    Ok(AudioProperties {
        duration_ms,
        bitrate_kbps: None,
        sample_rate_hz,
        channels,
    })
}

fn id3v2_total_size(f: &mut File) -> Option<u64> {
    f.seek(SeekFrom::Start(0)).ok()?;

    let mut head = [0u8; 10];
    f.read_exact(&mut head).ok()?;

    if &head[0..3] != b"ID3" {
        return Some(0);
    }

    // Syncsafe size bytes must have top bit clear.
    if head[6..10].iter().any(|b| b & 0x80 != 0) {
        return None;
    }

    let tag_size = syncsafe_u32([head[6], head[7], head[8], head[9]]) as u64;
    let footer_present = (head[5] & 0x10) != 0;

    Some(10 + tag_size + if footer_present { 10 } else { 0 })
}

fn id3v1_size(f: &mut File, total_size: u64) -> Option<u64> {
    if total_size < 128 {
        return Some(0);
    }

    f.seek(SeekFrom::End(-128)).ok()?;

    let mut tag = [0u8; 3];
    f.read_exact(&mut tag).ok()?;

    if &tag == b"TAG" { Some(128) } else { Some(0) }
}

fn syncsafe_u32(b: [u8; 4]) -> u32 {
    ((b[0] as u32) << 21) | ((b[1] as u32) << 14) | ((b[2] as u32) << 7) | (b[3] as u32)
}

fn find_first_mpeg_frame(buf: &[u8]) -> Option<usize> {
    if buf.len() < 4 {
        return None;
    }

    for i in 0..=(buf.len() - 4) {
        let header = [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]];
        if Mp3FrameHeader::parse(header).is_some() {
            return Some(i);
        }
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MpegVersion {
    V1,
    V2,
    V25,
}

#[derive(Debug, Clone, Copy)]
struct Mp3FrameHeader {
    version: MpegVersion,
    has_crc: bool,
    bitrate_kbps: u32,
    sample_rate_hz: u32,
    channel_mode_bits: u8,
}

impl Mp3FrameHeader {
    fn parse(h: [u8; 4]) -> Option<Self> {
        let bits = u32::from_be_bytes(h);

        let sync = (bits >> 21) & 0x7ff;
        if sync != 0x7ff {
            return None;
        }

        let version_bits = ((bits >> 19) & 0b11) as u8;
        let layer_bits = ((bits >> 17) & 0b11) as u8;
        let protection_bit = ((bits >> 16) & 0b1) as u8;
        let bitrate_index = ((bits >> 12) & 0b1111) as u8;
        let sample_rate_index = ((bits >> 10) & 0b11) as u8;
        let channel_mode_bits = ((bits >> 6) & 0b11) as u8;

        let version = match version_bits {
            0b11 => MpegVersion::V1,
            0b10 => MpegVersion::V2,
            0b00 => MpegVersion::V25,
            _ => return None,
        };

        // We only support Layer III here.
        if layer_bits != 0b01 {
            return None;
        }

        if bitrate_index == 0 || bitrate_index == 0b1111 {
            return None;
        }

        if sample_rate_index == 0b11 {
            return None;
        }

        let bitrate_kbps = bitrate_kbps_for_layer3(version, bitrate_index)?;
        let sample_rate_hz = sample_rate_hz(version, sample_rate_index)?;

        Some(Self {
            version,
            has_crc: protection_bit == 0,
            bitrate_kbps,
            sample_rate_hz,
            channel_mode_bits,
        })
    }

    fn channels(self) -> u8 {
        if self.channel_mode_bits == 0b11 { 1 } else { 2 }
    }

    fn is_mono(self) -> bool {
        self.channels() == 1
    }

    fn samples_per_frame(self) -> u32 {
        match self.version {
            MpegVersion::V1 => 1152,
            MpegVersion::V2 | MpegVersion::V25 => 576,
        }
    }

    fn side_info_size(self) -> usize {
        match (self.version, self.is_mono()) {
            (MpegVersion::V1, false) => 32,
            (MpegVersion::V1, true) => 17,
            (MpegVersion::V2, false) | (MpegVersion::V25, false) => 17,
            (MpegVersion::V2, true) | (MpegVersion::V25, true) => 9,
        }
    }

    fn xing_offset(self) -> usize {
        4 + if self.has_crc { 2 } else { 0 } + self.side_info_size()
    }

    fn vbri_offset(self) -> usize {
        // VBRI is stored 32 bytes after the MPEG audio header.
        4 + 32
    }
}

fn bitrate_kbps_for_layer3(version: MpegVersion, idx: u8) -> Option<u32> {
    let table_v1: [u32; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    let table_v2: [u32; 16] = [
        0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0,
    ];

    let v = match version {
        MpegVersion::V1 => table_v1[idx as usize],
        MpegVersion::V2 | MpegVersion::V25 => table_v2[idx as usize],
    };

    if v == 0 { None } else { Some(v) }
}

fn sample_rate_hz(version: MpegVersion, idx: u8) -> Option<u32> {
    let base = match version {
        MpegVersion::V1 => [44_100, 48_000, 32_000],
        MpegVersion::V2 => [22_050, 24_000, 16_000],
        MpegVersion::V25 => [11_025, 12_000, 8_000],
    };

    base.get(idx as usize).copied()
}

#[derive(Debug, Clone, Copy)]
struct XingInfo {
    frames: Option<u32>,
    bytes: Option<u32>,
}

fn parse_xing_or_info(frame: &[u8], header: &Mp3FrameHeader) -> Option<XingInfo> {
    let off = header.xing_offset();
    if frame.len() < off + 8 {
        return None;
    }

    let sig = &frame[off..off + 4];
    if sig != b"Xing" && sig != b"Info" {
        return None;
    }

    let flags = u32::from_be_bytes(frame[off + 4..off + 8].try_into().ok()?);
    let mut cursor = off + 8;

    let frames = if flags & 0x1 != 0 {
        let v = u32::from_be_bytes(frame.get(cursor..cursor + 4)?.try_into().ok()?);
        cursor += 4;
        Some(v)
    } else {
        None
    };

    let bytes = if flags & 0x2 != 0 {
        let v = u32::from_be_bytes(frame.get(cursor..cursor + 4)?.try_into().ok()?);
        Some(v)
    } else {
        None
    };

    Some(XingInfo { frames, bytes })
}

#[derive(Debug, Clone, Copy)]
struct VbriInfo {
    bytes: u32,
    frames: u32,
}

fn parse_vbri(frame: &[u8], header: &Mp3FrameHeader) -> Option<VbriInfo> {
    let off = header.vbri_offset();
    if frame.len() < off + 18 {
        return None;
    }

    if &frame[off..off + 4] != b"VBRI" {
        return None;
    }

    let bytes = u32::from_be_bytes(frame.get(off + 10..off + 14)?.try_into().ok()?);
    let frames = u32::from_be_bytes(frame.get(off + 14..off + 18)?.try_into().ok()?);

    Some(VbriInfo { bytes, frames })
}

fn duration_ms_from_frame_count(
    frames: Option<u32>,
    samples_per_frame: u32,
    sample_rate_hz: u32,
) -> Option<u32> {
    let frames = u128::from(frames?);
    let spf = u128::from(samples_per_frame);
    let sr = u128::from(sample_rate_hz);

    if sr == 0 {
        return None;
    }

    let total_samples = frames * spf;
    let ms = (total_samples * 1000) / sr;

    u32::try_from(ms).ok().filter(|v| *v > 0)
}

fn duration_from_params(time_base: Option<TimeBase>, n_frames: Option<u64>) -> Option<u32> {
    let tb = time_base?;
    let frames = n_frames?;
    let t = tb.calc_time(frames);
    let ms = time_to_ms(t);

    u32::try_from(ms).ok().filter(|ms| *ms > 0)
}

fn time_to_ms(t: Time) -> u64 {
    let ms = (t.seconds as f64 * 1000.0) + (t.frac * 1000.0);
    ms.round() as u64
}
