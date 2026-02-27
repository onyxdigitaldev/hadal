//! Audio decoding using Symphonia.

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use symphonia::core::audio::{AudioBufferRef, SampleBuffer, SignalSpec};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

use crate::error::{AudioError, AudioResult};
use crate::format::FormatInfo;

/// Audio decoder wrapping Symphonia.
pub struct AudioDecoder {
    /// Format reader for the audio container
    format: Box<dyn FormatReader>,

    /// Audio decoder for the track
    decoder: Box<dyn Decoder>,

    /// Track ID we're decoding
    track_id: u32,

    /// Total duration in time base units
    total_duration: Option<u64>,

    /// Current position in samples
    current_sample: u64,

    /// Sample rate
    sample_rate: u32,

    /// Number of channels
    channels: u8,

    /// Reusable sample buffer
    sample_buf: Option<SampleBuffer<f32>>,

    /// Detected codec
    codec: hadal_common::Codec,

    /// Container format name
    container: String,
}

impl AudioDecoder {
    /// Open an audio file for decoding.
    pub fn open<P: AsRef<Path>>(path: P) -> AudioResult<Self> {
        let path = path.as_ref();
        let file = File::open(path)
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
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| AudioError::FormatProbe(e.to_string()))?;

        let format = probed.format;

        // Find the first audio track
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or(AudioError::NoAudioTrack)?;

        let track_id = track.id;
        let total_duration = track.codec_params.n_frames;
        let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track
            .codec_params
            .channels
            .map(|c| c.count() as u8)
            .unwrap_or(2);

        // Detect codec type
        let codec = FormatInfo::extract_codec_from_params(&track.codec_params);

        // Detect container from file extension
        let container = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_uppercase())
            .unwrap_or_default();

        // Create decoder
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .map_err(|e| AudioError::UnsupportedCodec(e.to_string()))?;

        Ok(Self {
            format,
            decoder,
            track_id,
            total_duration,
            current_sample: 0,
            sample_rate,
            channels,
            sample_buf: None,
            codec,
            container,
        })
    }

    /// Get the signal specification for the decoded audio.
    pub fn signal_spec(&self) -> SignalSpec {
        SignalSpec::new(
            self.sample_rate,
            symphonia::core::audio::Channels::FRONT_LEFT
                | symphonia::core::audio::Channels::FRONT_RIGHT,
        )
    }

    /// Get the sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of channels.
    pub fn channels(&self) -> u8 {
        self.channels
    }

    /// Get the total duration if known.
    pub fn duration(&self) -> Option<Duration> {
        self.total_duration.map(|samples| {
            Duration::from_secs_f64(samples as f64 / self.sample_rate as f64)
        })
    }

    /// Get the current position.
    pub fn position(&self) -> Duration {
        Duration::from_secs_f64(self.current_sample as f64 / self.sample_rate as f64)
    }

    /// Get format information.
    pub fn format_info(&self) -> FormatInfo {
        FormatInfo {
            format: hadal_common::AudioFormat::new(
                self.sample_rate,
                self.channels,
                hadal_common::BitDepth::F32,
                self.codec,
            ),
            total_samples: self.total_duration,
            duration_secs: self.total_duration.map(|s| s as f64 / self.sample_rate as f64),
            seekable: true,
            container: self.container.clone(),
        }
    }

    /// Decode the next packet, returning interleaved f32 samples.
    ///
    /// Returns `None` when the stream ends.
    pub fn decode_next(&mut self) -> AudioResult<Option<Vec<f32>>> {
        loop {
            // Read the next packet
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None); // End of stream
                }
                Err(e) => return Err(AudioError::Decode(e.to_string())),
            };

            // Skip packets from other tracks
            if packet.track_id() != self.track_id {
                continue;
            }

            // Decode the packet
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(symphonia::core::errors::Error::DecodeError(e)) => {
                    tracing::warn!("Decode error (skipping): {}", e);
                    continue;
                }
                Err(e) => return Err(AudioError::Decode(e.to_string())),
            };

            // Convert to f32 samples - pass sample_buf by reference to avoid borrow conflict
            let samples = Self::audio_buffer_to_f32(&mut self.sample_buf, &decoded);
            self.current_sample += (samples.len() / self.channels as usize) as u64;

            return Ok(Some(samples));
        }
    }

    /// Convert any audio buffer to interleaved f32 samples.
    fn audio_buffer_to_f32(
        sample_buf: &mut Option<SampleBuffer<f32>>,
        buffer: &AudioBufferRef,
    ) -> Vec<f32> {
        let spec = *buffer.spec();
        let duration = buffer.capacity() as u64;

        // Create or reuse sample buffer
        if sample_buf.is_none()
            || sample_buf.as_ref().unwrap().capacity() < duration as usize
        {
            *sample_buf = Some(SampleBuffer::new(duration, spec));
        }

        let buf = sample_buf.as_mut().unwrap();
        buf.copy_interleaved_ref(buffer.clone());

        buf.samples().to_vec()
    }

    /// Seek to a specific position.
    pub fn seek(&mut self, position: Duration) -> AudioResult<()> {
        let time = Time::from(position.as_secs_f64());

        self.format
            .seek(SeekMode::Accurate, SeekTo::Time { time, track_id: None })
            .map_err(|e| AudioError::Seek(e.to_string()))?;

        // Reset decoder state
        self.decoder.reset();

        // Update current sample estimate
        self.current_sample = (position.as_secs_f64() * self.sample_rate as f64) as u64;

        Ok(())
    }

    /// Seek to a specific sample position.
    pub fn seek_to_sample(&mut self, sample: u64) -> AudioResult<()> {
        let time_stamp = sample;

        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::TimeStamp {
                    ts: time_stamp,
                    track_id: self.track_id,
                },
            )
            .map_err(|e| AudioError::Seek(e.to_string()))?;

        self.decoder.reset();
        self.current_sample = sample;

        Ok(())
    }
}

impl std::fmt::Debug for AudioDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioDecoder")
            .field("track_id", &self.track_id)
            .field("sample_rate", &self.sample_rate)
            .field("channels", &self.channels)
            .field("current_sample", &self.current_sample)
            .field("total_duration", &self.total_duration)
            .field("codec", &self.codec)
            .field("container", &self.container)
            .finish()
    }
}
