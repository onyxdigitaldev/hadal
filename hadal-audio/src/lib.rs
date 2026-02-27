//! # hadal-audio
//!
//! Audio decoding and playback engine for the Hadal music player.
//!
//! This crate provides:
//! - Audio decoding via Symphonia (FLAC, MP3, AAC, Vorbis, Opus, WAV, AIFF, ALAC)
//! - Real-time audio output via PipeWire
//! - Lock-free audio pipeline with ring buffer
//! - High-quality resampling when needed
//! - Audiophile mode for bit-perfect playback
//! - 10/31-band graphic equalizer with high-quality biquad filters
//! - Real-time spectrum analyzer and waveform visualization

pub mod decoder;
pub mod format;
pub mod pipeline;
pub mod player;
pub mod resampler;

mod error;

pub use error::{AudioError, AudioResult};
pub use format::FormatInfo;
pub use pipeline::{AudioPipeline, PipelineConfig, PipelineState};
pub use player::{AudioPlayer, PlayerCommand, PlayerState};
pub use resampler::ResamplerQuality;

// Re-export DSP types for convenience
pub use hadal_dsp::{
    Equalizer, EqualizerPreset, GraphicEqualizer, ParametricEqualizer,
    SpectrumData, VisualizationData, VisualizationMode, Visualizer,
};
