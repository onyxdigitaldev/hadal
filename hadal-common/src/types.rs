//! Common type definitions for Hadal.
//!
//! This module contains the core data structures used throughout the application.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Audio Format Types
// ─────────────────────────────────────────────────────────────────────────────

/// Supported audio codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    Flac,
    Mp3,
    Aac,
    Vorbis,
    Opus,
    Wav,
    Aiff,
    Alac,
    WavPack,
    Unknown,
}

impl Codec {
    /// Get the codec from a file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "flac" => Self::Flac,
            "mp3" => Self::Mp3,
            "m4a" | "aac" => Self::Aac,
            "ogg" | "oga" => Self::Vorbis,
            "opus" => Self::Opus,
            "wav" => Self::Wav,
            "aif" | "aiff" => Self::Aiff,
            "wv" => Self::WavPack,
            _ => Self::Unknown,
        }
    }

    /// Check if this is a lossless codec.
    pub fn is_lossless(&self) -> bool {
        matches!(
            self,
            Self::Flac | Self::Wav | Self::Aiff | Self::Alac | Self::WavPack
        )
    }

    /// Get the display name for this codec.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Flac => "FLAC",
            Self::Mp3 => "MP3",
            Self::Aac => "AAC",
            Self::Vorbis => "Vorbis",
            Self::Opus => "Opus",
            Self::Wav => "WAV",
            Self::Aiff => "AIFF",
            Self::Alac => "ALAC",
            Self::WavPack => "WavPack",
            Self::Unknown => "Unknown",
        }
    }
}

impl std::fmt::Display for Codec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Bit depth of audio samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BitDepth {
    /// 8-bit unsigned
    U8,
    /// 16-bit signed
    S16,
    /// 24-bit signed (packed or padded to 32-bit)
    S24,
    /// 32-bit signed
    S32,
    /// 32-bit float (internal processing format)
    F32,
    /// 64-bit float
    F64,
}

impl BitDepth {
    /// Get the number of bits per sample.
    pub fn bits(&self) -> u8 {
        match self {
            Self::U8 => 8,
            Self::S16 => 16,
            Self::S24 => 24,
            Self::S32 => 32,
            Self::F32 => 32,
            Self::F64 => 64,
        }
    }

    /// Get the number of bytes per sample.
    pub fn bytes(&self) -> u8 {
        self.bits().div_ceil(8)
    }
}

impl std::fmt::Display for BitDepth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-bit", self.bits())
    }
}

/// Complete audio format specification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AudioFormat {
    /// Sample rate in Hz (e.g., 44100, 48000, 96000, 192000)
    pub sample_rate: u32,

    /// Number of channels (1 = mono, 2 = stereo)
    pub channels: u8,

    /// Bit depth of samples
    pub bit_depth: BitDepth,

    /// Audio codec
    pub codec: Codec,

    /// Bitrate in kbps (for lossy formats, None for lossless)
    pub bitrate: Option<u32>,
}

impl AudioFormat {
    /// Create a new audio format specification.
    pub fn new(sample_rate: u32, channels: u8, bit_depth: BitDepth, codec: Codec) -> Self {
        Self {
            sample_rate,
            channels,
            bit_depth,
            codec,
            bitrate: None,
        }
    }

    /// Create a format with bitrate (for lossy formats).
    pub fn with_bitrate(mut self, bitrate: u32) -> Self {
        self.bitrate = Some(bitrate);
        self
    }

    /// Get a human-readable format string (e.g., "FLAC 96kHz/24bit").
    pub fn display_string(&self) -> String {
        let rate = if self.sample_rate >= 1000 {
            format!("{}kHz", self.sample_rate / 1000)
        } else {
            format!("{}Hz", self.sample_rate)
        };

        format!("{} {}/{}", self.codec, rate, self.bit_depth)
    }

    /// Check if this is a high-resolution format (>44.1kHz or >16-bit).
    pub fn is_high_res(&self) -> bool {
        self.sample_rate > 44100 || self.bit_depth.bits() > 16
    }
}

impl std::fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Library Types
// ─────────────────────────────────────────────────────────────────────────────

/// Unique identifier for database entities.
pub type EntityId = i64;

/// An artist in the music library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artist {
    /// Database ID
    pub id: EntityId,

    /// Artist name
    pub name: String,

    /// Name used for sorting (e.g., "Beatles, The")
    pub sort_name: Option<String>,

    /// Number of albums by this artist
    pub album_count: u32,

    /// Number of tracks by this artist
    pub track_count: u32,
}

impl Artist {
    /// Get the name to use for sorting.
    pub fn sort_key(&self) -> &str {
        self.sort_name.as_deref().unwrap_or(&self.name)
    }
}

/// An album in the music library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Album {
    /// Database ID
    pub id: EntityId,

    /// Album title
    pub title: String,

    /// Title used for sorting
    pub sort_title: Option<String>,

    /// Primary artist ID
    pub artist_id: Option<EntityId>,

    /// Album artist name (may differ from track artists)
    pub album_artist: Option<String>,

    /// Release year
    pub year: Option<u16>,

    /// Primary genre
    pub genre: Option<String>,

    /// Number of discs
    pub disc_total: u8,

    /// Number of tracks
    pub track_total: u16,

    /// Total duration of all tracks
    pub total_duration: Duration,

    /// Path to cached artwork (if any)
    pub artwork_path: Option<PathBuf>,
}

impl Album {
    /// Get the name to use for sorting.
    pub fn sort_key(&self) -> &str {
        self.sort_title.as_deref().unwrap_or(&self.title)
    }
}

/// A track in the music library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    /// Database ID
    pub id: EntityId,

    /// File path
    pub path: PathBuf,

    /// Track title
    pub title: String,

    /// Title used for sorting
    pub sort_title: Option<String>,

    /// Artist ID
    pub artist_id: Option<EntityId>,

    /// Artist name (denormalized for display)
    pub artist_name: Option<String>,

    /// Album ID
    pub album_id: Option<EntityId>,

    /// Album title (denormalized for display)
    pub album_title: Option<String>,

    /// Track number on the disc
    pub track_number: Option<u16>,

    /// Disc number
    pub disc_number: u8,

    /// Track duration
    pub duration: Duration,

    /// Release year
    pub year: Option<u16>,

    /// Genre
    pub genre: Option<String>,

    /// Audio format information
    pub format: AudioFormat,

    /// File size in bytes
    pub file_size: u64,

    /// Number of times played
    pub play_count: u32,

    /// Last played timestamp (Unix epoch)
    pub last_played: Option<i64>,

    /// User rating (0-5, 0 = unrated)
    pub rating: u8,
}

impl Track {
    /// Get the name to use for sorting.
    pub fn sort_key(&self) -> &str {
        self.sort_title.as_deref().unwrap_or(&self.title)
    }

    /// Get a formatted duration string (e.g., "3:45").
    pub fn duration_string(&self) -> String {
        let total_secs = self.duration.as_secs();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{}:{:02}", mins, secs)
    }

    /// Get the file extension.
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|e| e.to_str())
    }
}

/// A genre in the music library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genre {
    /// Database ID
    pub id: EntityId,

    /// Genre name
    pub name: String,

    /// Number of tracks in this genre
    pub track_count: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Playback Types
// ─────────────────────────────────────────────────────────────────────────────

/// Current playback status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlayStatus {
    /// No track loaded
    #[default]
    Stopped,

    /// Playing audio
    Playing,

    /// Playback paused
    Paused,

    /// Buffering audio data
    Buffering,
}

/// Repeat mode for playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RepeatMode {
    /// No repeat
    #[default]
    Off,

    /// Repeat entire queue
    All,

    /// Repeat current track
    One,
}

impl RepeatMode {
    /// Cycle to the next repeat mode.
    pub fn cycle(&self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sorting Types
// ─────────────────────────────────────────────────────────────────────────────

/// Fields available for sorting library items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    #[default]
    Artist,
    Album,
    Title,
    Year,
    Genre,
    DateAdded,
    Duration,
    PlayCount,
    Rating,
    TrackNumber,
    Path,
}

impl SortField {
    /// Get the display name for this sort field.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Title => "Title",
            Self::Year => "Year",
            Self::Genre => "Genre",
            Self::DateAdded => "Date Added",
            Self::Duration => "Duration",
            Self::PlayCount => "Play Count",
            Self::Rating => "Rating",
            Self::TrackNumber => "Track #",
            Self::Path => "Path",
        }
    }
}

impl std::fmt::Display for SortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Sort order direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

impl SortOrder {
    /// Toggle the sort order.
    pub fn toggle(&self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// Complete sort configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SortConfig {
    /// Primary sort field
    pub primary: SortField,

    /// Primary sort order
    pub primary_order: SortOrder,

    /// Optional secondary sort field
    pub secondary: Option<SortField>,

    /// Secondary sort order
    pub secondary_order: SortOrder,
}

impl SortConfig {
    /// Create a new sort configuration with a single field.
    pub fn new(field: SortField, order: SortOrder) -> Self {
        Self {
            primary: field,
            primary_order: order,
            secondary: None,
            secondary_order: SortOrder::Ascending,
        }
    }

    /// Add a secondary sort field.
    pub fn with_secondary(mut self, field: SortField, order: SortOrder) -> Self {
        self.secondary = Some(field);
        self.secondary_order = order;
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// View Types
// ─────────────────────────────────────────────────────────────────────────────

/// Display mode for library views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    /// Detailed list with metadata columns
    #[default]
    List,

    /// Grid of album artwork
    Grid,

    /// Compact list with minimal info
    Compact,
}

/// Which library view is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibraryTab {
    #[default]
    Artists,
    Albums,
    Tracks,
    Genres,
    Playlists,
}

impl LibraryTab {
    /// Get all available tabs in order.
    pub fn all() -> &'static [Self] {
        &[
            Self::Artists,
            Self::Albums,
            Self::Tracks,
            Self::Genres,
            Self::Playlists,
        ]
    }

    /// Get the display name for this tab.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Artists => "Artists",
            Self::Albums => "Albums",
            Self::Tracks => "Tracks",
            Self::Genres => "Genres",
            Self::Playlists => "Playlists",
        }
    }

    /// Get the next tab.
    pub fn next(&self) -> Self {
        match self {
            Self::Artists => Self::Albums,
            Self::Albums => Self::Tracks,
            Self::Tracks => Self::Genres,
            Self::Genres => Self::Playlists,
            Self::Playlists => Self::Artists,
        }
    }

    /// Get the previous tab.
    pub fn prev(&self) -> Self {
        match self {
            Self::Artists => Self::Playlists,
            Self::Albums => Self::Artists,
            Self::Tracks => Self::Albums,
            Self::Genres => Self::Tracks,
            Self::Playlists => Self::Genres,
        }
    }
}

impl std::fmt::Display for LibraryTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
