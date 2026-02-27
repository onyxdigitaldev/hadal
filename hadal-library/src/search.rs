//! Search functionality for the music library.

use std::collections::HashSet;

use crate::db::Database;
use crate::error::LibraryResult;
use crate::models::TrackRow;
use hadal_common::{SortField, SortOrder};

/// Search query builder.
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    /// Full-text search term
    pub text: Option<String>,

    /// Filter by artist ID
    pub artist_id: Option<i64>,

    /// Filter by album ID
    pub album_id: Option<i64>,

    /// Filter by genre
    pub genre: Option<String>,

    /// Filter by year range
    pub year_min: Option<i32>,
    pub year_max: Option<i32>,

    /// Filter by rating
    pub min_rating: Option<u8>,

    /// Sort field
    pub sort_by: SortField,

    /// Sort order
    pub sort_order: SortOrder,

    /// Maximum results
    pub limit: usize,

    /// Offset for pagination
    pub offset: usize,
}

impl SearchQuery {
    /// Create a new search query.
    pub fn new() -> Self {
        Self {
            text: None,
            artist_id: None,
            album_id: None,
            genre: None,
            year_min: None,
            year_max: None,
            min_rating: None,
            sort_by: SortField::Artist,
            sort_order: SortOrder::Ascending,
            limit: 100,
            offset: 0,
        }
    }

    /// Set the search text.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Filter by artist.
    pub fn artist(mut self, artist_id: i64) -> Self {
        self.artist_id = Some(artist_id);
        self
    }

    /// Filter by album.
    pub fn album(mut self, album_id: i64) -> Self {
        self.album_id = Some(album_id);
        self
    }

    /// Filter by genre.
    pub fn genre(mut self, genre: impl Into<String>) -> Self {
        self.genre = Some(genre.into());
        self
    }

    /// Filter by year range.
    pub fn years(mut self, min: i32, max: i32) -> Self {
        self.year_min = Some(min);
        self.year_max = Some(max);
        self
    }

    /// Filter by minimum rating.
    pub fn min_rating(mut self, rating: u8) -> Self {
        self.min_rating = Some(rating);
        self
    }

    /// Set sort field and order.
    pub fn sort(mut self, field: SortField, order: SortOrder) -> Self {
        self.sort_by = field;
        self.sort_order = order;
        self
    }

    /// Set result limit.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set result offset.
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// Execute the search query.
    pub fn execute(&self, db: &Database) -> LibraryResult<Vec<TrackRow>> {
        // If there's a text search, use FTS
        if let Some(ref text) = self.text {
            if !text.trim().is_empty() {
                let track_ids = db.search(text, self.limit)?;
                return self.fetch_tracks_by_ids(db, &track_ids);
            }
        }

        // Otherwise, use regular filtering
        db.get_tracks(self.album_id, self.artist_id, Some(self.limit), Some(self.offset))
    }

    /// Fetch tracks by IDs, preserving order.
    fn fetch_tracks_by_ids(&self, db: &Database, ids: &[i64]) -> LibraryResult<Vec<TrackRow>> {
        let mut tracks = Vec::with_capacity(ids.len());

        for id in ids {
            if let Ok(track) = db.get_track(*id) {
                tracks.push(track);
            }
        }

        Ok(tracks)
    }
}

/// Quick search that returns matches from all categories.
#[derive(Debug, Clone, Default)]
pub struct QuickSearchResults {
    pub tracks: Vec<TrackRow>,
    pub albums: Vec<i64>,
    pub artists: Vec<i64>,
}

/// Perform a quick search across all categories.
pub fn quick_search(db: &Database, query: &str, limit: usize) -> LibraryResult<QuickSearchResults> {
    let track_ids = db.search(query, limit)?;

    let mut results = QuickSearchResults::default();
    let mut seen_albums = HashSet::new();
    let mut seen_artists = HashSet::new();

    for id in track_ids {
        if let Ok(track) = db.get_track(id) {
            // Collect unique albums and artists
            if let Some(album_id) = track.album_id {
                if seen_albums.insert(album_id) {
                    results.albums.push(album_id);
                }
            }
            if let Some(artist_id) = track.artist_id {
                if seen_artists.insert(artist_id) {
                    results.artists.push(artist_id);
                }
            }

            results.tracks.push(track);
        }
    }

    Ok(results)
}
