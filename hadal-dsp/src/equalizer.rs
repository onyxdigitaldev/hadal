//! Parametric and graphic equalizer implementations.
//!
//! Provides both parametric EQ (adjustable frequency, Q, and gain per band)
//! and graphic EQ (fixed frequencies, adjustable gain per band).

use serde::{Deserialize, Serialize};

use crate::biquad::{Biquad, BiquadCoeffs, FilterType};
use crate::error::DspResult;

/// Standard 10-band graphic EQ frequencies (ISO centers).
pub const GRAPHIC_EQ_FREQUENCIES: [f64; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

/// Standard 31-band graphic EQ frequencies (1/3 octave).
pub const GRAPHIC_EQ_31_FREQUENCIES: [f64; 31] = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
    500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0,
    8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
];

/// Maximum gain adjustment in dB.
pub const MAX_GAIN_DB: f64 = 12.0;

/// Minimum gain adjustment in dB.
pub const MIN_GAIN_DB: f64 = -12.0;

/// Default Q factor for graphic EQ bands.
pub const DEFAULT_GRAPHIC_Q: f64 = 1.41; // ~1 octave bandwidth

/// A single EQ band configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Band {
    /// Center frequency in Hz
    pub frequency: f64,
    /// Gain in dB (-12 to +12 typical)
    pub gain_db: f64,
    /// Q factor (bandwidth)
    pub q: f64,
    /// Filter type
    pub filter_type: FilterType,
    /// Whether this band is enabled
    pub enabled: bool,
}

impl Band {
    /// Create a new EQ band.
    pub fn new(frequency: f64, gain_db: f64, q: f64, filter_type: FilterType) -> Self {
        Self {
            frequency,
            gain_db,
            q,
            filter_type,
            enabled: true,
        }
    }

    /// Create a peaking EQ band.
    pub fn peaking(frequency: f64, gain_db: f64, q: f64) -> Self {
        Self::new(frequency, gain_db, q, FilterType::PeakingEq)
    }

    /// Create a low shelf band.
    pub fn low_shelf(frequency: f64, gain_db: f64, q: f64) -> Self {
        Self::new(frequency, gain_db, q, FilterType::LowShelf)
    }

    /// Create a high shelf band.
    pub fn high_shelf(frequency: f64, gain_db: f64, q: f64) -> Self {
        Self::new(frequency, gain_db, q, FilterType::HighShelf)
    }

    /// Create a low pass band.
    pub fn low_pass(frequency: f64, q: f64) -> Self {
        Self::new(frequency, 0.0, q, FilterType::LowPass)
    }

    /// Create a high pass band.
    pub fn high_pass(frequency: f64, q: f64) -> Self {
        Self::new(frequency, 0.0, q, FilterType::HighPass)
    }
}

impl Default for Band {
    fn default() -> Self {
        Self {
            frequency: 1000.0,
            gain_db: 0.0,
            q: 1.0,
            filter_type: FilterType::PeakingEq,
            enabled: true,
        }
    }
}

/// Common equalizer interface.
pub trait Equalizer {
    /// Get the number of bands.
    fn num_bands(&self) -> usize;

    /// Get a band by index.
    fn band(&self, index: usize) -> Option<&Band>;

    /// Get mutable reference to a band.
    fn band_mut(&mut self, index: usize) -> Option<&mut Band>;

    /// Set the gain for a band.
    fn set_band_gain(&mut self, index: usize, gain_db: f64) -> DspResult<()>;

    /// Update filter coefficients (call after changing bands).
    fn update_filters(&mut self, sample_rate: u32) -> DspResult<()>;

    /// Process audio samples in-place.
    fn process(&mut self, samples: &mut [f32]);

    /// Reset all filter states.
    fn reset(&mut self);

    /// Set bypass state.
    fn set_bypass(&mut self, bypass: bool);

    /// Check if bypassed.
    fn is_bypassed(&self) -> bool;

    /// Get all band gains as a slice.
    fn gains(&self) -> Vec<f64>;

    /// Set all band gains from a slice.
    fn set_gains(&mut self, gains: &[f64]) -> DspResult<()>;
}

/// Parametric equalizer with fully configurable bands.
#[derive(Debug, Clone)]
pub struct ParametricEqualizer {
    /// EQ bands
    bands: Vec<Band>,
    /// Biquad filters (one per band)
    filters: Vec<Biquad>,
    /// Number of audio channels
    channels: usize,
    /// Current sample rate
    sample_rate: u32,
    /// Bypass flag
    bypass: bool,
}

impl ParametricEqualizer {
    /// Create a new parametric EQ.
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            bands: Vec::new(),
            filters: Vec::new(),
            channels,
            sample_rate,
            bypass: false,
        }
    }

    /// Create a parametric EQ with the specified number of bands.
    pub fn with_bands(channels: usize, sample_rate: u32, num_bands: usize) -> DspResult<Self> {
        let mut eq = Self::new(channels, sample_rate);

        // Create default peaking bands spread across the spectrum
        let frequencies = Self::calculate_band_frequencies(num_bands);
        for freq in frequencies {
            eq.add_band(Band::peaking(freq, 0.0, 1.0))?;
        }

        Ok(eq)
    }

    /// Calculate logarithmically spaced frequencies for N bands.
    fn calculate_band_frequencies(num_bands: usize) -> Vec<f64> {
        if num_bands == 0 {
            return Vec::new();
        }

        let min_freq = 20.0_f64;
        let max_freq = 20000.0_f64;
        let log_min = min_freq.ln();
        let log_max = max_freq.ln();

        (0..num_bands)
            .map(|i| {
                let t = i as f64 / (num_bands - 1).max(1) as f64;
                (log_min + t * (log_max - log_min)).exp()
            })
            .collect()
    }

    /// Add a new band.
    pub fn add_band(&mut self, band: Band) -> DspResult<()> {
        let coeffs = BiquadCoeffs::new(
            band.filter_type,
            self.sample_rate,
            band.frequency,
            band.q,
            band.gain_db,
        )?;

        self.bands.push(band);
        self.filters.push(Biquad::new(coeffs, self.channels));

        Ok(())
    }

    /// Remove a band by index.
    pub fn remove_band(&mut self, index: usize) -> Option<Band> {
        if index < self.bands.len() {
            self.filters.remove(index);
            Some(self.bands.remove(index))
        } else {
            None
        }
    }

    /// Set band frequency.
    pub fn set_band_frequency(&mut self, index: usize, frequency: f64) -> DspResult<()> {
        if let Some(band) = self.bands.get_mut(index) {
            band.frequency = frequency;
            self.update_band_filter(index)?;
        }
        Ok(())
    }

    /// Set band Q factor.
    pub fn set_band_q(&mut self, index: usize, q: f64) -> DspResult<()> {
        if let Some(band) = self.bands.get_mut(index) {
            band.q = q;
            self.update_band_filter(index)?;
        }
        Ok(())
    }

    /// Update a single band's filter coefficients.
    fn update_band_filter(&mut self, index: usize) -> DspResult<()> {
        if let Some(band) = self.bands.get(index) {
            let coeffs = BiquadCoeffs::new(
                band.filter_type,
                self.sample_rate,
                band.frequency,
                band.q,
                band.gain_db,
            )?;
            if let Some(filter) = self.filters.get_mut(index) {
                filter.set_coefficients(coeffs);
            }
        }
        Ok(())
    }

    /// Set sample rate and recalculate all filters.
    pub fn set_sample_rate(&mut self, sample_rate: u32) -> DspResult<()> {
        self.sample_rate = sample_rate;
        self.update_filters(sample_rate)
    }
}

impl Equalizer for ParametricEqualizer {
    fn num_bands(&self) -> usize {
        self.bands.len()
    }

    fn band(&self, index: usize) -> Option<&Band> {
        self.bands.get(index)
    }

    fn band_mut(&mut self, index: usize) -> Option<&mut Band> {
        self.bands.get_mut(index)
    }

    fn set_band_gain(&mut self, index: usize, gain_db: f64) -> DspResult<()> {
        let gain_db = gain_db.clamp(MIN_GAIN_DB, MAX_GAIN_DB);

        if let Some(band) = self.bands.get_mut(index) {
            band.gain_db = gain_db;
            self.update_band_filter(index)?;
        }
        Ok(())
    }

    fn update_filters(&mut self, sample_rate: u32) -> DspResult<()> {
        self.sample_rate = sample_rate;

        for (i, band) in self.bands.iter().enumerate() {
            if let Some(filter) = self.filters.get_mut(i) {
                let coeffs = BiquadCoeffs::new(
                    band.filter_type,
                    sample_rate,
                    band.frequency,
                    band.q,
                    band.gain_db,
                )?;
                filter.set_coefficients(coeffs);
            }
        }

        Ok(())
    }

    fn process(&mut self, samples: &mut [f32]) {
        if self.bypass {
            return;
        }

        for (filter, band) in self.filters.iter_mut().zip(self.bands.iter()) {
            if band.enabled {
                filter.process(samples);
            }
        }
    }

    fn reset(&mut self) {
        for filter in &mut self.filters {
            filter.reset();
        }
    }

    fn set_bypass(&mut self, bypass: bool) {
        self.bypass = bypass;
    }

    fn is_bypassed(&self) -> bool {
        self.bypass
    }

    fn gains(&self) -> Vec<f64> {
        self.bands.iter().map(|b| b.gain_db).collect()
    }

    fn set_gains(&mut self, gains: &[f64]) -> DspResult<()> {
        for (i, &gain) in gains.iter().enumerate() {
            self.set_band_gain(i, gain)?;
        }
        Ok(())
    }
}

/// Graphic equalizer with fixed frequency bands.
#[derive(Debug, Clone)]
pub struct GraphicEqualizer {
    /// Inner parametric EQ
    inner: ParametricEqualizer,
    /// Fixed frequencies for this EQ
    frequencies: Vec<f64>,
}

impl GraphicEqualizer {
    /// Create a standard 10-band graphic EQ.
    pub fn new_10_band(channels: usize, sample_rate: u32) -> DspResult<Self> {
        Self::new(channels, sample_rate, &GRAPHIC_EQ_FREQUENCIES)
    }

    /// Create a 31-band graphic EQ.
    pub fn new_31_band(channels: usize, sample_rate: u32) -> DspResult<Self> {
        Self::new(channels, sample_rate, &GRAPHIC_EQ_31_FREQUENCIES)
    }

    /// Create a graphic EQ with custom frequencies.
    pub fn new(channels: usize, sample_rate: u32, frequencies: &[f64]) -> DspResult<Self> {
        let mut inner = ParametricEqualizer::new(channels, sample_rate);

        for &freq in frequencies {
            // Skip frequencies above Nyquist
            if freq < sample_rate as f64 / 2.0 {
                inner.add_band(Band::peaking(freq, 0.0, DEFAULT_GRAPHIC_Q))?;
            }
        }

        Ok(Self {
            inner,
            frequencies: frequencies.to_vec(),
        })
    }

    /// Get the fixed frequencies.
    pub fn frequencies(&self) -> &[f64] {
        &self.frequencies
    }

    /// Set all gains at once.
    pub fn set_all_gains(&mut self, gains: &[f64]) -> DspResult<()> {
        self.inner.set_gains(gains)
    }

    /// Get band gain by frequency (finds closest band).
    pub fn gain_at_frequency(&self, frequency: f64) -> Option<f64> {
        let index = self
            .frequencies
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = (*a - frequency).abs();
                let db = (*b - frequency).abs();
                da.partial_cmp(&db).unwrap()
            })
            .map(|(i, _)| i)?;

        self.inner.band(index).map(|b| b.gain_db)
    }
}

impl Equalizer for GraphicEqualizer {
    fn num_bands(&self) -> usize {
        self.inner.num_bands()
    }

    fn band(&self, index: usize) -> Option<&Band> {
        self.inner.band(index)
    }

    fn band_mut(&mut self, index: usize) -> Option<&mut Band> {
        self.inner.band_mut(index)
    }

    fn set_band_gain(&mut self, index: usize, gain_db: f64) -> DspResult<()> {
        self.inner.set_band_gain(index, gain_db)
    }

    fn update_filters(&mut self, sample_rate: u32) -> DspResult<()> {
        self.inner.update_filters(sample_rate)
    }

    fn process(&mut self, samples: &mut [f32]) {
        self.inner.process(samples)
    }

    fn reset(&mut self) {
        self.inner.reset()
    }

    fn set_bypass(&mut self, bypass: bool) {
        self.inner.set_bypass(bypass)
    }

    fn is_bypassed(&self) -> bool {
        self.inner.is_bypassed()
    }

    fn gains(&self) -> Vec<f64> {
        self.inner.gains()
    }

    fn set_gains(&mut self, gains: &[f64]) -> DspResult<()> {
        self.inner.set_gains(gains)
    }
}

/// Equalizer preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqualizerPreset {
    /// Preset name
    pub name: String,
    /// Band gains in dB
    pub gains: Vec<f64>,
    /// Preset frequencies (for validation)
    pub frequencies: Vec<f64>,
}

impl EqualizerPreset {
    /// Create a new preset from current EQ state.
    pub fn from_equalizer<E: Equalizer>(name: &str, eq: &E) -> Self {
        Self {
            name: name.to_string(),
            gains: eq.gains(),
            frequencies: Vec::new(), // Could be populated from band frequencies
        }
    }

    /// Create a flat (0 dB) preset.
    pub fn flat(name: &str, num_bands: usize) -> Self {
        Self {
            name: name.to_string(),
            gains: vec![0.0; num_bands],
            frequencies: Vec::new(),
        }
    }

    /// Bass boost preset for 10-band EQ.
    pub fn bass_boost() -> Self {
        Self {
            name: "Bass Boost".to_string(),
            gains: vec![6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            frequencies: GRAPHIC_EQ_FREQUENCIES.to_vec(),
        }
    }

    /// Treble boost preset for 10-band EQ.
    pub fn treble_boost() -> Self {
        Self {
            name: "Treble Boost".to_string(),
            gains: vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 4.0, 5.0, 6.0],
            frequencies: GRAPHIC_EQ_FREQUENCIES.to_vec(),
        }
    }

    /// Vocal boost preset for 10-band EQ.
    pub fn vocal_boost() -> Self {
        Self {
            name: "Vocal Boost".to_string(),
            gains: vec![-2.0, -1.0, 0.0, 2.0, 4.0, 4.0, 3.0, 1.0, 0.0, -1.0],
            frequencies: GRAPHIC_EQ_FREQUENCIES.to_vec(),
        }
    }

    /// Rock preset for 10-band EQ.
    pub fn rock() -> Self {
        Self {
            name: "Rock".to_string(),
            gains: vec![4.0, 3.0, 2.0, 1.0, -1.0, -1.0, 2.0, 3.0, 4.0, 4.0],
            frequencies: GRAPHIC_EQ_FREQUENCIES.to_vec(),
        }
    }

    /// Jazz preset for 10-band EQ.
    pub fn jazz() -> Self {
        Self {
            name: "Jazz".to_string(),
            gains: vec![3.0, 2.0, 1.0, 2.0, -2.0, -2.0, 0.0, 2.0, 3.0, 4.0],
            frequencies: GRAPHIC_EQ_FREQUENCIES.to_vec(),
        }
    }

    /// Classical preset for 10-band EQ.
    pub fn classical() -> Self {
        Self {
            name: "Classical".to_string(),
            gains: vec![4.0, 3.0, 2.0, 1.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0],
            frequencies: GRAPHIC_EQ_FREQUENCIES.to_vec(),
        }
    }

    /// Electronic preset for 10-band EQ.
    pub fn electronic() -> Self {
        Self {
            name: "Electronic".to_string(),
            gains: vec![5.0, 4.0, 1.0, 0.0, -2.0, 2.0, 1.0, 2.0, 4.0, 5.0],
            frequencies: GRAPHIC_EQ_FREQUENCIES.to_vec(),
        }
    }

    /// Apply this preset to an equalizer.
    pub fn apply<E: Equalizer>(&self, eq: &mut E) -> DspResult<()> {
        eq.set_gains(&self.gains)
    }

    /// Get all built-in presets.
    pub fn all_presets() -> Vec<Self> {
        vec![
            Self::flat("Flat", 10),
            Self::bass_boost(),
            Self::treble_boost(),
            Self::vocal_boost(),
            Self::rock(),
            Self::jazz(),
            Self::classical(),
            Self::electronic(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphic_eq_10_band() {
        let mut eq = GraphicEqualizer::new_10_band(2, 48000).unwrap();
        assert_eq!(eq.num_bands(), 10);

        // Set bass boost
        eq.set_band_gain(0, 6.0).unwrap();
        eq.set_band_gain(1, 4.0).unwrap();

        // Process some samples
        let mut samples = vec![0.5f32; 100];
        eq.process(&mut samples);

        // Verify it processed (samples should be modified)
        assert!(samples.iter().any(|&s| (s - 0.5).abs() > 0.0001));
    }

    #[test]
    fn test_parametric_eq() {
        let mut eq = ParametricEqualizer::with_bands(2, 48000, 5).unwrap();
        assert_eq!(eq.num_bands(), 5);

        // Modify a band
        eq.set_band_gain(2, 3.0).unwrap();
        eq.set_band_q(2, 2.0).unwrap();

        // Verify the change
        let band = eq.band(2).unwrap();
        assert_eq!(band.gain_db, 3.0);
        assert_eq!(band.q, 2.0);
    }

    #[test]
    fn test_preset_application() {
        let mut eq = GraphicEqualizer::new_10_band(2, 48000).unwrap();

        // Apply bass boost preset
        let preset = EqualizerPreset::bass_boost();
        preset.apply(&mut eq).unwrap();

        // Verify gains were applied
        let gains = eq.gains();
        assert_eq!(gains[0], 6.0);
        assert_eq!(gains[1], 5.0);
    }
}
