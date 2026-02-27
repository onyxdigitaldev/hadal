//! Library browser — 3-column ranger-style with now-playing sidebar.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use ratatui_image::{Resize, StatefulImage};

use crate::state::AppState;
use crate::theme;
use crate::widgets::progress::render_progress;

/// Render the library view.
pub fn render(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let chunks = Layout::horizontal([
        Constraint::Length(24), // sidebar
        Constraint::Min(1),    // browser
    ])
    .split(area);

    render_sidebar(frame, chunks[0], state);
    render_browser(frame, chunks[1], &*state);
}

fn render_sidebar(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(theme::border())
        .style(theme::base());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(10), // album art (larger for actual images)
        Constraint::Length(3),  // track info
        Constraint::Length(2),  // progress
        Constraint::Min(0),    // spacer
        Constraint::Length(3),  // playback indicators
    ])
    .split(inner);

    // Album art
    if let Some(ref mut protocol) = state.artwork_protocol {
        let image_widget = StatefulImage::new(None).resize(Resize::Fit(None));
        frame.render_stateful_widget(image_widget, chunks[0], protocol);
    } else {
        let placeholder = Paragraph::new(Line::styled("  No Art", theme::dim()));
        frame.render_widget(placeholder, chunks[0]);
    }

    // Track info
    if let Some(track) = &state.playback.current_track {
        let w = inner.width as usize;
        let title = Line::from(Span::styled(
            format!(" {}", truncate(&track.title, w.saturating_sub(2))),
            theme::playing(),
        ));
        let artist_str = state
            .playback
            .artist_name
            .as_deref()
            .unwrap_or("Unknown");
        let artist = Line::from(Span::styled(
            format!(" {}", truncate(artist_str, w.saturating_sub(2))),
            theme::dim(),
        ));
        let album_str = state
            .playback
            .album_title
            .as_deref()
            .unwrap_or("");
        let album = Line::from(Span::styled(
            format!(" {}", truncate(album_str, w.saturating_sub(2))),
            theme::dim(),
        ));
        frame.render_widget(
            Paragraph::new(vec![title, artist, album]),
            chunks[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new("  No track playing").style(theme::dim()),
            chunks[1],
        );
    }

    // Progress bar
    render_progress(frame, chunks[2], state);

    // Playback indicators
    let status_char = match state.playback.status {
        hadal_common::PlayStatus::Playing => "▶",
        hadal_common::PlayStatus::Paused => "⏸",
        _ => "⏹",
    };
    let shuffle = if state.playback.shuffle { "🔀" } else { "  " };
    let repeat = match state.playback.repeat {
        hadal_common::RepeatMode::Off => "  ",
        hadal_common::RepeatMode::All => "🔁",
        hadal_common::RepeatMode::One => "🔂",
    };
    let vol = if state.playback.muted {
        "🔇".to_string()
    } else {
        format!("{}%", (state.playback.volume * 100.0) as u32)
    };

    let indicators = vec![
        Line::from(Span::styled(
            format!("  {} {} {} {}", status_char, shuffle, repeat, vol),
            theme::dim(),
        )),
    ];
    frame.render_widget(Paragraph::new(indicators), chunks[4]);
}

fn render_browser(frame: &mut Frame, area: Rect, state: &AppState) {
    let col_chunks = Layout::horizontal([
        Constraint::Percentage(30), // artists
        Constraint::Percentage(35), // albums
        Constraint::Percentage(35), // tracks
    ])
    .split(area);

    render_artist_column(frame, col_chunks[0], state);
    render_album_column(frame, col_chunks[1], state);
    render_track_column(frame, col_chunks[2], state);
}

fn render_artist_column(frame: &mut Frame, area: Rect, state: &AppState) {
    let focused = state.library.depth == 0;
    let block = Block::default()
        .title(" Artists ")
        .title_style(theme::header())
        .borders(Borders::RIGHT)
        .border_style(if focused { theme::border_focused() } else { theme::border() })
        .style(theme::base());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let height = inner.height as usize;
    let col = &state.library.columns[0];

    let items: Vec<ListItem> = state
        .library
        .artists
        .iter()
        .enumerate()
        .skip(col.offset)
        .take(height)
        .map(|(i, artist)| {
            let style = item_style(i, col.selected, focused, false);
            ListItem::new(Line::from(Span::styled(
                truncate(&artist.name, inner.width as usize),
                style,
            )))
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}

fn render_album_column(frame: &mut Frame, area: Rect, state: &AppState) {
    let focused = state.library.depth == 1;
    let block = Block::default()
        .title(" Albums ")
        .title_style(theme::header())
        .borders(Borders::RIGHT)
        .border_style(if focused { theme::border_focused() } else { theme::border() })
        .style(theme::base());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let height = inner.height as usize;
    let col = &state.library.columns[1];

    let items: Vec<ListItem> = state
        .library
        .albums
        .iter()
        .enumerate()
        .skip(col.offset)
        .take(height)
        .map(|(i, album)| {
            let year = album
                .year
                .map(|y| format!(" ({})", y))
                .unwrap_or_default();
            let text = format!("{}{}", album.title, year);
            let style = item_style(i, col.selected, focused, false);
            ListItem::new(Line::from(Span::styled(
                truncate(&text, inner.width as usize),
                style,
            )))
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}

fn render_track_column(frame: &mut Frame, area: Rect, state: &AppState) {
    let focused = state.library.depth == 2;
    let block = Block::default()
        .title(" Tracks ")
        .title_style(theme::header())
        .borders(Borders::NONE)
        .style(theme::base());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let height = inner.height as usize;
    let col = &state.library.columns[2];
    let current_track_id = state
        .playback
        .current_track
        .as_ref()
        .map(|t| t.id);

    let items: Vec<ListItem> = state
        .library
        .tracks
        .iter()
        .enumerate()
        .skip(col.offset)
        .take(height)
        .map(|(i, track)| {
            let is_playing = current_track_id == Some(track.id);
            let marker = if is_playing { "> " } else { "  " };
            let dur_secs = track.duration_ms / 1000;
            let dur = format!("{}:{:02}", dur_secs / 60, dur_secs % 60);

            let max_title = (inner.width as usize).saturating_sub(dur.len() + 4);
            let title_str = truncate(&track.title, max_title);

            let style = item_style(i, col.selected, focused, is_playing);
            let line = Line::from(vec![
                Span::styled(marker, if is_playing { theme::playing() } else { style }),
                Span::styled(title_str, style),
                Span::styled(format!(" {}", dur), theme::dim()),
            ]);
            ListItem::new(line)
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}

fn item_style(index: usize, selected: usize, focused: bool, is_playing: bool) -> Style {
    if is_playing && index == selected && focused {
        theme::playing_selected()
    } else if is_playing {
        theme::playing()
    } else if index == selected && focused {
        theme::selected()
    } else {
        theme::base()
    }
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        format!("{:<width$}", s, width = max)
    } else if max > 3 {
        let mut truncated: String = chars[..max - 3].iter().collect();
        truncated.push_str("...");
        truncated
    } else {
        chars[..max].iter().collect()
    }
}
