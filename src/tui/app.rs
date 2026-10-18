//! Application state: the editor, the parsed and evaluated lines, the
//! viewport, and the file and status fields the status bar shows. Every
//! key goes through [`App::handle_key`]; after it, lines are re-parsed and
//! re-evaluated if the text changed.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::completion::Completion;
use super::editor::{Editor, Mode};
use super::keys::Finds;
use crate::eval::{Evaluator, LineResult, section_total};
use crate::lang::{ParsedLine, parse_all};

pub struct App {
    pub(crate) editor: Editor,
    /// Cached copy of the editor's lines, refreshed after a key that may
    /// have changed them.
    lines: Vec<String>,
    /// [`Editor::generation`] when `lines` was last refreshed.
    generation: u64,
    pub(crate) parsed: Vec<ParsedLine>,
    pub(crate) results: Vec<LineResult>,
    evaluator: Evaluator,
    /// First buffer line shown on screen.
    pub(crate) viewport_start: usize,
    /// Terminal size from the last [`App::set_size`].
    width: u16,
    height: u16,
    pub(crate) file_path: Option<PathBuf>,
    /// Working without a file: content is the auto-saved session.
    pub(crate) session_mode: bool,
    /// The text changed since the last load or successful write.
    pub(crate) modified: bool,
    /// Where session mode saves the buffer on exit. `None` saves nothing
    /// (tests, or no home directory).
    pub(crate) session_path: Option<PathBuf>,
    /// The command line being typed after `:`, while command mode is open.
    pub(crate) command: Option<String>,
    /// The help modal is shown.
    pub(crate) help: bool,
    /// `Z` was typed in normal mode; the next key completes `ZZ` or `ZQ`.
    pub(super) pending_z: bool,
    /// One-shot message that replaces the status bar until the next key.
    pub(crate) status_msg: Option<String>,
    pub(super) should_quit: bool,
    /// Find-motion state for the key handling in `keys.rs`.
    pub(super) finds: Finds,
    /// The variable completion offered at the cursor (insert mode only).
    pub(crate) completion: Option<Completion>,
}

impl App {
    /// An app holding `lines` (one empty line if `lines` is empty), cursor
    /// at the origin, normal mode, unmodified.
    pub fn new(lines: Vec<String>, file_path: Option<PathBuf>, session_mode: bool) -> Self {
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        let editor = Editor::new(&lines);
        let mut app = Self {
            lines: editor.lines(),
            generation: editor.generation(),
            editor,
            parsed: Vec::new(),
            results: Vec::new(),
            evaluator: Evaluator::new(),
            viewport_start: 0,
            width: 0,
            height: 0,
            file_path,
            session_mode,
            modified: false,
            session_path: None,
            command: None,
            help: false,
            pending_z: false,
            status_msg: None,
            should_quit: false,
            finds: Finds::default(),
            completion: None,
        };
        app.evaluate();
        app
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The lines joined with `\n`.
    #[cfg(test)]
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    /// The line under the cursor.
    pub fn current_line(&self) -> &str {
        self.lines.get(self.cursor().0).map_or("", String::as_str)
    }

    /// `(row, col)`; `col` counts characters.
    pub fn cursor(&self) -> (usize, usize) {
        self.editor.cursor()
    }

    pub fn mode(&self) -> Mode {
        self.editor.mode()
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// The status-bar total for the section under the cursor.
    pub fn total(&self) -> f64 {
        section_total(&self.results, &self.parsed, self.cursor().0)
    }

    /// Records the terminal size and scrolls so the cursor stays visible.
    pub fn set_size(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.ensure_cursor_visible();
    }

    #[cfg(test)]
    pub fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// Handles one key event. Only presses count; releases and repeats
    /// reported by some terminals are ignored.
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        self.status_msg = None;

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        if self.help {
            // Any key closes the help modal and does nothing else.
            self.help = false;
            return;
        }
        if ctrl && key.code == KeyCode::Char('s') {
            // Saves from any mode; an open command line is dropped so the
            // result shows.
            self.command = None;
            self.write(None);
            return;
        }
        if self.command.is_some() {
            self.command_key(key);
            self.after_key();
            return;
        }

        let refresh = match self.mode() {
            Mode::Insert => self.insert_key(key),
            Mode::Normal => {
                self.normal_key(key);
                false
            }
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
                self.visual_key(key);
                false
            }
        };
        self.after_key();
        // Every insert-mode key but the completion keys
        // recomputes the completion (the lines are re-parsed by now).
        // Entering insert mode is not an insert-mode key, so `i` on a
        // partial word shows nothing yet; leaving insert mode clears it.
        if refresh || self.mode() != Mode::Insert {
            self.refresh_completion();
        }
    }

    /// Picks up text changes, re-evaluates, and keeps the cursor on screen.
    fn after_key(&mut self) {
        let generation = self.editor.generation();
        if generation != self.generation {
            self.generation = generation;
            let lines = self.editor.lines();
            if lines != self.lines {
                self.lines = lines;
                self.modified = true;
                self.evaluate();
            }
        }
        self.ensure_cursor_visible();
    }

    /// Replaces the whole buffer with loaded lines: cursor to the top,
    /// undo history cleared, not modified.
    pub(super) fn replace_lines(&mut self, lines: &[String]) {
        self.editor.set_lines(lines);
        self.generation = self.editor.generation();
        self.lines = self.editor.lines();
        self.modified = false;
        self.viewport_start = 0;
        self.evaluate();
    }

    fn evaluate(&mut self) {
        self.parsed = parse_all(&self.lines);
        self.results = self.evaluator.evaluate_all(&self.parsed);
    }

    fn ensure_cursor_visible(&mut self) {
        self.viewport_start = scroll_to_cursor(self.cursor().0, self.viewport_start, self.height);
    }
}

/// The new viewport start that keeps `cursor_row` visible on a screen of
/// `height` rows, one of which is the status bar.
/// A zero height leaves the viewport alone.
pub fn scroll_to_cursor(cursor_row: usize, viewport_start: usize, height: u16) -> usize {
    if height == 0 {
        return viewport_start;
    }
    let visible = usize::from(height - 1).max(1);
    if cursor_row < viewport_start {
        cursor_row
    } else if cursor_row >= viewport_start + visible {
        cursor_row + 1 - visible
    } else {
        viewport_start
    }
}
