//! # hadal-library
//!
//! Music library management and indexing for the Hadal music player.
//!
//! This crate provides:
//! - Directory scanning for audio files
//! - Tag extraction and metadata indexing
//! - SQLite database with FTS5 for search
//! - Album artwork extraction and caching
//! - File system watching for library updates

pub mod artwork;
pub mod db;
pub mod indexer;
pub mod models;
pub mod scanner;
pub mod schema;
pub mod search;

mod error;

pub use db::Database;
pub use error::{LibraryError, LibraryResult};
pub use indexer::Indexer;
pub use models::*;
pub use scanner::Scanner;
pub use search::SearchQuery;
