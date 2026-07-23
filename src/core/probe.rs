//! core/probe.rs
//!
//! Read-only probing of technical audio properties:
//! - duration
//! - average bitrate
//! - sample rate
//! - channel count
//!
//! MP3 files use a lightweight MPEG-header probe first. When that probe
//! succeeds but leaves fields unavailable, Symphonia is used to fill only the
//! missing properties. Other formats use Symphonia directly.

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

/// Technical properties derived from the media stream or container.
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioProperties {
    pub duration_ms: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
}

impl AudioProperties {
    fn needs_fallback(self) -> bool {
        self.duration_ms.is_none()
            || self.bitrate_kbps.is_none()
            || self.sample_rate_hz.is_none()
            || self.channels.is_none()
    }

    fn with_missing_filled_from(mut self, fallback: Self) -> Self {
        self.duration_ms = self.duration_ms.or(fallback.duration_ms);
        self.bitrate_kbps = self.bitrate_kbps.or(fallback.bitrate_kbps);
        self.sample_rate_hz = self.sample_rate_hz.or(fallback.sample_rate_hz);
        self.channels = self.channels.or(fallback.channels);

        self
    }
}

/// Probe a file for stream/container-derived technical properties.
///
/// MP3 behavior:
/// 1. Try the fast MPEG-header probe.
/// 2. If it returns incomplete properties, ask Symphonia to fill gaps.
/// 3. If Symphonia fails but the fast probe succeeded, preserve the fast result.
/// 4. If both probes fail, return a combined error.
pub fn probe_audio_properties(path: &Path) -> Result<AudioProperties, String> {
    if !is_mp3_path(path) {
        return probe_generic_audio_properties(path);
    }

    match probe_mp3_properties(path) {
        Ok(mp3_properties) if !mp3_properties.needs_fallback() => Ok(mp3_properties),

        Ok(mp3_properties) => match probe_generic_audio_properties(path) {
            Ok(generic_properties) => {
                Ok(mp3_properties.with_missing_filled_from(generic_properties))
            }
            Err(_) => Ok(mp3_properties),
        },

        Err(mp3_error) => match probe_generic_audio_properties(path) {
            Ok(generic_properties) => Ok(generic_properties),
            Err(generic_error) => Err(format!(
                "MP3 probe failed: {mp3_error}; generic probe failed: {generic_error}"
            )),
        },
    }
}

/// Return the estimated number of MP3 audio bytes after excluding common
/// leading ID3v2 and trailing ID3v1 tags.
///
/// This supports fallback average-bitrate calculation when duration is known
/// from another source, such as TLEN.
pub fn mp3_audio_bytes_excluding_id3(path: &Path) -> Option<u64> {
    if !is_mp3_path(path) {
        return None;
    }

    let mut file = File::open(path).ok()?;
    let total_size = file.metadata().ok()?.len();

    let leading_tag_size = id3v2_total_size(&mut file).unwrap_or(0);
    let trailing_tag_size = id3v1_size(&mut file, total_size).unwrap_or(0);

    total_size.checked_sub(leading_tag_size + trailing_tag_size)
}

/// Calculate average bitrate in decimal kilobits per second.
///
/// Because one bit per millisecond equals one decimal kilobit per second:
///
/// `kbps = (audio_bytes * 8) / duration_ms`
pub fn average_bitrate_kbps_from_audio_bytes(audio_bytes: u64, duration_ms: u32) -> Option<u32> {
    if audio_bytes == 0 || duration_ms == 0 {
        return None;
    }

    let bits = u128::from(audio_bytes) * 8;
    let kbps = bits / u128::from(duration_ms);

    u32::try_from(kbps).ok().filter(|value| *value > 0)
}

fn is_mp3_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"))
}

fn probe_mp3_properties(path: &Path) -> Result<AudioProperties, String> {
    let mut file = File::open(path).map_err(|e| format!("Open failed: {e}"))?;

    let audio_start = id3v2_total_size(&mut file).unwrap_or(0);

    file.seek(SeekFrom::Start(audio_start))
        .map_err(|e| format!("Seek failed: {e}"))?;

    let mut scan_buffer = vec![0_u8; MP3_SCAN_LIMIT];
    let scan_length = file
        .read(&mut scan_buffer)
        .map_err(|e| format!("Read failed: {e}"))?;
    scan_buffer.truncate(scan_length);

    let frame_relative_offset =
        find_first_mpeg_frame(&scan_buffer).ok_or_else(|| "No MPEG frame found.".to_string())?;

    let frame_absolute_offset = audio_start + frame_relative_offset as u64;

    file.seek(SeekFrom::Start(frame_absolute_offset))
        .map_err(|e| format!("Seek to frame failed: {e}"))?;

    let mut frame_buffer = vec![0_u8; FRAME_READ_LIMIT];
    let frame_length = file
        .read(&mut frame_buffer)
        .map_err(|e| format!("Frame read failed: {e}"))?;
    frame_buffer.truncate(frame_length);

    if frame_buffer.len() < 4 {
        return Err("Frame too short.".to_string());
    }

    let header = Mp3FrameHeader::parse([
        frame_buffer[0],
        frame_buffer[1],
        frame_buffer[2],
        frame_buffer[3],
    ])
    .ok_or_else(|| "Invalid MPEG frame header.".to_string())?;

    let mut properties = AudioProperties {
        duration_ms: None,
        bitrate_kbps: Some(header.bitrate_kbps),
        sample_rate_hz: Some(header.sample_rate_hz),
        channels: Some(header.channels()),
    };

    if let Some(xing) = parse_xing_or_info(&frame_buffer, &header) {
        let duration_ms = duration_ms_from_frame_count(
            xing.frames,
            header.samples_per_frame(),
            header.sample_rate_hz,
        );

        let average_bitrate = xing.bytes.and_then(|bytes| {
            duration_ms.and_then(|duration| {
                average_bitrate_kbps_from_audio_bytes(u64::from(bytes), duration)
            })
        });

        properties.duration_ms = duration_ms;
        properties.bitrate_kbps = average_bitrate.or(properties.bitrate_kbps);

        return Ok(properties);
    }

    if let Some(vbri) = parse_vbri(&frame_buffer, &header) {
        let duration_ms = duration_ms_from_frame_count(
            Some(vbri.frames),
            header.samples_per_frame(),
            header.sample_rate_hz,
        );

        let average_bitrate = duration_ms.and_then(|duration| {
            average_bitrate_kbps_from_audio_bytes(u64::from(vbri.bytes), duration)
        });

        properties.duration_ms = duration_ms;
        properties.bitrate_kbps = average_bitrate.or(properties.bitrate_kbps);

        return Ok(properties);
    }

    Ok(properties)
}

fn probe_generic_audio_properties(path: &Path) -> Result<AudioProperties, String> {
    let file = File::open(path).map_err(|e| format!("Open failed: {e}"))?;

    let media_source = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let mut hint = Hint::new();

    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }

    let mut format_options = FormatOptions::default();
    format_options.enable_gapless = true;

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            media_source,
            &format_options,
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Format probe failed: {e}"))?;

    let format = probed.format;

    let track = format
        .default_track()
        .ok_or_else(|| "No supported audio track found.".to_string())?;

    let parameters = &track.codec_params;

    let duration_ms = duration_from_params(parameters.time_base, parameters.n_frames);

    let channels = parameters
        .channels
        .map(|channels| channels.count())
        .and_then(|count| u8::try_from(count).ok())
        .filter(|count| *count > 0);

    Ok(AudioProperties {
        duration_ms,
        bitrate_kbps: None,
        sample_rate_hz: parameters.sample_rate,
        channels,
    })
}

fn id3v2_total_size(file: &mut File) -> Option<u64> {
    file.seek(SeekFrom::Start(0)).ok()?;

    let mut header = [0_u8; 10];
    file.read_exact(&mut header).ok()?;

    if &header[0..3] != b"ID3" {
        return Some(0);
    }

    if header[6..10].iter().any(|byte| byte & 0x80 != 0) {
        return None;
    }

    let tag_size = u64::from(syncsafe_u32([header[6], header[7], header[8], header[9]]));

    let footer_present = header[3] == 4 && (header[5] & 0x10) != 0;

    Some(10 + tag_size + if footer_present { 10 } else { 0 })
}

fn id3v1_size(file: &mut File, total_size: u64) -> Option<u64> {
    if total_size < 128 {
        return Some(0);
    }

    file.seek(SeekFrom::End(-128)).ok()?;

    let mut marker = [0_u8; 3];
    file.read_exact(&mut marker).ok()?;

    if &marker == b"TAG" {
        Some(128)
    } else {
        Some(0)
    }
}

fn syncsafe_u32(bytes: [u8; 4]) -> u32 {
    (u32::from(bytes[0]) << 21)
        | (u32::from(bytes[1]) << 14)
        | (u32::from(bytes[2]) << 7)
        | u32::from(bytes[3])
}

fn find_first_mpeg_frame(buffer: &[u8]) -> Option<usize> {
    if buffer.len() < 4 {
        return None;
    }

    for offset in 0..=(buffer.len() - 4) {
        let header = [
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
        ];

        if Mp3FrameHeader::parse(header).is_some() {
            return Some(offset);
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
    fn parse(header: [u8; 4]) -> Option<Self> {
        let bits = u32::from_be_bytes(header);

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
        4 + 32
    }
}

fn bitrate_kbps_for_layer3(version: MpegVersion, index: u8) -> Option<u32> {
    const V1: [u32; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];

    const V2: [u32; 16] = [
        0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0,
    ];

    let bitrate = match version {
        MpegVersion::V1 => V1[index as usize],
        MpegVersion::V2 | MpegVersion::V25 => V2[index as usize],
    };

    (bitrate > 0).then_some(bitrate)
}

fn sample_rate_hz(version: MpegVersion, index: u8) -> Option<u32> {
    let rates = match version {
        MpegVersion::V1 => [44_100, 48_000, 32_000],
        MpegVersion::V2 => [22_050, 24_000, 16_000],
        MpegVersion::V25 => [11_025, 12_000, 8_000],
    };

    rates.get(index as usize).copied()
}

#[derive(Debug, Clone, Copy)]
struct XingInfo {
    frames: Option<u32>,
    bytes: Option<u32>,
}

fn parse_xing_or_info(frame: &[u8], header: &Mp3FrameHeader) -> Option<XingInfo> {
    let offset = header.xing_offset();

    if frame.len() < offset + 8 {
        return None;
    }

    let signature = &frame[offset..offset + 4];

    if signature != b"Xing" && signature != b"Info" {
        return None;
    }

    let flags = u32::from_be_bytes(frame[offset + 4..offset + 8].try_into().ok()?);

    let mut cursor = offset + 8;

    let frames = if flags & 0x1 != 0 {
        let value = u32::from_be_bytes(frame.get(cursor..cursor + 4)?.try_into().ok()?);
        cursor += 4;
        Some(value)
    } else {
        None
    };

    let bytes = if flags & 0x2 != 0 {
        let value = u32::from_be_bytes(frame.get(cursor..cursor + 4)?.try_into().ok()?);
        Some(value)
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
    let offset = header.vbri_offset();

    if frame.len() < offset + 18 {
        return None;
    }

    if &frame[offset..offset + 4] != b"VBRI" {
        return None;
    }

    let bytes = u32::from_be_bytes(frame.get(offset + 10..offset + 14)?.try_into().ok()?);
    let frames = u32::from_be_bytes(frame.get(offset + 14..offset + 18)?.try_into().ok()?);

    Some(VbriInfo { bytes, frames })
}

fn duration_ms_from_frame_count(
    frames: Option<u32>,
    samples_per_frame: u32,
    sample_rate_hz: u32,
) -> Option<u32> {
    let frames = u128::from(frames?);
    let samples_per_frame = u128::from(samples_per_frame);
    let sample_rate = u128::from(sample_rate_hz);

    if sample_rate == 0 {
        return None;
    }

    let total_samples = frames * samples_per_frame;
    let milliseconds = (total_samples * 1000) / sample_rate;

    u32::try_from(milliseconds).ok().filter(|value| *value > 0)
}

fn duration_from_params(time_base: Option<TimeBase>, frame_count: Option<u64>) -> Option<u32> {
    let time_base = time_base?;
    let frame_count = frame_count?;

    let milliseconds = time_to_ms(time_base.calc_time(frame_count));

    u32::try_from(milliseconds).ok().filter(|value| *value > 0)
}

fn time_to_ms(time: Time) -> u64 {
    let milliseconds = (time.seconds as f64 * 1000.0) + (time.frac * 1000.0);

    milliseconds.round() as u64
}
