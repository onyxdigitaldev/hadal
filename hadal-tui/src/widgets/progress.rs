//! Playback progress bar widget.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::state::AppState;
use crate::theme;

pub fn render_progress(frame: &mut Frame, area: Rect, state: &AppState) {
    if area.height < 2 || area.width < 8 {
        return;
    }

    let pos = state.playback.position;
    let dur = state.playback.duration;

    let pos_str = format!(
        "{}:{:02}",
        pos.as_secs() / 60,
        pos.as_secs() % 60
    );
    let dur_str = format!(
        "{}:{:02}",
        dur.as_secs() / 60,
        dur.as_secs() % 60
    );

    // Progress bar
    let bar_width = (area.width as usize).saturating_sub(2);
    let filled = if dur.as_secs() > 0 {
        (pos.as_secs_f64() / dur.as_secs_f64() * bar_width as f64) as usize
    } else {
        0
    };
    let empty = bar_width.saturating_sub(filled);

    let bar = Line::from(vec![
        Span::styled(" ", theme::base()),
        Span::styled("━".repeat(filled), theme::progress_bar()),
        Span::styled("─".repeat(empty), theme::dim()),
    ]);

    let time = Line::from(vec![
        Span::styled(format!(" {}", pos_str), theme::dim()),
        Span::styled(
            " ".repeat(bar_width.saturating_sub(pos_str.len() + dur_str.len())),
            theme::base(),
        ),
        Span::styled(dur_str, theme::dim()),
    ]);

    let text = vec![bar, time];
    frame.render_widget(Paragraph::new(text), area);
}
