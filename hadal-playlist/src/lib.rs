//! # hadal-playlist
//!
//! Playlist management for the Hadal music player.
//!
//! This crate provides:
//! - M3U8 playlist import/export
//! - SQLite-backed internal playlists with rich metadata
//! - Play queue management

pub mod internal;
pub mod m3u8;
pub mod queue;

mod error;

pub use error::{PlaylistError, PlaylistResult};
pub use internal::{Playlist, PlaylistManager};
pub use m3u8::{M3u8Playlist, M3u8Reader, M3u8Writer};
pub use queue::{PlayQueue, QueueItem};
