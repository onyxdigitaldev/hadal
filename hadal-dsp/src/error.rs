//! DSP error types.

use thiserror::Error;

/// DSP error type.
#[derive(Error, Debug)]
pub enum DspError {
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("FFT error: {0}")]
    FftError(String),

    #[error("Buffer size mismatch: expected {expected}, got {actual}")]
    BufferSizeMismatch { expected: usize, actual: usize },

    #[error("Invalid sample rate: {0}")]
    InvalidSampleRate(u32),

    #[error("Invalid frequency: {0} Hz")]
    InvalidFrequency(f64),

    #[error("Invalid gain: {0} dB")]
    InvalidGain(f64),

    #[error("Invalid Q factor: {0}")]
    InvalidQ(f64),
}

/// DSP result type.
pub type DspResult<T> = Result<T, DspError>;
