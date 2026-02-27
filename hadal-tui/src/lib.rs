//! hadal-tui — Terminal user interface for the Hadal music player.

pub mod app;
pub mod input;
pub mod output;
pub mod state;
pub mod theme;
pub mod views;
pub mod widgets;

pub use app::run;

// Re-export for convenience
pub use hadal_common::PlaybackSettings;
