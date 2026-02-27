//! Audio pipeline orchestration.
//!
//! This module manages the flow of audio data from decoder to output,
//! using a lock-free ring buffer for thread-safe communication.
//!
//! The audio processing chain:
//! Decoder → Resampler → Equalizer → Volume → Visualizer → Output

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use hadal_dsp::{Equalizer, GraphicEqualizer, Visualizer, VisualizationData, VisualizationMode};
use parking_lot::{Mutex, RwLock};
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapRb,
};

use crate::decoder::AudioDecoder;
use crate::error::{AudioError, AudioResult};
use crate::resampler::{Resampler, ResamplerQuality};

/// Default ring buffer size in samples (per channel).
/// ~500ms at 48kHz stereo
const DEFAULT_BUFFER_SIZE: usize = 48000;

/// Low watermark for buffer refill (25% of buffer).
const BUFFER_LOW_WATERMARK: usize = DEFAULT_BUFFER_SIZE / 4;

/// Configuration for the audio pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Target output sample rate (None = use source rate)
    pub output_sample_rate: Option<u32>,

    /// Resampling quality
    pub resampler_quality: ResamplerQuality,

    /// Ring buffer size in samples
    pub buffer_size: usize,

    /// Enable gapless playback
    pub gapless: bool,

    /// Enable equalizer
    pub eq_enabled: bool,

    /// Enable visualizer
    pub visualizer_enabled: bool,

    /// Output channels (None = default to 2)
    pub output_channels: Option<u16>,

    /// Number of EQ bands (10 or 31)
    pub eq_bands: usize,

    /// Visualization mode
    pub visualization_mode: VisualizationMode,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            output_sample_rate: None,
            resampler_quality: ResamplerQuality::Medium,
            buffer_size: DEFAULT_BUFFER_SIZE,
            gapless: true,
            eq_enabled: true,
            visualizer_enabled: true,
            output_channels: None,
            eq_bands: 10,
            visualization_mode: VisualizationMode::Spectrum,
        }
    }
}

/// Shared state between decoder and output threads.
pub struct PipelineState {
    /// Current position in samples
    pub position_samples: AtomicU64,

    /// Total duration in samples
    pub total_samples: AtomicU64,

    /// Sample rate for position calculations
    pub sample_rate: AtomicU64,

    /// True if end of stream reached
    pub eos: AtomicBool,

    /// True if pipeline is active
    pub active: AtomicBool,

    /// Volume (0.0 - 1.0)
    pub volume: Mutex<f32>,

    /// Muted state
    pub muted: AtomicBool,
}

impl PipelineState {
    /// Create new pipeline state.
    pub fn new() -> Self {
        Self {
            position_samples: AtomicU64::new(0),
            total_samples: AtomicU64::new(0),
            sample_rate: AtomicU64::new(44100),
            eos: AtomicBool::new(false),
            active: AtomicBool::new(false),
            volume: Mutex::new(1.0),
            muted: AtomicBool::new(false),
        }
    }

    /// Get current playback position.
    pub fn position(&self) -> Duration {
        let samples = self.position_samples.load(Ordering::Acquire);
        let rate = self.sample_rate.load(Ordering::Acquire);
        if rate == 0 { return Duration::ZERO; }
        Duration::from_secs_f64(samples as f64 / rate as f64)
    }

    /// Get total duration.
    pub fn duration(&self) -> Duration {
        let samples = self.total_samples.load(Ordering::Acquire);
        let rate = self.sample_rate.load(Ordering::Acquire);
        if rate == 0 { return Duration::ZERO; }
        Duration::from_secs_f64(samples as f64 / rate as f64)
    }

    /// Get current volume.
    pub fn volume(&self) -> f32 {
        *self.volume.lock()
    }

    /// Set volume.
    pub fn set_volume(&self, volume: f32) {
        *self.volume.lock() = volume.clamp(0.0, 1.0);
    }

    /// Check if muted.
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Acquire)
    }

    /// Set muted state.
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Release);
    }

    /// Get effective volume (considering mute).
    pub fn effective_volume(&self) -> f32 {
        if self.is_muted() {
            0.0
        } else {
            self.volume()
        }
    }
}

impl Default for PipelineState {
    fn default() -> Self {
        Self::new()
    }
}

/// Audio pipeline managing decoder → buffer → output flow.
pub struct AudioPipeline {
    /// Pipeline configuration
    config: PipelineConfig,

    /// Shared state
    state: Arc<PipelineState>,

    /// Ring buffer producer (decoder side)
    producer: ringbuf::HeapProd<f32>,

    /// Ring buffer consumer (output side)
    consumer: ringbuf::HeapCons<f32>,

    /// Current decoder
    decoder: Option<AudioDecoder>,

    /// Current resampler (if needed)
    resampler: Option<Resampler>,

    /// Graphic equalizer
    equalizer: Option<GraphicEqualizer>,

    /// Audio visualizer
    visualizer: Option<Visualizer>,

    /// Shared visualization data for UI
    visualization_data: Arc<RwLock<VisualizationData>>,

    /// Number of channels
    channels: u8,

    /// Output sample rate
    output_rate: u32,
}

impl AudioPipeline {
    /// Create a new audio pipeline.
    pub fn new(config: PipelineConfig) -> Self {
        let buffer = HeapRb::new(config.buffer_size * 2); // Stereo
        let (producer, consumer) = buffer.split();

        let output_rate = 48000; // Default PipeWire rate
        let channels = config.output_channels.unwrap_or(2) as usize;

        // Initialize equalizer if enabled
        let equalizer = if config.eq_enabled {
            match config.eq_bands {
                31 => GraphicEqualizer::new_31_band(channels, output_rate).ok(),
                _ => GraphicEqualizer::new_10_band(channels, output_rate).ok(),
            }
        } else {
            None
        };

        // Initialize visualizer if enabled
        let visualizer = if config.visualizer_enabled {
            Visualizer::new_default(output_rate, channels).ok()
        } else {
            None
        };

        let visualization_data = visualizer
            .as_ref()
            .map(|v| v.data())
            .unwrap_or_else(|| Arc::new(RwLock::new(VisualizationData::default())));

        Self {
            config,
            state: Arc::new(PipelineState::new()),
            producer,
            consumer,
            decoder: None,
            resampler: None,
            equalizer,
            visualizer,
            visualization_data,
            channels: channels as u8,
            output_rate,
        }
    }

    /// Get shared state handle.
    pub fn state(&self) -> Arc<PipelineState> {
        Arc::clone(&self.state)
    }

    /// Set the output sample rate (for resampling decisions).
    pub fn set_output_rate(&mut self, rate: u32) {
        self.output_rate = rate;
    }

    /// Load a new track into the pipeline.
    pub fn load(&mut self, decoder: AudioDecoder) -> AudioResult<()> {
        // Reset state
        self.state.eos.store(false, Ordering::Release);
        self.state.position_samples.store(0, Ordering::Release);
        self.state.active.store(true, Ordering::Release);

        // Update sample rate info
        let source_rate = decoder.sample_rate();
        self.channels = decoder.channels();

        // Set up resampler if needed
        let output_rate = self.config.output_sample_rate.unwrap_or(self.output_rate);

        // Store the effective output rate — position tracking happens on the
        // output side (in read_samples), so PipelineState must use the rate
        // that matches those samples.
        let effective_rate = if source_rate != output_rate { output_rate } else { source_rate };
        let total_samples = decoder.duration().map(|d| {
            (d.as_secs_f64() * effective_rate as f64) as u64
        }).unwrap_or(0);

        self.state.sample_rate.store(effective_rate as u64, Ordering::Release);
        self.state.total_samples.store(total_samples, Ordering::Release);

        if source_rate != output_rate {
            self.resampler = Some(Resampler::new(
                source_rate,
                output_rate,
                self.channels as usize,
                self.config.resampler_quality,
                4096,
            )?);
            tracing::info!(
                "Resampling: {}Hz → {}Hz ({:?})",
                source_rate,
                output_rate,
                self.config.resampler_quality
            );
        } else {
            self.resampler = None;
            tracing::info!("Bit-perfect passthrough at {}Hz", source_rate);
        }

        // Clear buffer
        while self.consumer.try_pop().is_some() {}

        self.decoder = Some(decoder);

        // Pre-fill buffer
        self.fill_buffer()?;

        Ok(())
    }

    /// Fill the ring buffer from the decoder.
    ///
    /// Should be called regularly from a dedicated thread.
    pub fn fill_buffer(&mut self) -> AudioResult<bool> {
        let decoder = match &mut self.decoder {
            Some(d) => d,
            None => return Ok(false),
        };

        if self.state.eos.load(Ordering::Acquire) {
            return Ok(false);
        }

        // Check if buffer needs filling
        let available = self.producer.vacant_len();
        if available < BUFFER_LOW_WATERMARK * self.channels as usize {
            return Ok(false); // Buffer is full enough, sleep longer
        }

        // Decode next chunk
        match decoder.decode_next()? {
            Some(samples) => {
                // Apply resampling if needed
                let output_samples = if let Some(resampler) = &mut self.resampler {
                    resampler.process(&samples)?
                } else {
                    samples
                };

                // Push to ring buffer
                let pushed = self.producer.push_slice(&output_samples);
                if pushed < output_samples.len() {
                    tracing::warn!(
                        "Buffer overflow: {} samples dropped",
                        output_samples.len() - pushed
                    );
                }

                // Position is tracked in read_samples() (output/consumption side)
                // so it reflects actual playback position, not buffer fill.

                Ok(true)
            }
            None => {
                // End of stream
                self.state.eos.store(true, Ordering::Release);
                tracing::debug!("End of stream reached");
                Ok(false)
            }
        }
    }

    /// Read samples for audio output.
    ///
    /// This is called from the audio output callback — must be fast.
    /// Chain: buffer → EQ → volume → visualizer → output
    #[inline]
    pub fn read_samples(&mut self, output: &mut [f32]) -> usize {
        let read = self.consumer.pop_slice(output);

        // Apply equalizer (biquad cascade — fast in-place)
        if let Some(eq) = &mut self.equalizer {
            eq.process(&mut output[..read]);
        }

        // Apply volume (cheap scalar multiply)
        let volume = self.state.effective_volume();
        if volume < 1.0 {
            for sample in &mut output[..read] {
                *sample *= volume;
            }
        }

        // Feed visualizer
        if let Some(viz) = &mut self.visualizer {
            viz.process(&output[..read]);
        }

        // Track playback position from the output/consumption side
        // so it reflects what the user actually hears.
        if read > 0 {
            let frames = read / self.channels.max(1) as usize;
            self.state
                .position_samples
                .fetch_add(frames as u64, Ordering::AcqRel);
        }

        // Zero-fill if underrun
        if read < output.len() {
            for sample in &mut output[read..] {
                *sample = 0.0;
            }
        }

        read
    }

    /// Check if buffer has data available.
    pub fn has_data(&self) -> bool {
        !self.consumer.is_empty()
    }

    /// Check if end of stream and buffer empty.
    pub fn is_finished(&self) -> bool {
        self.state.eos.load(Ordering::Acquire) && self.consumer.is_empty()
    }

    /// Seek to a position.
    pub fn seek(&mut self, position: Duration) -> AudioResult<()> {
        let decoder = self.decoder.as_mut().ok_or(AudioError::NotInitialized)?;

        // Seek decoder
        decoder.seek(position)?;

        // Clear buffer
        while self.consumer.try_pop().is_some() {}

        // Update state — use the rate stored in PipelineState (effective output rate)
        let rate = self.state.sample_rate.load(Ordering::Acquire);
        let samples = (position.as_secs_f64() * rate as f64) as u64;
        self.state.position_samples.store(samples, Ordering::Release);
        self.state.eos.store(false, Ordering::Release);

        // Refill buffer
        self.fill_buffer()?;

        Ok(())
    }

    /// Stop playback and clear state.
    pub fn stop(&mut self) {
        self.decoder = None;
        self.resampler = None;
        self.state.active.store(false, Ordering::Release);
        self.state.eos.store(true, Ordering::Release);
        self.state.position_samples.store(0, Ordering::Release);

        // Clear buffer
        while self.consumer.try_pop().is_some() {}
    }

    /// Get buffer fill level (0.0 - 1.0).
    pub fn buffer_level(&self) -> f32 {
        let occupied = self.consumer.occupied_len();
        let capacity = self.config.buffer_size * self.channels as usize;
        occupied as f32 / capacity as f32
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Equalizer Control
    // ─────────────────────────────────────────────────────────────────────────

    /// Get mutable reference to the equalizer.
    pub fn equalizer_mut(&mut self) -> Option<&mut GraphicEqualizer> {
        self.equalizer.as_mut()
    }

    /// Get reference to the equalizer.
    pub fn equalizer(&self) -> Option<&GraphicEqualizer> {
        self.equalizer.as_ref()
    }

    /// Set EQ band gain.
    pub fn set_eq_band(&mut self, band: usize, gain_db: f64) {
        if let Some(eq) = &mut self.equalizer {
            if let Err(e) = eq.set_band_gain(band, gain_db) {
                tracing::warn!("Failed to set EQ band {} to {}dB: {}", band, gain_db, e);
            }
        }
    }

    /// Get all EQ band gains.
    pub fn eq_gains(&self) -> Vec<f64> {
        self.equalizer
            .as_ref()
            .map(|eq| eq.gains())
            .unwrap_or_default()
    }

    /// Set all EQ band gains.
    pub fn set_eq_gains(&mut self, gains: &[f64]) {
        if let Some(eq) = &mut self.equalizer {
            if let Err(e) = eq.set_gains(gains) {
                tracing::warn!("Failed to set EQ gains: {}", e);
            }
        }
    }

    /// Toggle EQ bypass.
    pub fn set_eq_bypass(&mut self, bypass: bool) {
        if let Some(eq) = &mut self.equalizer {
            eq.set_bypass(bypass);
        }
    }

    /// Check if EQ is bypassed.
    pub fn eq_bypassed(&self) -> bool {
        self.equalizer
            .as_ref()
            .map(|eq| eq.is_bypassed())
            .unwrap_or(true)
    }

    /// Get number of EQ bands.
    pub fn eq_num_bands(&self) -> usize {
        self.equalizer
            .as_ref()
            .map(|eq| eq.num_bands())
            .unwrap_or(0)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Visualizer Control
    // ─────────────────────────────────────────────────────────────────────────

    /// Get visualization data for UI rendering.
    pub fn visualization_data(&self) -> Arc<RwLock<VisualizationData>> {
        Arc::clone(&self.visualization_data)
    }

    /// Get current visualization snapshot.
    pub fn get_visualization(&self) -> VisualizationData {
        self.visualization_data.read().clone()
    }

    /// Set visualization mode.
    pub fn set_visualization_mode(&mut self, mode: VisualizationMode) {
        if let Some(viz) = &mut self.visualizer {
            viz.set_mode(mode);
        }
    }

    /// Get current visualization mode.
    pub fn visualization_mode(&self) -> VisualizationMode {
        self.visualizer
            .as_ref()
            .map(|v| v.mode())
            .unwrap_or(VisualizationMode::Off)
    }

    /// Set number of spectrum bands.
    pub fn set_spectrum_bands(&mut self, num_bands: usize) {
        if let Some(viz) = &mut self.visualizer {
            viz.set_num_bands(num_bands);
        }
    }

    /// Check if visualizer is enabled.
    pub fn visualizer_enabled(&self) -> bool {
        self.visualizer.is_some()
    }
}

impl std::fmt::Debug for AudioPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioPipeline")
            .field("config", &self.config)
            .field("channels", &self.channels)
            .field("output_rate", &self.output_rate)
            .field("has_decoder", &self.decoder.is_some())
            .field("has_resampler", &self.resampler.is_some())
            .field("buffer_level", &self.buffer_level())
            .finish()
    }
}
