//! Search overlay — renders on top of the current view.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::state::AppState;
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    // Center the overlay
    let overlay = centered_rect(60, 60, area);

    // Clear the area behind the overlay
    frame.render_widget(Clear, overlay);

    let block = Block::default()
        .title(" Search ")
        .title_style(theme::header())
        .borders(Borders::ALL)
        .border_style(theme::border_focused())
        .style(theme::base());

    let inner = block.inner(overlay);
    frame.render_widget(block, overlay);

    let chunks = Layout::vertical([
        Constraint::Length(1), // search input
        Constraint::Length(1), // separator
        Constraint::Min(1),   // results
    ])
    .split(inner);

    // Search input line
    let cursor = if state.search.query.is_empty() {
        "_"
    } else {
        ""
    };
    let input_line = Line::from(vec![
        Span::styled(" /", theme::header()),
        Span::styled(&state.search.query, theme::base()),
        Span::styled(cursor, theme::base()),
    ]);
    frame.render_widget(Paragraph::new(input_line), chunks[0]);

    // Separator
    let sep = "─".repeat(inner.width as usize);
    frame.render_widget(
        Paragraph::new(Line::styled(sep, theme::border())),
        chunks[1],
    );

    // Results
    if state.search.results.is_empty() {
        let msg = if state.search.query.is_empty() {
            "Type to search..."
        } else {
            "No results found"
        };
        frame.render_widget(
            Paragraph::new(Line::styled(format!("  {}", msg), theme::dim())),
            chunks[2],
        );
    } else {
        let height = chunks[2].height as usize;
        let col = &state.search.column;

        let items: Vec<ListItem> = state
            .search
            .results
            .iter()
            .enumerate()
            .skip(col.offset)
            .take(height)
            .map(|(i, track)| {
                let style = if i == col.selected {
                    theme::selected()
                } else {
                    theme::base()
                };
                let dur_secs = track.duration_ms / 1000;
                let line = Line::from(vec![
                    Span::styled(format!("  {}", track.title), style),
                    Span::styled(
                        format!("  {}:{:02}", dur_secs / 60, dur_secs % 60),
                        theme::dim(),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        frame.render_widget(List::new(items), chunks[2]);
    }
}

/// Create a centered rectangle with given percentage width/height.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
