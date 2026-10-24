#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

//! Audio file I/O off the audio thread: decoding samples and writing renders.
//!
//! FL Studio "only loads .wav, .mp3 and .ogg files" into samplers [Manual: channelrack,
//! "Red Channel buttons"] and imports FLAC [Manual: fformats_save_export]; we decode the
//! same set with Symphonia (MPL-2.0).

use jet_dsp::SampleData;
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;

/// Our safety limit on decoded length: 30 minutes at 192 kHz.
pub const MAX_FRAMES: usize = 192_000 * 60 * 30;

pub const SUPPORTED_EXTENSIONS: [&str; 4] = ["wav", "mp3", "ogg", "flac"];

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported or corrupt audio file: {0}")]
    Decode(String),
    #[error("audio file has no audio track")]
    NoTrack,
    #[error("audio file exceeds the supported length")]
    TooLong,
    #[error("invalid render parameters")]
    InvalidRender,
}

impl From<SymError> for MediaError {
    fn from(error: SymError) -> Self {
        match error {
            SymError::IoError(io) => Self::Io(io),
            other => Self::Decode(other.to_string()),
        }
    }
}

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(ext))
        })
}

/// Decode a whole file to planar float. Channels beyond two are folded into L/R.
pub fn decode_file(path: &Path) -> Result<SampleData, MediaError> {
    let file = File::open(path)?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe().format(
        &hint,
        stream,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(MediaError::NoTrack)?;
    let track_id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut stereo = false;
    let mut buffer: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymError::IoError(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymError::ResetRequired) => break,
            Err(error) => return Err(error.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A damaged packet is skipped, as players do; the rest of the file still decodes.
            Err(SymError::DecodeError(_)) => continue,
            Err(error) => return Err(error.into()),
        };
        let spec = *decoded.spec();
        rate = spec.rate;
        let channels = spec.channels.count().max(1);
        stereo |= channels > 1;
        let needs_new = buffer
            .as_ref()
            .is_none_or(|b| b.capacity() < decoded.capacity() * channels);
        if needs_new {
            buffer = Some(SampleBuffer::new(decoded.capacity() as u64, spec));
        }
        let Some(samples) = buffer.as_mut() else {
            continue;
        };
        samples.copy_interleaved_ref(decoded);
        for frame in samples.samples().chunks(channels) {
            let l = frame[0];
            let r = if channels > 1 { frame[1] } else { l };
            left.push(l);
            right.push(r);
        }
        if left.len() > MAX_FRAMES {
            return Err(MediaError::TooLong);
        }
    }
    if rate == 0 {
        return Err(MediaError::Decode("unknown sample rate".into()));
    }
    Ok(if stereo {
        SampleData::stereo(rate, left, right)
    } else {
        SampleData::mono(rate, left)
    })
}

/// WAV sample formats offered by the export dialog [Manual: fformats_save_export, "WAV bit depth"].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WavFormat {
    Int16,
    Int24,
    Float32,
}

impl WavFormat {
    fn bits(self) -> u16 {
        match self {
            Self::Int16 => 16,
            Self::Int24 => 24,
            Self::Float32 => 32,
        }
    }
}

/// Write interleaved stereo float frames as a RIFF/WAVE file.
/// Integer formats clip to full scale; 32-bit float keeps overs (as FL's float render does).
pub fn write_wav(
    path: &Path,
    rate: u32,
    channels: u16,
    interleaved: &[f32],
    format: WavFormat,
) -> Result<(), MediaError> {
    if rate == 0 || channels == 0 || !interleaved.len().is_multiple_of(usize::from(channels)) {
        return Err(MediaError::InvalidRender);
    }
    let bits = format.bits();
    let block_align = channels * bits / 8;
    let data_len = interleaved.len() as u64 * u64::from(bits / 8);
    if data_len > u64::from(u32::MAX - 64) {
        return Err(MediaError::InvalidRender);
    }
    let file = File::create(path)?;
    let mut out = BufWriter::new(file);
    let tag: u16 = if format == WavFormat::Float32 { 3 } else { 1 };
    let fmt_len: u32 = 16;
    out.write_all(b"RIFF")?;
    out.write_all(&(4 + 8 + fmt_len + 8 + data_len as u32).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&fmt_len.to_le_bytes())?;
    out.write_all(&tag.to_le_bytes())?;
    out.write_all(&channels.to_le_bytes())?;
    out.write_all(&rate.to_le_bytes())?;
    out.write_all(&(rate * u32::from(block_align)).to_le_bytes())?;
    out.write_all(&block_align.to_le_bytes())?;
    out.write_all(&bits.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&(data_len as u32).to_le_bytes())?;
    for &sample in interleaved {
        let s = if sample.is_finite() { sample } else { 0.0 };
        match format {
            WavFormat::Float32 => out.write_all(&s.to_le_bytes())?,
            WavFormat::Int16 => {
                let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                out.write_all(&v.to_le_bytes())?;
            }
            WavFormat::Int24 => {
                let v = (s.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
                out.write_all(&v.to_le_bytes()[..3])?;
            }
        }
    }
    out.flush()?;
    let mut file = out
        .into_inner()
        .map_err(|e| MediaError::Io(e.into_error()))?;
    file.seek(SeekFrom::End(0))?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("jet-media-{}-{name}", std::process::id()))
    }

    #[test]
    fn wav_round_trips_through_the_decoder() {
        let path = temp("roundtrip.wav");
        let frames: Vec<f32> = (0..2000)
            .flat_map(|i| {
                let v = (i as f32 * 0.01).sin() * 0.5;
                [v, -v]
            })
            .collect();
        for format in [WavFormat::Int16, WavFormat::Int24, WavFormat::Float32] {
            write_wav(&path, 44_100, 2, &frames, format).unwrap();
            let decoded = decode_file(&path).unwrap();
            assert_eq!(decoded.rate, 44_100);
            assert_eq!(decoded.frames(), 2000);
            let right = decoded.right.as_ref().unwrap();
            let tolerance = if format == WavFormat::Int16 {
                1e-4
            } else {
                1e-6
            };
            assert!((decoded.left[100] - frames[200]).abs() < tolerance);
            assert!((right[100] - frames[201]).abs() < tolerance);
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        let path = temp("garbage.wav");
        std::fs::write(&path, b"RIFF\x10\x00\x00\x00WAVEnonsense").unwrap();
        assert!(decode_file(&path).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn extension_filter_matches_supported_formats() {
        assert!(is_supported(Path::new("kick.WAV")));
        assert!(is_supported(Path::new("loop.ogg")));
        assert!(!is_supported(Path::new("song.flp")));
    }
}
