//! SQLite database wrapper for the music library.

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{params, Connection, OpenFlags};

use crate::error::{LibraryError, LibraryResult};
use crate::models::{AlbumRow, ArtistRow, TrackRow};
use crate::schema;

/// Thread-safe database wrapper.
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open or create a database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> LibraryResult<Self> {
        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        // Configure SQLite for performance
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            PRAGMA cache_size = -64000;  -- 64MB cache
            PRAGMA temp_store = MEMORY;
            ",
        )?;

        // Initialize schema
        schema::initialize(&conn)?;

        tracing::info!("Database opened: {}", path.display());

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open an in-memory database (for testing).
    pub fn open_memory() -> LibraryResult<Self> {
        let conn = Connection::open_in_memory()?;
        schema::initialize(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Artist Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get or create an artist by name.
    pub fn get_or_create_artist(&self, name: &str) -> LibraryResult<i64> {
        let conn = self.conn.lock();

        // Try to find existing
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM artists WHERE name = ?1 COLLATE NOCASE",
                [name],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            return Ok(id);
        }

        // Create new
        conn.execute(
            "INSERT INTO artists (name) VALUES (?1)",
            [name],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get an artist by ID.
    pub fn get_artist(&self, id: i64) -> LibraryResult<ArtistRow> {
        let conn = self.conn.lock();

        conn.query_row(
            "SELECT id, name, sort_name, created_at, updated_at FROM artists WHERE id = ?1",
            [id],
            |row| {
                Ok(ArtistRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sort_name: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .map_err(|_| LibraryError::ArtistNotFound(id))
    }

    /// Get all artists with track counts.
    pub fn get_all_artists(&self) -> LibraryResult<Vec<ArtistRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, sort_name, created_at, updated_at FROM artists ORDER BY COALESCE(sort_name, name) COLLATE NOCASE",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ArtistRow {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_name: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Album Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Get or create an album.
    pub fn get_or_create_album(
        &self,
        title: &str,
        artist_id: Option<i64>,
        year: Option<i32>,
        album_artist: Option<&str>,
    ) -> LibraryResult<i64> {
        let conn = self.conn.lock();

        // Try to find existing
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM albums WHERE title = ?1 COLLATE NOCASE AND artist_id IS ?2",
                params![title, artist_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            return Ok(id);
        }

        // Create new
        conn.execute(
            "INSERT INTO albums (title, artist_id, year, album_artist) VALUES (?1, ?2, ?3, ?4)",
            params![title, artist_id, year, album_artist],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get an album by ID.
    pub fn get_album(&self, id: i64) -> LibraryResult<AlbumRow> {
        let conn = self.conn.lock();

        conn.query_row(
            "SELECT id, title, sort_title, artist_id, album_artist, year, genre,
                    disc_total, track_total, artwork_hash, created_at, updated_at
             FROM albums WHERE id = ?1",
            [id],
            |row| {
                Ok(AlbumRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    sort_title: row.get(2)?,
                    artist_id: row.get(3)?,
                    album_artist: row.get(4)?,
                    year: row.get(5)?,
                    genre: row.get(6)?,
                    disc_total: row.get(7)?,
                    track_total: row.get(8)?,
                    artwork_hash: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            },
        )
        .map_err(|_| LibraryError::AlbumNotFound(id))
    }

    /// Get all albums, optionally filtered by artist.
    pub fn get_albums(&self, artist_id: Option<i64>) -> LibraryResult<Vec<AlbumRow>> {
        let conn = self.conn.lock();

        let sql = if artist_id.is_some() {
            "SELECT id, title, sort_title, artist_id, album_artist, year, genre,
                    disc_total, track_total, artwork_hash, created_at, updated_at
             FROM albums WHERE artist_id = ?1
             ORDER BY year DESC, COALESCE(sort_title, title) COLLATE NOCASE"
        } else {
            "SELECT id, title, sort_title, artist_id, album_artist, year, genre,
                    disc_total, track_total, artwork_hash, created_at, updated_at
             FROM albums
             ORDER BY COALESCE(sort_title, title) COLLATE NOCASE"
        };

        let mut stmt = conn.prepare(sql)?;
        let rows = if let Some(aid) = artist_id {
            stmt.query_map([aid], Self::map_album_row)?
        } else {
            stmt.query_map([], Self::map_album_row)?
        };

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn map_album_row(row: &rusqlite::Row) -> rusqlite::Result<AlbumRow> {
        Ok(AlbumRow {
            id: row.get(0)?,
            title: row.get(1)?,
            sort_title: row.get(2)?,
            artist_id: row.get(3)?,
            album_artist: row.get(4)?,
            year: row.get(5)?,
            genre: row.get(6)?,
            disc_total: row.get(7)?,
            track_total: row.get(8)?,
            artwork_hash: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }

    /// Set the artwork hash for an album.
    pub fn set_album_artwork_hash(&self, album_id: i64, hash: &str) -> LibraryResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE albums SET artwork_hash = ?2 WHERE id = ?1",
            params![album_id, hash],
        )?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Track Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Insert or update a track.
    pub fn upsert_track(&self, track: &TrackRow) -> LibraryResult<i64> {
        let conn = self.conn.lock();

        conn.execute(
            "INSERT INTO tracks (
                path, title, sort_title, artist_id, album_id, track_number, disc_number,
                duration_ms, year, genre, sample_rate, bit_depth, channels, codec,
                bitrate, file_size, file_mtime
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                sort_title = excluded.sort_title,
                artist_id = excluded.artist_id,
                album_id = excluded.album_id,
                track_number = excluded.track_number,
                disc_number = excluded.disc_number,
                duration_ms = excluded.duration_ms,
                year = excluded.year,
                genre = excluded.genre,
                sample_rate = excluded.sample_rate,
                bit_depth = excluded.bit_depth,
                channels = excluded.channels,
                codec = excluded.codec,
                bitrate = excluded.bitrate,
                file_size = excluded.file_size,
                file_mtime = excluded.file_mtime",
            params![
                track.path,
                track.title,
                track.sort_title,
                track.artist_id,
                track.album_id,
                track.track_number,
                track.disc_number,
                track.duration_ms,
                track.year,
                track.genre,
                track.sample_rate,
                track.bit_depth,
                track.channels,
                track.codec,
                track.bitrate,
                track.file_size,
                track.file_mtime,
            ],
        )?;

        // Get the ID (either inserted or existing)
        let id: i64 = conn.query_row(
            "SELECT id FROM tracks WHERE path = ?1",
            [&track.path],
            |row| row.get(0),
        )?;

        Ok(id)
    }

    /// Get a track by ID.
    pub fn get_track(&self, id: i64) -> LibraryResult<TrackRow> {
        let conn = self.conn.lock();

        conn.query_row(
            "SELECT id, path, title, sort_title, artist_id, album_id, track_number,
                    disc_number, duration_ms, year, genre, sample_rate, bit_depth,
                    channels, codec, bitrate, file_size, play_count, last_played,
                    rating, file_mtime, created_at, updated_at
             FROM tracks WHERE id = ?1",
            [id],
            Self::map_track_row,
        )
        .map_err(|_| LibraryError::TrackNotFound(id))
    }

    /// Get a track by path.
    pub fn get_track_by_path(&self, path: &str) -> LibraryResult<Option<TrackRow>> {
        let conn = self.conn.lock();

        match conn.query_row(
            "SELECT id, path, title, sort_title, artist_id, album_id, track_number,
                    disc_number, duration_ms, year, genre, sample_rate, bit_depth,
                    channels, codec, bitrate, file_size, play_count, last_played,
                    rating, file_mtime, created_at, updated_at
             FROM tracks WHERE path = ?1",
            [path],
            Self::map_track_row,
        ) {
            Ok(track) => Ok(Some(track)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all tracks, with optional filtering.
    pub fn get_tracks(
        &self,
        album_id: Option<i64>,
        artist_id: Option<i64>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> LibraryResult<Vec<TrackRow>> {
        let conn = self.conn.lock();

        let mut sql = String::from(
            "SELECT id, path, title, sort_title, artist_id, album_id, track_number,
                    disc_number, duration_ms, year, genre, sample_rate, bit_depth,
                    channels, codec, bitrate, file_size, play_count, last_played,
                    rating, file_mtime, created_at, updated_at
             FROM tracks WHERE 1=1",
        );

        let mut param_idx = 1;
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(aid) = album_id {
            use std::fmt::Write;
            let _ = write!(sql, " AND album_id = ?{}", param_idx);
            param_values.push(Box::new(aid));
            param_idx += 1;
        }
        if let Some(aid) = artist_id {
            use std::fmt::Write;
            let _ = write!(sql, " AND artist_id = ?{}", param_idx);
            param_values.push(Box::new(aid));
            param_idx += 1;
        }
        let _ = param_idx; // suppress unused warning

        sql.push_str(" ORDER BY disc_number, track_number, title COLLATE NOCASE");

        if let Some(lim) = limit {
            use std::fmt::Write;
            let _ = write!(sql, " LIMIT {}", lim);
        }
        if let Some(off) = offset {
            use std::fmt::Write;
            let _ = write!(sql, " OFFSET {}", off);
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), Self::map_track_row)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn map_track_row(row: &rusqlite::Row) -> rusqlite::Result<TrackRow> {
        Ok(TrackRow {
            id: row.get(0)?,
            path: row.get(1)?,
            title: row.get(2)?,
            sort_title: row.get(3)?,
            artist_id: row.get(4)?,
            album_id: row.get(5)?,
            track_number: row.get(6)?,
            disc_number: row.get(7)?,
            duration_ms: row.get(8)?,
            year: row.get(9)?,
            genre: row.get(10)?,
            sample_rate: row.get(11)?,
            bit_depth: row.get(12)?,
            channels: row.get(13)?,
            codec: row.get(14)?,
            bitrate: row.get(15)?,
            file_size: row.get(16)?,
            play_count: row.get(17)?,
            last_played: row.get(18)?,
            rating: row.get(19)?,
            file_mtime: row.get(20)?,
            created_at: row.get(21)?,
            updated_at: row.get(22)?,
        })
    }

    /// Update play count and last played timestamp.
    pub fn record_play(&self, track_id: i64) -> LibraryResult<()> {
        let conn = self.conn.lock();

        conn.execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played = unixepoch() WHERE id = ?1",
            [track_id],
        )?;

        Ok(())
    }

    /// Update track rating.
    pub fn set_rating(&self, track_id: i64, rating: u8) -> LibraryResult<()> {
        let conn = self.conn.lock();

        conn.execute(
            "UPDATE tracks SET rating = ?1 WHERE id = ?2",
            params![rating.min(5), track_id],
        )?;

        Ok(())
    }

    /// Get total track count.
    pub fn track_count(&self) -> LibraryResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))
            .map_err(Into::into)
    }

    /// Get total album count.
    pub fn album_count(&self) -> LibraryResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM albums", [], |row| row.get(0))
            .map_err(Into::into)
    }

    /// Get total artist count.
    pub fn artist_count(&self) -> LibraryResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM artists", [], |row| row.get(0))
            .map_err(Into::into)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Library Folder Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// Add a library folder.
    pub fn add_folder(&self, path: &str) -> LibraryResult<i64> {
        let conn = self.conn.lock();

        conn.execute(
            "INSERT OR IGNORE INTO library_folders (path) VALUES (?1)",
            [path],
        )?;

        let id: i64 = conn.query_row(
            "SELECT id FROM library_folders WHERE path = ?1",
            [path],
            |row| row.get(0),
        )?;

        Ok(id)
    }

    /// Get all library folders.
    pub fn get_folders(&self) -> LibraryResult<Vec<(i64, String, bool)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, path, enabled FROM library_folders ORDER BY path",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get::<_, i32>(2)? != 0))
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Update folder last scan time.
    pub fn update_folder_scan_time(&self, folder_id: i64) -> LibraryResult<()> {
        let conn = self.conn.lock();

        conn.execute(
            "UPDATE library_folders SET last_scan = unixepoch() WHERE id = ?1",
            [folder_id],
        )?;

        Ok(())
    }

    /// Remove tracks not in the specified paths.
    pub fn remove_missing_tracks(&self, valid_paths: &[String]) -> LibraryResult<usize> {
        if valid_paths.is_empty() {
            return Ok(0);
        }

        let conn = self.conn.lock();

        // This is a simplified version - for large libraries, we'd want to batch this
        let placeholders: Vec<_> = valid_paths.iter().map(|_| "?").collect();
        let sql = format!(
            "DELETE FROM tracks WHERE path NOT IN ({})",
            placeholders.join(",")
        );

        let params: Vec<&dyn rusqlite::ToSql> = valid_paths
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();

        let deleted = conn.execute(&sql, params.as_slice())?;

        Ok(deleted)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // FTS Search
    // ─────────────────────────────────────────────────────────────────────────

    /// Update the FTS index for a track.
    pub fn update_fts(&self, track_id: i64, title: &str, artist: &str, album: &str, genre: &str) -> LibraryResult<()> {
        let conn = self.conn.lock();

        // Delete existing entry
        conn.execute(
            "DELETE FROM tracks_fts WHERE rowid = ?1",
            [track_id],
        )?;

        // Insert new entry
        conn.execute(
            "INSERT INTO tracks_fts (rowid, title, artist_name, album_title, genre) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![track_id, title, artist, album, genre],
        )?;

        Ok(())
    }

    /// Search tracks using FTS.
    pub fn search(&self, query: &str, limit: usize) -> LibraryResult<Vec<i64>> {
        let conn = self.conn.lock();

        // Prepare search query (add wildcards for prefix matching)
        let search_query = query
            .split_whitespace()
            .map(|word| format!("{}*", word))
            .collect::<Vec<_>>()
            .join(" ");

        let mut stmt = conn.prepare(
            "SELECT rowid FROM tracks_fts WHERE tracks_fts MATCH ?1 ORDER BY rank LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![search_query, limit as i64], |row| row.get(0))?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
        }
    }
}
