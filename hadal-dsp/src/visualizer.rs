//! Audio visualization data and processing.
//!
//! Provides real-time visualization data for spectrum analyzers,
//! waveform displays, and other audio visualizations.

use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::analyzer::{AnalyzerConfig, SpectrumAnalyzer, SpectrumData};
use crate::error::DspResult;

/// Visualization modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualizationMode {
    /// No visualization
    Off,
    /// Spectrum analyzer (bar graph)
    #[default]
    Spectrum,
    /// Spectrum with peak hold
    SpectrumPeaks,
    /// Oscilloscope waveform
    Waveform,
    /// Stereo oscilloscope (Lissajous)
    Stereo,
    /// VU meter style
    VuMeter,
    /// Combined spectrum and waveform
    Combined,
}

/// Waveform visualization data.
#[derive(Debug, Clone)]
pub struct WaveformData {
    /// Left channel samples (most recent)
    pub left: Vec<f32>,
    /// Right channel samples (most recent)
    pub right: Vec<f32>,
    /// Peak level left (-1.0 to 1.0)
    pub peak_left: f32,
    /// Peak level right (-1.0 to 1.0)
    pub peak_right: f32,
    /// RMS level left (0.0 to 1.0)
    pub rms_left: f32,
    /// RMS level right (0.0 to 1.0)
    pub rms_right: f32,
}

impl WaveformData {
    /// Create empty waveform data.
    pub fn empty(buffer_size: usize) -> Self {
        Self {
            left: vec![0.0; buffer_size],
            right: vec![0.0; buffer_size],
            peak_left: 0.0,
            peak_right: 0.0,
            rms_left: 0.0,
            rms_right: 0.0,
        }
    }
}

impl Default for WaveformData {
    fn default() -> Self {
        Self::empty(512)
    }
}

/// VU meter data.
#[derive(Debug, Clone, Default)]
pub struct VuMeterData {
    /// Left channel level (0.0 to 1.0)
    pub level_left: f32,
    /// Right channel level (0.0 to 1.0)
    pub level_right: f32,
    /// Left channel peak (with hold/decay)
    pub peak_left: f32,
    /// Right channel peak (with hold/decay)
    pub peak_right: f32,
    /// Left channel level in dB
    pub db_left: f32,
    /// Right channel level in dB
    pub db_right: f32,
    /// Clipping indicator left
    pub clipping_left: bool,
    /// Clipping indicator right
    pub clipping_right: bool,
}

/// Peak hold state for spectrum display.
#[derive(Debug, Clone)]
struct PeakHold {
    /// Current peak values
    peaks: Vec<f32>,
    /// Time since last peak update (in frames)
    hold_counter: Vec<u32>,
    /// Hold time in frames
    hold_time: u32,
    /// Decay rate per frame
    decay_rate: f32,
}

impl PeakHold {
    fn new(num_bands: usize, hold_time: u32, decay_rate: f32) -> Self {
        Self {
            peaks: vec![0.0; num_bands],
            hold_counter: vec![0; num_bands],
            hold_time,
            decay_rate,
        }
    }

    fn update(&mut self, current: &[f32]) {
        for (i, &value) in current.iter().enumerate() {
            if i >= self.peaks.len() {
                break;
            }

            if value >= self.peaks[i] {
                self.peaks[i] = value;
                self.hold_counter[i] = 0;
            } else {
                self.hold_counter[i] += 1;
                if self.hold_counter[i] > self.hold_time {
                    self.peaks[i] = (self.peaks[i] - self.decay_rate).max(value);
                }
            }
        }
    }

    fn get_peaks(&self) -> &[f32] {
        &self.peaks
    }
}

/// Shared visualization data accessible from the UI.
#[derive(Debug, Clone)]
pub struct VisualizationData {
    /// Current visualization mode
    pub mode: VisualizationMode,
    /// Spectrum data (if available)
    pub spectrum: Option<SpectrumData>,
    /// Spectrum bands (logarithmically spaced)
    pub spectrum_bands: Vec<f32>,
    /// Peak hold values for spectrum
    pub spectrum_peaks: Vec<f32>,
    /// Waveform data
    pub waveform: WaveformData,
    /// VU meter data
    pub vu_meter: VuMeterData,
    /// Number of spectrum bands to display
    pub num_bands: usize,
}

impl Default for VisualizationData {
    fn default() -> Self {
        Self {
            mode: VisualizationMode::Spectrum,
            spectrum: None,
            spectrum_bands: vec![0.0; 32],
            spectrum_peaks: vec![0.0; 32],
            waveform: WaveformData::default(),
            vu_meter: VuMeterData::default(),
            num_bands: 32,
        }
    }
}

/// Configuration for the visualizer.
#[derive(Debug, Clone)]
pub struct VisualizerConfig {
    /// Number of spectrum bands
    pub num_bands: usize,
    /// Waveform buffer size
    pub waveform_size: usize,
    /// Peak hold time in frames (~60fps)
    pub peak_hold_frames: u32,
    /// Peak decay rate per frame
    pub peak_decay_rate: f32,
    /// VU meter attack time (0.0 to 1.0)
    pub vu_attack: f32,
    /// VU meter release time (0.0 to 1.0)
    pub vu_release: f32,
    /// Analyzer configuration
    pub analyzer_config: AnalyzerConfig,
}

impl Default for VisualizerConfig {
    fn default() -> Self {
        Self {
            num_bands: 32,
            waveform_size: 512,
            peak_hold_frames: 30,
            peak_decay_rate: 0.02,
            vu_attack: 0.9,
            vu_release: 0.95,
            analyzer_config: AnalyzerConfig::default(),
        }
    }
}

/// Audio visualizer combining spectrum analysis and waveform display.
pub struct Visualizer {
    /// Configuration
    config: VisualizerConfig,
    /// Spectrum analyzer
    analyzer: SpectrumAnalyzer,
    /// Peak hold state
    peak_hold: PeakHold,
    /// Waveform buffer (left channel)
    waveform_left: VecDeque<f32>,
    /// Waveform buffer (right channel)
    waveform_right: VecDeque<f32>,
    /// VU meter state
    vu_left: f32,
    vu_right: f32,
    vu_peak_left: f32,
    vu_peak_right: f32,
    vu_peak_hold_left: u32,
    vu_peak_hold_right: u32,
    /// Number of channels
    channels: usize,
    /// Current visualization mode
    mode: VisualizationMode,
    /// Shared data for UI access
    shared_data: Arc<RwLock<VisualizationData>>,
}

impl Visualizer {
    /// Create a new visualizer.
    pub fn new(config: VisualizerConfig, sample_rate: u32, channels: usize) -> DspResult<Self> {
        let analyzer_config = AnalyzerConfig {
            sample_rate,
            ..config.analyzer_config.clone()
        };

        let analyzer = SpectrumAnalyzer::new(analyzer_config, channels)?;

        let peak_hold =
            PeakHold::new(config.num_bands, config.peak_hold_frames, config.peak_decay_rate);

        let shared_data = Arc::new(RwLock::new(VisualizationData {
            num_bands: config.num_bands,
            ..Default::default()
        }));

        Ok(Self {
            config,
            analyzer,
            peak_hold,
            waveform_left: VecDeque::new(),
            waveform_right: VecDeque::new(),
            vu_left: 0.0,
            vu_right: 0.0,
            vu_peak_left: 0.0,
            vu_peak_right: 0.0,
            vu_peak_hold_left: 0,
            vu_peak_hold_right: 0,
            channels,
            mode: VisualizationMode::Spectrum,
            shared_data,
        })
    }

    /// Create with default configuration.
    pub fn new_default(sample_rate: u32, channels: usize) -> DspResult<Self> {
        Self::new(VisualizerConfig::default(), sample_rate, channels)
    }

    /// Get shared data handle for UI access.
    pub fn data(&self) -> Arc<RwLock<VisualizationData>> {
        Arc::clone(&self.shared_data)
    }

    /// Get current visualization data (cloned).
    pub fn get_data(&self) -> VisualizationData {
        self.shared_data.read().clone()
    }

    /// Set visualization mode.
    pub fn set_mode(&mut self, mode: VisualizationMode) {
        self.mode = mode;
        let mut data = self.shared_data.write();
        data.mode = mode;
    }

    /// Get current mode.
    pub fn mode(&self) -> VisualizationMode {
        self.mode
    }

    /// Process audio samples.
    ///
    /// Call this regularly with audio data from the playback pipeline.
    pub fn process(&mut self, samples: &[f32]) {
        if self.mode == VisualizationMode::Off {
            return;
        }

        // Process spectrum analysis
        self.analyzer.process(samples);

        // Process waveform
        self.process_waveform(samples);

        // Process VU meter
        self.process_vu_meter(samples);

        // Update shared data
        self.update_shared_data();
    }

    /// Process waveform data.
    fn process_waveform(&mut self, samples: &[f32]) {
        let frames = samples.len() / self.channels;

        for frame in 0..frames {
            let left = samples[frame * self.channels];
            let right = if self.channels > 1 {
                samples[frame * self.channels + 1]
            } else {
                left
            };

            self.waveform_left.push_back(left);
            self.waveform_right.push_back(right);

            // Keep buffer at configured size
            while self.waveform_left.len() > self.config.waveform_size {
                self.waveform_left.pop_front();
            }
            while self.waveform_right.len() > self.config.waveform_size {
                self.waveform_right.pop_front();
            }
        }
    }

    /// Process VU meter data.
    fn process_vu_meter(&mut self, samples: &[f32]) {
        let frames = samples.len() / self.channels;
        if frames == 0 {
            return;
        }

        // Calculate RMS for this block
        let mut sum_left = 0.0_f64;
        let mut sum_right = 0.0_f64;
        let mut peak_left = 0.0_f32;
        let mut peak_right = 0.0_f32;

        for frame in 0..frames {
            let left = samples[frame * self.channels];
            let right = if self.channels > 1 {
                samples[frame * self.channels + 1]
            } else {
                left
            };

            sum_left += (left as f64).powi(2);
            sum_right += (right as f64).powi(2);

            peak_left = peak_left.max(left.abs());
            peak_right = peak_right.max(right.abs());
        }

        let rms_left = (sum_left / frames as f64).sqrt() as f32;
        let rms_right = (sum_right / frames as f64).sqrt() as f32;

        // Apply attack/release smoothing
        let attack = self.config.vu_attack;
        let release = self.config.vu_release;

        if rms_left > self.vu_left {
            self.vu_left = attack * self.vu_left + (1.0 - attack) * rms_left;
        } else {
            self.vu_left = release * self.vu_left + (1.0 - release) * rms_left;
        }

        if rms_right > self.vu_right {
            self.vu_right = attack * self.vu_right + (1.0 - attack) * rms_right;
        } else {
            self.vu_right = release * self.vu_right + (1.0 - release) * rms_right;
        }

        // Update peak hold
        if peak_left >= self.vu_peak_left {
            self.vu_peak_left = peak_left;
            self.vu_peak_hold_left = 0;
        } else {
            self.vu_peak_hold_left += 1;
            if self.vu_peak_hold_left > self.config.peak_hold_frames {
                self.vu_peak_left = (self.vu_peak_left - self.config.peak_decay_rate).max(0.0);
            }
        }

        if peak_right >= self.vu_peak_right {
            self.vu_peak_right = peak_right;
            self.vu_peak_hold_right = 0;
        } else {
            self.vu_peak_hold_right += 1;
            if self.vu_peak_hold_right > self.config.peak_hold_frames {
                self.vu_peak_right = (self.vu_peak_right - self.config.peak_decay_rate).max(0.0);
            }
        }
    }

    /// Update shared visualization data.
    fn update_shared_data(&mut self) {
        let spectrum = self.analyzer.get_spectrum();
        let bands = spectrum.to_bands(self.config.num_bands);

        // Update peak hold
        self.peak_hold.update(&bands);

        // Calculate waveform stats
        let (peak_left, rms_left) = Self::calculate_stats(&self.waveform_left);
        let (peak_right, rms_right) = Self::calculate_stats(&self.waveform_right);

        // Convert to dB for VU meter
        let db_left = if self.vu_left > 0.0 {
            20.0 * self.vu_left.log10()
        } else {
            -96.0
        };
        let db_right = if self.vu_right > 0.0 {
            20.0 * self.vu_right.log10()
        } else {
            -96.0
        };

        let mut data = self.shared_data.write();
        data.mode = self.mode;
        data.spectrum = Some(spectrum);
        data.spectrum_bands = bands;
        data.spectrum_peaks = self.peak_hold.get_peaks().to_vec();
        data.waveform = WaveformData {
            left: self.waveform_left.iter().copied().collect(),
            right: self.waveform_right.iter().copied().collect(),
            peak_left,
            peak_right,
            rms_left,
            rms_right,
        };
        data.vu_meter = VuMeterData {
            level_left: self.vu_left,
            level_right: self.vu_right,
            peak_left: self.vu_peak_left,
            peak_right: self.vu_peak_right,
            db_left,
            db_right,
            clipping_left: self.vu_peak_left >= 1.0,
            clipping_right: self.vu_peak_right >= 1.0,
        };
    }

    /// Calculate peak and RMS for a buffer.
    fn calculate_stats(buffer: &VecDeque<f32>) -> (f32, f32) {
        if buffer.is_empty() {
            return (0.0, 0.0);
        }

        let mut peak = 0.0_f32;
        let mut sum_sq = 0.0_f64;

        for &sample in buffer {
            peak = peak.max(sample.abs());
            sum_sq += (sample as f64).powi(2);
        }

        let rms = (sum_sq / buffer.len() as f64).sqrt() as f32;
        (peak, rms)
    }

    /// Reset visualizer state.
    pub fn reset(&mut self) {
        self.analyzer.reset();
        self.waveform_left.clear();
        self.waveform_right.clear();
        self.vu_left = 0.0;
        self.vu_right = 0.0;
        self.vu_peak_left = 0.0;
        self.vu_peak_right = 0.0;

        let mut data = self.shared_data.write();
        *data = VisualizationData {
            num_bands: self.config.num_bands,
            mode: self.mode,
            ..Default::default()
        };
    }

    /// Set sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.analyzer.set_sample_rate(sample_rate);
    }

    /// Set number of spectrum bands.
    pub fn set_num_bands(&mut self, num_bands: usize) {
        self.config.num_bands = num_bands;
        self.peak_hold = PeakHold::new(
            num_bands,
            self.config.peak_hold_frames,
            self.config.peak_decay_rate,
        );

        let mut data = self.shared_data.write();
        data.num_bands = num_bands;
        data.spectrum_bands = vec![0.0; num_bands];
        data.spectrum_peaks = vec![0.0; num_bands];
    }
}

impl std::fmt::Debug for Visualizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Visualizer")
            .field("mode", &self.mode)
            .field("num_bands", &self.config.num_bands)
            .field("channels", &self.channels)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visualizer_creation() {
        let viz = Visualizer::new_default(48000, 2).unwrap();
        assert_eq!(viz.mode(), VisualizationMode::Spectrum);
    }

    #[test]
    fn test_process_samples() {
        let mut viz = Visualizer::new_default(48000, 2).unwrap();

        // Generate some test samples
        let samples: Vec<f32> = (0..2048)
            .flat_map(|i| {
                let t = i as f32 / 48000.0;
                let sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
                vec![sample * 0.5, sample * 0.5]
            })
            .collect();

        viz.process(&samples);

        let data = viz.get_data();
        assert!(!data.spectrum_bands.is_empty());
        assert!(data.vu_meter.level_left > 0.0);
    }

    #[test]
    fn test_mode_switching() {
        let mut viz = Visualizer::new_default(48000, 2).unwrap();

        viz.set_mode(VisualizationMode::Waveform);
        assert_eq!(viz.mode(), VisualizationMode::Waveform);

        viz.set_mode(VisualizationMode::VuMeter);
        assert_eq!(viz.mode(), VisualizationMode::VuMeter);
    }
}
