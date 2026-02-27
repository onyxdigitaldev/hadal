//! Database row models.

use serde::{Deserialize, Serialize};

/// Artist database row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistRow {
    pub id: i64,
    pub name: String,
    pub sort_name: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Album database row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumRow {
    pub id: i64,
    pub title: String,
    pub sort_title: Option<String>,
    pub artist_id: Option<i64>,
    pub album_artist: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub disc_total: Option<i32>,
    pub track_total: Option<i32>,
    pub artwork_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Track database row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackRow {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub sort_title: Option<String>,
    pub artist_id: Option<i64>,
    pub album_id: Option<i64>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub duration_ms: i64,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub sample_rate: Option<i32>,
    pub bit_depth: Option<i32>,
    pub channels: Option<i32>,
    pub codec: Option<String>,
    pub bitrate: Option<i32>,
    pub file_size: Option<i64>,
    pub play_count: i32,
    pub last_played: Option<i64>,
    pub rating: i32,
    pub file_mtime: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl TrackRow {
    /// Create a new track row for insertion.
    pub fn new(path: String, title: String, duration_ms: i64, file_mtime: i64) -> Self {
        Self {
            id: 0,
            path,
            title,
            sort_title: None,
            artist_id: None,
            album_id: None,
            track_number: None,
            disc_number: Some(1),
            duration_ms,
            year: None,
            genre: None,
            sample_rate: None,
            bit_depth: None,
            channels: None,
            codec: None,
            bitrate: None,
            file_size: None,
            play_count: 0,
            last_played: None,
            rating: 0,
            file_mtime,
            created_at: 0,
            updated_at: 0,
        }
    }
}

/// Scan progress information.
#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub total_files: usize,
    pub scanned_files: usize,
    pub current_file: Option<String>,
    pub added: usize,
    pub updated: usize,
    pub errors: usize,
}

impl ScanProgress {
    pub fn new(total_files: usize) -> Self {
        Self {
            total_files,
            scanned_files: 0,
            current_file: None,
            added: 0,
            updated: 0,
            errors: 0,
        }
    }

    pub fn percent(&self) -> f32 {
        if self.total_files == 0 {
            100.0
        } else {
            (self.scanned_files as f32 / self.total_files as f32) * 100.0
        }
    }
}

/// Genre with track count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenreRow {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
}

/// Statistics about the library.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryStats {
    pub total_tracks: i64,
    pub total_albums: i64,
    pub total_artists: i64,
    pub total_duration_ms: i64,
    pub total_size_bytes: i64,
}
