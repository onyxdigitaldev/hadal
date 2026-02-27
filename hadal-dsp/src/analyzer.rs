//! FFT-based spectrum analyzer.
//!
//! Provides real-time frequency analysis for audio visualization.

use std::sync::Arc;

use parking_lot::RwLock;
use rustfft::{num_complex::Complex, FftPlanner};

use crate::error::{DspError, DspResult};

/// Window function types for FFT analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowFunction {
    /// No windowing (rectangular)
    Rectangular,
    /// Hann window (good general purpose)
    #[default]
    Hann,
    /// Hamming window
    Hamming,
    /// Blackman window (better sidelobe rejection)
    Blackman,
    /// Blackman-Harris window (best sidelobe rejection)
    BlackmanHarris,
    /// Flat-top window (accurate amplitude measurement)
    FlatTop,
}

impl WindowFunction {
    /// Calculate window coefficient at position i of n.
    pub fn coefficient(&self, i: usize, n: usize) -> f64 {
        use std::f64::consts::PI;

        let t = i as f64 / (n - 1) as f64;

        match self {
            WindowFunction::Rectangular => 1.0,
            WindowFunction::Hann => 0.5 * (1.0 - (2.0 * PI * t).cos()),
            WindowFunction::Hamming => 0.54 - 0.46 * (2.0 * PI * t).cos(),
            WindowFunction::Blackman => {
                0.42 - 0.5 * (2.0 * PI * t).cos() + 0.08 * (4.0 * PI * t).cos()
            }
            WindowFunction::BlackmanHarris => {
                0.35875 - 0.48829 * (2.0 * PI * t).cos() + 0.14128 * (4.0 * PI * t).cos()
                    - 0.01168 * (6.0 * PI * t).cos()
            }
            WindowFunction::FlatTop => {
                0.21557895 - 0.41663158 * (2.0 * PI * t).cos()
                    + 0.277263158 * (4.0 * PI * t).cos()
                    - 0.083578947 * (6.0 * PI * t).cos()
                    + 0.006947368 * (8.0 * PI * t).cos()
            }
        }
    }

    /// Generate a window of the specified size.
    pub fn generate(&self, size: usize) -> Vec<f64> {
        (0..size).map(|i| self.coefficient(i, size)).collect()
    }
}

/// Spectrum data output from the analyzer.
#[derive(Debug, Clone)]
pub struct SpectrumData {
    /// Magnitude values (linear, 0.0 to 1.0 normalized)
    pub magnitudes: Vec<f32>,
    /// Magnitude values in decibels
    pub magnitudes_db: Vec<f32>,
    /// Frequency bin centers in Hz
    pub frequencies: Vec<f32>,
    /// Peak frequency in Hz
    pub peak_frequency: f32,
    /// Peak magnitude in dB
    pub peak_magnitude_db: f32,
    /// RMS level in dB
    pub rms_db: f32,
    /// Sample rate used for analysis
    pub sample_rate: u32,
    /// FFT size used
    pub fft_size: usize,
}

impl SpectrumData {
    /// Create empty spectrum data.
    pub fn empty(num_bins: usize, sample_rate: u32, fft_size: usize) -> Self {
        let frequencies: Vec<f32> = (0..num_bins)
            .map(|i| i as f32 * sample_rate as f32 / fft_size as f32)
            .collect();

        Self {
            magnitudes: vec![0.0; num_bins],
            magnitudes_db: vec![-100.0; num_bins],
            frequencies,
            peak_frequency: 0.0,
            peak_magnitude_db: -100.0,
            rms_db: -100.0,
            sample_rate,
            fft_size,
        }
    }

    /// Get the number of frequency bins.
    pub fn num_bins(&self) -> usize {
        self.magnitudes.len()
    }

    /// Get magnitude at a specific frequency (interpolated).
    pub fn magnitude_at(&self, frequency: f32) -> f32 {
        if self.frequencies.is_empty() {
            return 0.0;
        }

        let bin_width = self.sample_rate as f32 / self.fft_size as f32;
        let bin = frequency / bin_width;
        let bin_idx = bin.floor() as usize;

        if bin_idx >= self.magnitudes.len() - 1 {
            return self.magnitudes.last().copied().unwrap_or(0.0);
        }

        // Linear interpolation
        let frac = bin - bin_idx as f32;
        self.magnitudes[bin_idx] * (1.0 - frac) + self.magnitudes[bin_idx + 1] * frac
    }

    /// Get averaged magnitudes for a specified number of bands (logarithmic spacing).
    pub fn to_bands(&self, num_bands: usize) -> Vec<f32> {
        if num_bands == 0 || self.magnitudes.is_empty() {
            return Vec::new();
        }

        let min_freq = 20.0_f32;
        let max_freq = (self.sample_rate as f32 / 2.0).min(20000.0);
        let log_min = min_freq.ln();
        let log_max = max_freq.ln();

        let mut bands = Vec::with_capacity(num_bands);

        for i in 0..num_bands {
            let t0 = i as f32 / num_bands as f32;
            let t1 = (i + 1) as f32 / num_bands as f32;

            let freq_low = (log_min + t0 * (log_max - log_min)).exp();
            let freq_high = (log_min + t1 * (log_max - log_min)).exp();

            // Average magnitudes in this frequency range
            let bin_width = self.sample_rate as f32 / self.fft_size as f32;
            let bin_low = (freq_low / bin_width).floor() as usize;
            let bin_high = (freq_high / bin_width).ceil() as usize;

            let sum: f32 = self.magnitudes[bin_low.min(self.magnitudes.len() - 1)
                ..bin_high.min(self.magnitudes.len())]
                .iter()
                .sum();
            let count = (bin_high - bin_low).max(1) as f32;

            bands.push(sum / count);
        }

        bands
    }
}

/// Spectrum analyzer configuration.
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// FFT size (must be power of 2)
    pub fft_size: usize,
    /// Window function
    pub window: WindowFunction,
    /// Overlap factor (0.0 to 0.9)
    pub overlap: f32,
    /// Smoothing factor for temporal averaging (0.0 to 0.99)
    pub smoothing: f32,
    /// Reference level for dB calculation (typically 1.0)
    pub reference_level: f32,
    /// Floor level in dB
    pub floor_db: f32,
    /// Sample rate
    pub sample_rate: u32,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            fft_size: 2048,
            window: WindowFunction::Hann,
            overlap: 0.5,
            smoothing: 0.8,
            reference_level: 1.0,
            floor_db: -96.0,
            sample_rate: 48000,
        }
    }
}

/// Real-time spectrum analyzer.
pub struct SpectrumAnalyzer {
    /// Configuration
    config: AnalyzerConfig,
    /// FFT planner
    planner: FftPlanner<f64>,
    /// Pre-computed window coefficients
    window: Vec<f64>,
    /// Input buffer for accumulating samples
    input_buffer: Vec<f64>,
    /// Complex buffer for FFT
    fft_buffer: Vec<Complex<f64>>,
    /// Scratch buffer for FFT
    scratch: Vec<Complex<f64>>,
    /// Previous spectrum for smoothing
    prev_magnitudes: Vec<f32>,
    /// Pre-allocated magnitude buffer (reused each FFT frame)
    magnitudes_buf: Vec<f32>,
    /// Pre-allocated dB magnitude buffer (reused each FFT frame)
    magnitudes_db_buf: Vec<f32>,
    /// Pre-computed frequency bin centers (only changes on sample rate change)
    frequencies: Vec<f32>,
    /// Current spectrum data (shared for reading)
    current_data: Arc<RwLock<SpectrumData>>,
    /// Number of channels
    channels: usize,
    /// Hop size (samples between FFT frames)
    hop_size: usize,
    /// Position in input buffer
    buffer_pos: usize,
}

impl SpectrumAnalyzer {
    /// Create a new spectrum analyzer.
    pub fn new(config: AnalyzerConfig, channels: usize) -> DspResult<Self> {
        // Validate FFT size
        if !config.fft_size.is_power_of_two() {
            return Err(DspError::InvalidParameter(format!(
                "FFT size must be power of 2, got {}",
                config.fft_size
            )));
        }

        let fft_size = config.fft_size;
        let num_bins = fft_size / 2 + 1;
        let hop_size = ((1.0 - config.overlap) * fft_size as f32) as usize;

        let window = config.window.generate(fft_size);
        let prev_magnitudes = vec![0.0; num_bins];
        let bin_width = config.sample_rate as f32 / fft_size as f32;
        let frequencies: Vec<f32> = (0..num_bins).map(|i| i as f32 * bin_width).collect();
        let current_data = Arc::new(RwLock::new(SpectrumData::empty(
            num_bins,
            config.sample_rate,
            fft_size,
        )));

        let planner = FftPlanner::new();

        Ok(Self {
            config,
            planner,
            window,
            input_buffer: vec![0.0; fft_size],
            fft_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            scratch: vec![Complex::new(0.0, 0.0); fft_size],
            prev_magnitudes,
            magnitudes_buf: vec![0.0; num_bins],
            magnitudes_db_buf: vec![0.0; num_bins],
            frequencies,
            current_data,
            channels,
            hop_size: hop_size.max(1),
            buffer_pos: 0,
        })
    }

    /// Create with default configuration.
    pub fn new_default(sample_rate: u32, channels: usize) -> DspResult<Self> {
        Self::new(
            AnalyzerConfig {
                sample_rate,
                ..Default::default()
            },
            channels,
        )
    }

    /// Get a handle to read the current spectrum data.
    pub fn data(&self) -> Arc<RwLock<SpectrumData>> {
        Arc::clone(&self.current_data)
    }

    /// Get the current spectrum data (cloned).
    pub fn get_spectrum(&self) -> SpectrumData {
        self.current_data.read().clone()
    }

    /// Process interleaved audio samples.
    ///
    /// This is designed to be called from the audio thread or regularly
    /// with small buffers of audio data.
    pub fn process(&mut self, samples: &[f32]) {
        // Mix down to mono for analysis
        let frames = samples.len() / self.channels;

        for frame in 0..frames {
            // Average all channels
            let mut sample_sum = 0.0_f64;
            for ch in 0..self.channels {
                sample_sum += samples[frame * self.channels + ch] as f64;
            }
            let mono_sample = sample_sum / self.channels as f64;

            // Add to input buffer
            self.input_buffer[self.buffer_pos] = mono_sample;
            self.buffer_pos += 1;

            // When we have enough samples, perform FFT
            if self.buffer_pos >= self.config.fft_size {
                self.perform_fft();

                // Shift buffer by hop size
                let remaining = self.config.fft_size - self.hop_size;
                self.input_buffer.copy_within(self.hop_size.., 0);
                for i in remaining..self.config.fft_size {
                    self.input_buffer[i] = 0.0;
                }
                self.buffer_pos = remaining;
            }
        }
    }

    /// Perform FFT and update spectrum data.
    fn perform_fft(&mut self) {
        let fft_size = self.config.fft_size;
        let num_bins = fft_size / 2 + 1;

        // Apply window and copy to FFT buffer
        for i in 0..fft_size {
            self.fft_buffer[i] = Complex::new(self.input_buffer[i] * self.window[i], 0.0);
        }

        // Perform FFT
        let fft = self.planner.plan_fft_forward(fft_size);
        fft.process_with_scratch(&mut self.fft_buffer, &mut self.scratch);

        // Calculate magnitudes into pre-allocated buffers
        let scale = 2.0 / fft_size as f64;
        let mut peak_idx = 0;
        let mut peak_mag = 0.0_f32;
        let mut sum_sq = 0.0_f64;

        for i in 0..num_bins {
            let magnitude = self.fft_buffer[i].norm() * scale;
            let mag_f32 = magnitude as f32;

            // Apply smoothing with previous frame
            let smoothed = if self.config.smoothing > 0.0 && i < self.prev_magnitudes.len() {
                self.config.smoothing * self.prev_magnitudes[i]
                    + (1.0 - self.config.smoothing) * mag_f32
            } else {
                mag_f32
            };

            self.magnitudes_buf[i] = smoothed;

            // Calculate dB
            let db = if smoothed > 0.0 {
                20.0 * (smoothed / self.config.reference_level).log10()
            } else {
                self.config.floor_db
            };
            self.magnitudes_db_buf[i] = db.max(self.config.floor_db);

            // Track peak
            if smoothed > peak_mag {
                peak_mag = smoothed;
                peak_idx = i;
            }

            sum_sq += (smoothed as f64).powi(2);
        }

        // Calculate RMS
        let rms = (sum_sq / num_bins as f64).sqrt() as f32;
        let rms_db = if rms > 0.0 {
            20.0 * (rms / self.config.reference_level).log10()
        } else {
            self.config.floor_db
        };

        let bin_width = self.config.sample_rate as f32 / fft_size as f32;
        let peak_frequency = peak_idx as f32 * bin_width;
        let peak_magnitude_db = self.magnitudes_db_buf.get(peak_idx).copied().unwrap_or(self.config.floor_db);

        // Swap magnitudes into prev_magnitudes for next frame's smoothing (avoids clone)
        std::mem::swap(&mut self.prev_magnitudes, &mut self.magnitudes_buf);
        // Now prev_magnitudes has this frame's data, magnitudes_buf has old prev data

        // Update shared data
        let mut data = self.current_data.write();
        *data = SpectrumData {
            // prev_magnitudes now holds this frame's magnitudes
            magnitudes: self.prev_magnitudes.clone(),
            magnitudes_db: self.magnitudes_db_buf.clone(),
            frequencies: self.frequencies.clone(),
            peak_frequency,
            peak_magnitude_db,
            rms_db,
            sample_rate: self.config.sample_rate,
            fft_size,
        };
    }

    /// Reset the analyzer state.
    pub fn reset(&mut self) {
        self.input_buffer.fill(0.0);
        self.buffer_pos = 0;
        self.prev_magnitudes.fill(0.0);
        self.magnitudes_buf.fill(0.0);
        self.magnitudes_db_buf.fill(0.0);

        // Recompute frequencies (sample rate may have changed)
        let fft_size = self.config.fft_size;
        let num_bins = fft_size / 2 + 1;
        let bin_width = self.config.sample_rate as f32 / fft_size as f32;
        for i in 0..num_bins {
            self.frequencies[i] = i as f32 * bin_width;
        }

        let mut data = self.current_data.write();
        *data = SpectrumData::empty(num_bins, self.config.sample_rate, self.config.fft_size);
    }

    /// Update sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.config.sample_rate = sample_rate;
        self.reset();
    }

    /// Get FFT size.
    pub fn fft_size(&self) -> usize {
        self.config.fft_size
    }

    /// Get number of frequency bins.
    pub fn num_bins(&self) -> usize {
        self.config.fft_size / 2 + 1
    }
}

impl std::fmt::Debug for SpectrumAnalyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpectrumAnalyzer")
            .field("fft_size", &self.config.fft_size)
            .field("sample_rate", &self.config.sample_rate)
            .field("channels", &self.channels)
            .field("smoothing", &self.config.smoothing)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_creation() {
        let analyzer = SpectrumAnalyzer::new_default(48000, 2).unwrap();
        assert_eq!(analyzer.fft_size(), 2048);
        assert_eq!(analyzer.num_bins(), 1025);
    }

    #[test]
    fn test_process_samples() {
        let mut analyzer = SpectrumAnalyzer::new_default(48000, 2).unwrap();

        // Generate a 1kHz sine wave
        let sample_rate = 48000.0_f32;
        let freq = 1000.0_f32;
        let samples: Vec<f32> = (0..4096)
            .flat_map(|i| {
                let t = i as f32 / sample_rate;
                let sample = (2.0 * std::f32::consts::PI * freq * t).sin();
                vec![sample, sample] // Stereo
            })
            .collect();

        analyzer.process(&samples);

        let spectrum = analyzer.get_spectrum();

        // Peak should be near 1kHz
        let _peak_bin = (1000.0 / (sample_rate / 2048.0)).round() as usize;
        assert!(
            (spectrum.peak_frequency - 1000.0).abs() < 100.0,
            "Peak at {} Hz, expected ~1000 Hz",
            spectrum.peak_frequency
        );
    }

    #[test]
    fn test_window_functions() {
        let window = WindowFunction::Hann.generate(1024);
        assert_eq!(window.len(), 1024);

        // Hann window should be 0 at edges and 1 at center
        assert!(window[0].abs() < 0.01);
        assert!(window[1023].abs() < 0.01);
        assert!((window[512] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_spectrum_bands() {
        let mut data = SpectrumData::empty(1025, 48000, 2048);

        // Set some test magnitudes
        for i in 0..data.magnitudes.len() {
            data.magnitudes[i] = (i as f32 / data.magnitudes.len() as f32) * 0.5;
        }

        let bands = data.to_bands(10);
        assert_eq!(bands.len(), 10);

        // Higher bands should have higher average magnitudes
        assert!(bands[9] > bands[0]);
    }
}
