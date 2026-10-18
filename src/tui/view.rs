//! Drawing. nm paints every cell itself from the app
//! state; the editor crate draws nothing.
//!
//! Layout, top to bottom: content rows from the viewport start, blank rows,
//! then a one-row status bar. Each content row is the gutter (line number
//! right-aligned in `max(2, digits)` columns, a space, `│`, a space), the
//! syntax-highlighted line text in the rest of the left 60%, and the result
//! right-aligned in the right 40% with one trailing space.

use std::borrow::Cow;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::app::App;
use super::editor::{Mode, RowSelection};
use super::help;
use super::highlight::highlight_line;
use super::style;
use super::theme;
use crate::eval::format_number;
use crate::lang::parse_line;

/// Terminal cursor shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    /// Blinking bar, between characters (insert mode).
    Bar,
    /// Steady block, on a character (normal and visual).
    Block,
}

/// Where the terminal's own cursor goes. No cursor glyph is ever drawn
/// into the cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPlacement {
    pub x: u16,
    pub y: u16,
    pub shape: CursorShape,
}

/// Width of the line-number gutter, separator included: digits of the line
/// count (at least 2), then blank, `│`, blank.
pub fn gutter_width(line_count: usize) -> usize {
    line_count.to_string().len().max(2) + 3
}

/// Display width of a string in terminal cells.
fn text_width(s: &str) -> usize {
    Span::raw(s).width()
}

/// Draws the app into a frame and places the native cursor.
pub fn draw(frame: &mut Frame, app: &App) -> Option<CursorPlacement> {
    let area = frame.area();
    let placement = render(app, area, frame.buffer_mut());
    if let Some(p) = placement {
        frame.set_cursor_position(Position::new(p.x, p.y));
    }
    placement
}

/// Paints the app into `buf` within `area`. Returns where the cursor goes,
/// or `None` when it is off screen or the area is empty.
pub fn render(app: &App, area: Rect, buf: &mut Buffer) -> Option<CursorPlacement> {
    if area.width == 0 || area.height == 0 {
        return None;
    }
    let lines = app.lines();
    let (cursor_row, cursor_col) = app.cursor();
    let width = usize::from(area.width);
    let gutter = gutter_width(lines.len());
    let nr_width = gutter - 3;
    let input_width = width * 60 / 100;
    let content_width = input_width.saturating_sub(gutter);
    let result_width = width - input_width;
    let text_rows = usize::from(area.height - 1);
    let selection = app.editor.selection();

    let visible = lines
        .iter()
        .enumerate()
        .skip(app.viewport_start)
        .take(text_rows);
    for (screen_row, (i, line)) in visible.enumerate() {
        // `screen_row < text_rows < area.height`, so this fits in u16.
        let y = area.y + screen_row as u16;
        let x = area.x;

        let number = format!("{:>nr_width$}", i + 1);
        let (x, _) = buf.set_stringn(x, y, number, width, style::line_number(i == cursor_row));
        let (x, _) = buf.set_stringn(x, y, " ", width, Style::new());
        let (x, _) = buf.set_stringn(x, y, "│", width, style::separator());
        let (x, _) = buf.set_stringn(x, y, " ", width, Style::new());
        let room = (input_width.saturating_sub(usize::from(x - area.x))).min(content_width);
        let parsed = match app.parsed.get(i) {
            Some(p) if p.raw == *line => Cow::Borrowed(p),
            _ => Cow::Owned(parse_line(line)),
        };
        let mut spans = highlight_line(line, &parsed);
        if i == cursor_row
            && app.mode() == Mode::Insert
            && let Some(c) = &app.completion
        {
            spans = with_ghost(spans, line, cursor_col, c.ghost_text(), room);
        }
        let text = Line::from(spans);
        // `set_line` clips by display width, as `set_stringn` does.
        let (text_end, _) = buf.set_line(x, y, &text, u16::try_from(room).unwrap_or(u16::MAX));
        if let Some(sel) = selection.as_ref().and_then(|s| s.on_row(i)) {
            paint_selection(buf, line, sel, x, text_end, room, y);
        }

        if let Some((text, st)) = result_text(app, i) {
            render_result(buf, area, y, input_width, result_width, &text, st);
        }
    }

    render_status_bar(app, area, buf);
    if app.help {
        // The modal covers the text; no cursor while it is shown.
        help::render(area, buf);
        return None;
    }
    if let Some(command) = &app.command {
        return Some(command_cursor(area, command));
    }
    cursor_placement(app, area, gutter, cursor_row, cursor_col)
}

/// Paints the visual selection on one row over the text already drawn
/// from `x`. A whole row covers `room` cells; a character range covers the
/// cells of those characters, stopping at `text_end`, where the clipped
/// text stopped. Only the style changes, never the characters.
fn paint_selection(
    buf: &mut Buffer,
    line: &str,
    sel: RowSelection,
    x: u16,
    text_end: u16,
    room: usize,
    y: u16,
) {
    let (first, last) = match sel {
        RowSelection::Whole => {
            let w = u16::try_from(room).unwrap_or(u16::MAX);
            buf.set_style(Rect::new(x, y, w, 1), style::visual());
            return;
        }
        RowSelection::Chars { first, last } => (first, last),
    };
    // Cell offsets of the first selected character and of the cell after
    // the last one, by display width, as `set_line` laid them out.
    let mut at = 0usize;
    let mut from = None;
    let mut to = 0usize;
    for (col, c) in line.chars().enumerate() {
        if col > last {
            break;
        }
        let w = text_width(c.encode_utf8(&mut [0; 4]));
        if col >= first {
            from.get_or_insert(at);
            to = at + w;
        }
        at += w;
    }
    let Some(from) = from else {
        return;
    };
    let start = usize::from(x) + from;
    let end = (usize::from(x) + to).min(usize::from(text_end));
    if end > start {
        // Both are at most `text_end`, a u16.
        let rect = Rect::new(start as u16, y, (end - start) as u16, 1);
        buf.set_style(rect, style::visual());
    }
}

/// Puts `ghost` (the untyped rest of a completion) into the line's spans
/// at character column `col`, in the ghost style. The text after the
/// cursor moves right to make room. The ghost is clipped to what is left
/// of `room` cells after the real text, so the real text is never cut
/// for it. The native cursor stays at `col`, on
/// the ghost's first cell.
fn with_ghost(
    spans: Vec<Span<'static>>,
    line: &str,
    col: usize,
    ghost: &str,
    room: usize,
) -> Vec<Span<'static>> {
    let mut avail = room.saturating_sub(text_width(line));
    let ghost: String = ghost
        .chars()
        .take_while(|&c| {
            let w = text_width(c.encode_utf8(&mut [0; 4]));
            let fits = w <= avail;
            avail = avail.saturating_sub(w);
            fits
        })
        .collect();
    if ghost.is_empty() {
        return spans;
    }
    let mut out = Vec::with_capacity(spans.len() + 2);
    let mut ghost = Some(Span::styled(ghost, style::ghost()));
    let mut at = 0;
    for span in spans {
        let n = span.content.chars().count();
        if ghost.is_some() && col < at + n {
            let split = col - at;
            let head: String = span.content.chars().take(split).collect();
            let tail: String = span.content.chars().skip(split).collect();
            if !head.is_empty() {
                out.push(Span::styled(head, span.style));
            }
            out.extend(ghost.take());
            out.push(Span::styled(tail, span.style));
        } else {
            out.push(span);
        }
        at += n;
    }
    // Cursor at the end of the line.
    out.extend(ghost);
    out
}

/// In command mode the cursor sits after the typed command on the status
/// row (the bar has one cell of left padding), clamped to the last column.
fn command_cursor(area: Rect, command: &str) -> CursorPlacement {
    let x = (1 + 1 + text_width(command)).min(usize::from(area.width) - 1);
    CursorPlacement {
        // Below the area's u16 width.
        x: area.x + x as u16,
        y: area.bottom() - 1,
        shape: CursorShape::Bar,
    }
}

/// The text and style for line `i`'s result: the formatted value, the short
/// error code in red, or nothing.
fn result_text(app: &App, i: usize) -> Option<(String, Style)> {
    let r = app.results.get(i)?;
    if let Some(e) = &r.error {
        Some((e.short(), style::error()))
    } else {
        r.value.map(|v| (format_number(v), style::result()))
    }
}

/// Right-aligns `text` in the results column, leaving one trailing space.
fn render_result(
    buf: &mut Buffer,
    area: Rect,
    y: u16,
    input_width: usize,
    result_width: usize,
    text: &str,
    st: Style,
) {
    let room = result_width.saturating_sub(1);
    let w = text_width(text).min(room);
    let x = input_width + room - w;
    // `x < area.width`, which is a u16.
    buf.set_stringn(area.x + x as u16, y, text, w, st);
}

/// The status bar: mode badge, file or session indicator, section total and
/// line count, or a one-shot message instead.
fn render_status_bar(app: &App, area: Rect, buf: &mut Buffer) {
    let y = area.bottom() - 1;
    let row = Rect::new(area.x, y, area.width, 1);
    buf.set_style(row, style::status_bar());
    let width = usize::from(area.width);
    // One cell of padding on the left.
    let x = area.x + 1;
    let room = width.saturating_sub(1);

    if let Some(command) = &app.command {
        buf.set_stringn(x, y, format!(":{command}"), room, style::status_bar());
        return;
    }
    if let Some(msg) = &app.status_msg {
        buf.set_stringn(x, y, msg, room, style::status_bar());
        return;
    }

    let (label, bg) = badge(app.mode());
    let (x, _) = buf.set_stringn(x, y, format!(" {label} "), room, style::badge(bg));
    let used = usize::from(x - area.x);
    let rest = format!(
        "{} │ total: {} │ {} lines",
        file_indicator(app),
        format_number(app.total()),
        app.lines().len()
    );
    buf.set_stringn(x, y, rest, width.saturating_sub(used), style::status_bar());
}

fn badge(mode: Mode) -> (&'static str, ratatui::style::Color) {
    let t = theme::active();
    match mode {
        Mode::Normal => ("NORMAL", t.badge_normal),
        Mode::Insert => ("INSERT", t.badge_insert),
        Mode::Visual => ("VISUAL", t.badge_visual),
        Mode::VisualLine => ("VISUAL LINE", t.badge_visual),
        Mode::VisualBlock => ("VISUAL BLOCK", t.badge_visual),
    }
}

/// ` <path> [+]` for a named file, ` [session] [+]` when session content
/// exists, ` [+]` for a modified empty session, else nothing.
fn file_indicator(app: &App) -> String {
    let modified = if app.modified { " [+]" } else { "" };
    if let Some(path) = &app.file_path {
        format!(" {}{modified}", path.display())
    } else if app.session_mode && app.lines().iter().any(|l| !l.trim().is_empty()) {
        format!(" [session]{modified}")
    } else {
        modified.to_string()
    }
}

/// The native cursor: gutter plus the display width of the line up to the
/// cursor column. Bar in insert mode, block otherwise. Clamped to the last
/// column; `None` when the cursor row is scrolled off.
fn cursor_placement(
    app: &App,
    area: Rect,
    gutter: usize,
    cursor_row: usize,
    cursor_col: usize,
) -> Option<CursorPlacement> {
    let row = cursor_row.checked_sub(app.viewport_start)?;
    if row >= usize::from(area.height - 1) {
        return None;
    }
    let prefix: String = app.current_line().chars().take(cursor_col).collect();
    let x = (gutter + text_width(&prefix)).min(usize::from(area.width) - 1);
    let shape = if app.mode() == Mode::Insert {
        CursorShape::Bar
    } else {
        CursorShape::Block
    };
    Some(CursorPlacement {
        // Both are below the area's u16 width and height.
        x: area.x + x as u16,
        y: area.y + row as u16,
        shape,
    })
}
