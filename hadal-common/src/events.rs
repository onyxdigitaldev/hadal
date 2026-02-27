//! Cross-crate event definitions for Hadal.
//!
//! This module defines the events that flow between different components
//! of the application via channels.

use crate::types::{AudioFormat, EntityId, PlayStatus, RepeatMode, Track};
use std::path::PathBuf;
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// UI → Audio Events (Commands)
// ─────────────────────────────────────────────────────────────────────────────

/// Commands sent from the UI to the audio engine.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    /// Start playing the specified track
    Play(Box<Track>),

    /// Pause playback
    Pause,

    /// Resume playback
    Resume,

    /// Stop playback and clear current track
    Stop,

    /// Toggle play/pause
    PlayPause,

    /// Seek to a specific position
    Seek(Duration),

    /// Seek forward by the specified amount
    SeekForward(Duration),

    /// Seek backward by the specified amount
    SeekBackward(Duration),

    /// Set volume (0.0 - 1.0)
    SetVolume(f32),

    /// Adjust volume by delta (-1.0 to 1.0)
    AdjustVolume(f32),

    /// Mute/unmute
    ToggleMute,

    /// Set mute state explicitly
    SetMute(bool),

    /// Request current playback state
    GetState,

    /// Shutdown the audio engine
    Shutdown,
}

// ─────────────────────────────────────────────────────────────────────────────
// Audio → UI Events (Status Updates)
// ─────────────────────────────────────────────────────────────────────────────

/// Status updates sent from the audio engine to the UI.
#[derive(Debug, Clone)]
pub enum AudioEvent {
    /// Playback status changed
    StatusChanged(PlayStatus),

    /// Playback position updated
    Position {
        current: Duration,
        total: Duration,
    },

    /// Now playing a new track
    NowPlaying {
        track: Box<Track>,
        source_format: AudioFormat,
        output_format: AudioFormat,
    },

    /// Current track ended
    TrackEnded,

    /// Volume changed
    VolumeChanged {
        volume: f32,
        muted: bool,
    },

    /// Error occurred during playback
    Error(String),

    /// Audio engine is ready
    Ready,

    /// Audio engine is shutting down
    ShuttingDown,
}

// ─────────────────────────────────────────────────────────────────────────────
// Library Events
// ─────────────────────────────────────────────────────────────────────────────

/// Events related to library scanning and updates.
#[derive(Debug, Clone)]
pub enum LibraryEvent {
    /// Library scan started
    ScanStarted {
        folders: Vec<PathBuf>,
    },

    /// Progress update during scan
    ScanProgress {
        scanned: u32,
        total: u32,
        current_file: PathBuf,
    },

    /// Library scan completed
    ScanCompleted {
        tracks_added: u32,
        tracks_updated: u32,
        tracks_removed: u32,
        duration: Duration,
    },

    /// Library scan encountered an error
    ScanError {
        path: PathBuf,
        error: String,
    },

    /// A track was added to the library
    TrackAdded(EntityId),

    /// A track was updated in the library
    TrackUpdated(EntityId),

    /// A track was removed from the library
    TrackRemoved(EntityId),

    /// Library folder added
    FolderAdded(PathBuf),

    /// Library folder removed
    FolderRemoved(PathBuf),

    /// File system change detected
    FileChanged(PathBuf),
}

// ─────────────────────────────────────────────────────────────────────────────
// Queue Events
// ─────────────────────────────────────────────────────────────────────────────

/// Events related to the play queue.
#[derive(Debug, Clone)]
pub enum QueueEvent {
    /// Queue contents changed
    Updated {
        length: usize,
        total_duration: Duration,
    },

    /// Current queue position changed
    PositionChanged {
        index: usize,
        track_id: EntityId,
    },

    /// Track added to queue
    TrackAdded {
        index: usize,
        track_id: EntityId,
    },

    /// Track removed from queue
    TrackRemoved {
        index: usize,
        track_id: EntityId,
    },

    /// Queue was cleared
    Cleared,

    /// Shuffle mode changed
    ShuffleChanged(bool),

    /// Repeat mode changed
    RepeatChanged(RepeatMode),
}

// ─────────────────────────────────────────────────────────────────────────────
// Playlist Events
// ─────────────────────────────────────────────────────────────────────────────

/// Events related to playlists.
#[derive(Debug, Clone)]
pub enum PlaylistEvent {
    /// Playlist created
    Created {
        id: EntityId,
        name: String,
    },

    /// Playlist renamed
    Renamed {
        id: EntityId,
        old_name: String,
        new_name: String,
    },

    /// Playlist deleted
    Deleted {
        id: EntityId,
        name: String,
    },

    /// Tracks added to playlist
    TracksAdded {
        playlist_id: EntityId,
        track_ids: Vec<EntityId>,
    },

    /// Tracks removed from playlist
    TracksRemoved {
        playlist_id: EntityId,
        track_ids: Vec<EntityId>,
    },

    /// Playlist order changed
    Reordered {
        playlist_id: EntityId,
    },

    /// Playlist imported from file
    Imported {
        id: EntityId,
        name: String,
        path: PathBuf,
        track_count: u32,
    },

    /// Playlist exported to file
    Exported {
        id: EntityId,
        path: PathBuf,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// UI Events
// ─────────────────────────────────────────────────────────────────────────────

/// High-level UI action events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    // Navigation
    NavigateUp,
    NavigateDown,
    NavigateLeft,
    NavigateRight,
    PageUp,
    PageDown,
    GoToTop,
    GoToBottom,
    Select,
    Back,

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

    // Views
    OpenSearch,
    CloseSearch,
    OpenQueue,
    OpenPlaylists,
    OpenSettings,
    OpenHelp,
    SwitchTab(usize),
    NextTab,
    PrevTab,

    // Actions
    AddToQueue,
    AddToPlaylist,
    RemoveFromQueue,
    RemoveFromPlaylist,
    ToggleShuffle,
    CycleRepeat,
    RefreshLibrary,
    ChangeSortField,
    ToggleSortOrder,

    // Application
    Quit,
    ForceQuit,
    Resize(u16, u16),
}

// ─────────────────────────────────────────────────────────────────────────────
// Unified App Event
// ─────────────────────────────────────────────────────────────────────────────

/// Unified event type for the main application event loop.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Audio engine event
    Audio(AudioEvent),

    /// Library event
    Library(LibraryEvent),

    /// Queue event
    Queue(QueueEvent),

    /// Playlist event
    Playlist(PlaylistEvent),

    /// UI action
    Ui(UiAction),

    /// Tick event for periodic updates
    Tick,

    /// Terminal resize
    Resize { width: u16, height: u16 },

    /// Request to quit the application
    Quit,
}

impl From<AudioEvent> for AppEvent {
    fn from(event: AudioEvent) -> Self {
        Self::Audio(event)
    }
}

impl From<LibraryEvent> for AppEvent {
    fn from(event: LibraryEvent) -> Self {
        Self::Library(event)
    }
}

impl From<QueueEvent> for AppEvent {
    fn from(event: QueueEvent) -> Self {
        Self::Queue(event)
    }
}

impl From<PlaylistEvent> for AppEvent {
    fn from(event: PlaylistEvent) -> Self {
        Self::Playlist(event)
    }
}

impl From<UiAction> for AppEvent {
    fn from(action: UiAction) -> Self {
        Self::Ui(action)
    }
}
