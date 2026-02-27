//! Configuration management for Hadal.
//!
//! Handles loading, saving, and validating user configuration from
//! `~/.config/hadal/config.toml`.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::paths::Paths;

/// Main application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// General settings.
    pub general: GeneralConfig,

    /// Audio settings.
    pub audio: AudioConfig,

    /// User interface settings.
    pub ui: UiConfig,

    /// Library settings.
    pub library: LibraryConfig,

    /// Keybindings (optional overrides).
    pub keys: KeyConfig,
}


impl Config {
    /// Load configuration from file, or create default if not found.
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents = fs::read_to_string(&config_path)
                .map_err(|e| ConfigError::Read(config_path.clone(), e))?;
            let config: Config = toml::from_str(&contents)
                .map_err(|e| ConfigError::Parse(config_path.clone(), e))?;
            Ok(config)
        } else {
            // Create default config file
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to file.
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ConfigError::CreateDir(parent.to_path_buf(), e))?;
        }

        let contents = toml::to_string_pretty(self)
            .map_err(ConfigError::Serialize)?;

        fs::write(&config_path, contents)
            .map_err(|e| ConfigError::Write(config_path.clone(), e))?;

        Ok(())
    }

    /// Get the configuration file path.
    pub fn config_path() -> Result<PathBuf, ConfigError> {
        let paths = Paths::new().map_err(|_| ConfigError::NoConfigDir)?;
        Ok(paths.config_dir.join("config.toml"))
    }
}

/// General application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Check for updates on startup.
    pub check_updates: bool,

    /// Show notifications.
    pub notifications: bool,

    /// Log level: error, warn, info, debug, trace.
    pub log_level: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            check_updates: false,
            notifications: true,
            log_level: "info".to_string(),
        }
    }
}

/// Audio engine settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Output device name (empty for default).
    pub device: String,

    /// Preferred sample rate (0 for native).
    pub sample_rate: u32,

    /// Buffer size in samples.
    pub buffer_size: usize,

    /// Enable gapless playback.
    pub gapless: bool,

    /// Resampling quality: low, medium, high, highest.
    pub resampler_quality: String,

    /// Enable equalizer.
    pub eq_enabled: bool,

    /// Number of EQ bands: 10 or 31.
    pub eq_bands: u8,

    /// Default volume (0.0 - 1.0).
    pub default_volume: f32,

    /// ReplayGain mode: off, track, album.
    pub replay_gain: String,

    /// ReplayGain preamp in dB.
    pub replay_gain_preamp: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device: String::new(),
            sample_rate: 0,
            buffer_size: 8192,
            gapless: true,
            resampler_quality: "high".to_string(),
            eq_enabled: true,
            eq_bands: 10,
            default_volume: 1.0,
            replay_gain: "off".to_string(),
            replay_gain_preamp: 0.0,
        }
    }
}

/// User interface settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Color theme name.
    pub theme: String,

    /// Custom colors (override theme).
    pub colors: ColorConfig,

    /// Use vim-like keybindings (hjkl navigation).
    pub vim_keys: bool,

    /// Show album art.
    pub show_album_art: bool,

    /// Album art size (small, medium, large).
    pub album_art_size: String,

    /// Enable visualizer.
    pub visualizer_enabled: bool,

    /// Visualizer mode: spectrum, waveform, vu_meter, stereo, combined.
    pub visualizer_mode: String,

    /// Number of spectrum bands.
    pub spectrum_bands: u16,

    /// Show track duration in library.
    pub show_duration: bool,

    /// Show bitrate in status bar.
    pub show_bitrate: bool,

    /// Date format for display.
    pub date_format: String,

    /// Time format for display.
    pub time_format: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            colors: ColorConfig::default(),
            vim_keys: true,
            show_album_art: true,
            album_art_size: "medium".to_string(),
            visualizer_enabled: true,
            visualizer_mode: "spectrum".to_string(),
            spectrum_bands: 64,
            show_duration: true,
            show_bitrate: true,
            date_format: "%Y-%m-%d".to_string(),
            time_format: "%H:%M".to_string(),
        }
    }
}

/// Color configuration.
///
/// Colors are specified as hex strings: "#RRGGBB" or "rgb(r,g,b)".
/// Use empty string to use theme default.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    /// Primary accent color (light blue default).
    pub primary: String,

    /// Secondary accent color (slate default).
    pub secondary: String,

    /// Text color (white default).
    pub text: String,

    /// Muted/dimmed text color.
    pub text_muted: String,

    /// Border color.
    pub border: String,

    /// Focused border color.
    pub border_focused: String,

    /// Selection/highlight background.
    pub selection: String,

    /// Currently playing indicator.
    pub playing: String,

    /// Error color.
    pub error: String,

    /// Success color.
    pub success: String,

    /// Warning color.
    pub warning: String,

    /// Spectrum bar color.
    pub spectrum: String,

    /// Spectrum peak color.
    pub spectrum_peak: String,

    /// Waveform color.
    pub waveform: String,

    /// VU meter color.
    pub vu_meter: String,

    /// VU meter peak color.
    pub vu_peak: String,

    /// EQ positive gain color.
    pub eq_positive: String,

    /// EQ negative gain color.
    pub eq_negative: String,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            // Light blue accent
            primary: "#87CEEB".to_string(),
            // Dark slate for secondary elements
            secondary: "#4A5568".to_string(),
            // White text
            text: "#E5E5E5".to_string(),
            // Muted gray
            text_muted: "#6B7280".to_string(),
            // Subtle border
            border: "#374151".to_string(),
            // Focused border (light blue)
            border_focused: "#87CEEB".to_string(),
            // Subtle selection
            selection: "#1F2937".to_string(),
            // Light blue for playing
            playing: "#87CEEB".to_string(),
            // Subtle red
            error: "#EF4444".to_string(),
            // Subtle green
            success: "#10B981".to_string(),
            // Subtle yellow
            warning: "#F59E0B".to_string(),
            // Spectrum analyzer
            spectrum: "#87CEEB".to_string(),
            spectrum_peak: "#E5E5E5".to_string(),
            // Waveform
            waveform: "#87CEEB".to_string(),
            // VU meter
            vu_meter: "#10B981".to_string(),
            vu_peak: "#E5E5E5".to_string(),
            // EQ
            eq_positive: "#10B981".to_string(),
            eq_negative: "#EF4444".to_string(),
        }
    }
}

/// Library settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryConfig {
    /// Music directories to scan.
    pub music_dirs: Vec<String>,

    /// Watch directories for changes.
    pub watch_dirs: bool,

    /// Scan subdirectories.
    pub recursive: bool,

    /// File extensions to include.
    pub extensions: Vec<String>,

    /// Ignore patterns (glob).
    pub ignore_patterns: Vec<String>,

    /// Sort order: artist, album, title, year, date_added.
    pub default_sort: String,

    /// Sort direction: asc, desc.
    pub sort_direction: String,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        let music_dir = dirs::audio_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join("Music")))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            music_dirs: if music_dir.is_empty() {
                Vec::new()
            } else {
                vec![music_dir]
            },
            watch_dirs: true,
            recursive: true,
            extensions: vec![
                "flac".to_string(),
                "mp3".to_string(),
                "ogg".to_string(),
                "opus".to_string(),
                "m4a".to_string(),
                "wav".to_string(),
                "aiff".to_string(),
                "aif".to_string(),
                "wv".to_string(),
                "ape".to_string(),
            ],
            ignore_patterns: vec![
                ".*".to_string(),
                "_*".to_string(),
            ],
            default_sort: "artist".to_string(),
            sort_direction: "asc".to_string(),
        }
    }
}

/// Keybinding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyConfig {
    /// Quit application.
    pub quit: String,

    /// Play/pause toggle.
    pub play_pause: String,

    /// Stop playback.
    pub stop: String,

    /// Next track.
    pub next: String,

    /// Previous track.
    pub previous: String,

    /// Seek forward.
    pub seek_forward: String,

    /// Seek backward.
    pub seek_backward: String,

    /// Volume up.
    pub volume_up: String,

    /// Volume down.
    pub volume_down: String,

    /// Toggle mute.
    pub mute: String,

    /// Toggle shuffle.
    pub shuffle: String,

    /// Cycle repeat mode.
    pub repeat: String,

    /// Open search.
    pub search: String,

    /// Show help.
    pub help: String,

    /// Navigate to library.
    pub goto_library: String,

    /// Navigate to queue.
    pub goto_queue: String,

    /// Navigate to playlists.
    pub goto_playlists: String,

    /// Navigate to equalizer.
    pub goto_equalizer: String,

    /// Navigate to visualizer.
    pub goto_visualizer: String,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            quit: "q".to_string(),
            play_pause: "space".to_string(),
            stop: "s".to_string(),
            next: "n".to_string(),
            previous: "p".to_string(),
            seek_forward: "l".to_string(),
            seek_backward: "h".to_string(),
            volume_up: "+".to_string(),
            volume_down: "-".to_string(),
            mute: "m".to_string(),
            shuffle: "z".to_string(),
            repeat: "r".to_string(),
            search: "/".to_string(),
            help: "?".to_string(),
            goto_library: "1".to_string(),
            goto_queue: "2".to_string(),
            goto_playlists: "3".to_string(),
            goto_equalizer: "e".to_string(),
            goto_visualizer: "v".to_string(),
        }
    }
}

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file {0}: {1}")]
    Read(PathBuf, std::io::Error),

    #[error("Failed to parse config file {0}: {1}")]
    Parse(PathBuf, toml::de::Error),

    #[error("Failed to serialize config: {0}")]
    Serialize(toml::ser::Error),

    #[error("Failed to write config file {0}: {1}")]
    Write(PathBuf, std::io::Error),

    #[error("Failed to create directory {0}: {1}")]
    CreateDir(PathBuf, std::io::Error),

    #[error("Could not determine config directory")]
    NoConfigDir,
}

/// Parse a color string to RGB values.
///
/// Supports formats:
/// - "#RRGGBB" (hex)
/// - "rgb(r, g, b)" (decimal 0-255)
/// - Named colors: "white", "black", "red", etc.
pub fn parse_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();

    // Hex format: #RRGGBB
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some((r, g, b));
        }
    }

    // RGB format: rgb(r, g, b)
    if s.starts_with("rgb(") && s.ends_with(')') {
        let inner = &s[4..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            let r: u8 = parts[0].parse().ok()?;
            let g: u8 = parts[1].parse().ok()?;
            let b: u8 = parts[2].parse().ok()?;
            return Some((r, g, b));
        }
    }

    // Named colors
    match s.to_lowercase().as_str() {
        "white" => Some((255, 255, 255)),
        "black" => Some((0, 0, 0)),
        "red" => Some((255, 0, 0)),
        "green" => Some((0, 255, 0)),
        "blue" => Some((0, 0, 255)),
        "yellow" => Some((255, 255, 0)),
        "cyan" => Some((0, 255, 255)),
        "magenta" => Some((255, 0, 255)),
        "gray" | "grey" => Some((128, 128, 128)),
        "lightgray" | "lightgrey" => Some((192, 192, 192)),
        "darkgray" | "darkgrey" => Some((64, 64, 64)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_hex() {
        assert_eq!(parse_color("#FF0000"), Some((255, 0, 0)));
        assert_eq!(parse_color("#00FF00"), Some((0, 255, 0)));
        assert_eq!(parse_color("#0000FF"), Some((0, 0, 255)));
        assert_eq!(parse_color("#87CEEB"), Some((135, 206, 235)));
    }

    #[test]
    fn test_parse_color_rgb() {
        assert_eq!(parse_color("rgb(255, 0, 0)"), Some((255, 0, 0)));
        assert_eq!(parse_color("rgb(0,255,0)"), Some((0, 255, 0)));
    }

    #[test]
    fn test_parse_color_named() {
        assert_eq!(parse_color("white"), Some((255, 255, 255)));
        assert_eq!(parse_color("BLACK"), Some((0, 0, 0)));
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.ui.colors.primary.is_empty());
        assert!(!config.ui.colors.text.is_empty());
    }
}
