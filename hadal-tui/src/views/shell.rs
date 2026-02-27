//! Title bar + tab bar + status bar shell that wraps view content.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::state::{AppState, ViewId};
use crate::theme;

/// Render the full shell: title bar, tab bar, content area, status bar.
pub fn render(frame: &mut Frame, state: &mut AppState) {
    let size = frame.area();

    // Clear background
    frame.render_widget(Block::default().style(theme::base()), size);

    let chunks = Layout::vertical([
        Constraint::Length(1), // title bar
        Constraint::Length(1), // tab bar
        Constraint::Min(1),   // content
        Constraint::Length(1), // status bar
    ])
    .split(size);

    render_title_bar(frame, chunks[0], state);
    render_tab_bar(frame, chunks[1], state);
    super::render_content(frame, chunks[2], state);
    // Re-borrow as immutable for remaining renders
    render_status_bar(frame, chunks[3], state);
}

fn render_title_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let left = Span::styled("hadal", theme::title());
    let left_sub = Span::styled(
        " | audiophile music player",
        theme::title().remove_modifier(Modifier::BOLD),
    );

    let right_text = if let Some(fmt) = &state.playback.source_format {
        format!(
            "{} | {}kHz/{}bit | {}%",
            fmt.codec.display_name(),
            format_sample_rate(fmt.sample_rate),
            fmt.bit_depth.bits(),
            (state.playback.volume * 100.0) as u32
        )
    } else {
        format!("-- | --kHz/--bit | {}%", (state.playback.volume * 100.0) as u32)
    };

    let right = Span::styled(right_text, theme::title().remove_modifier(Modifier::BOLD));

    // Calculate padding
    let left_len = 5 + " | audiophile music player".len();
    let right_len = right.width();
    let padding = area.width as usize - left_len.min(area.width as usize) - right_len.min(area.width as usize);

    let line = Line::from(vec![
        left,
        left_sub,
        Span::styled(" ".repeat(padding.max(1)), theme::title()),
        right,
    ]);

    frame.render_widget(
        Paragraph::new(line).style(theme::title().remove_modifier(Modifier::BOLD)),
        area,
    );
}

fn render_tab_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let views = [
        ViewId::Library,
        ViewId::NowPlaying,
        ViewId::Queue,
        ViewId::Equalizer,
        ViewId::Search,
        ViewId::Playlists,
    ];

    let mut spans = Vec::new();
    spans.push(Span::styled(" ", theme::base()));

    for (i, view) in views.iter().enumerate() {
        let label = format!("{}:{}", i + 1, view.label());
        let style = if *view == state.active_view {
            theme::tab_active()
        } else {
            theme::tab_inactive()
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::styled("  ", theme::base()));
    }

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line).style(theme::base()), area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    // Show status message if active, otherwise show key hints
    let left_text = if let Some((msg, _)) = &state.status_message {
        format!(" {}", msg)
    } else {
        let keyhints = match state.active_view {
            ViewId::Library => "hjkl:navigate  Enter:play  a:queue  /:search",
            ViewId::NowPlaying => "Space:pause  <>:seek  +-:vol  m:mute",
            ViewId::Queue => "jk:navigate  d:remove  JK:reorder",
            ViewId::Equalizer => "hl:band  jk:gain  b:bypass  p:preset  0:reset",
            ViewId::Search => "Type to search  Enter:select  Esc:close",
            ViewId::Playlists => "hl:panes  n:new  d:delete  r:rename  x:remove track  Enter:play",
        };
        format!(" {}", keyhints)
    };

    let left_style = if state.status_message.is_some() {
        ratatui::style::Style::default().fg(theme::GREEN).bg(theme::BG_DARK)
    } else {
        theme::status_bar()
    };

    let left = Span::styled(&left_text, left_style);
    let right = Span::styled(
        format!("hadal v{} ", env!("CARGO_PKG_VERSION")),
        theme::status_bar(),
    );

    let padding = (area.width as usize)
        .saturating_sub(left_text.len())
        .saturating_sub(right.width());

    let line = Line::from(vec![
        left,
        Span::styled(" ".repeat(padding), theme::status_bar()),
        right,
    ]);

    frame.render_widget(Paragraph::new(line).style(theme::status_bar()), area);
}

/// Format sample rate for display (e.g. 44100 → "44.1", 96000 → "96", 192000 → "192").
fn format_sample_rate(rate: u32) -> String {
    let khz = rate / 1000;
    let remainder = (rate % 1000) / 100;
    if remainder == 0 {
        format!("{}", khz)
    } else {
        format!("{}.{}", khz, remainder)
    }
}
