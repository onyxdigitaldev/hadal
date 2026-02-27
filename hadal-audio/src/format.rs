//! Audio format detection and metadata.

use hadal_common::{AudioFormat, BitDepth, Codec};
use std::path::Path;
use symphonia::core::audio::SignalSpec;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::error::{AudioError, AudioResult};

/// Detailed format information extracted from an audio file.
#[derive(Debug, Clone)]
pub struct FormatInfo {
    /// Core audio format
    pub format: AudioFormat,

    /// Total duration in samples
    pub total_samples: Option<u64>,

    /// Total duration in seconds
    pub duration_secs: Option<f64>,

    /// Whether the stream is seekable
    pub seekable: bool,

    /// Container format name
    pub container: String,
}

impl FormatInfo {
    /// Probe a file and extract format information.
    pub fn probe<P: AsRef<Path>>(path: P) -> AudioResult<Self> {
        let path = path.as_ref();
        let file = std::fs::File::open(path)
            .map_err(|e| AudioError::FileOpen(format!("{}: {}", path.display(), e)))?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Build hint from file extension
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let metadata_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| AudioError::FormatProbe(e.to_string()))?;

        let format_reader = probed.format;

        // Find the first audio track
        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or(AudioError::NoAudioTrack)?;

        let codec_params = &track.codec_params;
        // Get container name from file extension
        let container = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        // Extract format details
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params
            .channels
            .map(|c| c.count() as u8)
            .unwrap_or(2);
        let bit_depth = Self::extract_bit_depth(codec_params);
        let codec = Self::extract_codec(codec_params);

        let total_samples = codec_params.n_frames;
        let duration_secs = total_samples.map(|n| n as f64 / sample_rate as f64);

        let format = AudioFormat {
            sample_rate,
            channels,
            bit_depth,
            codec,
            bitrate: codec_params.bits_per_coded_sample,
        };

        Ok(Self {
            format,
            total_samples,
            duration_secs,
            seekable: true, // Assume seekable for files
            container,
        })
    }

    /// Extract bit depth from codec parameters.
    fn extract_bit_depth(params: &CodecParameters) -> BitDepth {
        match params.bits_per_sample {
            Some(8) => BitDepth::U8,
            Some(16) => BitDepth::S16,
            Some(24) => BitDepth::S24,
            Some(32) => BitDepth::S32,
            _ => BitDepth::S16, // Default assumption
        }
    }

    /// Extract codec type from codec parameters.
    pub fn extract_codec_from_params(params: &CodecParameters) -> Codec {
        Self::extract_codec(params)
    }

    /// Extract codec type from codec parameters (internal).
    fn extract_codec(params: &CodecParameters) -> Codec {
        use symphonia::core::codecs::*;

        match params.codec {
            CODEC_TYPE_FLAC => Codec::Flac,
            CODEC_TYPE_MP3 => Codec::Mp3,
            CODEC_TYPE_AAC => Codec::Aac,
            CODEC_TYPE_VORBIS => Codec::Vorbis,
            CODEC_TYPE_OPUS => Codec::Opus,
            CODEC_TYPE_PCM_S16LE | CODEC_TYPE_PCM_S16BE | CODEC_TYPE_PCM_S24LE
            | CODEC_TYPE_PCM_S24BE | CODEC_TYPE_PCM_S32LE | CODEC_TYPE_PCM_S32BE => Codec::Wav,
            CODEC_TYPE_ALAC => Codec::Alac,
            CODEC_TYPE_WAVPACK => Codec::WavPack,
            _ => Codec::Unknown,
        }
    }

    /// Create format info from a SignalSpec (used during decoding).
    pub fn from_signal_spec(spec: &SignalSpec, codec: Codec) -> Self {
        Self {
            format: AudioFormat {
                sample_rate: spec.rate,
                channels: spec.channels.count() as u8,
                bit_depth: BitDepth::F32, // Symphonia outputs F32
                codec,
                bitrate: None,
            },
            total_samples: None,
            duration_secs: None,
            seekable: true,
            container: String::new(),
        }
    }
}

/// Supported file extensions for audio files.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "flac", "mp3", "m4a", "aac", "ogg", "oga", "opus", "wav", "wave", "aif", "aiff", "wv",
];

/// Check if a file extension is supported.
pub fn is_supported_extension(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS
        .iter()
        .any(|&e| e.eq_ignore_ascii_case(ext))
}

/// Check if a path points to a supported audio file.
pub fn is_supported_file<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref()
        .extension()
        .and_then(|e| e.to_str())
        .map(is_supported_extension)
        .unwrap_or(false)
}
