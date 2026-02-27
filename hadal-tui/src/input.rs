//! Key event → UiAction mapping.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::state::{AppState, InputMode, ViewId};

/// Actions the TUI can perform in response to input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    // Navigation
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    GoTop,
    GoBottom,
    Select,

    // Playback
    PlayPause,
    Stop,
    NextTrack,
    PrevTrack,
    SeekForward,
    SeekBackward,
    VolumeUp,
    VolumeDown,
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,

    // Views
    SwitchView(ViewId),
    OpenSearch,
    CloseOverlay,

    // Queue
    AddToQueue,
    AddAlbumToQueue,
    RemoveFromQueue,
    MoveUp,
    MoveDown,

    // Search
    SearchInput(char),
    SearchBackspace,
    SearchSubmit,

    // Equalizer
    EqBandLeft,
    EqBandRight,
    EqGainUp,
    EqGainDown,
    EqToggleBypass,
    EqNextPreset,
    EqReset,

    // Playlists
    PlaylistCreate,
    PlaylistDelete,
    PlaylistRename,
    PlaylistRemoveTrack,
    PlaylistPaneLeft,
    PlaylistPaneRight,
    AddToPlaylist,

    // Playlist name input
    PlaylistNameInput(char),
    PlaylistNameBackspace,
    PlaylistNameSubmit,
    PlaylistNameCancel,

    // App
    Quit,
    Refresh,

    None,
}

/// Map a key event to an action based on the current state.
pub fn handle_key(key: KeyEvent, state: &AppState) -> Action {
    // Ctrl+C / Ctrl+Q always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') | KeyCode::Char('q') => return Action::Quit,
            _ => {}
        }
    }

    match state.input_mode {
        InputMode::Search => handle_search_key(key),
        InputMode::PlaylistName => handle_playlist_name_key(key),
        InputMode::Normal => handle_normal_key(key, state),
    }
}

fn handle_search_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::CloseOverlay,
        KeyCode::Enter => Action::SearchSubmit,
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Char(c) => Action::SearchInput(c),
        KeyCode::Up => Action::Up,
        KeyCode::Down => Action::Down,
        _ => Action::None,
    }
}

fn handle_playlist_name_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::PlaylistNameCancel,
        KeyCode::Enter => Action::PlaylistNameSubmit,
        KeyCode::Backspace => Action::PlaylistNameBackspace,
        KeyCode::Char(c) => Action::PlaylistNameInput(c),
        _ => Action::None,
    }
}

fn handle_normal_key(key: KeyEvent, state: &AppState) -> Action {
    // Playlist-specific keybindings when in playlist view
    if state.active_view == ViewId::Playlists {
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => return Action::PlaylistPaneLeft,
            KeyCode::Char('l') | KeyCode::Right => return Action::PlaylistPaneRight,
            KeyCode::Char('n') => return Action::PlaylistCreate,
            KeyCode::Char('d') => return Action::PlaylistDelete,
            KeyCode::Char('r') => return Action::PlaylistRename,
            KeyCode::Char('x') => return Action::PlaylistRemoveTrack,
            _ => {} // fall through to global bindings
        }
    }

    // EQ-specific keybindings when in equalizer view
    if state.active_view == ViewId::Equalizer {
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => return Action::EqBandLeft,
            KeyCode::Char('l') | KeyCode::Right => return Action::EqBandRight,
            KeyCode::Char('k') | KeyCode::Up => return Action::EqGainUp,
            KeyCode::Char('j') | KeyCode::Down => return Action::EqGainDown,
            KeyCode::Char('b') => return Action::EqToggleBypass,
            KeyCode::Char('p') => return Action::EqNextPreset,
            KeyCode::Char('0') => return Action::EqReset,
            _ => {} // fall through to global bindings
        }
    }

    match key.code {
        // Quit
        KeyCode::Char('q') => Action::Quit,

        // View switching
        KeyCode::Char('1') => Action::SwitchView(ViewId::Library),
        KeyCode::Char('2') => Action::SwitchView(ViewId::NowPlaying),
        KeyCode::Char('3') => Action::SwitchView(ViewId::Queue),
        KeyCode::Char('4') => Action::SwitchView(ViewId::Equalizer),
        KeyCode::Char('5') | KeyCode::Char('/') => Action::OpenSearch,
        KeyCode::Char('6') => Action::SwitchView(ViewId::Playlists),

        // Navigation (vim)
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('h') | KeyCode::Left => Action::Left,
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
            // In library view, l/Enter drills into the next column or plays
            if state.active_view == ViewId::Library && matches!(key.code, KeyCode::Char('l') | KeyCode::Right) {
                Action::Right
            } else if key.code == KeyCode::Enter {
                Action::Select
            } else {
                Action::Right
            }
        }
        KeyCode::Char('g') => Action::GoTop,
        KeyCode::Char('G') => Action::GoBottom,

        // Page navigation
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::PageUp,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::PageDown,

        // Playback
        KeyCode::Char(' ') => Action::PlayPause,
        KeyCode::Char('s') => Action::Stop,
        KeyCode::Char('n') => Action::NextTrack,
        KeyCode::Char('p') => Action::PrevTrack,
        KeyCode::Char('>') => Action::SeekForward,
        KeyCode::Char('<') => Action::SeekBackward,
        KeyCode::Char('+') | KeyCode::Char('=') => Action::VolumeUp,
        KeyCode::Char('-') => Action::VolumeDown,
        KeyCode::Char('m') => Action::ToggleMute,
        KeyCode::Char('z') => Action::ToggleShuffle,
        KeyCode::Char('r') => Action::CycleRepeat,

        // Queue operations
        KeyCode::Char('a') => Action::AddToQueue,
        KeyCode::Char('A') => Action::AddAlbumToQueue,
        KeyCode::Char('d') if state.active_view == ViewId::Queue => Action::RemoveFromQueue,
        KeyCode::Char('J') => Action::MoveDown,
        KeyCode::Char('K') => Action::MoveUp,

        // Add to playlist
        KeyCode::Char('P') => Action::AddToPlaylist,

        // Library refresh
        KeyCode::Char('R') => Action::Refresh,

        _ => Action::None,
    }
}
