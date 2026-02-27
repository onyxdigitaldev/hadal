//! Playlist-specific error types.

use std::path::PathBuf;
use thiserror::Error;

/// Playlist-specific errors.
#[derive(Debug, Error)]
pub enum PlaylistError {
    #[error("Playlist not found: {0}")]
    NotFound(String),

    #[error("Playlist already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid playlist name: {0}")]
    InvalidName(String),

    #[error("Track not in playlist: {0}")]
    TrackNotInPlaylist(i64),

    #[error("Invalid position: {0}")]
    InvalidPosition(usize),

    #[error("M3U8 parse error at line {line}: {message}")]
    M3u8Parse { line: usize, message: String },

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Queue is empty")]
    QueueEmpty,

    #[error("Invalid queue index: {0}")]
    InvalidIndex(usize),
}

/// Result type for playlist operations.
pub type PlaylistResult<T> = Result<T, PlaylistError>;
