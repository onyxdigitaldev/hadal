//! High-level audio player interface.
//!
//! All methods take `&self` so the player can be shared via `Arc<AudioPlayer>`
//! between the UI thread and the audio output callback without an outer mutex.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use parking_lot::{Mutex, RwLock};

use crate::decoder::AudioDecoder;
use crate::error::AudioResult;
use crate::pipeline::{AudioPipeline, PipelineConfig, PipelineState};
use hadal_dsp::VisualizationData;

/// Commands that can be sent to the audio player.
#[derive(Debug, Clone)]
pub enum PlayerCommand {
    /// Load and play a file
    Play(std::path::PathBuf),
    Pause,
    Resume,
    Stop,
    Seek(Duration),
    SetVolume(f32),
    ToggleMute,
    Shutdown,
}

/// Current state of the audio player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
    Loading,
}

/// Audio player controller.
///
/// All methods take `&self`. Share via `Arc<AudioPlayer>` between the UI thread
/// and the cpal output callback. The output callback only contends on the
/// pipeline mutex (shared with the decoder thread), never on an outer lock.
pub struct AudioPlayer {
    /// Shared pipeline state (lock-free atomics for position/duration)
    state: Arc<PipelineState>,

    /// Current player state
    player_state: Mutex<PlayerState>,

    /// Pause flag — Arc so the decoder thread can check it
    paused: Arc<AtomicBool>,

    /// Pipeline (shared with decoder thread)
    pipeline: Arc<Mutex<AudioPipeline>>,

    /// Decoder thread handle
    decoder_thread: Mutex<Option<JoinHandle<()>>>,

    /// Shutdown flag — Arc so the decoder thread can check it
    shutdown: Arc<AtomicBool>,
}

// SAFETY: All fields are inherently Send+Sync:
// - state: Arc<PipelineState> — PipelineState contains only atomics and a Mutex
// - player_state: parking_lot::Mutex<PlayerState> — Mutex is Sync
// - paused, shutdown: Arc<AtomicBool> — atomics are Sync
// - pipeline: Arc<Mutex<AudioPipeline>> — Mutex wrapping non-Sync pipeline is Sync
// - decoder_thread: Mutex<Option<JoinHandle<()>>> — Mutex is Sync
// The manual impl is needed because AudioPipeline contains ringbuf types that
// don't implement Sync, but are only accessed behind the pipeline Mutex.
unsafe impl Sync for AudioPlayer {}

impl AudioPlayer {
    /// Create a new audio player with default configuration.
    pub fn new() -> Self {
        Self::with_config(PipelineConfig::default())
    }

    /// Create a new audio player with custom configuration.
    pub fn with_config(config: PipelineConfig) -> Self {
        let pipeline = AudioPipeline::new(config);
        let state = pipeline.state();

        Self {
            state,
            player_state: Mutex::new(PlayerState::Stopped),
            paused: Arc::new(AtomicBool::new(false)),
            pipeline: Arc::new(Mutex::new(pipeline)),
            decoder_thread: Mutex::new(None),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the pipeline state for lock-free position/duration queries.
    pub fn pipeline_state(&self) -> Arc<PipelineState> {
        Arc::clone(&self.state)
    }

    /// Get current player state.
    pub fn state(&self) -> PlayerState {
        *self.player_state.lock()
    }

    /// Check if currently playing.
    pub fn is_playing(&self) -> bool {
        self.state() == PlayerState::Playing && !self.paused.load(Ordering::Acquire)
    }

    /// Check if paused.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// Get current playback position.
    pub fn position(&self) -> Duration {
        self.state.position()
    }

    /// Get total duration of current track.
    pub fn duration(&self) -> Duration {
        self.state.duration()
    }

    /// Get current volume.
    pub fn volume(&self) -> f32 {
        self.state.volume()
    }

    /// Check if muted.
    pub fn is_muted(&self) -> bool {
        self.state.is_muted()
    }

    /// Play a file.
    pub fn play<P: AsRef<Path>>(&self, path: P) -> AudioResult<()> {
        let path = path.as_ref();

        // Update state to loading
        *self.player_state.lock() = PlayerState::Loading;
        self.paused.store(false, Ordering::Release);

        // Open decoder
        let decoder = AudioDecoder::open(path)?;

        tracing::info!(
            "Playing: {} ({}Hz, {} channels)",
            path.display(),
            decoder.sample_rate(),
            decoder.channels()
        );

        // Load into pipeline (briefly holds pipeline lock)
        {
            let mut pipeline = self.pipeline.lock();
            pipeline.load(decoder)?;
        }

        // Start decoder thread if not running
        self.start_decoder_thread();

        // Update state
        *self.player_state.lock() = PlayerState::Playing;

        Ok(())
    }

    /// Pause playback.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
        *self.player_state.lock() = PlayerState::Paused;
    }

    /// Resume playback.
    pub fn resume(&self) {
        if *self.player_state.lock() == PlayerState::Paused {
            self.paused.store(false, Ordering::Release);
            *self.player_state.lock() = PlayerState::Playing;
        }
    }

    /// Toggle play/pause.
    pub fn toggle_pause(&self) {
        if self.is_paused() {
            self.resume();
        } else {
            self.pause();
        }
    }

    /// Stop playback.
    pub fn stop(&self) {
        self.pipeline.lock().stop();
        *self.player_state.lock() = PlayerState::Stopped;
        self.paused.store(false, Ordering::Release);
    }

    /// Seek to a position.
    pub fn seek(&self, position: Duration) -> AudioResult<()> {
        self.pipeline.lock().seek(position)
    }

    /// Seek forward by amount.
    pub fn seek_forward(&self, amount: Duration) -> AudioResult<()> {
        let new_pos = self.position().saturating_add(amount);
        let duration = self.duration();
        self.seek(if new_pos > duration { duration } else { new_pos })
    }

    /// Seek backward by amount.
    pub fn seek_backward(&self, amount: Duration) -> AudioResult<()> {
        self.seek(self.position().saturating_sub(amount))
    }

    /// Set volume.
    pub fn set_volume(&self, volume: f32) {
        self.state.set_volume(volume);
    }

    /// Adjust volume by delta.
    pub fn adjust_volume(&self, delta: f32) {
        let current = self.volume();
        self.set_volume(current + delta);
    }

    /// Toggle mute.
    pub fn toggle_mute(&self) {
        let muted = !self.state.is_muted();
        self.state.set_muted(muted);
    }

    /// Set mute state.
    pub fn set_mute(&self, muted: bool) {
        self.state.set_muted(muted);
    }

    /// Read samples for audio output.
    ///
    /// Called from the cpal output callback. Checks the atomic pause flag
    /// (no lock), then briefly locks the pipeline mutex to pop samples.
    #[inline]
    pub fn read_samples(&self, output: &mut [f32]) -> usize {
        if self.paused.load(Ordering::Acquire) {
            output.fill(0.0);
            return output.len();
        }

        self.pipeline.lock().read_samples(output)
    }

    /// Check if playback has finished.
    pub fn is_finished(&self) -> bool {
        self.pipeline.lock().is_finished()
    }

    /// Get buffer fill level (0.0 - 1.0).
    pub fn buffer_level(&self) -> f32 {
        self.pipeline.lock().buffer_level()
    }

    // ─── Equalizer control ────────────────────────────────────────────────

    /// Set a single EQ band gain (in dB).
    pub fn set_eq_band(&self, band: usize, gain_db: f64) {
        self.pipeline.lock().set_eq_band(band, gain_db);
    }

    /// Set all EQ band gains.
    pub fn set_eq_gains(&self, gains: &[f64]) {
        self.pipeline.lock().set_eq_gains(gains);
    }

    /// Get all EQ band gains.
    pub fn eq_gains(&self) -> Vec<f64> {
        self.pipeline.lock().eq_gains()
    }

    /// Set EQ bypass.
    pub fn set_eq_bypass(&self, bypass: bool) {
        self.pipeline.lock().set_eq_bypass(bypass);
    }

    // ─── Visualizer control ────────────────────────────────────────────

    /// Get the shared visualization data for UI rendering.
    pub fn visualization_data(&self) -> Arc<RwLock<VisualizationData>> {
        self.pipeline.lock().visualization_data()
    }

    /// Check if EQ is bypassed.
    pub fn eq_bypassed(&self) -> bool {
        self.pipeline.lock().eq_bypassed()
    }

    /// Shutdown the player.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        self.stop();

        if let Some(handle) = self.decoder_thread.lock().take() {
            let _ = handle.join();
        }

        tracing::info!("Audio player shutdown complete");
    }

    /// Start the decoder thread for buffer filling.
    fn start_decoder_thread(&self) {
        let mut thread_guard = self.decoder_thread.lock();
        if thread_guard.is_some() {
            return;
        }

        let pipeline = Arc::clone(&self.pipeline);
        let shutdown = Arc::clone(&self.shutdown);
        let paused = Arc::clone(&self.paused);

        let handle = thread::Builder::new()
            .name("hadal-decoder".to_string())
            .spawn(move || {
                tracing::debug!("Decoder thread started");

                while !shutdown.load(Ordering::Acquire) {
                    if paused.load(Ordering::Acquire) {
                        thread::sleep(Duration::from_millis(50));
                        continue;
                    }

                    let result = {
                        let mut pipeline = pipeline.lock();
                        pipeline.fill_buffer()
                    };

                    match result {
                        Ok(true) => {
                            // More data, brief yield
                            thread::sleep(Duration::from_millis(1));
                        }
                        Ok(false) => {
                            // Buffer full or EOS
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(e) => {
                            tracing::error!("Decoder error: {}", e);
                            thread::sleep(Duration::from_millis(100));
                        }
                    }
                }

                tracing::debug!("Decoder thread stopped");
            })
            .expect("Failed to spawn decoder thread");

        *thread_guard = Some(handle);
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.shutdown();
    }
}
