//! Application event loop, terminal setup/teardown, audio integration.

use std::io::{self, Stdout};
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use hadal_audio::{AudioPlayer, PipelineConfig, PipelineState, PlayerState, ResamplerQuality};
use hadal_common::{PlayStatus, PlaybackSettings};
use hadal_library::Database;
use hadal_playlist::QueueItem;

use crate::input::{self, Action};
use crate::output;
use crate::state::{AppState, InputMode, ViewId};
use crate::views;

type Term = Terminal<CrosstermBackend<Stdout>>;

const SEEK_AMOUNT: Duration = Duration::from_secs(5);
const VOLUME_STEP: f32 = 0.05;

/// Run the TUI application.
pub fn run(db: Database, paths: hadal_common::Paths, settings: PlaybackSettings) -> Result<()> {
    // Redirect stderr to /dev/null so ALSA/PipeWire C library messages
    // don't corrupt the TUI. Our logging goes to a file via tracing.
    suppress_stderr();

    let mut terminal = setup_terminal().context("Failed to setup terminal")?;
    let result = event_loop(&mut terminal, db, paths, settings);
    restore_terminal(&mut terminal).context("Failed to restore terminal")?;
    result
}

/// Redirect stderr (fd 2) to /dev/null.
///
/// ALSA's C library prints warnings directly to stderr which corrupts the TUI.
/// Since all Hadal logging goes through tracing → log file, we don't need stderr.
fn suppress_stderr() {
    use std::fs::File;
    unsafe {
        let devnull = File::open("/dev/null").expect("Failed to open /dev/null for stderr redirect");
        libc::dup2(devnull.as_raw_fd(), 2);
        // devnull is dropped but fd 2 now points to /dev/null
    }
}

fn setup_terminal() -> Result<Term> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Term) -> Result<()> {
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn event_loop(terminal: &mut Term, db: Database, paths: hadal_common::Paths, settings: PlaybackSettings) -> Result<()> {
    let mut state = AppState::new(paths.artwork_cache.clone());

    // Map resampler quality string to enum
    let resampler_quality = match settings.resampler_quality.as_str() {
        "fast" => ResamplerQuality::Fast,
        "best" => ResamplerQuality::Best,
        _ => ResamplerQuality::Medium,
    };

    tracing::info!(
        "Playback settings: resampler={:?}, buffer={}, gapless={}, volume={}",
        resampler_quality, settings.buffer_size, settings.gapless, settings.default_volume
    );

    // Set initial volume from config
    state.playback.volume = settings.default_volume.clamp(0.0, 1.0);

    // Load persisted EQ state if available
    if paths.eq_state.exists() {
        match std::fs::read_to_string(&paths.eq_state) {
            Ok(contents) => match toml::from_str::<crate::state::EqViewState>(&contents) {
                Ok(eq) => {
                    tracing::info!("Loaded EQ state: preset={}, bypassed={}", eq.preset_name, eq.bypassed);
                    state.eq = eq;
                }
                Err(e) => tracing::warn!("Failed to parse EQ state: {}", e),
            },
            Err(e) => tracing::warn!("Failed to read EQ state: {}", e),
        }
    }

    // Probe the audio device first so we can configure the pipeline correctly
    let device_config = output::probe_device().context("Failed to probe audio device")?;

    // Create audio player with config-driven pipeline settings.
    // The ring buffer needs ~2s of headroom to avoid underruns.
    // config.buffer_size is the cpal device buffer (frames), not the ring buffer.
    let ring_buffer_size = 96000_usize.max(device_config.sample_rate as usize * 2);
    let pipeline_config = PipelineConfig {
        output_sample_rate: Some(device_config.sample_rate),
        output_channels: Some(device_config.channels),
        buffer_size: ring_buffer_size,
        resampler_quality,
        gapless: settings.gapless,
        ..PipelineConfig::default()
    };
    let player = Arc::new(AudioPlayer::with_config(pipeline_config));

    // Start audio output stream (cpal drains samples to speakers)
    let _output = match output::start(Arc::clone(&player), &device_config) {
        Ok(stream) => {
            tracing::info!("Audio output started ({}Hz, {} ch)", device_config.sample_rate, device_config.channels);
            Some(stream)
        }
        Err(e) => {
            tracing::error!("Failed to start audio output: {}", e);
            state.set_status(format!("Audio output error: {}", e));
            None
        }
    };

    // Grab pipeline state for lock-free position polling
    let pipeline_state = player.pipeline_state();

    // Set initial volume on the pipeline
    pipeline_state.set_volume(state.playback.volume);

    // Apply persisted EQ state to player
    player.set_eq_gains(&state.eq.gains);
    player.set_eq_bypass(state.eq.bypassed);

    // Wire visualization data into state for UI rendering
    state.visualization_data = Some(player.visualization_data());

    // Initialize playlist manager
    let playlists_db_path = paths.data_dir.join("playlists.db");
    match hadal_playlist::PlaylistManager::open(&playlists_db_path) {
        Ok(pm) => {
            state.playlist_manager = Some(pm);
            tracing::info!("Playlist manager initialized at {}", playlists_db_path.display());
        }
        Err(e) => {
            tracing::error!("Failed to open playlist database: {}", e);
            state.set_status(format!("Playlist DB error: {}", e));
        }
    }

    // Load initial library data
    load_artists(&db, &mut state);

    while state.running {
        // Render
        terminal.draw(|frame| {
            views::render(frame, &mut state);
        })?;

        // Poll for crossterm events (~60fps)
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    let action = input::handle_key(key, &state);
                    handle_action(action, &mut state, &db, &player);
                }
                Event::Resize(w, h) => {
                    state.terminal_size = (w, h);
                }
                _ => {}
            }
        }

        // Tick: poll pipeline atomics for position, detect track end, auto-advance
        tick(&mut state, &player, &pipeline_state, &db);
    }

    // Persist EQ state before shutdown
    match toml::to_string_pretty(&state.eq) {
        Ok(contents) => {
            if let Err(e) = std::fs::write(&paths.eq_state, contents) {
                tracing::warn!("Failed to save EQ state: {}", e);
            } else {
                tracing::info!("Saved EQ state: preset={}", state.eq.preset_name);
            }
        }
        Err(e) => tracing::warn!("Failed to serialize EQ state: {}", e),
    }

    // Clean shutdown
    player.shutdown();

    Ok(())
}

/// Per-frame tick: poll pipeline state, detect track end, auto-advance queue.
fn tick(
    state: &mut AppState,
    player: &Arc<AudioPlayer>,
    pipeline_state: &Arc<PipelineState>,
    db: &Database,
) {
    // Update position/duration from lock-free atomics (no mutex needed)
    if state.playback.status == PlayStatus::Playing
        || state.playback.status == PlayStatus::Paused
    {
        state.playback.position = pipeline_state.position();
        state.playback.duration = pipeline_state.duration();
    }

    // Sync player state → UI status
    state.playback.status = match player.state() {
        PlayerState::Playing => PlayStatus::Playing,
        PlayerState::Paused => PlayStatus::Paused,
        PlayerState::Stopped => PlayStatus::Stopped,
        PlayerState::Loading => PlayStatus::Buffering,
    };

    // Detect track end → auto-advance through queue
    if state.playback.current_track.is_some() && player.is_finished() {
        // Try to advance to next track in queue
        let next_track_id = state.play_queue.next().map(|item| item.track_id);

        if let Some(track_id) = next_track_id {
            if let Ok(track) = db.get_track(track_id) {
                play_track(state, player, &track, db);
                return; // Don't clear playback state
            }
        }

        // No next track — clear playback state
        state.playback.status = PlayStatus::Stopped;
        state.playback.current_track = None;
        state.playback.source_format = None;
        state.playback.artist_name = None;
        state.playback.album_title = None;
        state.playback.artwork_path = None;
        state.playback.artwork_image = None;
        state.artwork_protocol = None;
        state.artwork_protocol_large = None;
        state.playback.position = Duration::ZERO;
        state.playback.duration = Duration::ZERO;
    }

    // Sync volume/mute from pipeline (in case something else changed it)
    state.playback.volume = pipeline_state.volume();
    state.playback.muted = pipeline_state.is_muted();

    // Clear expired status messages
    state.clear_expired_status();
}

fn handle_action(
    action: Action,
    state: &mut AppState,
    db: &Database,
    player: &Arc<AudioPlayer>,
) {
    match action {
        Action::Quit => state.running = false,
        Action::Refresh => {
            load_artists(db, state);
            state.set_status("Library refreshed");
        }
        Action::None => {}

        // Dispatch by category
        Action::SwitchView(_) | Action::OpenSearch | Action::CloseOverlay => {
            handle_view_action(action, state);
        }

        Action::Up | Action::Down | Action::Left | Action::Right
        | Action::Select | Action::GoTop | Action::GoBottom
        | Action::PageUp | Action::PageDown => {
            handle_navigation_action(action, state, db, player);
        }

        Action::SearchInput(_) | Action::SearchBackspace | Action::SearchSubmit => {
            handle_search_action(action, state, db, player);
        }

        Action::PlayPause | Action::Stop | Action::NextTrack | Action::PrevTrack
        | Action::SeekForward | Action::SeekBackward | Action::VolumeUp | Action::VolumeDown
        | Action::ToggleMute | Action::ToggleShuffle | Action::CycleRepeat => {
            handle_playback_action(action, state, db, player);
        }

        Action::AddToQueue | Action::AddAlbumToQueue | Action::RemoveFromQueue
        | Action::MoveUp | Action::MoveDown => {
            handle_queue_action(action, state, db);
        }

        Action::EqBandLeft | Action::EqBandRight | Action::EqGainUp | Action::EqGainDown
        | Action::EqToggleBypass | Action::EqNextPreset | Action::EqReset => {
            handle_eq_action(action, state, player);
        }

        Action::PlaylistCreate | Action::PlaylistDelete | Action::PlaylistRename
        | Action::PlaylistRemoveTrack | Action::PlaylistPaneLeft | Action::PlaylistPaneRight
        | Action::AddToPlaylist | Action::PlaylistNameInput(_) | Action::PlaylistNameBackspace
        | Action::PlaylistNameSubmit | Action::PlaylistNameCancel => {
            handle_playlist_action(action, state, db);
        }
    }
}

fn handle_view_action(action: Action, state: &mut AppState) {
    match action {
        Action::SwitchView(view) => {
            let old_view = state.active_view;
            state.active_view = view;

            // Force artwork protocol reset when switching views so the Kitty
            // terminal doesn't keep a stale image placement from the old view.
            if old_view != view {
                state.force_artwork_reload();
            }

            if view == ViewId::Search {
                state.search.active = true;
                state.input_mode = InputMode::Search;
            } else if view == ViewId::Playlists {
                reload_playlists(state);
            }
        }
        Action::OpenSearch => {
            state.search.active = true;
            state.search.query.clear();
            state.search.results.clear();
            state.search.column.selected = 0;
            state.input_mode = InputMode::Search;
            state.active_view = ViewId::Search;
        }
        Action::CloseOverlay => {
            state.search.active = false;
            state.input_mode = InputMode::Normal;
            if state.active_view == ViewId::Search {
                state.active_view = ViewId::Library;
            }
        }
        _ => {}
    }
}

fn handle_navigation_action(
    action: Action,
    state: &mut AppState,
    db: &Database,
    player: &Arc<AudioPlayer>,
) {
    match action {
        Action::Up => {
            navigate_up(state);
            reload_on_selection_change(state, db);
        }
        Action::Down => {
            navigate_down(state);
            reload_on_selection_change(state, db);
        }
        Action::Left => navigate_left(state),
        Action::Right => navigate_right(state, db),
        Action::Select => handle_select(state, db, player),
        Action::GoTop => {
            navigate_top(state);
            reload_on_selection_change(state, db);
        }
        Action::GoBottom => {
            navigate_bottom(state);
            reload_on_selection_change(state, db);
        }
        Action::PageUp => {
            navigate_page_up(state);
            reload_on_selection_change(state, db);
        }
        Action::PageDown => {
            navigate_page_down(state);
            reload_on_selection_change(state, db);
        }
        _ => {}
    }
}

fn handle_search_action(
    action: Action,
    state: &mut AppState,
    db: &Database,
    player: &Arc<AudioPlayer>,
) {
    match action {
        Action::SearchInput(c) => {
            state.search.query.push(c);
            state.search.cursor = state.search.query.len();
            run_search(db, state);
        }
        Action::SearchBackspace => {
            state.search.query.pop();
            state.search.cursor = state.search.query.len();
            run_search(db, state);
        }
        Action::SearchSubmit => {
            if let Some(track) = state.search.results.get(state.search.column.selected).cloned() {
                play_track(state, player, &track, db);
            }
            state.search.active = false;
            state.input_mode = InputMode::Normal;
            state.active_view = ViewId::Library;
        }
        _ => {}
    }
}

fn handle_playback_action(
    action: Action,
    state: &mut AppState,
    db: &Database,
    player: &Arc<AudioPlayer>,
) {
    match action {
        Action::PlayPause => {
            match player.state() {
                PlayerState::Playing => player.pause(),
                PlayerState::Paused => player.resume(),
                PlayerState::Stopped => {
                    if let Some(track) = state.library.selected_track().cloned() {
                        play_track(state, player, &track, db);
                    }
                }
                _ => {}
            }
        }
        Action::Stop => {
            player.stop();
            state.playback.status = PlayStatus::Stopped;
            state.playback.current_track = None;
            state.playback.position = Duration::ZERO;
            state.playback.duration = Duration::ZERO;
            state.playback.source_format = None;
            state.playback.artwork_path = None;
            state.playback.artwork_image = None;
            state.artwork_protocol = None;
            state.artwork_protocol_large = None;
        }
        Action::NextTrack => {
            let next_track_id = state.play_queue.next().map(|item| item.track_id);
            if let Some(track_id) = next_track_id {
                if let Ok(track) = db.get_track(track_id) {
                    play_track(state, player, &track, db);
                }
            } else {
                let col = &state.library.columns[2];
                let total = state.library.tracks.len();
                if total > 0 {
                    let next = (col.selected + 1) % total;
                    state.library.columns[2].selected = next;
                    if let Some(track) = state.library.tracks.get(next).cloned() {
                        play_track(state, player, &track, db);
                    }
                }
            }
        }
        Action::PrevTrack => {
            let prev_track_id = state.play_queue.previous().map(|item| item.track_id);
            if let Some(track_id) = prev_track_id {
                if let Ok(track) = db.get_track(track_id) {
                    play_track(state, player, &track, db);
                }
            } else {
                let col = &state.library.columns[2];
                let total = state.library.tracks.len();
                if total > 0 {
                    let prev = if col.selected == 0 { total - 1 } else { col.selected - 1 };
                    state.library.columns[2].selected = prev;
                    if let Some(track) = state.library.tracks.get(prev).cloned() {
                        play_track(state, player, &track, db);
                    }
                }
            }
        }
        Action::SeekForward => {
            if let Err(e) = player.seek_forward(SEEK_AMOUNT) {
                state.set_status(format!("Seek error: {}", e));
            }
        }
        Action::SeekBackward => {
            if let Err(e) = player.seek_backward(SEEK_AMOUNT) {
                state.set_status(format!("Seek error: {}", e));
            }
        }
        Action::VolumeUp => {
            state.playback.volume = (state.playback.volume + VOLUME_STEP).min(1.0);
            player.set_volume(state.playback.volume);
        }
        Action::VolumeDown => {
            state.playback.volume = (state.playback.volume - VOLUME_STEP).max(0.0);
            player.set_volume(state.playback.volume);
        }
        Action::ToggleMute => {
            state.playback.muted = !state.playback.muted;
            player.set_mute(state.playback.muted);
        }
        Action::ToggleShuffle => {
            state.play_queue.toggle_shuffle();
            state.playback.shuffle = state.play_queue.shuffle();
        }
        Action::CycleRepeat => {
            state.play_queue.cycle_repeat();
            state.playback.repeat = state.play_queue.repeat();
        }
        _ => {}
    }
}

fn handle_queue_action(action: Action, state: &mut AppState, db: &Database) {
    match action {
        Action::AddToQueue => add_to_queue(state, db),
        Action::AddAlbumToQueue => add_album_to_queue(state, db),
        Action::RemoveFromQueue => {
            if state.active_view == ViewId::Queue {
                let idx = state.queue_view.column.selected;
                if state.play_queue.remove(idx).is_ok() {
                    let total = state.play_queue.len();
                    state.queue_view.column.select(idx.min(total.saturating_sub(1)), total);
                    state.set_status("Removed from queue");
                }
            }
        }
        Action::MoveUp => {
            if state.active_view == ViewId::Queue {
                let idx = state.queue_view.column.selected;
                if idx > 0 && state.play_queue.move_item(idx, idx - 1).is_ok() {
                    state.queue_view.column.selected = idx - 1;
                }
            }
        }
        Action::MoveDown => {
            if state.active_view == ViewId::Queue {
                let idx = state.queue_view.column.selected;
                if idx + 1 < state.play_queue.len() && state.play_queue.move_item(idx, idx + 1).is_ok() {
                    state.queue_view.column.selected = idx + 1;
                }
            }
        }
        _ => {}
    }
}

fn handle_eq_action(action: Action, state: &mut AppState, player: &Arc<AudioPlayer>) {
    match action {
        Action::EqBandLeft => {
            if state.eq.selected_band > 0 {
                state.eq.selected_band -= 1;
            }
        }
        Action::EqBandRight => {
            if state.eq.selected_band < 9 {
                state.eq.selected_band += 1;
            }
        }
        Action::EqGainUp => {
            let band = state.eq.selected_band;
            state.eq.gains[band] = (state.eq.gains[band] + 1.0).min(12.0);
            player.set_eq_band(band, state.eq.gains[band]);
        }
        Action::EqGainDown => {
            let band = state.eq.selected_band;
            state.eq.gains[band] = (state.eq.gains[band] - 1.0).max(-12.0);
            player.set_eq_band(band, state.eq.gains[band]);
        }
        Action::EqToggleBypass => {
            state.eq.bypassed = !state.eq.bypassed;
            player.set_eq_bypass(state.eq.bypassed);
            state.set_status(if state.eq.bypassed { "EQ bypassed" } else { "EQ active" });
        }
        Action::EqNextPreset => {
            let presets = hadal_dsp::EqualizerPreset::all_presets();
            let current_idx = presets.iter().position(|p| p.name == state.eq.preset_name).unwrap_or(0);
            let next = (current_idx + 1) % presets.len();
            let preset = &presets[next];
            for (i, &gain) in preset.gains.iter().enumerate().take(10) {
                state.eq.gains[i] = gain;
            }
            state.eq.preset_name = preset.name.clone();
            player.set_eq_gains(&state.eq.gains);
            state.set_status(format!("Preset: {}", state.eq.preset_name));
        }
        Action::EqReset => {
            state.eq.gains = [0.0; 10];
            state.eq.preset_name = "Flat".to_string();
            player.set_eq_gains(&state.eq.gains);
            state.set_status("EQ reset to flat");
        }
        _ => {}
    }
}

fn handle_playlist_action(action: Action, state: &mut AppState, db: &Database) {
    match action {
        Action::PlaylistCreate => {
            state.playlist_view.creating = true;
            state.playlist_view.name_buffer.clear();
            state.input_mode = InputMode::PlaylistName;
        }
        Action::PlaylistNameInput(c) => {
            state.playlist_view.name_buffer.push(c);
        }
        Action::PlaylistNameBackspace => {
            state.playlist_view.name_buffer.pop();
        }
        Action::PlaylistNameSubmit => {
            let name = state.playlist_view.name_buffer.clone();
            if !name.is_empty() {
                if let Some(pm) = &state.playlist_manager {
                    let selected_pl = state.playlist_view.playlists.get(state.playlist_view.list_column.selected);
                    let is_rename = selected_pl.is_some_and(|pl| pl.name != name);

                    if is_rename {
                        if let Some(pl) = selected_pl {
                            let id = pl.id;
                            match pm.rename(id, &name) {
                                Ok(()) => {
                                    state.set_status(format!("Renamed to: {}", name));
                                    reload_playlists(state);
                                }
                                Err(e) => state.set_status(format!("Error: {}", e)),
                            }
                        }
                    } else {
                        match pm.create(&name, None) {
                            Ok(_) => {
                                state.set_status(format!("Created playlist: {}", name));
                                reload_playlists(state);
                            }
                            Err(e) => state.set_status(format!("Error: {}", e)),
                        }
                    }
                }
            }
            state.playlist_view.creating = false;
            state.playlist_view.name_buffer.clear();
            state.input_mode = InputMode::Normal;
        }
        Action::PlaylistNameCancel => {
            state.playlist_view.creating = false;
            state.playlist_view.name_buffer.clear();
            state.input_mode = InputMode::Normal;
        }
        Action::PlaylistDelete => {
            if state.active_view == ViewId::Playlists && state.playlist_view.depth == 0 {
                if let Some(pl) = state.playlist_view.playlists.get(state.playlist_view.list_column.selected) {
                    let id = pl.id;
                    let name = pl.name.clone();
                    if let Some(pm) = &state.playlist_manager {
                        match pm.delete(id) {
                            Ok(()) => {
                                state.set_status(format!("Deleted playlist: {}", name));
                                reload_playlists(state);
                            }
                            Err(e) => state.set_status(format!("Error: {}", e)),
                        }
                    }
                }
            }
        }
        Action::PlaylistRename => {
            if state.active_view == ViewId::Playlists && state.playlist_view.depth == 0 {
                if let Some(pl) = state.playlist_view.playlists.get(state.playlist_view.list_column.selected) {
                    state.playlist_view.creating = true;
                    state.playlist_view.name_buffer = pl.name.clone();
                    state.input_mode = InputMode::PlaylistName;
                }
            }
        }
        Action::PlaylistRemoveTrack => {
            if state.active_view == ViewId::Playlists && state.playlist_view.depth == 1 {
                if let Some(pl) = state.playlist_view.playlists.get(state.playlist_view.list_column.selected) {
                    let position = state.playlist_view.track_column.selected as i32 + 1;
                    if let Some(pm) = &state.playlist_manager {
                        match pm.remove_track(pl.id, position) {
                            Ok(()) => {
                                state.set_status("Removed track from playlist");
                                reload_playlist_tracks(state, db);
                            }
                            Err(e) => state.set_status(format!("Error: {}", e)),
                        }
                    }
                }
            }
        }
        Action::PlaylistPaneLeft => {
            if state.playlist_view.depth > 0 {
                state.playlist_view.depth = 0;
            }
        }
        Action::PlaylistPaneRight => {
            if state.playlist_view.depth == 0 && !state.playlist_view.playlists.is_empty() {
                state.playlist_view.depth = 1;
                reload_playlist_tracks(state, db);
            }
        }
        Action::AddToPlaylist => add_to_playlist(state, db),
        _ => {}
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Playback
// ─────────────────────────────────────────────────────────────────────────────

/// Play a track from a TrackRow.
fn play_track(
    state: &mut AppState,
    player: &Arc<AudioPlayer>,
    track: &hadal_library::TrackRow,
    db: &Database,
) {
    let path = &track.path;

    match player.play(path) {
        Ok(()) => {
            // Build source format info from track metadata
            let source_format = build_format(track);

            // Look up artist and album
            let artist_name = track
                .artist_id
                .and_then(|id| match db.get_artist(id) {
                    Ok(a) => Some(a),
                    Err(e) => { tracing::debug!("Failed to get artist {}: {}", id, e); None }
                })
                .map(|a| a.name);
            let album = track
                .album_id
                .and_then(|id| match db.get_album(id) {
                    Ok(a) => Some(a),
                    Err(e) => { tracing::debug!("Failed to get album {}: {}", id, e); None }
                });
            let album_title = album.as_ref().map(|a| a.title.clone());

            // Resolve artwork from album's artwork_hash
            let artwork_path = album
                .and_then(|a| a.artwork_hash)
                .map(|hash| state.artwork_cache_dir.join(format!("{}.png", hash)));

            // Load artwork image if path changed
            let needs_load = match (&artwork_path, &state.playback.artwork_image) {
                (Some(new_path), Some((old_path, _))) => new_path != old_path,
                (Some(_), None) => true,
                (None, _) => false,
            };
            if needs_load {
                if let Some(ref art_file) = artwork_path {
                    if art_file.exists() {
                        match image::open(art_file) {
                            Ok(img) => {
                                // Update protocol states for rendering
                                if let Some(ref mut picker) = state.image_picker {
                                    state.artwork_protocol = Some(picker.new_resize_protocol(img.clone()));
                                    state.artwork_protocol_large = Some(picker.new_resize_protocol(img.clone()));
                                }
                                state.playback.artwork_image = Some((art_file.clone(), img));
                            }
                            Err(e) => {
                                tracing::warn!("Failed to load artwork {}: {}", art_file.display(), e);
                                state.playback.artwork_image = None;
                                state.artwork_protocol = None;
                                state.artwork_protocol_large = None;
                            }
                        }
                    } else {
                        state.playback.artwork_image = None;
                        state.artwork_protocol = None;
                        state.artwork_protocol_large = None;
                    }
                }
            } else if artwork_path.is_none() {
                state.playback.artwork_image = None;
                state.artwork_protocol = None;
                state.artwork_protocol_large = None;
            }

            state.playback.artwork_path = artwork_path;
            state.playback.status = PlayStatus::Playing;
            state.playback.current_track = Some(track.clone());
            state.playback.source_format = source_format;
            state.playback.artist_name = artist_name;
            state.playback.album_title = album_title;
            state.playback.position = Duration::ZERO;
            state.playback.duration =
                Duration::from_millis(track.duration_ms.max(0) as u64);

            tracing::info!("Playing: {}", track.title);
        }
        Err(e) => {
            tracing::error!("Failed to play {}: {}", track.title, e);
            state.set_status(format!("Playback error: {}", e));
        }
    }
}

/// Build an AudioFormat from TrackRow metadata.
fn build_format(
    track: &hadal_library::TrackRow,
) -> Option<hadal_common::AudioFormat> {
    let sample_rate = track.sample_rate? as u32;
    let bit_depth = match track.bit_depth? {
        8 => hadal_common::BitDepth::U8,
        16 => hadal_common::BitDepth::S16,
        24 => hadal_common::BitDepth::S24,
        32 => hadal_common::BitDepth::S32,
        _ => hadal_common::BitDepth::S16,
    };
    let codec = track
        .codec
        .as_deref()
        .map(|c| match c.to_lowercase().as_str() {
            "flac" => hadal_common::Codec::Flac,
            "mp3" => hadal_common::Codec::Mp3,
            "aac" | "m4a" => hadal_common::Codec::Aac,
            "vorbis" | "ogg" => hadal_common::Codec::Vorbis,
            "opus" => hadal_common::Codec::Opus,
            "wav" => hadal_common::Codec::Wav,
            "aiff" | "aif" => hadal_common::Codec::Aiff,
            "alac" => hadal_common::Codec::Alac,
            "wavpack" | "wv" => hadal_common::Codec::WavPack,
            _ => hadal_common::Codec::Unknown,
        })
        .unwrap_or(hadal_common::Codec::Unknown);
    let channels = track.channels.unwrap_or(2) as u8;

    let mut fmt = hadal_common::AudioFormat::new(sample_rate, channels, bit_depth, codec);
    if let Some(br) = track.bitrate {
        fmt = fmt.with_bitrate(br as u32);
    }
    Some(fmt)
}

// ─────────────────────────────────────────────────────────────────────────────
// Navigation helpers
// ─────────────────────────────────────────────────────────────────────────────

fn navigate_up(state: &mut AppState) {
    match state.active_view {
        ViewId::Library => {
            let depth = state.library.depth;
            let total = state.library.column_len(depth);
            state.library.columns[depth].up(total);
            state.library.columns[depth].scroll_into_view(20);
        }
        ViewId::Queue => {
            let total = state.play_queue.len();
            state.queue_view.column.up(total);
            state.queue_view.column.scroll_into_view(20);
        }
        ViewId::Playlists => {
            if state.playlist_view.depth == 0 {
                let total = state.playlist_view.playlists.len();
                state.playlist_view.list_column.up(total);
                state.playlist_view.list_column.scroll_into_view(20);
            } else {
                let total = state.playlist_view.tracks.len();
                state.playlist_view.track_column.up(total);
                state.playlist_view.track_column.scroll_into_view(20);
            }
        }
        ViewId::Search if state.search.active => {
            let total = state.search.results.len();
            state.search.column.up(total);
        }
        _ => {}
    }
}

fn navigate_down(state: &mut AppState) {
    match state.active_view {
        ViewId::Library => {
            let depth = state.library.depth;
            let total = state.library.column_len(depth);
            state.library.columns[depth].down(total);
            state.library.columns[depth].scroll_into_view(20);
        }
        ViewId::Queue => {
            let total = state.play_queue.len();
            state.queue_view.column.down(total);
            state.queue_view.column.scroll_into_view(20);
        }
        ViewId::Playlists => {
            if state.playlist_view.depth == 0 {
                let total = state.playlist_view.playlists.len();
                state.playlist_view.list_column.down(total);
                state.playlist_view.list_column.scroll_into_view(20);
            } else {
                let total = state.playlist_view.tracks.len();
                state.playlist_view.track_column.down(total);
                state.playlist_view.track_column.scroll_into_view(20);
            }
        }
        ViewId::Search if state.search.active => {
            let total = state.search.results.len();
            state.search.column.down(total);
        }
        _ => {}
    }
}

fn navigate_left(state: &mut AppState) {
    if state.active_view == ViewId::Library && state.library.depth > 0 {
        state.library.depth -= 1;
    }
}

fn navigate_right(state: &mut AppState, db: &Database) {
    if state.active_view == ViewId::Library && state.library.depth < 2 {
        state.library.depth += 1;
        match state.library.depth {
            1 => load_albums(db, state),
            2 => load_tracks(db, state),
            _ => {}
        }
    }
}

fn handle_select(
    state: &mut AppState,
    db: &Database,
    player: &Arc<AudioPlayer>,
) {
    match state.active_view {
        ViewId::Library => {
            if state.library.depth < 2 {
                navigate_right(state, db);
            } else {
                // Play selected track — add to queue and jump to it
                if let Some(track) = state.library.selected_track().cloned() {
                    let item = track_to_queue_item(&track, db);
                    state.play_queue.push_back(item);
                    let idx = state.play_queue.len() - 1;
                    let _ = state.play_queue.jump_to(idx);
                    play_track(state, player, &track, db);
                }
            }
        }
        ViewId::Queue => {
            let idx = state.queue_view.column.selected;
            if state.play_queue.jump_to(idx).is_ok() {
                let track_id = state.play_queue.current().map(|item| item.track_id);
                if let Some(track_id) = track_id {
                    if let Ok(track) = db.get_track(track_id) {
                        play_track(state, player, &track, db);
                    }
                }
            }
        }
        ViewId::Playlists => {
            if state.playlist_view.depth == 0 {
                // Enter on playlist list → switch to track pane
                if !state.playlist_view.playlists.is_empty() {
                    state.playlist_view.depth = 1;
                    reload_playlist_tracks(state, db);
                }
            } else {
                // Enter on track → play it
                if let Some(track) = state.playlist_view.tracks.get(state.playlist_view.track_column.selected).cloned() {
                    let item = track_to_queue_item(&track, db);
                    state.play_queue.push_back(item);
                    let idx = state.play_queue.len() - 1;
                    let _ = state.play_queue.jump_to(idx);
                    play_track(state, player, &track, db);
                }
            }
        }
        ViewId::Search if state.search.active => {
            if let Some(track) = state.search.results.get(state.search.column.selected).cloned() {
                let item = track_to_queue_item(&track, db);
                state.play_queue.push_back(item);
                let idx = state.play_queue.len() - 1;
                let _ = state.play_queue.jump_to(idx);
                play_track(state, player, &track, db);
            }
            state.search.active = false;
            state.input_mode = InputMode::Normal;
            state.active_view = ViewId::Library;
        }
        _ => {}
    }
}

fn navigate_top(state: &mut AppState) {
    match state.active_view {
        ViewId::Library => {
            let depth = state.library.depth;
            state.library.columns[depth].go_top();
        }
        ViewId::Queue => state.queue_view.column.go_top(),
        ViewId::Playlists => {
            if state.playlist_view.depth == 0 {
                state.playlist_view.list_column.go_top();
            } else {
                state.playlist_view.track_column.go_top();
            }
        }
        _ => {}
    }
}

fn navigate_bottom(state: &mut AppState) {
    match state.active_view {
        ViewId::Library => {
            let depth = state.library.depth;
            let total = state.library.column_len(depth);
            state.library.columns[depth].go_bottom(total);
        }
        ViewId::Queue => {
            let total = state.play_queue.len();
            state.queue_view.column.go_bottom(total);
        }
        ViewId::Playlists => {
            if state.playlist_view.depth == 0 {
                let total = state.playlist_view.playlists.len();
                state.playlist_view.list_column.go_bottom(total);
            } else {
                let total = state.playlist_view.tracks.len();
                state.playlist_view.track_column.go_bottom(total);
            }
        }
        _ => {}
    }
}

fn navigate_page_up(state: &mut AppState) {
    match state.active_view {
        ViewId::Library => {
            let depth = state.library.depth;
            state.library.columns[depth].page_up(20);
        }
        ViewId::Queue => state.queue_view.column.page_up(20),
        ViewId::Playlists => {
            if state.playlist_view.depth == 0 {
                state.playlist_view.list_column.page_up(20);
            } else {
                state.playlist_view.track_column.page_up(20);
            }
        }
        _ => {}
    }
}

fn navigate_page_down(state: &mut AppState) {
    match state.active_view {
        ViewId::Library => {
            let depth = state.library.depth;
            let total = state.library.column_len(depth);
            state.library.columns[depth].page_down(total, 20);
        }
        ViewId::Queue => {
            let total = state.play_queue.len();
            state.queue_view.column.page_down(total, 20);
        }
        ViewId::Playlists => {
            if state.playlist_view.depth == 0 {
                let total = state.playlist_view.playlists.len();
                state.playlist_view.list_column.page_down(total, 20);
            } else {
                let total = state.playlist_view.tracks.len();
                state.playlist_view.track_column.page_down(total, 20);
            }
        }
        _ => {}
    }
}

/// After moving the selection in the library/playlists, reload child data.
fn reload_on_selection_change(state: &mut AppState, db: &Database) {
    match state.active_view {
        ViewId::Library => {
            match state.library.depth {
                0 => load_albums(db, state),
                1 => load_tracks(db, state),
                _ => {}
            }
        }
        ViewId::Playlists => {
            if state.playlist_view.depth == 0 {
                reload_playlist_tracks(state, db);
            }
        }
        _ => {}
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Data loading
// ─────────────────────────────────────────────────────────────────────────────

fn load_artists(db: &Database, state: &mut AppState) {
    match db.get_all_artists() {
        Ok(artists) => {
            state.library.artists = artists;
            state.library.columns[0].select(0, state.library.artists.len());
            load_albums(db, state);
        }
        Err(e) => {
            tracing::error!("Failed to load artists: {}", e);
            state.set_status(format!("Error loading artists: {}", e));
        }
    }
}

fn load_albums(db: &Database, state: &mut AppState) {
    let artist_id = state.library.selected_artist().map(|a| a.id);
    state.library.filter_artist_id = artist_id;

    match db.get_albums(artist_id) {
        Ok(albums) => {
            state.library.albums = albums;
            state.library.columns[1].select(0, state.library.albums.len());
            load_tracks(db, state);
        }
        Err(e) => {
            tracing::error!("Failed to load albums: {}", e);
        }
    }
}

fn load_tracks(db: &Database, state: &mut AppState) {
    let album_id = state.library.selected_album().map(|a| a.id);
    let artist_id = state.library.filter_artist_id;
    state.library.filter_album_id = album_id;

    match db.get_tracks(album_id, artist_id, None, None) {
        Ok(tracks) => {
            state.library.tracks = tracks;
            state.library.columns[2].select(0, state.library.tracks.len());
        }
        Err(e) => {
            tracing::error!("Failed to load tracks: {}", e);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Queue helpers
// ─────────────────────────────────────────────────────────────────────────────

fn track_to_queue_item(track: &hadal_library::TrackRow, db: &Database) -> QueueItem {
    let artist_name = track
        .artist_id
        .and_then(|id| match db.get_artist(id) {
            Ok(a) => Some(a),
            Err(e) => { tracing::debug!("Failed to get artist {}: {}", id, e); None }
        })
        .map(|a| a.name);
    QueueItem::new(
        track.id,
        track.title.clone(),
        artist_name,
        track.duration_ms,
        track.path.clone(),
    )
}

fn add_to_queue(state: &mut AppState, db: &Database) {
    match state.active_view {
        ViewId::Library => {
            match state.library.depth {
                2 => {
                    // Single track
                    if let Some(track) = state.library.selected_track().cloned() {
                        let item = track_to_queue_item(&track, db);
                        state.play_queue.push_back(item);
                        state.set_status(format!("Added: {}", track.title));
                    }
                }
                1 => {
                    // All tracks in selected album
                    add_album_tracks_to_queue(state, db);
                }
                0 => {
                    // All tracks by selected artist (all albums)
                    if let Some(artist) = state.library.selected_artist().cloned() {
                        if let Ok(albums) = db.get_albums(Some(artist.id)) {
                            let mut count = 0;
                            for album in &albums {
                                if let Ok(tracks) = db.get_tracks(Some(album.id), Some(artist.id), None, None) {
                                    for track in &tracks {
                                        let item = track_to_queue_item(track, db);
                                        state.play_queue.push_back(item);
                                        count += 1;
                                    }
                                }
                            }
                            state.set_status(format!("Added {} tracks by {}", count, artist.name));
                        }
                    }
                }
                _ => {}
            }
        }
        ViewId::Search => {
            if let Some(track) = state.search.results.get(state.search.column.selected).cloned() {
                let item = track_to_queue_item(&track, db);
                state.play_queue.push_back(item);
                state.set_status(format!("Added: {}", track.title));
            }
        }
        _ => {}
    }
}

fn add_album_to_queue(state: &mut AppState, db: &Database) {
    if state.active_view == ViewId::Library {
        add_album_tracks_to_queue(state, db);
    }
}

fn add_album_tracks_to_queue(state: &mut AppState, db: &Database) {
    // Get the album from the current selection, regardless of depth
    let album = if state.library.depth >= 1 {
        state.library.selected_album().cloned()
    } else {
        None
    };

    if let Some(album) = album {
        if let Ok(tracks) = db.get_tracks(Some(album.id), album.artist_id, None, None) {
            let count = tracks.len();
            for track in &tracks {
                let item = track_to_queue_item(track, db);
                state.play_queue.push_back(item);
            }
            state.set_status(format!("Added album: {} ({} tracks)", album.title, count));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Playlist helpers
// ─────────────────────────────────────────────────────────────────────────────

fn reload_playlists(state: &mut AppState) {
    if let Some(pm) = &state.playlist_manager {
        match pm.list() {
            Ok(playlists) => {
                state.playlist_view.playlists = playlists;
                let total = state.playlist_view.playlists.len();
                state.playlist_view.list_column.select(
                    state.playlist_view.list_column.selected.min(total.saturating_sub(1)),
                    total,
                );
            }
            Err(e) => {
                tracing::error!("Failed to load playlists: {}", e);
            }
        }
    }
}

fn reload_playlist_tracks(state: &mut AppState, db: &Database) {
    let pl_id = state
        .playlist_view
        .playlists
        .get(state.playlist_view.list_column.selected)
        .map(|p| p.id);

    if let (Some(pl_id), Some(pm)) = (pl_id, &state.playlist_manager) {
        match pm.get_tracks(pl_id) {
            Ok(playlist_tracks) => {
                let mut tracks = Vec::new();
                for pt in &playlist_tracks {
                    if let Ok(track) = db.get_track(pt.track_id) {
                        tracks.push(track);
                    }
                }
                state.playlist_view.tracks = tracks;
                let total = state.playlist_view.tracks.len();
                state.playlist_view.track_column.select(
                    state.playlist_view.track_column.selected.min(total.saturating_sub(1)),
                    total,
                );
            }
            Err(e) => {
                tracing::error!("Failed to load playlist tracks: {}", e);
                state.playlist_view.tracks.clear();
            }
        }
    } else {
        state.playlist_view.tracks.clear();
    }
}

fn add_to_playlist(state: &mut AppState, _db: &Database) {
    // Get the track to add
    let track = match state.active_view {
        ViewId::Library => state.library.selected_track().cloned(),
        ViewId::Search => state.search.results.get(state.search.column.selected).cloned(),
        _ => None,
    };

    let track = match track {
        Some(t) => t,
        None => {
            state.set_status("No track selected");
            return;
        }
    };

    // Use first playlist (or prompt if none)
    if let Some(pm) = &state.playlist_manager {
        match pm.list() {
            Ok(playlists) if !playlists.is_empty() => {
                let pl = &playlists[0];
                match pm.add_track(pl.id, track.id) {
                    Ok(()) => {
                        state.set_status(format!("Added '{}' to '{}'", track.title, pl.name));
                    }
                    Err(e) => state.set_status(format!("Error: {}", e)),
                }
            }
            Ok(_) => {
                state.set_status("No playlists — press 6 then 'n' to create one");
            }
            Err(e) => state.set_status(format!("Error: {}", e)),
        }
    }
}

fn run_search(db: &Database, state: &mut AppState) {
    if state.search.query.is_empty() {
        state.search.results.clear();
        return;
    }

    match db.search(&state.search.query, 50) {
        Ok(track_ids) => {
            let mut results = Vec::new();
            for id in track_ids {
                if let Ok(track) = db.get_track(id) {
                    results.push(track);
                }
            }
            state.search.results = results;
            state.search.column.select(0, state.search.results.len());
        }
        Err(e) => {
            tracing::error!("Search error: {}", e);
            state.search.results.clear();
        }
    }
}
