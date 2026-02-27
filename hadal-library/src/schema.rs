//! Database schema and migrations.

use rusqlite::Connection;

use crate::error::LibraryResult;

/// Current schema version.
pub const SCHEMA_VERSION: i32 = 1;

/// Initialize the database schema.
pub fn initialize(conn: &Connection) -> LibraryResult<()> {
    // Check current version
    let current_version = get_schema_version(conn)?;

    if current_version == 0 {
        // Fresh database, create all tables
        create_schema(conn)?;
        set_schema_version(conn, SCHEMA_VERSION)?;
    } else if current_version < SCHEMA_VERSION {
        // Need to migrate
        migrate(conn, current_version, SCHEMA_VERSION)?;
    }

    Ok(())
}

/// Get the current schema version.
fn get_schema_version(conn: &Connection) -> LibraryResult<i32> {
    let result: Result<i32, _> = conn.query_row(
        "SELECT version FROM schema_version LIMIT 1",
        [],
        |row| row.get(0),
    );

    match result {
        Ok(version) => Ok(version),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(rusqlite::Error::SqliteFailure(_, _)) => Ok(0), // Table doesn't exist
        Err(e) => Err(e.into()),
    }
}

/// Set the schema version.
fn set_schema_version(conn: &Connection, version: i32) -> LibraryResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO schema_version (id, version) VALUES (1, ?1)",
        [version],
    )?;
    Ok(())
}

/// Create the complete database schema.
fn create_schema(conn: &Connection) -> LibraryResult<()> {
    conn.execute_batch(SCHEMA_SQL)?;
    tracing::info!("Database schema created (version {})", SCHEMA_VERSION);
    Ok(())
}

/// Migrate between schema versions.
fn migrate(conn: &Connection, from: i32, to: i32) -> LibraryResult<()> {
    tracing::info!("Migrating database from version {} to {}", from, to);

    // Add migration logic here as needed
    // For now, we only have version 1

    set_schema_version(conn, to)?;
    Ok(())
}

/// The complete database schema.
const SCHEMA_SQL: &str = r#"
-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_version (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    version     INTEGER NOT NULL
);

-- Artists
CREATE TABLE IF NOT EXISTS artists (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL COLLATE NOCASE,
    sort_name   TEXT COLLATE NOCASE,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(name)
);

-- Albums
CREATE TABLE IF NOT EXISTS albums (
    id              INTEGER PRIMARY KEY,
    title           TEXT NOT NULL COLLATE NOCASE,
    sort_title      TEXT COLLATE NOCASE,
    artist_id       INTEGER REFERENCES artists(id) ON DELETE SET NULL,
    album_artist    TEXT COLLATE NOCASE,
    year            INTEGER,
    genre           TEXT,
    disc_total      INTEGER DEFAULT 1,
    track_total     INTEGER,
    artwork_hash    TEXT,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Tracks
CREATE TABLE IF NOT EXISTS tracks (
    id              INTEGER PRIMARY KEY,
    path            TEXT NOT NULL UNIQUE,
    title           TEXT NOT NULL COLLATE NOCASE,
    sort_title      TEXT COLLATE NOCASE,
    artist_id       INTEGER REFERENCES artists(id) ON DELETE SET NULL,
    album_id        INTEGER REFERENCES albums(id) ON DELETE SET NULL,
    track_number    INTEGER,
    disc_number     INTEGER DEFAULT 1,
    duration_ms     INTEGER NOT NULL,
    year            INTEGER,
    genre           TEXT,
    -- Audio format info
    sample_rate     INTEGER,
    bit_depth       INTEGER,
    channels        INTEGER,
    codec           TEXT,
    bitrate         INTEGER,
    file_size       INTEGER,
    -- Metadata
    play_count      INTEGER DEFAULT 0,
    last_played     INTEGER,
    rating          INTEGER DEFAULT 0 CHECK(rating BETWEEN 0 AND 5),
    -- File tracking
    file_mtime      INTEGER NOT NULL,
    file_hash       TEXT,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Genres (normalized)
CREATE TABLE IF NOT EXISTS genres (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL UNIQUE COLLATE NOCASE
);

-- Track-Genre many-to-many
CREATE TABLE IF NOT EXISTS track_genres (
    track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    genre_id    INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, genre_id)
);

-- Library folders
CREATE TABLE IF NOT EXISTS library_folders (
    id          INTEGER PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    enabled     INTEGER DEFAULT 1,
    last_scan   INTEGER,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Full-text search index for tracks
CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
    title,
    artist_name,
    album_title,
    genre,
    content='',
    tokenize='unicode61 remove_diacritics 2'
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist_id);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_id);
CREATE INDEX IF NOT EXISTS idx_tracks_genre ON tracks(genre);
CREATE INDEX IF NOT EXISTS idx_tracks_year ON tracks(year);
CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(path);
CREATE INDEX IF NOT EXISTS idx_tracks_rating ON tracks(rating) WHERE rating > 0;
CREATE INDEX IF NOT EXISTS idx_tracks_play_count ON tracks(play_count) WHERE play_count > 0;
CREATE INDEX IF NOT EXISTS idx_albums_artist ON albums(artist_id);
CREATE INDEX IF NOT EXISTS idx_albums_year ON albums(year);
CREATE INDEX IF NOT EXISTS idx_albums_genre ON albums(genre);

-- Triggers for updated_at
CREATE TRIGGER IF NOT EXISTS artists_updated AFTER UPDATE ON artists
BEGIN
    UPDATE artists SET updated_at = unixepoch() WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS albums_updated AFTER UPDATE ON albums
BEGIN
    UPDATE albums SET updated_at = unixepoch() WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS tracks_updated AFTER UPDATE ON tracks
BEGIN
    UPDATE tracks SET updated_at = unixepoch() WHERE id = NEW.id;
END;
"#;
