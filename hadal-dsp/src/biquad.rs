//! Biquad filter implementation.
//!
//! Implements second-order IIR (biquad) filters using the Audio EQ Cookbook
//! formulas by Robert Bristow-Johnson for high-quality audio processing.

use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use crate::error::{DspError, DspResult};

/// Biquad filter types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    /// Low-pass filter
    LowPass,
    /// High-pass filter
    HighPass,
    /// Band-pass filter (constant skirt gain)
    BandPass,
    /// Band-pass filter (constant 0 dB peak gain)
    BandPassPeak,
    /// Notch filter (band-reject)
    Notch,
    /// All-pass filter
    AllPass,
    /// Peaking EQ filter
    PeakingEq,
    /// Low shelf filter
    LowShelf,
    /// High shelf filter
    HighShelf,
}

/// Biquad filter coefficients.
#[derive(Debug, Clone, Copy, Default)]
pub struct BiquadCoeffs {
    /// Feedforward coefficient b0
    pub b0: f64,
    /// Feedforward coefficient b1
    pub b1: f64,
    /// Feedforward coefficient b2
    pub b2: f64,
    /// Feedback coefficient a1 (negated for direct form I)
    pub a1: f64,
    /// Feedback coefficient a2 (negated for direct form I)
    pub a2: f64,
}

impl BiquadCoeffs {
    /// Create new biquad coefficients for the specified filter type.
    ///
    /// # Arguments
    ///
    /// * `filter_type` - Type of filter to create
    /// * `sample_rate` - Sample rate in Hz
    /// * `frequency` - Center/cutoff frequency in Hz
    /// * `q` - Q factor (quality factor)
    /// * `gain_db` - Gain in dB (only used for peaking/shelf filters)
    pub fn new(
        filter_type: FilterType,
        sample_rate: u32,
        frequency: f64,
        q: f64,
        gain_db: f64,
    ) -> DspResult<Self> {
        // Validate parameters
        if sample_rate == 0 {
            return Err(DspError::InvalidSampleRate(sample_rate));
        }
        if frequency <= 0.0 || frequency >= sample_rate as f64 / 2.0 {
            return Err(DspError::InvalidFrequency(frequency));
        }
        if q <= 0.0 {
            return Err(DspError::InvalidQ(q));
        }

        let omega = 2.0 * PI * frequency / sample_rate as f64;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let alpha = sin_omega / (2.0 * q);

        // For peaking and shelf filters
        let a = 10.0_f64.powf(gain_db / 40.0);

        let (b0, b1, b2, a0, a1, a2) = match filter_type {
            FilterType::LowPass => {
                let b1 = 1.0 - cos_omega;
                let b0 = b1 / 2.0;
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighPass => {
                let b1 = -(1.0 + cos_omega);
                let b0 = (1.0 + cos_omega) / 2.0;
                let b2 = b0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::BandPass => {
                let b0 = sin_omega / 2.0;
                let b1 = 0.0;
                let b2 = -sin_omega / 2.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::BandPassPeak => {
                let b0 = alpha;
                let b1 = 0.0;
                let b2 = -alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Notch => {
                let b0 = 1.0;
                let b1 = -2.0 * cos_omega;
                let b2 = 1.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::AllPass => {
                let b0 = 1.0 - alpha;
                let b1 = -2.0 * cos_omega;
                let b2 = 1.0 + alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::PeakingEq => {
                let b0 = 1.0 + alpha * a;
                let b1 = -2.0 * cos_omega;
                let b2 = 1.0 - alpha * a;
                let a0 = 1.0 + alpha / a;
                let a1 = -2.0 * cos_omega;
                let a2 = 1.0 - alpha / a;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::LowShelf => {
                let sqrt_a = a.sqrt();
                let b0 = a * ((a + 1.0) - (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha);
                let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_omega);
                let b2 = a * ((a + 1.0) - (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha);
                let a0 = (a + 1.0) + (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha;
                let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_omega);
                let a2 = (a + 1.0) + (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighShelf => {
                let sqrt_a = a.sqrt();
                let b0 = a * ((a + 1.0) + (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha);
                let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_omega);
                let b2 = a * ((a + 1.0) + (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha);
                let a0 = (a + 1.0) - (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha;
                let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_omega);
                let a2 = (a + 1.0) - (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha;
                (b0, b1, b2, a0, a1, a2)
            }
        };

        // Normalize coefficients by a0
        Ok(Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        })
    }

    /// Create bypass coefficients (no filtering).
    pub fn bypass() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

/// Biquad filter state for one channel.
#[derive(Debug, Clone, Copy, Default)]
struct BiquadState {
    /// Input delay line
    x1: f64,
    x2: f64,
    /// Output delay line
    y1: f64,
    y2: f64,
}

/// Biquad filter processor.
///
/// Implements a second-order IIR filter using Direct Form I.
/// Supports multiple channels.
#[derive(Debug, Clone)]
pub struct Biquad {
    /// Filter coefficients
    coeffs: BiquadCoeffs,
    /// Per-channel state
    state: Vec<BiquadState>,
    /// Number of channels
    channels: usize,
    /// Bypass flag
    bypass: bool,
}

impl Biquad {
    /// Create a new biquad filter.
    pub fn new(coeffs: BiquadCoeffs, channels: usize) -> Self {
        Self {
            coeffs,
            state: vec![BiquadState::default(); channels],
            channels,
            bypass: false,
        }
    }

    /// Create a bypassed (unity gain) filter.
    pub fn bypassed(channels: usize) -> Self {
        Self::new(BiquadCoeffs::bypass(), channels)
    }

    /// Update filter coefficients.
    pub fn set_coefficients(&mut self, coeffs: BiquadCoeffs) {
        self.coeffs = coeffs;
    }

    /// Set bypass state.
    pub fn set_bypass(&mut self, bypass: bool) {
        self.bypass = bypass;
    }

    /// Check if filter is bypassed.
    pub fn is_bypassed(&self) -> bool {
        self.bypass
    }

    /// Reset filter state (clear delay lines).
    pub fn reset(&mut self) {
        for state in &mut self.state {
            *state = BiquadState::default();
        }
    }

    /// Process a single sample for one channel.
    #[inline]
    fn process_sample(&mut self, sample: f32, channel: usize) -> f32 {
        if self.bypass {
            return sample;
        }

        let state = &mut self.state[channel];
        let x = sample as f64;

        // Direct Form I
        let y = self.coeffs.b0 * x
            + self.coeffs.b1 * state.x1
            + self.coeffs.b2 * state.x2
            - self.coeffs.a1 * state.y1
            - self.coeffs.a2 * state.y2;

        // Update delay lines
        state.x2 = state.x1;
        state.x1 = x;
        state.y2 = state.y1;
        state.y1 = y;

        y as f32
    }

    /// Process interleaved audio samples in-place.
    ///
    /// The input buffer should be interleaved: [L, R, L, R, ...] for stereo.
    pub fn process(&mut self, samples: &mut [f32]) {
        if self.bypass {
            return;
        }

        for (i, sample) in samples.iter_mut().enumerate() {
            let channel = i % self.channels;
            *sample = self.process_sample(*sample, channel);
        }
    }

    /// Process a block of samples and return a new buffer.
    pub fn process_block(&mut self, input: &[f32]) -> Vec<f32> {
        if self.bypass {
            return input.to_vec();
        }

        let mut output = input.to_vec();
        self.process(&mut output);
        output
    }

    /// Get the filter coefficients.
    pub fn coefficients(&self) -> &BiquadCoeffs {
        &self.coeffs
    }

    /// Get the number of channels.
    pub fn channels(&self) -> usize {
        self.channels
    }
}

/// Cascade of biquad filters for higher-order filtering.
#[derive(Debug, Clone)]
pub struct BiquadCascade {
    /// Individual biquad stages
    stages: Vec<Biquad>,
    /// Number of channels
    channels: usize,
}

impl BiquadCascade {
    /// Create a new empty cascade.
    pub fn new(channels: usize) -> Self {
        Self {
            stages: Vec::new(),
            channels,
        }
    }

    /// Add a stage to the cascade.
    pub fn add_stage(&mut self, coeffs: BiquadCoeffs) {
        self.stages.push(Biquad::new(coeffs, self.channels));
    }

    /// Clear all stages.
    pub fn clear(&mut self) {
        self.stages.clear();
    }

    /// Reset all filter states.
    pub fn reset(&mut self) {
        for stage in &mut self.stages {
            stage.reset();
        }
    }

    /// Process interleaved samples in-place.
    pub fn process(&mut self, samples: &mut [f32]) {
        for stage in &mut self.stages {
            stage.process(samples);
        }
    }

    /// Get number of stages.
    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lowpass_coefficients() {
        let coeffs = BiquadCoeffs::new(FilterType::LowPass, 48000, 1000.0, 0.707, 0.0).unwrap();

        // Verify coefficients are reasonable
        assert!(coeffs.b0 > 0.0);
        assert!(coeffs.b1 > 0.0);
        assert!(coeffs.b2 > 0.0);
    }

    #[test]
    fn test_peaking_eq() {
        let coeffs = BiquadCoeffs::new(FilterType::PeakingEq, 48000, 1000.0, 1.0, 6.0).unwrap();

        let mut filter = Biquad::new(coeffs, 2);

        // Process some samples
        let mut samples = vec![1.0f32; 100];
        filter.process(&mut samples);

        // Output should be modified (boosted at 1kHz)
        assert!(samples.iter().any(|&s| s != 1.0));
    }

    #[test]
    fn test_bypass() {
        let coeffs =
            BiquadCoeffs::new(FilterType::LowPass, 48000, 1000.0, 0.707, 0.0).unwrap();

        let mut filter = Biquad::new(coeffs, 2);
        filter.set_bypass(true);

        let input: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
        let mut output = input.clone();
        filter.process(&mut output);

        // Output should equal input when bypassed
        assert_eq!(input, output);
    }
}
