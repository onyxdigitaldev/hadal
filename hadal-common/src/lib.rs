//! # hadal-common
//!
//! Shared types, errors, and utilities for the Hadal music player.
//!
//! This crate provides the foundational types used across all Hadal crates:
//! - Configuration management
//! - Error types and result aliases
//! - XDG path utilities
//! - Common event definitions
//! - Shared type definitions (Track, Album, Artist, etc.)

pub mod config;
pub mod error;
pub mod events;
pub mod paths;
pub mod types;

pub use config::{AudioConfig, ColorConfig, Config, UiConfig, parse_color};
pub use error::{Error, Result};
pub use paths::Paths;
pub use types::*;

/// Playback settings resolved from config, passed to the TUI at startup.
#[derive(Debug, Clone)]
pub struct PlaybackSettings {
    /// Resampler quality: "fast", "medium", or "best"
    pub resampler_quality: String,
    /// Audio buffer size in samples
    pub buffer_size: usize,
    /// Enable gapless playback
    pub gapless: bool,
    /// Initial volume (0.0–1.0)
    pub default_volume: f32,
}
