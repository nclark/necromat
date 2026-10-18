//! The styles every renderer paints with, and the Tokyo Night colours.
//!
//! The constants are the Tokyo Night palette, the built-in default
//! ([`Theme::tokyo_night`](super::theme::Theme::tokyo_night)). The style
//! functions read the active theme ([`theme::active`]), so a user or
//! Omarchy theme changes them all.

use ratatui::style::{Color, Modifier, Style};

use super::theme::{self, Theme};

pub const NUMBER: Color = Color::Rgb(0x7D, 0xCF, 0xFF);
pub const OPERATOR: Color = Color::Rgb(0xBB, 0x9A, 0xF7);
pub const VARIABLE: Color = Color::Rgb(0x9E, 0xCE, 0x6A);
pub const COMMENT: Color = Color::Rgb(0x56, 0x5F, 0x89);
/// The `sum` keyword shares the error red.
pub const KEYWORD: Color = ERROR;
pub const ERROR: Color = Color::Rgb(0xF7, 0x76, 0x8E);
pub const RESULT: Color = Color::Rgb(0xA9, 0xB1, 0xD6);
pub const STATUS_BG: Color = Color::Rgb(0x1A, 0x1B, 0x26);
pub const STATUS_FG: Color = Color::Rgb(0xA9, 0xB1, 0xD6);
pub const BADGE_NORMAL: Color = Color::Rgb(0x7A, 0xA2, 0xF7);
pub const BADGE_INSERT: Color = Color::Rgb(0x9E, 0xCE, 0x6A);
pub const BADGE_VISUAL: Color = Color::Rgb(0xFF, 0x9E, 0x64);
pub const GUTTER: Color = Color::Rgb(0x66, 0x66, 0x66);
pub const CURRENT_LINE_NR: Color = Color::Rgb(0xE0, 0xAF, 0x68);
pub const SEPARATOR: Color = Color::Rgb(0x3B, 0x42, 0x61);
pub const GHOST: Color = Color::Rgb(0x56, 0x5F, 0x89);
pub const VISUAL_BG: Color = Color::Rgb(0x3B, 0x42, 0x61);
pub const VISUAL_FG: Color = Color::Rgb(0xE0, 0xAF, 0x68);

fn t() -> Theme {
    theme::active()
}

pub fn number() -> Style {
    Style::new().fg(t().number)
}

pub fn operator() -> Style {
    Style::new().fg(t().operator)
}

pub fn variable() -> Style {
    Style::new().fg(t().variable)
}

pub fn comment() -> Style {
    Style::new().fg(t().comment).add_modifier(Modifier::ITALIC)
}

pub fn keyword() -> Style {
    Style::new().fg(t().keyword)
}

/// Completion ghost text.
pub fn ghost() -> Style {
    Style::new().fg(t().ghost)
}

/// The visual selection.
pub fn line_number(current: bool) -> Style {
    if current {
        Style::new()
            .fg(t().current_line_nr)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(t().gutter)
    }
}

pub fn separator() -> Style {
    Style::new().fg(t().separator)
}

pub fn result() -> Style {
    Style::new().fg(t().result)
}

pub fn error() -> Style {
    Style::new().fg(t().error)
}

pub fn status_bar() -> Style {
    let t = t();
    Style::new().bg(t.status_bg).fg(t.status_fg)
}

pub fn badge(bg: Color) -> Style {
    Style::new()
        .bg(bg)
        .fg(t().status_bg)
        .add_modifier(Modifier::BOLD)
}

/// The visual-mode selection.
#[allow(dead_code)]
pub fn visual() -> Style {
    let t = t();
    Style::new().bg(t.visual_bg).fg(t.visual_fg)
}
