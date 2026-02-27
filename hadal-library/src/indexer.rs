//! Tag extraction and library indexing.

use std::path::{Path, PathBuf};
use std::time::Duration;

use lofty::{Accessor, AudioFile, ItemKey, TaggedFileExt};

use crate::artwork::ArtworkManager;
use crate::db::Database;
use crate::error::{LibraryError, LibraryResult};
use crate::models::{ScanProgress, TrackRow};

/// Extracted tag metadata from a single audio file.
struct TrackMetadata {
    title: String,
    artist_name: Option<String>,
    album_artist_name: Option<String>,
    album_name: Option<String>,
    genre: Option<String>,
    year: Option<i32>,
    track_number: Option<i32>,
    disc_number: Option<i32>,
    duration_ms: i64,
    sample_rate: Option<i32>,
    bit_depth: Option<i32>,
    channels: Option<i32>,
    bitrate: Option<i32>,
    codec: Option<String>,
}

/// Music library indexer.
pub struct Indexer {
    db: Database,
    artwork: Option<ArtworkManager>,
}

impl Indexer {
    /// Create a new indexer with the given database.
    pub fn new(db: Database) -> Self {
        Self { db, artwork: None }
    }

    /// Create a new indexer with artwork extraction enabled.
    pub fn with_artwork(db: Database, artwork_cache_dir: PathBuf) -> Self {
        Self {
            db,
            artwork: Some(ArtworkManager::new(artwork_cache_dir, 200)),
        }
    }

    /// Index a single audio file.
    pub fn index_file(&self, path: &Path) -> LibraryResult<i64> {
        let (file_mtime, file_size) = Self::file_metadata(path)?;

        // Check if file is already indexed and unchanged
        let path_str = path.to_string_lossy().to_string();
        if let Some(existing) = self.check_file_unchanged(&path_str, file_mtime, path)? {
            return Ok(existing);
        }

        // Extract metadata from tags
        let meta = Self::extract_metadata(path)?;

        // Resolve artist/album in database
        let (artist_id, album_id) = self.resolve_artist_album(&meta)?;

        // Extract artwork if album exists and doesn't have artwork yet
        self.try_extract_artwork(album_id, path);

        // Insert track into database
        self.insert_track(
            &path_str, &meta, artist_id, album_id, file_mtime, file_size,
        )
    }

    /// Get file modification time and size.
    fn file_metadata(path: &Path) -> LibraryResult<(i64, i64)> {
        let metadata = std::fs::metadata(path)?;
        let file_mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let file_size = metadata.len() as i64;
        Ok((file_mtime, file_size))
    }

    /// Check if a file is already indexed and unchanged. Returns `Some(-1)` if skipped.
    fn check_file_unchanged(
        &self,
        path_str: &str,
        file_mtime: i64,
        path: &Path,
    ) -> LibraryResult<Option<i64>> {
        if let Ok(Some(existing)) = self.db.get_track_by_path(path_str) {
            if existing.file_mtime == file_mtime {
                self.try_extract_artwork(existing.album_id, path);
                return Ok(Some(-1));
            }
        }
        Ok(None)
    }

    /// Extract all tag metadata from an audio file.
    fn extract_metadata(path: &Path) -> LibraryResult<TrackMetadata> {
        let tagged_file = lofty::read_from_path(path)
            .map_err(|e| LibraryError::TagRead(e.to_string()))?;

        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
        let properties = tagged_file.properties();

        let title = tag
            .and_then(|t| t.title().map(|s| s.to_string()))
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string()
            });

        Ok(TrackMetadata {
            title,
            artist_name: tag.and_then(|t| t.artist().map(|s| s.to_string())),
            album_artist_name: tag.and_then(|t| t.get_string(&ItemKey::AlbumArtist).map(|s| s.to_string())),
            album_name: tag.and_then(|t| t.album().map(|s| s.to_string())),
            genre: tag.and_then(|t| t.genre().map(|s| s.to_string())),
            year: tag.and_then(|t| t.year()).map(|y| y as i32),
            track_number: tag.and_then(|t| t.track()).map(|n| n as i32),
            disc_number: tag.and_then(|t| t.disk()).map(|n| n as i32),
            duration_ms: properties.duration().as_millis() as i64,
            sample_rate: properties.sample_rate().map(|r| r as i32),
            bit_depth: properties.bit_depth().map(|d| d as i32),
            channels: properties.channels().map(|c| c as i32),
            bitrate: properties.audio_bitrate().map(|b| b as i32),
            codec: path.extension().and_then(|e| e.to_str()).map(|s| s.to_uppercase()),
        })
    }

    /// Resolve artist and album IDs from metadata, creating entries as needed.
    fn resolve_artist_album(&self, meta: &TrackMetadata) -> LibraryResult<(Option<i64>, Option<i64>)> {
        let grouping_artist = meta.album_artist_name.as_deref().or(meta.artist_name.as_deref());

        let artist_id = match grouping_artist {
            Some(name) => Some(self.db.get_or_create_artist(name)?),
            None => None,
        };

        let album_id = match &meta.album_name {
            Some(name) => Some(self.db.get_or_create_album(
                name,
                artist_id,
                meta.year,
                meta.album_artist_name.as_deref(),
            )?),
            None => None,
        };

        Ok((artist_id, album_id))
    }

    /// Insert a track into the database and update the FTS index.
    fn insert_track(
        &self,
        path_str: &str,
        meta: &TrackMetadata,
        artist_id: Option<i64>,
        album_id: Option<i64>,
        file_mtime: i64,
        file_size: i64,
    ) -> LibraryResult<i64> {
        let mut track = TrackRow::new(
            path_str.to_string(),
            meta.title.clone(),
            meta.duration_ms,
            file_mtime,
        );
        track.artist_id = artist_id;
        track.album_id = album_id;
        track.track_number = meta.track_number;
        track.disc_number = meta.disc_number;
        track.year = meta.year;
        track.genre = meta.genre.clone();
        track.sample_rate = meta.sample_rate;
        track.bit_depth = meta.bit_depth;
        track.channels = meta.channels;
        track.codec = meta.codec.clone();
        track.bitrate = meta.bitrate;
        track.file_size = Some(file_size);

        let track_id = self.db.upsert_track(&track)?;

        let grouping_artist = meta.album_artist_name.as_deref().or(meta.artist_name.as_deref());
        self.db.update_fts(
            track_id,
            &meta.title,
            grouping_artist.unwrap_or(""),
            meta.album_name.as_deref().unwrap_or(""),
            meta.genre.as_deref().unwrap_or(""),
        )?;

        tracing::debug!("Indexed: {} - {}", grouping_artist.unwrap_or("Unknown"), meta.title);

        Ok(track_id)
    }

    /// Index multiple files with progress tracking.
    pub fn index_files<F>(
        &self,
        files: &[std::path::PathBuf],
        progress_callback: F,
    ) -> LibraryResult<ScanProgress>
    where
        F: Fn(&ScanProgress),
    {
        let mut progress = ScanProgress::new(files.len());

        for path in files {
            progress.current_file = Some(path.to_string_lossy().to_string());
            progress_callback(&progress);

            match self.index_file(path) {
                Ok(id) if id >= 0 => {
                    progress.added += 1;
                }
                Ok(_) => {
                    // Skipped (unchanged file)
                }
                Err(e) => {
                    tracing::warn!("Failed to index {}: {}", path.display(), e);
                    progress.errors += 1;
                }
            }

            progress.scanned_files += 1;
        }

        progress.current_file = None;
        progress_callback(&progress);

        Ok(progress)
    }

    /// Try to extract and cache artwork for an album if it doesn't have one yet.
    fn try_extract_artwork(&self, album_id: Option<i64>, track_path: &Path) {
        let (aid, artwork_mgr) = match (album_id, &self.artwork) {
            (Some(aid), Some(mgr)) => (aid, mgr),
            _ => return,
        };

        let needs_artwork = match self.db.get_album(aid) {
            Ok(a) => a.artwork_hash.is_none(),
            Err(e) => {
                tracing::warn!("Failed to get album {} for artwork check: {}", aid, e);
                false
            }
        };

        if !needs_artwork {
            return;
        }

        match artwork_mgr.get_artwork(track_path) {
            Ok(Some(cache_path)) => {
                if let Some(stem) = cache_path.file_stem().and_then(|s| s.to_str()) {
                    tracing::debug!("Artwork cached for album {}: {}", aid, stem);
                    if let Err(e) = self.db.set_album_artwork_hash(aid, stem) {
                        tracing::warn!("Failed to set artwork hash for album {}: {}", aid, e);
                    }
                }
            }
            Ok(None) => {
                tracing::debug!("No artwork found for album {} via {}", aid, track_path.display());
            }
            Err(e) => {
                tracing::debug!("Artwork extraction failed for {}: {}", track_path.display(), e);
            }
        }
    }

    /// Extract duration from an audio file.
    pub fn get_duration(path: &Path) -> LibraryResult<Duration> {
        let tagged_file = lofty::read_from_path(path)
            .map_err(|e| LibraryError::TagRead(e.to_string()))?;

        Ok(tagged_file.properties().duration())
    }

    /// Get database reference.
    pub fn database(&self) -> &Database {
        &self.db
    }
}
