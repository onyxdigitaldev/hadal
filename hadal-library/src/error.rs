//! Library-specific error types.

use std::path::PathBuf;
use thiserror::Error;

/// Library-specific errors.
#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Tag reading error: {0}")]
    TagRead(String),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),

    #[error("Scan error: {0}")]
    Scan(String),

    #[error("Image error: {0}")]
    Image(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Track not found: {0}")]
    TrackNotFound(i64),

    #[error("Album not found: {0}")]
    AlbumNotFound(i64),

    #[error("Artist not found: {0}")]
    ArtistNotFound(i64),

    #[error("Migration error: {0}")]
    Migration(String),
}

/// Result type for library operations.
pub type LibraryResult<T> = Result<T, LibraryError>;

impl From<lofty::error::LoftyError> for LibraryError {
    fn from(e: lofty::error::LoftyError) -> Self {
        Self::TagRead(e.to_string())
    }
}

impl From<image::ImageError> for LibraryError {
    fn from(e: image::ImageError) -> Self {
        Self::Image(e.to_string())
    }
}
