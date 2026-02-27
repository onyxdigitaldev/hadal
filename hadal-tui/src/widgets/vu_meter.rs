//! VU meter widget — horizontal level bars for left and right channels.

use std::sync::Arc;

use hadal_dsp::VisualizationData;
use parking_lot::RwLock;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme;

/// Render the VU meter into the given area (needs at least 2 rows).
pub fn render_vu_meter(frame: &mut Frame, area: Rect, viz_data: &Arc<RwLock<VisualizationData>>) {
    if area.width < 10 || area.height < 2 {
        return;
    }

    let data = viz_data.read();
    let vu = &data.vu_meter;

    let meter_width = (area.width as usize).saturating_sub(14); // "L ████ -XX.XdB"

    let left_line = build_meter_line('L', vu.level_left, vu.db_left, vu.clipping_left, meter_width);
    let right_line = build_meter_line('R', vu.level_right, vu.db_right, vu.clipping_right, meter_width);

    let lines = vec![left_line, right_line];
    frame.render_widget(Paragraph::new(lines).style(theme::base()), area);
}

/// Build a single channel meter line: "L ████████░░░░░ -6.2dB" or "R ████ CLIP"
fn build_meter_line(
    channel: char,
    level: f32,
    db: f32,
    clipping: bool,
    meter_width: usize,
) -> Line<'static> {
    let filled = ((level.clamp(0.0, 1.0) * meter_width as f32) as usize).min(meter_width);
    let empty = meter_width - filled;

    let mut spans = Vec::new();
    spans.push(Span::styled(
        format!("  {} ", channel),
        Style::default().fg(theme::FG_DARK),
    ));

    // Build the bar with color segments
    // Map level to approximate dB thresholds for coloring:
    // green < -6dB (~0.5 linear), yellow -6 to -3dB (~0.5–0.7), red > -3dB (~0.7+)
    let green_end = (0.5 * meter_width as f32) as usize;
    let yellow_end = (0.707 * meter_width as f32) as usize;

    for i in 0..filled {
        let color = if i < green_end {
            theme::GREEN
        } else if i < yellow_end {
            theme::YELLOW
        } else {
            theme::RED
        };
        spans.push(Span::styled("█", Style::default().fg(color)));
    }

    // Empty portion
    if empty > 0 {
        let empty_str = "░".repeat(empty);
        spans.push(Span::styled(empty_str, Style::default().fg(theme::BG_HIGHLIGHT)));
    }

    // dB readout or CLIP indicator
    if clipping {
        spans.push(Span::styled(" CLIP", Style::default().fg(theme::RED)));
    } else {
        spans.push(Span::styled(
            format!(" {:>5.1}dB", db),
            Style::default().fg(theme::FG_DARK),
        ));
    }

    Line::from(spans)
}
