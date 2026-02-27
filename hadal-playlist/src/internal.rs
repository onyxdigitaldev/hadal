//! SQLite-backed internal playlists.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::{PlaylistError, PlaylistResult};
use crate::m3u8::{M3u8Entry, M3u8Playlist, M3u8Writer};

/// A playlist with rich metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub track_count: u32,
    pub total_duration_ms: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A track in a playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTrack {
    pub id: i64,
    pub playlist_id: i64,
    pub track_id: i64,
    pub position: i32,
    pub added_at: i64,
}

/// Manages playlists in the database.
pub struct PlaylistManager {
    conn: Arc<Mutex<Connection>>,
}

impl PlaylistManager {
    /// Create a new playlist manager.
    pub fn new(conn: Arc<Mutex<Connection>>) -> PlaylistResult<Self> {
        let manager = Self { conn };
        manager.initialize_schema()?;
        Ok(manager)
    }

    /// Open or create a playlist database.
    pub fn open<P: AsRef<Path>>(path: P) -> PlaylistResult<Self> {
        let conn = Connection::open(path)?;
        Self::new(Arc::new(Mutex::new(conn)))
    }

    /// Initialize the playlist schema.
    fn initialize_schema(&self) -> PlaylistResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS playlists (
                id          INTEGER PRIMARY KEY,
                name        TEXT NOT NULL UNIQUE,
                description TEXT,
                created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
            );

            CREATE TABLE IF NOT EXISTS playlist_tracks (
                id          INTEGER PRIMARY KEY,
                playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                track_id    INTEGER NOT NULL,
                position    INTEGER NOT NULL,
                added_at    INTEGER NOT NULL DEFAULT (unixepoch()),
                UNIQUE(playlist_id, position)
            );

            CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist
                ON playlist_tracks(playlist_id, position);

            CREATE TRIGGER IF NOT EXISTS playlists_updated AFTER UPDATE ON playlists
            BEGIN
                UPDATE playlists SET updated_at = unixepoch() WHERE id = NEW.id;
            END;
            ",
        )?;
        Ok(())
    }

    /// Create a new playlist.
    pub fn create(&self, name: &str, description: Option<&str>) -> PlaylistResult<i64> {
        let conn = self.conn.lock();

        // Check if name already exists
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM playlists WHERE name = ?1",
                [name],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if exists {
            return Err(PlaylistError::AlreadyExists(name.to_string()));
        }

        conn.execute(
            "INSERT INTO playlists (name, description) VALUES (?1, ?2)",
            params![name, description],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get all playlists.
    pub fn list(&self) -> PlaylistResult<Vec<Playlist>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, p.description,
                    COUNT(pt.id) as track_count,
                    p.created_at, p.updated_at
             FROM playlists p
             LEFT JOIN playlist_tracks pt ON p.id = pt.playlist_id
             GROUP BY p.id
             ORDER BY p.name COLLATE NOCASE",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                track_count: row.get(3)?,
                total_duration_ms: 0, // Would need to join with tracks table
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get a playlist by ID.
    pub fn get(&self, id: i64) -> PlaylistResult<Playlist> {
        self.query_playlist(
            "WHERE p.id = ?1",
            [&id as &dyn rusqlite::types::ToSql],
            &id.to_string(),
        )
    }

    /// Get a playlist by name.
    pub fn get_by_name(&self, name: &str) -> PlaylistResult<Playlist> {
        self.query_playlist(
            "WHERE p.name = ?1 COLLATE NOCASE",
            [&name as &dyn rusqlite::types::ToSql],
            name,
        )
    }

    /// Shared playlist query helper.
    fn query_playlist(
        &self,
        where_clause: &str,
        params: impl rusqlite::Params,
        not_found_key: &str,
    ) -> PlaylistResult<Playlist> {
        let conn = self.conn.lock();

        let sql = format!(
            "SELECT p.id, p.name, p.description,
                    COUNT(pt.id) as track_count,
                    p.created_at, p.updated_at
             FROM playlists p
             LEFT JOIN playlist_tracks pt ON p.id = pt.playlist_id
             {} GROUP BY p.id",
            where_clause
        );

        conn.query_row(&sql, params, |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                track_count: row.get(3)?,
                total_duration_ms: 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|_| PlaylistError::NotFound(not_found_key.to_string()))
    }

    /// Rename a playlist.
    pub fn rename(&self, id: i64, new_name: &str) -> PlaylistResult<()> {
        let conn = self.conn.lock();

        let affected = conn.execute(
            "UPDATE playlists SET name = ?1 WHERE id = ?2",
            params![new_name, id],
        )?;

        if affected == 0 {
            return Err(PlaylistError::NotFound(id.to_string()));
        }

        Ok(())
    }

    /// Delete a playlist.
    pub fn delete(&self, id: i64) -> PlaylistResult<()> {
        let conn = self.conn.lock();

        let affected = conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;

        if affected == 0 {
            return Err(PlaylistError::NotFound(id.to_string()));
        }

        Ok(())
    }

    /// Get tracks in a playlist, ordered by position.
    pub fn get_tracks(&self, playlist_id: i64) -> PlaylistResult<Vec<PlaylistTrack>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, playlist_id, track_id, position, added_at
             FROM playlist_tracks
             WHERE playlist_id = ?1
             ORDER BY position",
        )?;

        let rows = stmt.query_map([playlist_id], |row| {
            Ok(PlaylistTrack {
                id: row.get(0)?,
                playlist_id: row.get(1)?,
                track_id: row.get(2)?,
                position: row.get(3)?,
                added_at: row.get(4)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Add a track to a playlist.
    pub fn add_track(&self, playlist_id: i64, track_id: i64) -> PlaylistResult<()> {
        let conn = self.conn.lock();

        // Get the next position
        let next_pos: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
                [playlist_id],
                |row| row.get(0),
            )?;

        conn.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            params![playlist_id, track_id, next_pos],
        )?;

        Ok(())
    }

    /// Add multiple tracks to a playlist.
    pub fn add_tracks(&self, playlist_id: i64, track_ids: &[i64]) -> PlaylistResult<()> {
        let conn = self.conn.lock();

        // Get the next position
        let mut next_pos: i32 = conn.query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            [playlist_id],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
        )?;

        for track_id in track_ids {
            stmt.execute(params![playlist_id, track_id, next_pos])?;
            next_pos += 1;
        }

        Ok(())
    }

    /// Remove a track from a playlist.
    pub fn remove_track(&self, playlist_id: i64, position: i32) -> PlaylistResult<()> {
        let conn = self.conn.lock();

        let affected = conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND position = ?2",
            params![playlist_id, position],
        )?;

        if affected == 0 {
            return Err(PlaylistError::InvalidPosition(position as usize));
        }

        // Reorder remaining tracks
        conn.execute(
            "UPDATE playlist_tracks SET position = position - 1
             WHERE playlist_id = ?1 AND position > ?2",
            params![playlist_id, position],
        )?;

        Ok(())
    }

    /// Move a track within a playlist.
    pub fn move_track(&self, playlist_id: i64, from: i32, to: i32) -> PlaylistResult<()> {
        if from == to {
            return Ok(());
        }

        let conn = self.conn.lock();

        // Use a temporary position to avoid conflicts
        let temp_pos = -1;

        // Move the track to temporary position
        conn.execute(
            "UPDATE playlist_tracks SET position = ?1
             WHERE playlist_id = ?2 AND position = ?3",
            params![temp_pos, playlist_id, from],
        )?;

        // Shift other tracks
        if from < to {
            conn.execute(
                "UPDATE playlist_tracks SET position = position - 1
                 WHERE playlist_id = ?1 AND position > ?2 AND position <= ?3",
                params![playlist_id, from, to],
            )?;
        } else {
            conn.execute(
                "UPDATE playlist_tracks SET position = position + 1
                 WHERE playlist_id = ?1 AND position >= ?2 AND position < ?3",
                params![playlist_id, to, from],
            )?;
        }

        // Move track to final position
        conn.execute(
            "UPDATE playlist_tracks SET position = ?1
             WHERE playlist_id = ?2 AND position = ?3",
            params![to, playlist_id, temp_pos],
        )?;

        Ok(())
    }

    /// Clear all tracks from a playlist.
    pub fn clear(&self, playlist_id: i64) -> PlaylistResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
            [playlist_id],
        )?;
        Ok(())
    }

    /// Export a playlist to M3U8 format.
    pub fn export_m3u8<P: AsRef<Path>>(
        &self,
        playlist_id: i64,
        path: P,
        track_paths: &[(i64, String, Option<Duration>, Option<String>)], // (id, path, duration, title)
    ) -> PlaylistResult<()> {
        let playlist = self.get(playlist_id)?;
        let tracks = self.get_tracks(playlist_id)?;

        let mut m3u8 = M3u8Playlist::new(&playlist.name);

        for pt in tracks {
            if let Some((_, path, duration, title)) = track_paths.iter().find(|(id, _, _, _)| *id == pt.track_id) {
                let entry = M3u8Entry::with_metadata(
                    path.into(),
                    *duration,
                    title.clone(),
                    None,
                );
                m3u8.add(entry);
            }
        }

        M3u8Writer::write(&m3u8, path)?;

        Ok(())
    }
}

impl Clone for PlaylistManager {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
        }
    }
}
