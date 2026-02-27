//! View rendering dispatch.

pub mod equalizer;
pub mod library;
pub mod now_playing;
pub mod playlists;
pub mod queue;
pub mod search;
pub mod shell;

use ratatui::Frame;

use crate::state::{AppState, ViewId};

/// Render the full UI frame.
pub fn render(frame: &mut Frame, state: &mut AppState) {
    shell::render(frame, state);
}

/// Render the content area for the active view.
pub fn render_content(frame: &mut Frame, area: ratatui::layout::Rect, state: &mut AppState) {
    match state.active_view {
        ViewId::Library => library::render(frame, area, state),
        ViewId::NowPlaying => now_playing::render(frame, area, state),
        ViewId::Queue => queue::render(frame, area, &*state),
        ViewId::Equalizer => equalizer::render(frame, area, &*state),
        ViewId::Playlists => playlists::render(frame, area, &*state),
        ViewId::Search => {
            // Search is an overlay — render library underneath
            library::render(frame, area, state);
        }
    }

    // Render search overlay if active
    if state.search.active {
        search::render(frame, area, &*state);
    }
}
