//! Playlist view — two-pane: playlist list (left) + tracks (right).

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::state::AppState;
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().style(theme::base());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Two-pane split: 30% playlist list | 70% tracks
    let h_chunks = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(70),
    ])
    .split(inner);

    render_playlist_list(frame, h_chunks[0], state);
    render_track_list(frame, h_chunks[1], state);

    // Name input overlay when creating/renaming
    if state.playlist_view.creating {
        render_name_input(frame, area, state);
    }
}

fn render_playlist_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let focused = state.playlist_view.depth == 0 && !state.playlist_view.creating;
    let border_style = if focused { theme::border_focused() } else { theme::border() };

    let block = Block::default()
        .title(" Playlists ")
        .title_style(if focused { theme::title() } else { theme::dim() })
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(theme::base());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.playlist_view.playlists.is_empty() {
        let text = vec![
            Line::from(""),
            Line::styled("  No playlists", theme::dim()),
            Line::styled("  Press 'n' to create", theme::dim()),
        ];
        frame.render_widget(Paragraph::new(text), inner);
        return;
    }

    let height = inner.height as usize;
    let offset = state.playlist_view.list_column.offset;
    let selected = state.playlist_view.list_column.selected;

    let mut lines: Vec<Line> = Vec::new();
    for (i, playlist) in state.playlist_view.playlists.iter().enumerate().skip(offset).take(height) {
        let is_selected = i == selected;
        let label = format!(" {} ({})", playlist.name, playlist.track_count);
        let style = if is_selected && focused {
            theme::selected()
        } else if is_selected {
            theme::selected().remove_modifier(Modifier::BOLD)
        } else {
            theme::base()
        };
        lines.push(Line::styled(label, style));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_track_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let focused = state.playlist_view.depth == 1 && !state.playlist_view.creating;
    let border_style = if focused { theme::border_focused() } else { theme::border() };

    let title = if let Some(pl) = state.playlist_view.playlists.get(state.playlist_view.list_column.selected) {
        format!(" {} ", pl.name)
    } else {
        " Tracks ".to_string()
    };

    let block = Block::default()
        .title(title)
        .title_style(if focused { theme::title() } else { theme::dim() })
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(theme::base());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.playlist_view.tracks.is_empty() {
        let msg = if state.playlist_view.playlists.is_empty() {
            "  Select a playlist"
        } else {
            "  No tracks — add from Library with 'P'"
        };
        let text = vec![Line::from(""), Line::styled(msg, theme::dim())];
        frame.render_widget(Paragraph::new(text), inner);
        return;
    }

    let height = inner.height as usize;
    let offset = state.playlist_view.track_column.offset;
    let selected = state.playlist_view.track_column.selected;

    let mut lines: Vec<Line> = Vec::new();
    for (i, track) in state.playlist_view.tracks.iter().enumerate().skip(offset).take(height) {
        let is_selected = i == selected;
        let duration_str = format_duration(track.duration_ms);
        let style = if is_selected && focused {
            theme::selected()
        } else if is_selected {
            theme::selected().remove_modifier(Modifier::BOLD)
        } else {
            theme::base()
        };

        let line = Line::from(vec![
            Span::styled(format!(" {}", track.title), style),
            Span::styled(format!("  {}", duration_str), style.fg(theme::FG_DARK)),
        ]);
        lines.push(line);
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_name_input(frame: &mut Frame, area: Rect, state: &AppState) {
    let width = 40.min(area.width.saturating_sub(4));
    let x = area.x + (area.width - width) / 2;
    let y = area.y + area.height / 3;
    let popup_area = Rect::new(x, y, width, 3);

    let block = Block::default()
        .title(" New Playlist ")
        .title_style(theme::title())
        .borders(Borders::ALL)
        .border_style(theme::border_focused())
        .style(theme::base());

    let input = Paragraph::new(Line::styled(
        format!(" {}_", state.playlist_view.name_buffer),
        theme::playing(),
    ))
    .block(block);

    // Clear the background behind the popup
    frame.render_widget(
        Block::default().style(theme::base()),
        Rect::new(popup_area.x.saturating_sub(1), popup_area.y.saturating_sub(1), popup_area.width + 2, popup_area.height + 2),
    );
    frame.render_widget(input, popup_area);
}

fn format_duration(ms: i64) -> String {
    let secs = ms / 1000;
    let m = secs / 60;
    let s = secs % 60;
    format!("{}:{:02}", m, s)
}
