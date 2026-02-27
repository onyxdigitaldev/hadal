//! Digital Signal Processing for Hadal.
//!
//! This crate provides audio DSP capabilities including:
//! - Parametric and graphic equalizer with biquad filters
//! - FFT-based spectrum analyzer
//! - Real-time visualization data

pub mod analyzer;
pub mod biquad;
pub mod equalizer;
pub mod error;
pub mod visualizer;

pub use analyzer::{SpectrumAnalyzer, SpectrumData};
pub use biquad::{Biquad, BiquadCoeffs, FilterType};
pub use equalizer::{Band, Equalizer, EqualizerPreset, GraphicEqualizer, ParametricEqualizer};
pub use error::{DspError, DspResult};
pub use visualizer::{VisualizationData, VisualizationMode, Visualizer, WaveformData};
