//! Queue view — track list with current-playing marker.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::state::{AppState, ViewId};
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().style(theme::base());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(1), // column headers
        Constraint::Min(1),   // track list
    ])
    .split(inner);

    let queue = &state.play_queue;
    let is_active = state.active_view == ViewId::Queue;

    // Header: "Queue — N tracks, M:SS total"
    let total_ms = queue.total_duration_ms();
    let total_secs = total_ms / 1000;
    let total_min = total_secs / 60;
    let total_sec = total_secs % 60;
    let header_text = format!(
        "  Queue — {} track{}, {}:{:02} total",
        queue.len(),
        if queue.len() == 1 { "" } else { "s" },
        total_min,
        total_sec,
    );
    frame.render_widget(
        Paragraph::new(Line::styled(header_text, theme::header())),
        chunks[0],
    );

    // Column headers
    let col_header = Line::from(vec![
        Span::styled("     ", theme::dim()),
        Span::styled("#  ", theme::dim()),
        Span::styled("Title", theme::dim()),
        Span::styled(" — ", theme::dim()),
        Span::styled("Artist", theme::dim()),
        Span::styled("        Duration", theme::dim()),
    ]);
    frame.render_widget(Paragraph::new(col_header), chunks[1]);

    // Empty state
    if queue.is_empty() {
        let empty = Paragraph::new(Line::styled(
            "  Press 'a' to add tracks",
            theme::dim(),
        ));
        frame.render_widget(empty, chunks[2]);
        return;
    }

    // Track list
    let list_area = chunks[2];
    let visible_height = list_area.height as usize;
    let offset = state.queue_view.column.offset;
    let selected = state.queue_view.column.selected;
    let current_pos = queue.position();

    let mut lines: Vec<Line> = Vec::with_capacity(visible_height);

    for i in offset..queue.len().min(offset + visible_height) {
        if let Some(item) = queue.get(i) {
            let is_current = i == current_pos && state.playback.current_track.is_some();
            let is_selected = i == selected && is_active;

            let marker = if is_current { " > " } else { "   " };
            let num = format!("{:<3}", i + 1);
            let artist = item.artist.as_deref().unwrap_or("Unknown");
            let dur_secs = item.duration_ms / 1000;
            let dur = format!("{}:{:02}", dur_secs / 60, dur_secs % 60);

            // Determine available width for title-artist
            let fixed_width = 3 + 3 + 3 + 7; // marker + num + " — " + duration padding
            let avail = (list_area.width as usize).saturating_sub(fixed_width);
            let title_artist = format!("{} — {}", item.title, artist);
            let title_artist_display = if title_artist.len() > avail {
                format!("{}…", &title_artist[..avail.saturating_sub(1)])
            } else {
                title_artist
            };

            let style = if is_current && is_selected {
                theme::playing_selected()
            } else if is_current {
                theme::playing()
            } else if is_selected {
                theme::selected()
            } else {
                theme::base()
            };

            let line = Line::from(vec![
                Span::styled(marker, style),
                Span::styled(num, if is_current { theme::playing() } else { theme::dim() }),
                Span::styled(title_artist_display, style),
                Span::styled(format!("  {}", dur), if is_current { theme::playing() } else { theme::dim() }),
            ]);

            lines.push(line);
        }
    }

    frame.render_widget(Paragraph::new(lines), list_area);
}
