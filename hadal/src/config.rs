//! Configuration loading and management.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Main configuration structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Library configuration
    pub library: LibraryConfig,

    /// Playback configuration
    pub playback: PlaybackConfig,

    /// UI configuration
    pub ui: UiConfig,

    /// Audio configuration
    pub audio: AudioConfig,
}

impl Config {
    /// Load configuration from a file, creating a default if it doesn't exist.
    /// Validates and clamps out-of-range values after loading.
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path = path.as_ref();

        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let mut config: Config = toml::from_str(&contents)?;
            config.validate();
            Ok(config)
        } else {
            // Create default config file
            let config = Config::default();
            config.save(path)?;
            Ok(config)
        }
    }

    /// Validate and clamp configuration values to safe ranges.
    fn validate(&mut self) {
        if !(0.0..=1.0).contains(&self.playback.default_volume) {
            tracing::warn!(
                "default_volume {} out of range, clamping to 0.0–1.0",
                self.playback.default_volume
            );
            self.playback.default_volume = self.playback.default_volume.clamp(0.0, 1.0);
        }

        if !(1..=240).contains(&self.ui.fps) {
            tracing::warn!("fps {} out of range, clamping to 1–240", self.ui.fps);
            self.ui.fps = self.ui.fps.clamp(1, 240);
        }

        if !(64..=65536).contains(&self.playback.buffer_size) {
            tracing::warn!(
                "buffer_size {} out of range, clamping to 64–65536",
                self.playback.buffer_size
            );
            self.playback.buffer_size = self.playback.buffer_size.clamp(64, 65536);
        }

        if !matches!(
            self.playback.resampler_quality.as_str(),
            "fast" | "medium" | "best"
        ) {
            tracing::warn!(
                "resampler_quality '{}' invalid, defaulting to 'medium'",
                self.playback.resampler_quality
            );
            self.playback.resampler_quality = "medium".to_string();
        }
    }

    /// Save configuration to a file.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let contents = toml::to_string_pretty(self)?;

        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, contents)?;
        Ok(())
    }
}


/// Library configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryConfig {
    /// Folders to scan for music
    pub folders: Vec<String>,

    /// Scan library on startup
    pub scan_on_startup: bool,

    /// Watch for file changes
    pub watch_for_changes: bool,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            folders: vec!["~/Music".to_string()],
            scan_on_startup: true,
            watch_for_changes: true,
        }
    }
}

/// Playback configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackConfig {
    /// Attempt bit-perfect passthrough
    pub passthrough: bool,

    /// Allow resampling if device doesn't support source rate
    pub allow_resampling: bool,

    /// Resampler quality (fast, medium, best)
    pub resampler_quality: String,

    /// Enable gapless playback
    pub gapless: bool,

    /// ReplayGain mode (off, track, album)
    pub replay_gain: String,

    /// Audio buffer size in frames
    pub buffer_size: usize,

    /// Default volume (0.0–1.0)
    pub default_volume: f32,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            passthrough: true,
            allow_resampling: true,
            resampler_quality: "medium".to_string(),
            gapless: true,
            replay_gain: "off".to_string(),
            buffer_size: 1024,
            default_volume: 0.85,
        }
    }
}

/// UI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Theme name (default, monochrome)
    pub theme: String,

    /// Custom color overrides
    pub colors: hadal_common::ColorConfig,

    /// Show album art
    pub show_album_art: bool,

    /// Album art position (left, right)
    pub album_art_position: String,

    /// Target FPS
    pub fps: u32,

    /// Use vim-like keybindings
    pub vim_keys: bool,

    /// Default sort field
    pub default_sort: String,

    /// Default sort order (ascending, descending)
    pub default_sort_order: String,

    /// Default library view (artists, albums, tracks, genres)
    pub default_library_view: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            colors: hadal_common::ColorConfig::default(),
            show_album_art: true,
            album_art_position: "left".to_string(),
            fps: 60,
            vim_keys: true,
            default_sort: "artist".to_string(),
            default_sort_order: "ascending".to_string(),
            default_library_view: "artists".to_string(),
        }
    }
}

/// Audio configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Output device (default or specific name)
    pub output_device: String,

    /// Request exclusive access to audio device
    pub exclusive_mode: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_device: "default".to_string(),
            exclusive_mode: false,
        }
    }
}

/// Configuration error type.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}
