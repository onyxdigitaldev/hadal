//! Spectrum analyzer widget — renders frequency bands as vertical bars.

use std::sync::Arc;

use hadal_dsp::VisualizationData;
use parking_lot::RwLock;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme;

/// Bar characters from empty to full (8 levels).
const BAR_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Get a color for a spectrum band based on its index in the frequency range.
/// Low frequencies → BLUE, mid → CYAN, high → GREEN.
fn band_color(band_index: usize, total_bands: usize) -> Color {
    if total_bands == 0 {
        return theme::BLUE;
    }
    let ratio = band_index as f32 / total_bands as f32;
    if ratio < 0.33 {
        theme::BLUE
    } else if ratio < 0.66 {
        theme::CYAN
    } else {
        theme::GREEN
    }
}

/// Render the spectrum analyzer into the given area.
pub fn render_spectrum(frame: &mut Frame, area: Rect, viz_data: &Arc<RwLock<VisualizationData>>) {
    if area.width < 4 || area.height < 2 {
        return;
    }

    let data = viz_data.read();
    let bands = &data.spectrum_bands;
    let peaks = &data.spectrum_peaks;

    if bands.is_empty() {
        return;
    }

    let display_width = area.width as usize;
    let display_height = area.height as usize;
    let num_bands = bands.len().min(display_width);

    // Each column represents one band — distribute evenly across width
    let bar_width = display_width / num_bands.max(1);
    let gap = if bar_width > 1 { 1 } else { 0 };
    let effective_bar = bar_width.saturating_sub(gap).max(1);

    let mut lines: Vec<Line> = Vec::with_capacity(display_height);

    for row in 0..display_height {
        let row_from_bottom = display_height - 1 - row;
        let row_threshold = row_from_bottom as f32 / display_height as f32;

        let mut spans: Vec<Span> = Vec::new();

        for (band_idx, &band_level) in bands.iter().enumerate().take(num_bands) {
            let level = band_level.clamp(0.0, 1.0);
            let peak = peaks.get(band_idx).copied().unwrap_or(0.0).clamp(0.0, 1.0);
            let color = band_color(band_idx, num_bands);

            if level > row_threshold {
                let cell_fill = (level - row_threshold) * display_height as f32;
                let char_idx = ((cell_fill * 8.0).ceil() as usize).clamp(1, 8) - 1;
                let ch = BAR_CHARS[char_idx];
                let bar_str = String::from(ch).repeat(effective_bar);
                spans.push(Span::styled(bar_str, Style::default().fg(color).bg(theme::BG)));
            } else if (peak - row_threshold).abs() < (1.0 / display_height as f32) && peak > 0.05 {
                let bar_str = "─".repeat(effective_bar);
                spans.push(Span::styled(
                    bar_str,
                    Style::default().fg(theme::FG_DARK).bg(theme::BG),
                ));
            } else {
                let bar_str = " ".repeat(effective_bar);
                spans.push(Span::styled(bar_str, Style::default().bg(theme::BG)));
            }

            if gap > 0 && band_idx + 1 < num_bands {
                spans.push(Span::styled(" ", Style::default().bg(theme::BG)));
            }
        }

        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines).style(theme::base()), area);
}
