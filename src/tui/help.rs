//! The help modal (`?`): a centred rounded box listing the main keys by
//! mode, in two columns so it fits an 80x24 terminal (one tall column
//! would run off a 24-row screen).
//!
//! The text is a short pick from `doc/keys.md`, kept in sync by hand; a
//! test checks that every key named here is also in that file.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Widget};

use super::theme;

/// One row of a column: a section header, or keys and what they do.
#[derive(Debug, Clone, Copy)]
pub enum Row {
    Header(&'static str),
    Key(Group, &'static str, &'static str),
}

/// Which colour a key gets.
#[derive(Debug, Clone, Copy)]
pub enum Group {
    Normal,
    Insert,
    Command,
}

use Group::{Command, Insert, Normal};
use Row::{Header, Key};

pub const TITLE: &str = "nm Keybindings";
pub const FOOTER: &str = "Press any key to close";

pub const LEFT: &[Row] = &[
    Header("NORMAL MODE"),
    Key(Normal, "h j k l", "Move"),
    Key(Normal, "w b e", "Word motions"),
    Key(Normal, "0 ^ $", "Start/first char/end"),
    Key(Normal, "gg G 3G", "Top/bottom/Go to line 3"),
    Key(Normal, "f t ; ,", "Find character/repeat"),
    Key(Normal, "i a", "Insert before/after"),
    Key(Normal, "I A o O", "Insert at ends/new line"),
    Key(Normal, "x X dd", "Delete char/before/line"),
    Key(Normal, "D C", "Delete/change to end"),
    Key(Normal, "J r ~ .", "Join/replace/case/repeat"),
    Key(Normal, "p P", "Paste after/before"),
    Key(Normal, "u Ctrl+R", "Undo/redo"),
    Key(Normal, "v V", "Visual mode (char/line)"),
    Key(Normal, "yy yr", "Yank line/result"),
    Key(Normal, ":", "Command mode"),
    Key(Normal, "?", "Show this help"),
    Key(Normal, "q", "Quit"),
];

pub const RIGHT: &[Row] = &[
    Header("COUNTS & OPERATORS"),
    Key(Normal, "3j 2dd", "Count repeats a command"),
    Key(Normal, "d c y", "Delete/change/yank + motion"),
    Key(Normal, "iw i(", "Word/parentheses objects"),
    Key(Normal, "Esc", "Cancel count/operator"),
    Header("VISUAL MODE"),
    Key(Normal, "y d c", "Yank/delete/change"),
    Key(Normal, "Esc", "Cancel visual mode"),
    Header("INSERT MODE"),
    Key(Insert, "Esc", "Return to normal mode"),
    Key(Insert, "Tab", "Accept completion"),
    Key(Insert, "Ctrl+N Ctrl+P", "Next/previous match"),
    Header("COMMANDS"),
    Key(Command, ":w [file]", "Save (as file)"),
    Key(Command, ":e file", "Open file (:e! discards)"),
    Key(Command, ":q", "Quit"),
    Key(Command, ":q! ZQ", "Quit, discard changes"),
    Key(Command, ":wq :x ZZ", "Save and quit"),
    Key(Command, "Ctrl+S", "Save from any mode"),
];

/// Width of the key column, its trailing space included. Longer keys push
/// their text right, keeping one space.
const KEY_WIDTH: usize = 10;
/// Blank cells between the two columns.
const GAP: usize = 2;
/// Blank cells inside the border on the left and right.
const PAD: usize = 1;

fn row_width(row: &Row) -> usize {
    match row {
        Header(h) => h.chars().count(),
        Key(_, keys, what) => key_width(keys) + what.chars().count(),
    }
}

/// Cells the keys take, their trailing space included.
fn key_width(keys: &str) -> usize {
    KEY_WIDTH.max(keys.chars().count() + 1)
}

fn column_width(rows: &[Row]) -> usize {
    rows.iter().map(row_width).max().unwrap_or(0)
}

fn render_row(row: &Row, width: usize) -> Line<'static> {
    match *row {
        Header(h) => Line::from(Span::styled(h, Style::new().add_modifier(Modifier::BOLD))),
        Key(group, keys, what) => {
            let fg = match group {
                Normal => theme::active().badge_visual,
                Insert => theme::active().badge_insert,
                Command => theme::active().number,
            };
            let key_style = Style::new().fg(fg).add_modifier(Modifier::BOLD);
            let pad = key_width(keys) - keys.chars().count();
            let what = format!("{:<w$}", what, w = width.saturating_sub(key_width(keys)));
            Line::from(vec![
                Span::styled(keys, key_style),
                Span::raw(" ".repeat(pad)),
                Span::raw(what),
            ])
        }
    }
}

/// Draws the modal centred in `area`, over whatever is there. A terminal
/// too small for it shows the top-left part.
pub fn render(area: Rect, buf: &mut Buffer) {
    let left_w = column_width(LEFT);
    let right_w = column_width(RIGHT);
    let body_rows = LEFT.len().max(RIGHT.len());
    // Border, padding, columns.
    let width = 2 + 2 * PAD + left_w + GAP + right_w;
    // Border, title, body, blank, footer.
    let height = 2 + 1 + body_rows + 1 + 1;
    let w = u16::try_from(width).unwrap_or(u16::MAX).min(area.width);
    let h = u16::try_from(height).unwrap_or(u16::MAX).min(area.height);
    let rect = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    Clear.render(rect, buf);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme::active().badge_normal));
    let inner = block.inner(rect);
    block.render(rect, buf);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::with_capacity(body_rows + 3);
    lines.push(
        Line::from(Span::styled(
            TITLE,
            Style::new().add_modifier(Modifier::BOLD),
        ))
        .centered(),
    );
    for i in 0..body_rows {
        let mut spans = vec![Span::raw(" ".repeat(PAD))];
        let left = LEFT
            .get(i)
            .map_or_else(|| Line::from(" ".repeat(left_w)), |r| render_row(r, left_w));
        let left_len = left.width();
        spans.extend(left.spans);
        spans.push(Span::raw(" ".repeat(left_w.saturating_sub(left_len) + GAP)));
        if let Some(r) = RIGHT.get(i) {
            spans.extend(render_row(r, right_w).spans);
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::default());
    lines.push(
        Line::from(Span::styled(
            FOOTER,
            Style::new().fg(theme::active().comment),
        ))
        .centered(),
    );

    for (i, line) in lines.iter().enumerate() {
        let Ok(dy) = u16::try_from(i) else { break };
        if dy >= inner.height {
            break;
        }
        line.render(Rect::new(inner.x, inner.y + dy, inner.width, 1), buf);
    }
}
