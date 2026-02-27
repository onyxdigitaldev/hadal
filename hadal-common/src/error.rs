//! Error types for the Hadal music player.

use std::path::PathBuf;
use thiserror::Error;

/// The main error type for Hadal operations.
#[derive(Debug, Error)]
pub enum Error {
    // ─────────────────────────────────────────────────────────────────────────
    // Audio errors
    // ─────────────────────────────────────────────────────────────────────────
    #[error("Failed to decode audio: {0}")]
    Decode(String),

    #[error("PipeWire error: {0}")]
    PipeWire(String),

    #[error("Audio output error: {0}")]
    AudioOutput(String),

    #[error("Unsupported audio format: {0}")]
    UnsupportedFormat(String),

    #[error("Failed to seek: {0}")]
    Seek(String),

    // ─────────────────────────────────────────────────────────────────────────
    // Library errors
    // ─────────────────────────────────────────────────────────────────────────
    #[error("Database error: {0}")]
    Database(String),

    #[error("Failed to read tags from file: {0}")]
    TagRead(String),

    #[error("Library scan error: {0}")]
    LibraryScan(String),

    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Invalid path: {path}")]
    InvalidPath { path: PathBuf },

    // ─────────────────────────────────────────────────────────────────────────
    // Playlist errors
    // ─────────────────────────────────────────────────────────────────────────
    #[error("Playlist error: {0}")]
    Playlist(String),

    #[error("M3U8 parse error: {0}")]
    M3u8Parse(String),

    #[error("Playlist not found: {name}")]
    PlaylistNotFound { name: String },

    // ─────────────────────────────────────────────────────────────────────────
    // Graphics errors
    // ─────────────────────────────────────────────────────────────────────────
    #[error("Graphics error: {0}")]
    Graphics(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("Terminal capability not supported: {0}")]
    CapabilityNotSupported(String),

    // ─────────────────────────────────────────────────────────────────────────
    // Configuration errors
    // ─────────────────────────────────────────────────────────────────────────
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Failed to parse configuration: {0}")]
    ConfigParse(String),

    #[error("Missing configuration directory")]
    NoConfigDir,

    #[error("Missing data directory")]
    NoDataDir,

    #[error("Missing cache directory")]
    NoCacheDir,

    // ─────────────────────────────────────────────────────────────────────────
    // I/O errors
    // ─────────────────────────────────────────────────────────────────────────
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // ─────────────────────────────────────────────────────────────────────────
    // Channel errors
    // ─────────────────────────────────────────────────────────────────────────
    #[error("Channel send error: {0}")]
    ChannelSend(String),

    #[error("Channel receive error: {0}")]
    ChannelRecv(String),

    // ─────────────────────────────────────────────────────────────────────────
    // Generic errors
    // ─────────────────────────────────────────────────────────────────────────
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("{0}")]
    Other(String),
}

/// A specialized Result type for Hadal operations.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a new decode error.
    pub fn decode(msg: impl Into<String>) -> Self {
        Self::Decode(msg.into())
    }

    /// Create a new PipeWire error.
    pub fn pipewire(msg: impl Into<String>) -> Self {
        Self::PipeWire(msg.into())
    }

    /// Create a new database error.
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    /// Create a new configuration error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a new internal error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}
