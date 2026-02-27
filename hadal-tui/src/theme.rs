//! Tokyo Night color theme for Hadal TUI.

use ratatui::style::{Color, Modifier, Style};

// ─────────────────────────────────────────────────────────────────────────────
// Tokyo Night palette
// ─────────────────────────────────────────────────────────────────────────────

pub const BG: Color = Color::Rgb(26, 27, 38);
pub const BG_DARK: Color = Color::Rgb(22, 23, 33);
pub const BG_HIGHLIGHT: Color = Color::Rgb(41, 46, 66);
pub const FG: Color = Color::Rgb(192, 202, 245);
pub const FG_DARK: Color = Color::Rgb(86, 95, 137);
pub const COMMENT: Color = Color::Rgb(86, 95, 137);
pub const BLUE: Color = Color::Rgb(122, 162, 247);
pub const CYAN: Color = Color::Rgb(125, 207, 255);
pub const GREEN: Color = Color::Rgb(158, 206, 106);
pub const MAGENTA: Color = Color::Rgb(187, 154, 247);
pub const RED: Color = Color::Rgb(247, 118, 142);
pub const YELLOW: Color = Color::Rgb(224, 175, 104);
pub const ORANGE: Color = Color::Rgb(255, 158, 100);
pub const BORDER: Color = Color::Rgb(41, 46, 66);
pub const BORDER_FOCUSED: Color = Color::Rgb(122, 162, 247);

// ─────────────────────────────────────────────────────────────────────────────
// Style helpers
// ─────────────────────────────────────────────────────────────────────────────

pub fn base() -> Style {
    Style::default().fg(FG).bg(BG)
}

pub fn title() -> Style {
    Style::default().fg(BLUE).bg(BG_DARK).add_modifier(Modifier::BOLD)
}

pub fn tab_active() -> Style {
    Style::default()
        .fg(BLUE)
        .bg(BG)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

pub fn tab_inactive() -> Style {
    Style::default().fg(FG_DARK).bg(BG)
}

pub fn status_bar() -> Style {
    Style::default().fg(FG_DARK).bg(BG_DARK)
}

pub fn selected() -> Style {
    Style::default().fg(FG).bg(BG_HIGHLIGHT)
}

pub fn playing() -> Style {
    Style::default().fg(GREEN)
}

pub fn playing_selected() -> Style {
    Style::default().fg(GREEN).bg(BG_HIGHLIGHT)
}

pub fn header() -> Style {
    Style::default()
        .fg(MAGENTA)
        .add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::default().fg(FG_DARK)
}

pub fn border() -> Style {
    Style::default().fg(BORDER)
}

pub fn border_focused() -> Style {
    Style::default().fg(BORDER_FOCUSED)
}

pub fn error() -> Style {
    Style::default().fg(RED)
}

pub fn search_highlight() -> Style {
    Style::default().fg(BG).bg(YELLOW)
}

pub fn progress_bar() -> Style {
    Style::default().fg(BLUE).bg(BG_HIGHLIGHT)
}

pub fn volume() -> Style {
    Style::default().fg(CYAN)
}
