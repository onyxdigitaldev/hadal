//! Audio-specific error types.

use thiserror::Error;

/// Audio-specific errors.
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Failed to open file: {0}")]
    FileOpen(String),

    #[error("Failed to probe format: {0}")]
    FormatProbe(String),

    #[error("No suitable audio track found")]
    NoAudioTrack,

    #[error("Unsupported codec: {0}")]
    UnsupportedCodec(String),

    #[error("Decode error: {0}")]
    Decode(String),

    #[error("Seek error: {0}")]
    Seek(String),

    #[error("PipeWire error: {0}")]
    PipeWire(String),

    #[error("Resampler error: {0}")]
    Resampler(String),

    #[error("Buffer underrun")]
    BufferUnderrun,

    #[error("Pipeline not initialized")]
    NotInitialized,

    #[error("Channel closed")]
    ChannelClosed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for audio operations.
pub type AudioResult<T> = Result<T, AudioError>;
