//! XDG path utilities for Hadal.
//!
//! Provides standardized paths following the XDG Base Directory Specification:
//! - Config: `~/.config/hadal/`
//! - Data: `~/.local/share/hadal/`
//! - Cache: `~/.cache/hadal/`

use crate::error::{Error, Result};
use std::path::PathBuf;

/// Application name used for directory naming.
pub const APP_NAME: &str = "hadal";

/// Centralized path management for all Hadal files.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Configuration directory: `~/.config/hadal/`
    pub config_dir: PathBuf,

    /// Main configuration file: `~/.config/hadal/config.toml`
    pub config_file: PathBuf,

    /// Keybindings configuration: `~/.config/hadal/keybindings.toml`
    pub keybindings_file: PathBuf,

    /// Data directory: `~/.local/share/hadal/`
    pub data_dir: PathBuf,

    /// SQLite database: `~/.local/share/hadal/library.db`
    pub database: PathBuf,

    /// Playlists directory: `~/.local/share/hadal/playlists/`
    pub playlists_dir: PathBuf,

    /// Cache directory: `~/.cache/hadal/`
    pub cache_dir: PathBuf,

    /// Artwork cache: `~/.cache/hadal/artwork/`
    pub artwork_cache: PathBuf,

    /// Waveform cache: `~/.cache/hadal/waveforms/`
    pub waveform_cache: PathBuf,

    /// Log file: `~/.cache/hadal/hadal.log`
    pub log_file: PathBuf,

    /// EQ state file: `~/.local/share/hadal/eq_state.toml`
    pub eq_state: PathBuf,
}

impl Paths {
    /// Initialize all Hadal paths, creating directories as needed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - XDG directories cannot be determined
    /// - Directory creation fails
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or(Error::NoConfigDir)?
            .join(APP_NAME);

        let data_dir = dirs::data_dir()
            .ok_or(Error::NoDataDir)?
            .join(APP_NAME);

        let cache_dir = dirs::cache_dir()
            .ok_or(Error::NoCacheDir)?
            .join(APP_NAME);

        let paths = Self {
            config_file: config_dir.join("config.toml"),
            keybindings_file: config_dir.join("keybindings.toml"),
            config_dir,

            database: data_dir.join("library.db"),
            playlists_dir: data_dir.join("playlists"),
            eq_state: data_dir.join("eq_state.toml"),
            data_dir,

            artwork_cache: cache_dir.join("artwork"),
            waveform_cache: cache_dir.join("waveforms"),
            log_file: cache_dir.join("hadal.log"),
            cache_dir,
        };

        paths.ensure_directories()?;

        Ok(paths)
    }

    /// Create all necessary directories if they don't exist.
    fn ensure_directories(&self) -> Result<()> {
        let dirs_to_create = [
            &self.config_dir,
            &self.data_dir,
            &self.playlists_dir,
            &self.cache_dir,
            &self.artwork_cache,
            &self.waveform_cache,
        ];

        for dir in dirs_to_create {
            if !dir.exists() {
                std::fs::create_dir_all(dir)?;
                tracing::debug!("Created directory: {}", dir.display());
            }
        }

        Ok(())
    }

    /// Get the path for a specific playlist file.
    ///
    /// # Arguments
    ///
    /// * `name` - The playlist name (without extension)
    /// * `format` - The playlist format extension (e.g., "m3u8")
    pub fn playlist_file(&self, name: &str, format: &str) -> PathBuf {
        self.playlists_dir.join(format!("{}.{}", name, format))
    }

    /// Get the cache path for album artwork.
    ///
    /// Uses a hash-based filename to avoid filesystem issues with special characters.
    ///
    /// # Arguments
    ///
    /// * `cache_key` - A unique identifier for the artwork (typically a hash)
    pub fn artwork_file(&self, cache_key: u64) -> PathBuf {
        self.artwork_cache.join(format!("{:016x}.png", cache_key))
    }

    /// Get the cache path for a waveform file.
    ///
    /// # Arguments
    ///
    /// * `track_id` - The database ID of the track
    pub fn waveform_file(&self, track_id: i64) -> PathBuf {
        self.waveform_cache.join(format!("{}.waveform", track_id))
    }

    /// Clean the artwork cache, removing files older than the specified age.
    ///
    /// # Arguments
    ///
    /// * `max_age` - Maximum age of cache files in seconds
    ///
    /// # Returns
    ///
    /// The number of files removed.
    pub fn clean_artwork_cache(&self, max_age: u64) -> Result<usize> {
        self.clean_cache_dir(&self.artwork_cache, max_age)
    }

    /// Clean a cache directory, removing files older than the specified age.
    fn clean_cache_dir(&self, dir: &PathBuf, max_age: u64) -> Result<usize> {
        use std::time::SystemTime;

        let now = SystemTime::now();
        let mut removed = 0;

        if !dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            if let Ok(modified) = metadata.modified() {
                if let Ok(age) = now.duration_since(modified) {
                    if age.as_secs() > max_age {
                        std::fs::remove_file(entry.path())?;
                        removed += 1;
                    }
                }
            }
        }

        Ok(removed)
    }

    /// Get the total size of the artwork cache in bytes.
    pub fn artwork_cache_size(&self) -> Result<u64> {
        self.dir_size(&self.artwork_cache)
    }

    /// Calculate the total size of a directory in bytes.
    fn dir_size(&self, dir: &PathBuf) -> Result<u64> {
        let mut total = 0;

        if !dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            total += metadata.len();
        }

        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playlist_file() {
        let paths = Paths::new().unwrap();
        let playlist_path = paths.playlist_file("favorites", "m3u8");
        assert!(playlist_path.to_string_lossy().ends_with("favorites.m3u8"));
    }

    #[test]
    fn test_artwork_file() {
        let paths = Paths::new().unwrap();
        let artwork_path = paths.artwork_file(0x1234567890abcdef);
        assert!(artwork_path
            .to_string_lossy()
            .ends_with("1234567890abcdef.png"));
    }
}
