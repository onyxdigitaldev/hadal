//! Application state — single struct that owns all mutable UI state.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use hadal_common::{AudioFormat, PlayStatus, RepeatMode};
use hadal_dsp::VisualizationData;
use hadal_library::models::{AlbumRow, ArtistRow, TrackRow};
use hadal_playlist::PlayQueue;
use parking_lot::RwLock;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

// ─────────────────────────────────────────────────────────────────────────────
// View Identifiers
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewId {
    Library,
    NowPlaying,
    Queue,
    Equalizer,
    Search,
    Playlists,
}

impl ViewId {
    pub fn index(self) -> usize {
        match self {
            Self::Library => 0,
            Self::NowPlaying => 1,
            Self::Queue => 2,
            Self::Equalizer => 3,
            Self::Search => 4,
            Self::Playlists => 5,
        }
    }

    pub fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::Library),
            1 => Some(Self::NowPlaying),
            2 => Some(Self::Queue),
            3 => Some(Self::Equalizer),
            4 => Some(Self::Search),
            5 => Some(Self::Playlists),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::NowPlaying => "Playing",
            Self::Queue => "Queue",
            Self::Equalizer => "EQ",
            Self::Search => "Search",
            Self::Playlists => "Plists",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Input Modes
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    PlaylistName,
}

// ─────────────────────────────────────────────────────────────────────────────
// Column state for ranger-style navigation
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ColumnState {
    pub selected: usize,
    pub offset: usize,
}

impl ColumnState {
    pub fn select(&mut self, index: usize, total: usize) {
        if total == 0 {
            self.selected = 0;
            self.offset = 0;
            return;
        }
        self.selected = index.min(total.saturating_sub(1));
    }

    pub fn up(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn down(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        self.selected = (self.selected + 1).min(total.saturating_sub(1));
    }

    pub fn page_up(&mut self, page_size: usize) {
        self.selected = self.selected.saturating_sub(page_size);
    }

    pub fn page_down(&mut self, total: usize, page_size: usize) {
        if total == 0 {
            return;
        }
        self.selected = (self.selected + page_size).min(total.saturating_sub(1));
    }

    pub fn go_top(&mut self) {
        self.selected = 0;
    }

    pub fn go_bottom(&mut self, total: usize) {
        if total > 0 {
            self.selected = total - 1;
        }
    }

    /// Adjust scroll offset so selected is visible within `height` rows.
    pub fn scroll_into_view(&mut self, height: usize) {
        if height == 0 {
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected - height + 1;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Library State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct LibraryState {
    /// Which column (0=artists, 1=albums, 2=tracks) is focused
    pub depth: usize,
    pub columns: [ColumnState; 3],
    pub artists: Vec<ArtistRow>,
    pub albums: Vec<AlbumRow>,
    pub tracks: Vec<TrackRow>,
    pub filter_artist_id: Option<i64>,
    pub filter_album_id: Option<i64>,
}


impl LibraryState {
    pub fn selected_artist(&self) -> Option<&ArtistRow> {
        self.artists.get(self.columns[0].selected)
    }

    pub fn selected_album(&self) -> Option<&AlbumRow> {
        self.albums.get(self.columns[1].selected)
    }

    pub fn selected_track(&self) -> Option<&TrackRow> {
        self.tracks.get(self.columns[2].selected)
    }

    pub fn column_len(&self, depth: usize) -> usize {
        match depth {
            0 => self.artists.len(),
            1 => self.albums.len(),
            2 => self.tracks.len(),
            _ => 0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Playback State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub status: PlayStatus,
    pub current_track: Option<TrackRow>,
    pub source_format: Option<AudioFormat>,
    pub output_format: Option<AudioFormat>,
    pub position: Duration,
    pub duration: Duration,
    pub volume: f32,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// Resolved artist name for the current track
    pub artist_name: Option<String>,
    /// Resolved album title for the current track
    pub album_title: Option<String>,
    /// Path to cached artwork PNG for the current track's album
    pub artwork_path: Option<PathBuf>,
    /// Loaded artwork image (cached to avoid disk I/O during rendering)
    pub artwork_image: Option<(PathBuf, image::DynamicImage)>,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            status: PlayStatus::Stopped,
            current_track: None,
            source_format: None,
            output_format: None,
            position: Duration::ZERO,
            duration: Duration::ZERO,
            volume: 0.85,
            muted: false,
            shuffle: false,
            repeat: RepeatMode::Off,
            artist_name: None,
            album_title: None,
            artwork_path: None,
            artwork_image: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Queue View State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct QueueViewState {
    pub column: ColumnState,
}

// ─────────────────────────────────────────────────────────────────────────────
// Search State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub query: String,
    pub cursor: usize,
    pub results: Vec<TrackRow>,
    pub column: ColumnState,
    pub active: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Playlist View State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct PlaylistViewState {
    /// Selection in playlist list (left pane)
    pub list_column: ColumnState,
    /// Selection in track list (right pane)
    pub track_column: ColumnState,
    /// 0 = playlist list focused, 1 = track list focused
    pub depth: usize,
    /// Loaded playlists
    pub playlists: Vec<hadal_playlist::Playlist>,
    /// Tracks in the selected playlist (resolved from library DB)
    pub tracks: Vec<TrackRow>,
    /// Whether we're in name-input mode (creating or renaming)
    pub creating: bool,
    /// Name input buffer
    pub name_buffer: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// EQ View State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EqViewState {
    pub selected_band: usize,
    pub gains: [f64; 10],
    pub bypassed: bool,
    pub preset_name: String,
}

impl Default for EqViewState {
    fn default() -> Self {
        Self {
            selected_band: 0,
            gains: [0.0; 10],
            bypassed: false,
            preset_name: "Flat".to_string(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Top-level App State
// ─────────────────────────────────────────────────────────────────────────────

pub struct AppState {
    pub active_view: ViewId,
    pub input_mode: InputMode,
    pub library: LibraryState,
    pub playback: PlaybackState,
    pub play_queue: PlayQueue,
    pub queue_view: QueueViewState,
    pub search: SearchState,
    pub eq: EqViewState,
    pub playlist_view: PlaylistViewState,
    pub playlist_manager: Option<hadal_playlist::PlaylistManager>,
    pub status_message: Option<(String, Instant)>,
    pub terminal_size: (u16, u16),
    pub running: bool,
    /// Path to artwork cache directory
    pub artwork_cache_dir: PathBuf,
    /// Image picker for terminal protocol detection
    pub image_picker: Option<Picker>,
    /// Current artwork protocol state for sidebar rendering
    pub artwork_protocol: Option<Box<dyn StatefulProtocol>>,
    /// Current artwork protocol state for now-playing rendering (larger)
    pub artwork_protocol_large: Option<Box<dyn StatefulProtocol>>,
    /// Shared visualization data from the audio pipeline
    pub visualization_data: Option<Arc<RwLock<VisualizationData>>>,
}

impl AppState {
    pub fn new(artwork_cache_dir: PathBuf) -> Self {
        // Probe terminal for image protocol support
        let image_picker = Picker::from_termios().ok().map(|mut p| {
            p.guess_protocol();
            p
        });

        Self {
            active_view: ViewId::Library,
            input_mode: InputMode::Normal,
            library: LibraryState::default(),
            playback: PlaybackState::default(),
            play_queue: PlayQueue::new(),
            queue_view: QueueViewState::default(),
            search: SearchState::default(),
            eq: EqViewState::default(),
            playlist_view: PlaylistViewState::default(),
            playlist_manager: None,
            status_message: None,
            terminal_size: (80, 24),
            running: true,
            artwork_cache_dir,
            image_picker,
            artwork_protocol: None,
            artwork_protocol_large: None,
            visualization_data: None,
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    pub fn clear_expired_status(&mut self) {
        if let Some((_, time)) = &self.status_message {
            if time.elapsed() > Duration::from_secs(5) {
                self.status_message = None;
            }
        }
    }

    /// Clear all Kitty image placements and recreate protocols for a fresh render.
    ///
    /// Kitty's virtual placement mode (U=1) auto-removes images when their
    /// Unicode placeholders are overwritten — but only within the same screen
    /// area. When switching views, the old image's cells may not be redrawn,
    /// leaving a stale overlay. We fix this by sending the Kitty delete-all
    /// escape sequence directly.
    pub fn force_artwork_reload(&mut self) {
        // Send Kitty graphics protocol "delete all images" command.
        // \x1b_G is the Kitty graphics APC intro, a=d means action=delete,
        // d=A means delete all placements on all layers.
        use std::io::Write;
        let _ = std::io::stdout().write_all(b"\x1b_Ga=d,d=A\x1b\\");
        let _ = std::io::stdout().flush();

        self.artwork_protocol = None;
        self.artwork_protocol_large = None;

        // Recreate protocols from the cached image if available
        if let Some((_, ref img)) = self.playback.artwork_image {
            if let Some(ref mut picker) = self.image_picker {
                self.artwork_protocol = Some(picker.new_resize_protocol(img.clone()));
                self.artwork_protocol_large = Some(picker.new_resize_protocol(img.clone()));
            }
        }
    }
}
