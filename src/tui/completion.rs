//! Variable completion in insert mode.
//!
//! After each insert-mode key the app looks at the identifier left of the
//! cursor and offers the variables assigned on lines above, nearest
//! first, then `sum`. The rest of the selected match is drawn as ghost
//! text at the cursor (`view.rs`); `Tab` accepts it, `Ctrl+N`/`Ctrl+P`
//! cycle. Leaving insert mode clears it.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::App;
use super::editor::Mode;
use crate::lang::{LineKind, ParsedLine, is_ident_continue, is_ident_start};

/// The `sum` keyword, always the last candidate.
const SUM: &str = "sum";

/// Completion candidates for the cursor on line `cursor_line`: the names
/// assigned on the lines above it, nearest first and without repeats,
/// then `sum`. Lines that are not assignments (errors included) are
/// skipped, and a variable named `sum` is never offered as a variable.
pub fn candidates(parsed: &[ParsedLine], cursor_line: usize) -> Vec<String> {
    let end = cursor_line.min(parsed.len());
    let mut out: Vec<String> = Vec::new();
    for line in parsed[..end].iter().rev() {
        if let LineKind::Assignment { name, .. } = &line.kind
            && !name.is_empty()
            && name != SUM
            && !out.contains(name)
        {
            out.push(name.clone());
        }
    }
    out.push(SUM.to_string());
    out
}

/// The identifier being typed: the identifier characters just left of
/// `cursor_col` (a character index, clamped to the line), and the column
/// where they start.
///
/// `None` when there is nothing to complete:
/// - no identifier characters left of the cursor, or they start with a
///   digit (`5x`);
/// - a `#` left of the cursor (inside a comment);
/// - the cursor at or left of the first `=` (the name being assigned);
/// - an identifier character under the cursor, so the cursor is inside a
///   word, not at its end.
pub fn prefix(line: &str, cursor_col: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = line.chars().collect();
    let col = cursor_col.min(chars.len());
    if chars[..col].contains(&'#') {
        return None;
    }
    if let Some(eq) = chars.iter().position(|&c| c == '=')
        && col <= eq
    {
        return None;
    }
    if chars.get(col).copied().is_some_and(is_ident_continue) {
        return None;
    }
    let mut start = col;
    while start > 0 && is_ident_continue(chars[start - 1]) {
        start -= 1;
    }
    if start == col || !is_ident_start(chars[start]) {
        return None;
    }
    Some((chars[start..col].iter().collect(), start))
}

/// An active completion: at least one match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// Candidates that extend the prefix, in candidate order. Never empty.
    pub matches: Vec<String>,
    /// Index into `matches`.
    pub selected: usize,
    /// The identifier typed so far.
    pub prefix: String,
    /// Character column where the prefix starts.
    pub start: usize,
}

impl Completion {
    /// The completion for the cursor at `(row, col)` on `line`, or `None`
    /// when there is no prefix or no candidate extends it. A candidate
    /// equal to the prefix has nothing left to add and does not count.
    pub fn refresh(parsed: &[ParsedLine], line: &str, row: usize, col: usize) -> Option<Self> {
        let (prefix, start) = prefix(line, col)?;
        let matches: Vec<String> = candidates(parsed, row)
            .into_iter()
            .filter(|c| c.len() > prefix.len() && c.starts_with(&prefix))
            .collect();
        if matches.is_empty() {
            return None;
        }
        Some(Self {
            matches,
            selected: 0,
            prefix,
            start,
        })
    }

    /// The selected match.
    pub fn current(&self) -> &str {
        &self.matches[self.selected]
    }

    /// Moves the selection by `delta`, wrapping at both ends.
    pub fn cycle(&mut self, delta: isize) {
        let n = self.matches.len() as isize;
        // `rem_euclid` keeps the result in `0..n`.
        self.selected = (self.selected as isize + delta).rem_euclid(n) as usize;
    }

    /// The untyped rest of the selected match.
    pub fn ghost_text(&self) -> &str {
        // Every match starts with the prefix, so this is a char boundary.
        &self.current()[self.prefix.len()..]
    }
}

impl App {
    /// Recomputes the completion for the cursor. Only insert mode has one.
    pub(super) fn refresh_completion(&mut self) {
        self.completion = if self.mode() == Mode::Insert {
            let (row, col) = self.cursor();
            Completion::refresh(&self.parsed, self.current_line(), row, col)
        } else {
            None
        };
    }

    /// `Tab`: replaces the prefix with the selected match and leaves the
    /// cursor after it. Does nothing (and inserts no tab) when there is no
    /// completion.
    ///
    /// The prefix is a proper prefix of the match, so typing the ghost
    /// text gives the same line as deleting the prefix and typing the
    /// match, and it stays part of the same insert session (one undo
    /// step).
    pub(super) fn accept_completion(&mut self) {
        let Some(c) = self.completion.take() else {
            return;
        };
        for ch in c.ghost_text().chars() {
            let mods = if ch.is_uppercase() {
                KeyModifiers::SHIFT
            } else {
                KeyModifiers::NONE
            };
            self.editor.send(KeyEvent::new(KeyCode::Char(ch), mods));
        }
    }

    /// `Ctrl+N` (`delta` 1) and `Ctrl+P` (-1).
    pub(super) fn cycle_completion(&mut self, delta: isize) {
        if let Some(c) = &mut self.completion {
            c.cycle(delta);
        }
    }
}
