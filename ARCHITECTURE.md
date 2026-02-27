# Hadal Architecture Document

> **Hadal** — An audiophile-grade Rust TUI music player for Linux
>
> Named after the hadal zone, the deepest oceanic trenches — where only the purest signals reach.

---

## Table of Contents

1. [Goals & Constraints](#1-goals--constraints)
2. [Crate & Module Structure](#2-crate--module-structure)
3. [Threading & Async Model](#3-threading--async-model)
4. [Audio Pipeline](#4-audio-pipeline)
5. [Library Database Schema](#5-library-database-schema)
6. [TUI Layout & State Model](#6-tui-layout--state-model)
7. [Album Art Capability Abstraction](#7-album-art-capability-abstraction)
8. [Configuration System](#8-configuration-system)
9. [Milestones](#9-milestones)
10. [Dependency Justification](#10-dependency-justification)

---

## 1. Goals & Constraints

### 1.1 Product Goals

| Category | Requirement |
|----------|-------------|
| **Playback** | Glitch-free, low-latency local file playback via PipeWire |
| **Codecs** | FLAC, MP3, AAC/M4A, Ogg/Vorbis, Opus, WAV, AIFF, ALAC |
| **Audiophile Mode** | Bit-perfect passthrough, preserve sample rate/bit depth, zero processing unless enabled |
| **Library** | Indexed, searchable, sortable by artist/album/title/genre/date/track |
| **Playlists** | M3U8 + SQLite persistence, full CRUD, drag-reorder |
| **TUI** | ratatui-based, vim-like keybindings, fast incremental search |
| **Album Art** | Kitty graphics protocol (primary), fallback abstraction layer |
| **Organization** | User-configurable sorting/grouping on the fly |

### 1.2 Engineering Constraints

| Constraint | Rationale |
|------------|-----------|
| Rust stable | Broad compatibility, no nightly footguns |
| Cross-distro Linux | Target Ubuntu/Fedora/Arch at minimum |
| PipeWire output | Modern Linux audio standard |
| No GUI | Terminal-first, kitty primary target |
| No streaming | MVP is local files only |
| Mature dependencies | Minimize maintenance burden |

### 1.3 Scale Targets

| Library Size | Classification | Strategy |
|--------------|----------------|----------|
| ~1k tracks | Small | Full in-memory index viable |
| ~20k tracks | Medium | SQLite FTS, lazy thumbnail loading |
| ~200k+ tracks | Large/Power | Paginated queries, background indexing, memory-mapped where beneficial |

---

## 2. Crate & Module Structure

### 2.1 Workspace Layout

```
hadal/
├── Cargo.toml                 # Workspace root
├── hadal/                     # Main binary crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs            # Entry point, CLI parsing
│       ├── app.rs             # Application state machine
│       ├── config.rs          # Configuration loading
│       └── logging.rs         # Tracing setup
│
├── hadal-audio/               # Audio engine crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── decoder.rs         # Symphonia-based decoding
│       ├── pipeline.rs        # Audio pipeline orchestration
│       ├── pipewire.rs        # PipeWire client implementation
│       ├── resampler.rs       # Optional high-quality resampling
│       └── format.rs          # Format detection, metadata
│
├── hadal-library/             # Library management crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── scanner.rs         # Directory scanner
│       ├── indexer.rs         # Tag extraction, DB insertion
│       ├── db.rs              # SQLite operations
│       ├── schema.rs          # Database schema migrations
│       ├── search.rs          # FTS queries
│       ├── models.rs          # Track, Album, Artist structs
│       └── artwork.rs         # Artwork extraction & caching
│
├── hadal-playlist/            # Playlist management crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── m3u8.rs            # M3U8 import/export
│       ├── internal.rs        # SQLite-backed playlists
│       └── queue.rs           # Play queue management
│
├── hadal-tui/                 # Terminal UI crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── app.rs             # TUI application loop
│       ├── state.rs           # View state management
│       ├── input.rs           # Keybinding handling
│       ├── views/
│       │   ├── mod.rs
│       │   ├── now_playing.rs # Now playing panel
│       │   ├── library.rs     # Library browser
│       │   ├── queue.rs       # Queue view
│       │   ├── search.rs      # Search overlay
│       │   ├── playlists.rs   # Playlist management
│       │   └── settings.rs    # Settings view
│       ├── widgets/
│       │   ├── mod.rs
│       │   ├── track_list.rs  # Scrollable track list
│       │   ├── album_grid.rs  # Album grid view
│       │   ├── progress.rs    # Playback progress bar
│       │   ├── spectrum.rs    # Optional spectrum analyzer
│       │   └── artwork.rs     # Album art widget
│       └── theme.rs           # Color theming
│
├── hadal-graphics/            # Terminal graphics abstraction
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── capability.rs      # Terminal capability detection
│       ├── kitty.rs           # Kitty graphics protocol
│       ├── sixel.rs           # Sixel fallback (optional)
│       ├── block.rs           # Unicode block fallback
│       └── cache.rs           # Image cache management
│
└── hadal-common/              # Shared types and utilities
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── types.rs           # Common type definitions
        ├── error.rs           # Error types
        ├── events.rs          # Cross-crate event definitions
        └── paths.rs           # XDG path utilities
```

### 2.2 Crate Dependency Graph

```
                    ┌─────────────────┐
                    │      hadal      │  (binary)
                    │   main entry    │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐   ┌────────────────┐   ┌───────────────┐
│  hadal-tui    │   │  hadal-audio   │   │hadal-library  │
│   (ratatui)   │   │  (symphonia)   │   │   (rusqlite)  │
└───────┬───────┘   └────────────────┘   └───────┬───────┘
        │                                        │
        ▼                                        │
┌───────────────┐                               │
│hadal-graphics │                               │
│(kitty/sixel)  │                               │
└───────────────┘                               │
        │                                        │
        └──────────────┬─────────────────────────┘
                       ▼
              ┌────────────────┐
              │ hadal-playlist │
              │    (m3u8)      │
              └────────────────┘
                       │
                       ▼
              ┌────────────────┐
              │  hadal-common  │
              │ (shared types) │
              └────────────────┘
```

---

## 3. Threading & Async Model

### 3.1 Overview

Hadal uses a **hybrid model**:
- **Async (tokio)** for I/O-bound operations (DB queries, file scanning, network)
- **Dedicated threads** for real-time audio (no async in the hot path)

### 3.2 Thread Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         MAIN THREAD                              │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    TUI Event Loop                        │    │
│  │  • ratatui rendering (16-60 FPS configurable)           │    │
│  │  • crossterm input handling                              │    │
│  │  • State machine transitions                             │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ mpsc channels
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      AUDIO THREAD (dedicated)                    │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                 Real-time Audio Pipeline                 │    │
│  │  • Ring buffer consumer                                  │    │
│  │  • PipeWire client callback                              │    │
│  │  • MUST NOT BLOCK (no allocations, no locks)            │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ lock-free ring buffer
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     DECODER THREAD (dedicated)                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                  Symphonia Decoding                      │    │
│  │  • Reads from disk (may block)                          │    │
│  │  • Decodes to PCM                                        │    │
│  │  • Pushes to ring buffer                                 │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    TOKIO RUNTIME (async)                         │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  • Library scanning tasks                                │    │
│  │  • Database queries                                      │    │
│  │  • Thumbnail generation                                  │    │
│  │  • File watching (notify)                                │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### 3.3 Communication Channels

| Channel | Type | Purpose |
|---------|------|---------|
| `ui_tx/ui_rx` | `mpsc::channel<UiEvent>` | Commands from TUI to audio engine |
| `audio_tx/audio_rx` | `mpsc::channel<AudioEvent>` | Status updates from audio to TUI |
| `lib_tx/lib_rx` | `mpsc::channel<LibraryEvent>` | Library scan progress/results |
| `audio_buffer` | `ringbuf::HeapRb` | Lock-free PCM transfer decoder→audio |

### 3.4 Real-time Audio Guarantees

The audio thread follows strict real-time rules:

```rust
// FORBIDDEN in audio thread:
// - std::sync::Mutex (use parking_lot or atomics)
// - Vec::push (may allocate)
// - Box::new (allocation)
// - File I/O
// - Any syscall that may block

// ALLOWED:
// - Atomic operations
// - Lock-free ring buffer reads
// - Pre-allocated buffer writes
// - PipeWire buffer callbacks
```

---

## 4. Audio Pipeline

### 4.1 Pipeline Stages

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   File I/O   │───▶│   Decoder    │───▶│  Resampler   │───▶│   Output     │
│              │    │  (Symphonia) │    │  (optional)  │    │  (PipeWire)  │
└──────────────┘    └──────────────┘    └──────────────┘    └──────────────┘
       │                   │                   │                   │
       ▼                   ▼                   ▼                   ▼
   MediaSource         AudioBuffer         AudioBuffer         PWStream
   (seekable)          (decoded PCM)       (resampled)         callback
```

### 4.2 Format Flow

```rust
pub struct AudioFormat {
    pub sample_rate: u32,      // e.g., 44100, 48000, 96000, 192000
    pub channels: u8,          // 1 = mono, 2 = stereo
    pub bit_depth: BitDepth,   // U8, S16, S24, S32, F32
    pub codec: Codec,          // FLAC, MP3, AAC, etc.
}

pub enum BitDepth {
    U8,
    S16,
    S24,  // Packed or padded to S32
    S32,
    F32,  // Internal processing format
}
```

### 4.3 Audiophile Mode

When enabled, audiophile mode:

1. **Bit-perfect passthrough**: No sample rate conversion if device supports source rate
2. **No dithering**: Unless bit depth reduction required
3. **No ReplayGain**: Unless explicitly enabled
4. **Format display**: Shows source format vs output format in UI

```rust
pub struct AudiophileConfig {
    pub passthrough: bool,           // Attempt bit-perfect
    pub allow_resampling: bool,      // If device doesn't support rate
    pub resampler_quality: Quality,  // SincBest, SincMedium, Linear
    pub dither: DitherMode,          // None, TPDF, Shaped
    pub replay_gain: ReplayGainMode, // Off, Track, Album
}
```

### 4.4 PipeWire Integration

```rust
// Using pipewire-rs crate
pub struct PipeWireOutput {
    core: pw::Core,
    stream: pw::stream::Stream,
    format: AudioFormat,
    buffer_size: usize,  // Configurable, default 1024 frames
}

impl PipeWireOutput {
    pub fn new(format: AudioFormat) -> Result<Self> {
        let main_loop = pw::MainLoop::new()?;
        let context = pw::Context::new(&main_loop)?;
        let core = context.connect(None)?;

        let stream = pw::stream::Stream::new(
            &core,
            "hadal",
            properties! {
                *pw::keys::MEDIA_TYPE => "Audio",
                *pw::keys::MEDIA_CATEGORY => "Playback",
                *pw::keys::MEDIA_ROLE => "Music",
            },
        )?;

        // Configure for bit-perfect if possible
        // ...
    }
}
```

### 4.5 Gapless Playback

```rust
pub struct GaplessQueue {
    current: Option<DecoderHandle>,
    next: Option<DecoderHandle>,  // Pre-decoded next track
    crossfade_samples: usize,     // 0 = true gapless
}

// Pre-buffer next track when current reaches 80% position
// Seamless switch in audio callback
```

---

## 5. Library Database Schema

### 5.1 Technology Choice: rusqlite

**Why rusqlite over sqlx:**
- Synchronous API better fits our hybrid async model
- Simpler compile-time setup (no proc macros for queries)
- Lower compile times
- WAL mode for concurrent reads during playback
- FTS5 for full-text search built-in

### 5.2 Schema

```sql
-- Core tables
CREATE TABLE IF NOT EXISTS artists (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL COLLATE NOCASE,
    sort_name   TEXT COLLATE NOCASE,  -- "Beatles, The" for sorting
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(name)
);

CREATE TABLE IF NOT EXISTS albums (
    id              INTEGER PRIMARY KEY,
    title           TEXT NOT NULL COLLATE NOCASE,
    sort_title      TEXT COLLATE NOCASE,
    artist_id       INTEGER REFERENCES artists(id) ON DELETE SET NULL,
    album_artist    TEXT COLLATE NOCASE,  -- May differ from track artist
    year            INTEGER,
    genre           TEXT,
    disc_total      INTEGER DEFAULT 1,
    track_total     INTEGER,
    artwork_path    TEXT,  -- Cached thumbnail path
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

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
    bitrate         INTEGER,  -- For lossy formats
    file_size       INTEGER,
    -- Metadata
    play_count      INTEGER DEFAULT 0,
    last_played     INTEGER,
    rating          INTEGER CHECK(rating BETWEEN 0 AND 5),
    -- File tracking
    file_mtime      INTEGER NOT NULL,  -- For change detection
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS genres (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE IF NOT EXISTS track_genres (
    track_id    INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    genre_id    INTEGER REFERENCES genres(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, genre_id)
);

-- Playlist tables
CREATE TABLE IF NOT EXISTS playlists (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    is_smart    INTEGER DEFAULT 0,  -- Smart playlist (query-based)
    query       TEXT,               -- For smart playlists
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    id          INTEGER PRIMARY KEY,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    added_at    INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(playlist_id, position)
);

-- Play queue (persisted for resume)
CREATE TABLE IF NOT EXISTS queue (
    id          INTEGER PRIMARY KEY,
    track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    is_current  INTEGER DEFAULT 0
);

-- Scan folders
CREATE TABLE IF NOT EXISTS library_folders (
    id      INTEGER PRIMARY KEY,
    path    TEXT NOT NULL UNIQUE,
    enabled INTEGER DEFAULT 1
);

-- Full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
    title,
    artist_name,
    album_title,
    genre,
    content='tracks',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);

-- Triggers for FTS sync
CREATE TRIGGER IF NOT EXISTS tracks_ai AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_fts(rowid, title, artist_name, album_title, genre)
    SELECT NEW.id, NEW.title,
           (SELECT name FROM artists WHERE id = NEW.artist_id),
           (SELECT title FROM albums WHERE id = NEW.album_id),
           NEW.genre;
END;

CREATE TRIGGER IF NOT EXISTS tracks_ad AFTER DELETE ON tracks BEGIN
    INSERT INTO tracks_fts(tracks_fts, rowid, title, artist_name, album_title, genre)
    VALUES('delete', OLD.id, OLD.title,
           (SELECT name FROM artists WHERE id = OLD.artist_id),
           (SELECT title FROM albums WHERE id = OLD.album_id),
           OLD.genre);
END;

CREATE TRIGGER IF NOT EXISTS tracks_au AFTER UPDATE ON tracks BEGIN
    INSERT INTO tracks_fts(tracks_fts, rowid, title, artist_name, album_title, genre)
    VALUES('delete', OLD.id, OLD.title,
           (SELECT name FROM artists WHERE id = OLD.artist_id),
           (SELECT title FROM albums WHERE id = OLD.album_id),
           OLD.genre);
    INSERT INTO tracks_fts(rowid, title, artist_name, album_title, genre)
    SELECT NEW.id, NEW.title,
           (SELECT name FROM artists WHERE id = NEW.artist_id),
           (SELECT title FROM albums WHERE id = NEW.album_id),
           NEW.genre;
END;

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist_id);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_id);
CREATE INDEX IF NOT EXISTS idx_tracks_genre ON tracks(genre);
CREATE INDEX IF NOT EXISTS idx_tracks_year ON tracks(year);
CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(path);
CREATE INDEX IF NOT EXISTS idx_albums_artist ON albums(artist_id);
CREATE INDEX IF NOT EXISTS idx_albums_year ON albums(year);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist ON playlist_tracks(playlist_id, position);
```

### 5.3 Query Patterns

```rust
// User-configurable sorting
pub enum SortField {
    Artist,
    Album,
    Title,
    Year,
    Genre,
    DateAdded,
    Duration,
    PlayCount,
    Rating,
}

pub enum SortOrder {
    Ascending,
    Descending,
}

pub struct LibraryQuery {
    pub sort_primary: (SortField, SortOrder),
    pub sort_secondary: Option<(SortField, SortOrder)>,
    pub filter_genre: Option<String>,
    pub filter_artist: Option<i64>,
    pub filter_album: Option<i64>,
    pub filter_year_range: Option<(u16, u16)>,
    pub search_term: Option<String>,
    pub limit: usize,
    pub offset: usize,
}
```

---

## 6. TUI Layout & State Model

### 6.1 Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ HADAL                                               ♫ FLAC 96kHz/24bit  │
├────────────────────────┬────────────────────────────────────────────────────┤
│                        │                                                    │
│   ┌──────────────┐     │  Library Browser                                   │
│   │              │     │  ─────────────────────────────────────────────    │
│   │  Album Art   │     │  [Artists] [Albums] [Tracks] [Genres] [Playlists] │
│   │   (kitty)    │     │                                                    │
│   │              │     │  > Pink Floyd                                      │
│   └──────────────┘     │    The Dark Side of the Moon                       │
│                        │    Wish You Were Here                              │
│   Now Playing          │    Animals                                         │
│   ─────────────        │    The Wall                                        │
│   Comfortably Numb     │    ▶ Meddle                                        │
│   Pink Floyd           │      One of These Days                             │
│   The Wall             │      A Pillow of Winds                             │
│                        │    ▶ Echoes                                        │
│    advancement█░░░  5:23│      San Tropez                                    │
│    advancement────── 6:47│      Seamus                                        │
│                        │                                                    │
│    advancement◀◀  advancement▶  advancement▶▶  advancement🔀  advancement🔁│                                                    │
│                        │                                                    │
├────────────────────────┴────────────────────────────────────────────────────┤
│ [/] Search  [q] Queue  [p] Playlists  [a] Add  [?] Help       Vol: ████░ 80%│
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 View Hierarchy

```rust
pub enum View {
    Library(LibraryView),
    Queue,
    Playlists(PlaylistView),
    Search,
    Settings,
    Help,
}

pub enum LibraryView {
    Artists { selected: usize, scroll: usize },
    Albums { selected: usize, scroll: usize, filter_artist: Option<i64> },
    Tracks { selected: usize, scroll: usize, filter: TrackFilter },
    Genres { selected: usize, scroll: usize },
}

pub enum PlaylistView {
    List { selected: usize },
    Detail { playlist_id: i64, selected: usize },
    Edit { playlist_id: i64, selected: usize },
}
```

### 6.3 State Machine

```rust
pub struct AppState {
    // Navigation
    pub current_view: View,
    pub view_stack: Vec<View>,  // For back navigation

    // Playback
    pub playback: PlaybackState,
    pub queue: QueueState,

    // Library
    pub library: LibraryState,

    // UI state
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub status_message: Option<(String, Instant)>,

    // User preferences (runtime)
    pub sort_mode: SortConfig,
    pub view_mode: ViewMode,  // List, Grid, Compact
}

pub struct PlaybackState {
    pub status: PlayStatus,
    pub current_track: Option<Track>,
    pub position: Duration,
    pub duration: Duration,
    pub volume: f32,  // 0.0 - 1.0
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    pub audio_format: Option<AudioFormat>,
    pub output_format: Option<AudioFormat>,
}

pub enum PlayStatus {
    Stopped,
    Playing,
    Paused,
    Buffering,
}

pub enum RepeatMode {
    Off,
    All,
    One,
}
```

### 6.4 Keybindings

```rust
pub struct KeyBindings {
    // Navigation
    pub up: Vec<KeyCode>,           // [k, Up]
    pub down: Vec<KeyCode>,         // [j, Down]
    pub left: Vec<KeyCode>,         // [h, Left]
    pub right: Vec<KeyCode>,        // [l, Right]
    pub page_up: Vec<KeyCode>,      // [Ctrl+u, PageUp]
    pub page_down: Vec<KeyCode>,    // [Ctrl+d, PageDown]
    pub top: Vec<KeyCode>,          // [g, Home]
    pub bottom: Vec<KeyCode>,       // [G, End]
    pub select: Vec<KeyCode>,       // [Enter, Space]
    pub back: Vec<KeyCode>,         // [Esc, Backspace]

    // Playback
    pub play_pause: Vec<KeyCode>,   // [Space] (context-dependent)
    pub stop: Vec<KeyCode>,         // [s]
    pub next_track: Vec<KeyCode>,   // [n, >]
    pub prev_track: Vec<KeyCode>,   // [p, <]
    pub seek_forward: Vec<KeyCode>, // [., Shift+Right]
    pub seek_backward: Vec<KeyCode>,// [,, Shift+Left]
    pub volume_up: Vec<KeyCode>,    // [+, =]
    pub volume_down: Vec<KeyCode>,  // [-]
    pub mute: Vec<KeyCode>,         // [m]

    // Views
    pub search: Vec<KeyCode>,       // [/, Ctrl+f]
    pub queue: Vec<KeyCode>,        // [q]
    pub playlists: Vec<KeyCode>,    // [P]
    pub library: Vec<KeyCode>,      // [1-4] for tabs
    pub help: Vec<KeyCode>,         // [?, F1]
    pub settings: Vec<KeyCode>,     // [Ctrl+,]

    // Actions
    pub add_to_queue: Vec<KeyCode>, // [a]
    pub add_to_playlist: Vec<KeyCode>, // [A]
    pub remove: Vec<KeyCode>,       // [d, Delete]
    pub shuffle_toggle: Vec<KeyCode>, // [z]
    pub repeat_cycle: Vec<KeyCode>, // [r]

    // Application
    pub quit: Vec<KeyCode>,         // [q] (when not in queue view)
    pub refresh: Vec<KeyCode>,      // [Ctrl+r]
}
```

### 6.5 Event Loop

```rust
pub async fn run_tui(mut app: AppState, mut events: EventStream) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let tick_rate = Duration::from_millis(16);  // ~60 FPS
    let mut last_tick = Instant::now();

    loop {
        // Render
        terminal.draw(|f| ui::render(&app, f))?;

        // Handle events with timeout
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());

        if crossterm::event::poll(timeout)? {
            match crossterm::event::read()? {
                Event::Key(key) => {
                    if let Some(action) = app.keybindings.match_key(key) {
                        app.handle_action(action).await?;
                    }
                }
                Event::Resize(w, h) => {
                    app.handle_resize(w, h);
                }
                _ => {}
            }
        }

        // Process async events (audio, library, etc.)
        while let Ok(event) = app.event_rx.try_recv() {
            app.handle_event(event).await?;
        }

        // Tick updates
        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    restore_terminal()?;
    Ok(())
}
```

---

## 7. Album Art Capability Abstraction

### 7.1 Capability Detection

```rust
pub enum GraphicsCapability {
    Kitty(KittyProtocol),    // Full kitty graphics protocol
    Sixel(SixelConfig),       // Sixel graphics (xterm, foot, etc.)
    Iterm2,                   // iTerm2 inline images (macOS terminal)
    Blocks,                   // Unicode block characters fallback
    None,                     // No graphics support
}

pub struct KittyProtocol {
    pub version: u32,
    pub supports_animation: bool,
    pub supports_unicode_placeholders: bool,
    pub max_graphic_size: (u32, u32),
}

impl GraphicsCapability {
    pub fn detect() -> Self {
        // 1. Check TERM_PROGRAM for kitty
        if std::env::var("TERM_PROGRAM").map(|v| v == "kitty").unwrap_or(false) {
            return Self::detect_kitty();
        }

        // 2. Check KITTY_WINDOW_ID
        if std::env::var("KITTY_WINDOW_ID").is_ok() {
            return Self::detect_kitty();
        }

        // 3. Check terminal response for kitty graphics
        if Self::query_kitty_graphics() {
            return Self::detect_kitty();
        }

        // 4. Check for sixel support
        if Self::query_sixel_support() {
            return Self::Sixel(SixelConfig::detect());
        }

        // 5. Check for iTerm2
        if std::env::var("TERM_PROGRAM").map(|v| v == "iTerm.app").unwrap_or(false) {
            return Self::Iterm2;
        }

        // 6. Fallback to block characters
        Self::Blocks
    }

    fn detect_kitty() -> Self {
        Self::Kitty(KittyProtocol {
            version: Self::query_kitty_version(),
            supports_animation: true,
            supports_unicode_placeholders: true,
            max_graphic_size: (4096, 4096),
        })
    }
}
```

### 7.2 Image Renderer Trait

```rust
#[async_trait]
pub trait ImageRenderer: Send + Sync {
    /// Render image at specified cell position and size
    async fn render(
        &self,
        image: &DynamicImage,
        x: u16,
        y: u16,
        width_cells: u16,
        height_cells: u16,
    ) -> Result<RenderHandle>;

    /// Clear a previously rendered image
    fn clear(&self, handle: RenderHandle) -> Result<()>;

    /// Check if position/size changed requires re-render
    fn needs_rerender(&self, handle: &RenderHandle, new_bounds: Rect) -> bool;
}

pub struct RenderHandle {
    id: u32,
    bounds: Rect,
    image_hash: u64,  // For cache invalidation
}
```

### 7.3 Kitty Graphics Implementation

```rust
pub struct KittyRenderer {
    next_id: AtomicU32,
    cell_size: (u16, u16),  // Pixels per cell
}

impl KittyRenderer {
    pub fn new() -> Result<Self> {
        let cell_size = Self::query_cell_size()?;
        Ok(Self {
            next_id: AtomicU32::new(1),
            cell_size,
        })
    }

    fn query_cell_size() -> Result<(u16, u16)> {
        // Query terminal for cell pixel dimensions
        // Using CSI 14 t / CSI 16 t
        // Fallback to reasonable defaults (10x20 for typical fonts)
        Ok((10, 20))
    }
}

#[async_trait]
impl ImageRenderer for KittyRenderer {
    async fn render(
        &self,
        image: &DynamicImage,
        x: u16,
        y: u16,
        width_cells: u16,
        height_cells: u16,
    ) -> Result<RenderHandle> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        // Calculate pixel dimensions
        let width_px = width_cells as u32 * self.cell_size.0 as u32;
        let height_px = height_cells as u32 * self.cell_size.1 as u32;

        // Resize image
        let resized = image.resize_exact(
            width_px,
            height_px,
            image::imageops::FilterType::Lanczos3,
        );

        // Encode to PNG
        let mut png_data = Vec::new();
        resized.write_to(&mut Cursor::new(&mut png_data), ImageFormat::Png)?;

        // Base64 encode
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);

        // Send kitty graphics protocol escape sequence
        // a=T (transmit), f=100 (PNG), t=d (direct), i=<id>
        // Split into chunks of 4096 bytes
        let mut first = true;
        for chunk in b64.as_bytes().chunks(4096) {
            let m = if chunk.len() < 4096 { 0 } else { 1 };
            if first {
                print!(
                    "\x1b_Ga=T,f=100,t=d,i={},m={};{}\x1b\\",
                    id, m, std::str::from_utf8(chunk)?
                );
                first = false;
            } else {
                print!("\x1b_Gm={};{}\x1b\\", m, std::str::from_utf8(chunk)?);
            }
        }

        // Position the image using Unicode placeholders
        print!("\x1b[{};{}H", y + 1, x + 1);
        for row in 0..height_cells {
            for col in 0..width_cells {
                // Unicode placeholder with image ID
                print!("\x1b_Ga=p,i={},p={},q={}\x1b\\\u{10EEEE}", id, col, row);
            }
            if row < height_cells - 1 {
                print!("\x1b[1B\x1b[{}D", width_cells);
            }
        }

        std::io::stdout().flush()?;

        Ok(RenderHandle {
            id,
            bounds: Rect { x, y, width: width_cells, height: height_cells },
            image_hash: hash_image(image),
        })
    }

    fn clear(&self, handle: RenderHandle) -> Result<()> {
        // Delete image by ID
        print!("\x1b_Ga=d,d=i,i={}\x1b\\", handle.id);
        std::io::stdout().flush()?;
        Ok(())
    }

    fn needs_rerender(&self, handle: &RenderHandle, new_bounds: Rect) -> bool {
        handle.bounds != new_bounds
    }
}
```

### 7.4 Artwork Cache

```rust
pub struct ArtworkCache {
    cache_dir: PathBuf,           // ~/.cache/hadal/artwork/
    memory_cache: LruCache<u64, Arc<DynamicImage>>,
    max_memory_items: usize,
    thumbnail_size: (u32, u32),   // Default 300x300
}

impl ArtworkCache {
    pub async fn get_artwork(&mut self, track: &Track) -> Option<Arc<DynamicImage>> {
        let cache_key = self.compute_key(track);

        // Check memory cache
        if let Some(img) = self.memory_cache.get(&cache_key) {
            return Some(Arc::clone(img));
        }

        // Check disk cache
        let cache_path = self.cache_dir.join(format!("{:016x}.png", cache_key));
        if cache_path.exists() {
            if let Ok(img) = image::open(&cache_path) {
                let arc = Arc::new(img);
                self.memory_cache.put(cache_key, Arc::clone(&arc));
                return Some(arc);
            }
        }

        // Extract and cache
        if let Some(img) = self.extract_artwork(track).await {
            let thumbnail = img.resize(
                self.thumbnail_size.0,
                self.thumbnail_size.1,
                image::imageops::FilterType::Lanczos3,
            );

            // Save to disk cache
            let _ = thumbnail.save(&cache_path);

            let arc = Arc::new(thumbnail);
            self.memory_cache.put(cache_key, Arc::clone(&arc));
            return Some(arc);
        }

        None
    }

    async fn extract_artwork(&self, track: &Track) -> Option<DynamicImage> {
        // 1. Try embedded artwork via lofty
        if let Some(img) = self.extract_embedded(track).await {
            return Some(img);
        }

        // 2. Try common filenames in album directory
        let track_dir = Path::new(&track.path).parent()?;
        for name in ["cover.jpg", "cover.png", "folder.jpg", "folder.png",
                     "front.jpg", "front.png", "album.jpg", "album.png"] {
            let art_path = track_dir.join(name);
            if art_path.exists() {
                if let Ok(img) = image::open(&art_path) {
                    return Some(img);
                }
            }
        }

        None
    }
}
```

---

## 8. Configuration System

### 8.1 XDG Paths

```rust
pub struct Paths {
    pub config_dir: PathBuf,    // ~/.config/hadal/
    pub config_file: PathBuf,   // ~/.config/hadal/config.toml
    pub data_dir: PathBuf,      // ~/.local/share/hadal/
    pub database: PathBuf,      // ~/.local/share/hadal/library.db
    pub playlists_dir: PathBuf, // ~/.local/share/hadal/playlists/
    pub cache_dir: PathBuf,     // ~/.cache/hadal/
    pub artwork_cache: PathBuf, // ~/.cache/hadal/artwork/
    pub log_file: PathBuf,      // ~/.cache/hadal/hadal.log
}

impl Paths {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or(Error::NoConfigDir)?
            .join("hadal");
        let data_dir = dirs::data_dir()
            .ok_or(Error::NoDataDir)?
            .join("hadal");
        let cache_dir = dirs::cache_dir()
            .ok_or(Error::NoCacheDir)?
            .join("hadal");

        // Create directories if needed
        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(&cache_dir)?;

        Ok(Self {
            config_file: config_dir.join("config.toml"),
            config_dir,
            database: data_dir.join("library.db"),
            playlists_dir: data_dir.join("playlists"),
            data_dir,
            artwork_cache: cache_dir.join("artwork"),
            log_file: cache_dir.join("hadal.log"),
            cache_dir,
        })
    }
}
```

### 8.2 Configuration Schema

```toml
# ~/.config/hadal/config.toml

[library]
folders = [
    "~/Music",
    "/mnt/nas/music"
]
scan_on_startup = true
watch_for_changes = true

[playback]
# Audiophile mode settings
passthrough = true           # Attempt bit-perfect output
allow_resampling = true      # If device doesn't support source rate
resampler_quality = "sinc_best"  # sinc_best, sinc_medium, linear
gapless = true
replay_gain = "off"          # off, track, album
buffer_size = 1024           # Audio buffer in frames

[ui]
# Display
theme = "default"            # Theme name
show_album_art = true
album_art_position = "left"  # left, right, hidden
fps = 60                     # TUI refresh rate

# Default sorting
default_sort = "artist"
default_sort_order = "ascending"
secondary_sort = "album"

# Library view
default_library_view = "artists"  # artists, albums, tracks, genres

[keybindings]
# Override specific keys (vim-like defaults)
# up = ["k", "Up"]
# down = ["j", "Down"]
# ... see full defaults in source

[audio]
# PipeWire settings
output_device = "default"    # Or specific device name
exclusive_mode = false       # Request exclusive access
```

### 8.3 Config Loading

```rust
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub library: LibraryConfig,
    pub playback: PlaybackConfig,
    pub ui: UiConfig,
    pub keybindings: KeybindingsConfig,
    pub audio: AudioConfig,
}

impl Config {
    pub fn load(paths: &Paths) -> Result<Self> {
        if paths.config_file.exists() {
            let contents = std::fs::read_to_string(&paths.config_file)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            // Create default config file
            let config = Config::default();
            let contents = toml::to_string_pretty(&config)?;
            std::fs::write(&paths.config_file, contents)?;
            Ok(config)
        }
    }
}
```

---

## 9. Milestones

### Phase 1: MVP (Core Playback)

**Goal:** Play a single FLAC file through PipeWire with TUI controls.

| Task | Crate | Priority |
|------|-------|----------|
| Project scaffold, workspace setup | all | P0 |
| PipeWire output stream | hadal-audio | P0 |
| Symphonia FLAC decoder | hadal-audio | P0 |
| Basic ring buffer pipeline | hadal-audio | P0 |
| Minimal TUI (now playing, progress) | hadal-tui | P0 |
| Play/pause/stop/seek controls | hadal-tui | P0 |
| Volume control | hadal-audio | P0 |
| CLI file argument playback | hadal | P0 |

**Deliverable:** `hadal /path/to/song.flac` plays with basic controls.

---

### Phase 2: Library Foundation

**Goal:** Scan folders, build searchable library, browse and play from library.

| Task | Crate | Priority |
|------|-------|----------|
| SQLite schema + migrations | hadal-library | P0 |
| Directory scanner | hadal-library | P0 |
| Tag extraction (lofty) | hadal-library | P0 |
| Artist/Album/Track models | hadal-library | P0 |
| FTS5 search | hadal-library | P0 |
| Library browser view | hadal-tui | P0 |
| Multi-codec support (MP3, AAC, Ogg, Opus, WAV) | hadal-audio | P0 |
| Queue management | hadal-playlist | P1 |
| Keyboard navigation (vim-like) | hadal-tui | P1 |

**Deliverable:** Scan `~/Music`, browse artists/albums, play any supported format.

---

### Phase 3: Album Art & Polish

**Goal:** Album art display, playlists, refined UX.

| Task | Crate | Priority |
|------|-------|----------|
| Kitty graphics protocol | hadal-graphics | P0 |
| Embedded art extraction | hadal-library | P0 |
| Artwork cache | hadal-graphics | P0 |
| Fallback graphics (sixel, blocks) | hadal-graphics | P1 |
| Playlist CRUD | hadal-playlist | P0 |
| M3U8 import/export | hadal-playlist | P0 |
| User-configurable sorting | hadal-tui | P0 |
| Incremental search overlay | hadal-tui | P0 |
| Settings view | hadal-tui | P1 |
| Theme support | hadal-tui | P2 |

**Deliverable:** Full-featured music player with album art and playlists.

---

### Phase 4: Audiophile Features

**Goal:** Bit-perfect playback, format transparency, power user features.

| Task | Crate | Priority |
|------|-------|----------|
| Bit-perfect passthrough mode | hadal-audio | P0 |
| Sample rate display (source vs output) | hadal-tui | P0 |
| High-quality resampler (rubato) | hadal-audio | P1 |
| Gapless playback | hadal-audio | P0 |
| ReplayGain support | hadal-audio | P1 |
| Play count / last played tracking | hadal-library | P1 |
| Smart playlists (query-based) | hadal-playlist | P2 |
| ALAC support (if symphonia supports) | hadal-audio | P1 |
| File watcher for library updates | hadal-library | P2 |
| Spectrum analyzer widget | hadal-tui | P3 |

**Deliverable:** Audiophile-grade playback with full format transparency.

---

### Phase 5: Beta Polish

**Goal:** Stability, performance, documentation.

| Task | Crate | Priority |
|------|-------|----------|
| Large library optimization (200k+) | hadal-library | P1 |
| Memory profiling & optimization | all | P1 |
| Error handling audit | all | P0 |
| Logging infrastructure (tracing) | hadal-common | P1 |
| Man page / --help refinement | hadal | P1 |
| README, user documentation | docs | P1 |
| Integration tests | all | P1 |
| Benchmarks | hadal-audio | P2 |

**Deliverable:** Production-ready beta release.

---

## 10. Dependency Justification

### Core Dependencies

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `symphonia` | 0.5 | Audio decoding | Best pure-Rust decoder, broad codec support |
| `lofty` | 0.18 | Tag reading | Modern, well-maintained, supports all target formats |
| `rusqlite` | 0.31 | Database | Sync API fits hybrid model, FTS5 built-in |
| `ratatui` | 0.26 | TUI framework | Industry standard, active development |
| `crossterm` | 0.27 | Terminal backend | Cross-platform, works everywhere |
| `pipewire` | 0.8 | Audio output | Direct PipeWire integration, low latency |
| `tokio` | 1.0 | Async runtime | For I/O-bound tasks, file scanning |
| `ringbuf` | 0.4 | Lock-free buffer | Real-time audio transfer |
| `image` | 0.25 | Image processing | Thumbnail generation |
| `base64` | 0.22 | Encoding | Kitty protocol image transfer |

### Supporting Dependencies

| Crate | Purpose |
|-------|---------|
| `dirs` | XDG directory discovery |
| `toml` | Config parsing |
| `serde` | Serialization |
| `thiserror` | Error types |
| `tracing` | Structured logging |
| `parking_lot` | Fast mutexes (non-audio threads) |
| `notify` | File system watching |
| `clap` | CLI argument parsing |

---

## Appendix A: File Format Support Matrix

| Format | Container | Codec | Symphonia Support | Priority |
|--------|-----------|-------|-------------------|----------|
| FLAC | .flac | FLAC | ✅ Native | P0 |
| MP3 | .mp3 | MP3 | ✅ Native | P0 |
| AAC | .m4a, .aac | AAC-LC | ✅ Native | P0 |
| Ogg Vorbis | .ogg | Vorbis | ✅ Native | P0 |
| Opus | .opus | Opus | ✅ Native | P0 |
| WAV | .wav | PCM | ✅ Native | P0 |
| AIFF | .aiff, .aif | PCM | ✅ Native | P0 |
| ALAC | .m4a | ALAC | ✅ Native | P1 |
| WavPack | .wv | WavPack | ✅ Native | P2 |

---

## Appendix B: Error Handling Strategy

```rust
// hadal-common/src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum HadalError {
    // Audio errors
    #[error("Failed to decode audio: {0}")]
    Decode(#[from] symphonia::core::errors::Error),

    #[error("PipeWire error: {0}")]
    PipeWire(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    // Library errors
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Tag reading error: {0}")]
    TagRead(#[from] lofty::error::LoftyError),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    // Config errors
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, HadalError>;
```

---

*Document version: 1.0*
*Last updated: 2024*
*Author: Onyx/Claude*
