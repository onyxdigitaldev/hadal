//! 10-band graphic equalizer view.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::state::AppState;
use crate::theme;

/// 10-band center frequencies.
const FREQ_LABELS: [&str; 10] = [
    "31", "62", "125", "250", "500", "1K", "2K", "4K", "8K", "16K",
];

/// Maximum gain range in dB.
const MAX_DB: f64 = 12.0;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default().style(theme::base());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 10 || inner.width < 40 {
        frame.render_widget(
            Paragraph::new("  Terminal too small for EQ").style(theme::dim()),
            inner,
        );
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(2), // header
        Constraint::Min(1),   // sliders
        Constraint::Length(2), // freq labels + help
    ])
    .split(inner);

    // Header
    let status = if state.eq.bypassed { "BYPASSED" } else { "ACTIVE" };
    let status_style = if state.eq.bypassed { theme::dim() } else { Style::default().fg(theme::GREEN) };
    let header = Line::from(vec![
        Span::styled("  10-Band Equalizer  ", theme::header()),
        Span::styled(format!("[{}]", status), status_style),
        Span::styled(format!("  Preset: {}", state.eq.preset_name), theme::dim()),
    ]);
    frame.render_widget(Paragraph::new(vec![Line::from(""), header]), chunks[0]);

    // Sliders area
    render_sliders(frame, chunks[1], state);

    // Frequency labels and help
    let help = Line::from(vec![
        Span::styled("  h/l", Style::default().fg(theme::BLUE)),
        Span::styled(":band  ", theme::dim()),
        Span::styled("j/k", Style::default().fg(theme::BLUE)),
        Span::styled(":gain  ", theme::dim()),
        Span::styled("b", Style::default().fg(theme::BLUE)),
        Span::styled(":bypass  ", theme::dim()),
        Span::styled("p", Style::default().fg(theme::BLUE)),
        Span::styled(":preset  ", theme::dim()),
        Span::styled("0", Style::default().fg(theme::BLUE)),
        Span::styled(":reset", theme::dim()),
    ]);
    frame.render_widget(Paragraph::new(vec![Line::from(""), help]), chunks[2]);
}

fn render_sliders(frame: &mut Frame, area: Rect, state: &AppState) {
    let height = area.height as usize;
    if height < 3 {
        return;
    }

    // Reserve 2 chars left margin, then divide remaining width among 10 bands
    let usable_width = area.width.saturating_sub(6) as usize; // 6 for dB label margin
    let band_width = (usable_width / 10).max(3);

    // The slider area: each row maps to a dB value from +12 (top) to -12 (bottom)
    // Middle row = 0 dB
    let slider_height = height.saturating_sub(1); // leave 1 row for freq labels
    if slider_height < 3 {
        return;
    }

    // Pre-compute constant threshold for zero-line detection
    let zero_threshold = MAX_DB / (slider_height as f64 - 1.0) * 0.6;

    // Build lines
    for row in 0..slider_height {
        let db_value = MAX_DB - (row as f64 / (slider_height - 1) as f64) * (MAX_DB * 2.0);

        let mut spans = Vec::new();

        // dB label on left (every few rows)
        let is_zero = db_value.abs() < zero_threshold;
        let is_top = row == 0;
        let is_bottom = row == slider_height - 1;
        let label = if is_top {
            format!("{:>4} ", MAX_DB as i32)
        } else if is_bottom {
            format!("{:>4} ", -(MAX_DB as i32))
        } else if is_zero {
            "   0 ".to_string()
        } else {
            "     ".to_string()
        };
        spans.push(Span::styled(label, theme::dim()));

        // Render each band column
        for band in 0..10 {
            let gain = state.eq.gains[band];
            let selected = band == state.eq.selected_band;

            // How many rows from center does this gain fill?
            let center_row = slider_height / 2;
            let gain_rows = ((gain / MAX_DB) * center_row as f64).round() as i32;

            let is_filled = if gain >= 0.0 {
                // Positive gain: fill from center upward
                let fill_top = (center_row as i32 - gain_rows).max(0) as usize;
                row >= fill_top && row <= center_row
            } else {
                // Negative gain: fill from center downward
                let fill_bottom = (center_row as i32 - gain_rows).min(slider_height as i32 - 1) as usize;
                row >= center_row && row <= fill_bottom
            };

            let is_center = row == center_row;

            // Choose character and color
            let (ch, style) = if is_filled && !state.eq.bypassed {
                let color = if gain > 6.0 {
                    theme::RED
                } else if gain > 0.0 {
                    theme::GREEN
                } else if gain > -6.0 {
                    theme::CYAN
                } else {
                    theme::BLUE
                };
                if selected {
                    ("█", Style::default().fg(color).bg(theme::BG_HIGHLIGHT))
                } else {
                    ("█", Style::default().fg(color))
                }
            } else if is_center {
                if selected {
                    ("─", Style::default().fg(theme::FG).bg(theme::BG_HIGHLIGHT))
                } else {
                    ("─", Style::default().fg(theme::BORDER))
                }
            } else if selected {
                ("│", Style::default().fg(theme::BORDER).bg(theme::BG_HIGHLIGHT))
                } else {
                ("·", Style::default().fg(theme::BG_HIGHLIGHT))
            };

            // Pad to band_width with the character in the center
            let pad_left = (band_width - 1) / 2;
            let pad_right = band_width - 1 - pad_left;

            let pad_style = if selected {
                Style::default().bg(theme::BG_HIGHLIGHT)
            } else {
                theme::base()
            };

            spans.push(Span::styled(" ".repeat(pad_left), pad_style));
            spans.push(Span::styled(ch, style));
            spans.push(Span::styled(" ".repeat(pad_right), pad_style));
        }

        let line = Line::from(spans);
        let line_area = Rect::new(area.x, area.y + row as u16, area.width, 1);
        frame.render_widget(Paragraph::new(line), line_area);
    }

    // Frequency labels row
    let mut freq_spans = Vec::new();
    freq_spans.push(Span::styled("     ", theme::dim())); // align with dB labels
    for (band, freq_label) in FREQ_LABELS.iter().enumerate() {
        let selected = band == state.eq.selected_band;
        let label = format!("{:^width$}", freq_label, width = band_width);
        let style = if selected {
            Style::default().fg(theme::BLUE).bg(theme::BG_HIGHLIGHT)
        } else {
            theme::dim()
        };
        freq_spans.push(Span::styled(label, style));
    }
    let freq_line = Line::from(freq_spans);
    let freq_area = Rect::new(area.x, area.y + slider_height as u16, area.width, 1);
    frame.render_widget(Paragraph::new(freq_line), freq_area);
}
