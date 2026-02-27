//! Now Playing view — album art + track info.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use ratatui_image::{Resize, StatefulImage};

use crate::state::AppState;
use crate::theme;
use crate::widgets::progress::render_progress;
use crate::widgets::spectrum::render_spectrum;
use crate::widgets::vu_meter::render_vu_meter;

pub fn render(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let block = Block::default().style(theme::base());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.playback.current_track.is_none() {
        let text = vec![
            Line::from(""),
            Line::styled("  No track playing", theme::dim()),
        ];
        frame.render_widget(Paragraph::new(text), inner);
        return;
    }

    // Split: album art on left, info on right
    let h_chunks = Layout::horizontal([
        Constraint::Length(32), // album art
        Constraint::Min(1),    // track info
    ])
    .split(inner);

    // Album art
    let art_area = h_chunks[0];
    if let Some(ref mut protocol) = state.artwork_protocol_large {
        let art_block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(theme::border())
            .style(theme::base());
        let art_inner = art_block.inner(art_area);
        frame.render_widget(art_block, art_area);

        let image_widget = StatefulImage::new(None).resize(Resize::Fit(None));
        frame.render_stateful_widget(image_widget, art_inner, protocol);
    } else {
        let art_block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(theme::border())
            .style(theme::base());
        let art_inner = art_block.inner(art_area);
        frame.render_widget(art_block, art_area);

        let placeholder = Paragraph::new(Line::styled("  No Art", theme::dim()));
        frame.render_widget(placeholder, art_inner);
    }

    // Track info on right side
    let info_area = h_chunks[1];
    let info_chunks = Layout::vertical([
        Constraint::Length(2), // top padding
        Constraint::Length(2), // title
        Constraint::Length(1), // artist
        Constraint::Length(1), // album
        Constraint::Length(1), // spacer
        Constraint::Length(1), // format info
        Constraint::Length(1), // spacer
        Constraint::Length(2), // progress bar
        Constraint::Length(1), // spacer
        Constraint::Length(8), // spectrum analyzer
        Constraint::Length(2), // VU meter
        Constraint::Length(1), // playback indicators
    ])
    .split(info_area);

    let track = state.playback.current_track.as_ref().unwrap();

    // Title
    let title = Line::from(Span::styled(
        format!("  {}", &track.title),
        theme::playing(),
    ));
    frame.render_widget(Paragraph::new(vec![Line::from(""), title]), info_chunks[1]);

    // Artist
    let artist_str = state
        .playback
        .artist_name
        .as_deref()
        .unwrap_or("Unknown Artist");
    frame.render_widget(
        Paragraph::new(Line::styled(format!("  {}", artist_str), theme::dim())),
        info_chunks[2],
    );

    // Album
    let album_str = state
        .playback
        .album_title
        .as_deref()
        .unwrap_or("");
    if !album_str.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled(format!("  {}", album_str), theme::dim())),
            info_chunks[3],
        );
    }

    // Format info
    if let Some(fmt) = &state.playback.source_format {
        let fmt_str = format!(
            "  {} | {}kHz / {}bit / {} ch",
            fmt.codec.display_name(),
            fmt.sample_rate / 1000,
            fmt.bit_depth.bits(),
            fmt.channels,
        );
        frame.render_widget(
            Paragraph::new(Line::styled(fmt_str, theme::dim())),
            info_chunks[5],
        );
    }

    // Progress bar
    render_progress(frame, info_chunks[7], &*state);

    // Spectrum analyzer + VU meter
    if let Some(ref viz_data) = state.visualization_data {
        render_spectrum(frame, info_chunks[9], viz_data);
        render_vu_meter(frame, info_chunks[10], viz_data);
    }

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

    frame.render_widget(
        Paragraph::new(Line::styled(
            format!("  {} {} {} {}", status_char, shuffle, repeat, vol),
            theme::dim(),
        )),
        info_chunks[11],
    );
}
