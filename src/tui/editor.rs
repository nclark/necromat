//! The editor core: hjkl-engine, hjkl-buffer and hjkl-vim behind a small
//! nm-owned surface. This is the only module that names hjkl types, so a
//! crate upgrade or a swap touches one file (see `doc/design/editor.md`).
//!
//! The engine owns the buffer, cursor, modes, motions, registers and undo.
//! It draws nothing; `view.rs` reads the state from here and paints every
//! cell itself.

use std::sync::Arc;

use hjkl_engine::types::{CursorShape, Host, Viewport};
use hjkl_engine::{DefaultHost, Input, Key as HKey, Options, Slot, VimMode};
use hjkl_vim::VimEditorExt;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::clipboard::{Clipboard, MemoryClipboard};

/// The editor modes nm shows in the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    VisualBlock,
}

/// The visual selection. Rows and columns are zero-based; columns count
/// characters, never bytes, and every range is inclusive at both ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// Charwise (`v`): from `start` to `end` in reading order, the
    /// character under the cursor included.
    Chars {
        start: (usize, usize),
        end: (usize, usize),
    },
    /// Linewise (`V`): whole lines `top..=bottom`.
    Lines { top: usize, bottom: usize },
    /// Blockwise (`Ctrl+V`): columns `left..=right` of rows
    /// `top..=bottom`. With `to_eol` (after `$`) each row runs to its own
    /// end instead of stopping at `right`.
    Block {
        top: usize,
        bottom: usize,
        left: usize,
        right: usize,
        to_eol: bool,
    },
}

/// What a selection covers on one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowSelection {
    /// Characters `first..=last` (`last` may pass the end of the line).
    Chars { first: usize, last: usize },
    /// The whole row, the space after the text included.
    Whole,
}

impl Selection {
    /// What the selection covers on `row`, if anything.
    pub fn on_row(&self, row: usize) -> Option<RowSelection> {
        match *self {
            Self::Chars { start, end } => {
                if row < start.0 || row > end.0 {
                    return None;
                }
                let first = if row == start.0 { start.1 } else { 0 };
                let last = if row == end.0 { end.1 } else { usize::MAX };
                Some(RowSelection::Chars { first, last })
            }
            Self::Lines { top, bottom } => {
                (top..=bottom).contains(&row).then_some(RowSelection::Whole)
            }
            Self::Block {
                top,
                bottom,
                left,
                right,
                to_eol,
            } => (top..=bottom)
                .contains(&row)
                .then_some(RowSelection::Chars {
                    first: left,
                    last: if to_eol { usize::MAX } else { right },
                }),
        }
    }
}

/// A vim editor over a list of lines. Cursor columns are character
/// indices, never bytes.
pub struct Editor {
    inner: hjkl_engine::Editor<hjkl_buffer::View, NmHost>,
    /// The text and undo depth when the current insert session began.
    /// See [`Editor::drop_empty_undo_steps`].
    session_start: Option<(Arc<String>, usize)>,
}

/// The engine's view of the outside world.
///
/// hjkl hands every yank **and** every delete to
/// [`Host::write_clipboard`], but in nm only `y` may reach
/// the system clipboard. So the host never writes the clipboard itself: it
/// only notes that the engine filled its register, and the key code
/// (`keys.rs`) decides whether that key was a yank. Reads go straight to
/// the clipboard (hjkl reads it only for `"+p`).
struct NmHost {
    clipboard: Box<dyn Clipboard>,
    /// Set when the engine stored yanked or deleted text since the last
    /// [`Editor::send`].
    stored: bool,
    started: std::time::Instant,
    viewport: Viewport,
}

impl Host for NmHost {
    type Intent = ();

    fn write_clipboard(&mut self, _text: String) {
        self.stored = true;
    }

    fn read_clipboard(&mut self) -> Option<String> {
        self.clipboard.get()
    }

    fn now(&self) -> std::time::Duration {
        self.started.elapsed()
    }

    fn prompt_search(&mut self) -> Option<String> {
        None
    }

    fn emit_cursor_shape(&mut self, _shape: CursorShape) {}

    fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
    }

    fn emit_intent(&mut self, _intent: ()) {}
}

/// The engine options nm needs. hjkl defaults to an IDE-like setup; a
/// calculator pad wants plain vim.
fn options() -> Options {
    Options {
        // `s`/`S` substitute as in vim instead of vim-sneak.
        motion_sneak: false,
        // Enter starts the new line at column 0.
        autoindent: false,
        smartindent: false,
        // No comment-leader continuation and no hard wrap while typing.
        formatoptions: String::new(),
        textwidth: 0,
        modeline: false,
        // Undo history is capped at 200 steps.
        undo_levels: 200,
        ..Options::default()
    }
}

impl Editor {
    /// An editor holding `lines`, cursor at (0, 0), normal mode, with an
    /// in-memory clipboard. An empty list becomes one empty line.
    pub fn new(lines: &[String]) -> Self {
        Self::with_clipboard(lines, Box::new(MemoryClipboard::default()))
    }

    /// Like [`Editor::new`], yanking to `clipboard`.
    pub fn with_clipboard(lines: &[String], clipboard: Box<dyn Clipboard>) -> Self {
        let host = NmHost {
            clipboard,
            stored: false,
            started: std::time::Instant::now(),
            viewport: DefaultHost::DEFAULT_VIEWPORT,
        };
        let mut inner = hjkl_vim::vim_editor(hjkl_buffer::View::new(), host, options());
        // `Options` has no autopair field and `Editor::new` turns it on:
        // typing `(` would insert `()`.
        inner.settings_mut().autopair = false;
        inner.settings_mut().autoclose_tag = false;
        let mut editor = Self {
            inner,
            session_start: None,
        };
        editor.set_lines(lines);
        editor
    }

    /// Replaces the whole buffer and clears undo history.
    pub fn set_lines(&mut self, lines: &[String]) {
        let mut text = lines.join("\n");
        text.push('\n');
        self.inner.set_content(&text);
        self.inner.jump_cursor(0, 0);
        self.session_start = None;
    }

    /// Number of rows hjkl holds, the phantom row included (see
    /// [`Editor::lines`]).
    fn rows(&self) -> usize {
        self.inner.row_count()
    }

    /// True when the rope ends in `\n`, so its last row is the phantom.
    fn terminated(&self) -> bool {
        let rows = self.rows();
        rows > 1 && self.inner.line_char_count(rows - 1) == 0
    }

    /// The buffer as lines, without line terminators. Never empty.
    ///
    /// hjkl follows the file convention: the text ends in `\n`, ropey shows
    /// that as an empty last row, and motions never go there. So nm stores
    /// its lines with a trailing `\n` and leaves that phantom row out here.
    /// Without it, a real empty last line (a section divider) could not be
    /// reached with `j` or `G`.
    pub fn lines(&self) -> Vec<String> {
        (0..self.line_count())
            .map(|row| self.inner.line(row).unwrap_or_default())
            .collect()
    }

    /// A number that changes whenever the text may have changed, so the
    /// app can skip re-reading unchanged lines.
    pub fn generation(&self) -> u64 {
        self.inner.buffer().dirty_gen()
    }

    /// Number of real lines (the phantom row left out). At least 1.
    pub fn line_count(&self) -> usize {
        let rows = self.rows();
        if self.terminated() {
            rows - 1
        } else {
            rows.max(1)
        }
    }

    /// Restores the trailing `\n` after an edit removed it, and moves the
    /// cursor off the phantom row.
    ///
    /// Deleting every line (`dd` on the only line, `dG`, `VGd`) leaves hjkl
    /// with an empty rope; without the fix, a following `o` would put the
    /// cursor on the phantom row. The fix goes straight to the buffer, so it
    /// is not an undo step and leaves the cursor where it was. Redo can also
    /// leave the cursor on the phantom row.
    fn keep_terminated(&mut self) {
        let (row, col) = self.cursor();
        if !self.terminated() {
            let last = self.rows() - 1;
            let at = hjkl_buffer::Position::new(last, self.inner.line_char_count(last));
            self.inner.mutate_edit(hjkl_buffer::Edit::InsertStr {
                at,
                text: "\n".to_string(),
            });
            self.inner.set_cursor_quiet(row, col);
        }
        let last = self.rows().saturating_sub(2);
        if row > last {
            let len = self.inner.line_char_count(last);
            let max = if self.mode() == Mode::Insert {
                len
            } else {
                len.saturating_sub(1)
            };
            self.inner.set_cursor_quiet(last, col.min(max));
        }
    }

    /// `(row, col)`, both zero-based; `col` counts characters.
    pub fn cursor(&self) -> (usize, usize) {
        self.inner.cursor()
    }

    /// Moves the cursor, clamped to the buffer. Outside insert mode the
    /// column stops on the last character, as in vim. In visual mode this
    /// extends the selection.
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        let row = row.min(self.line_count() - 1);
        let len = self.inner.line_char_count(row);
        let max = if self.mode() == Mode::Insert {
            len
        } else {
            len.saturating_sub(1)
        };
        self.inner.jump_cursor(row, col.min(max));
    }

    /// Number of characters in `row` (0 when out of range).
    pub fn line_len(&self, row: usize) -> usize {
        if row < self.rows() {
            self.inner.line_char_count(row)
        } else {
            0
        }
    }

    pub fn mode(&self) -> Mode {
        match self.inner.vim_mode() {
            VimMode::Normal => Mode::Normal,
            VimMode::Insert => Mode::Insert,
            VimMode::Visual => Mode::Visual,
            VimMode::VisualLine => Mode::VisualLine,
            VimMode::VisualBlock => Mode::VisualBlock,
        }
    }

    /// The visual selection, or `None` outside visual mode. hjkl's
    /// selection is inclusive at both ends, as in vim.
    pub fn selection(&self) -> Option<Selection> {
        if let Some((start, end)) = self.inner.char_highlight() {
            return Some(Selection::Chars { start, end });
        }
        if let Some((top, bottom)) = self.inner.line_highlight() {
            return Some(Selection::Lines { top, bottom });
        }
        self.inner
            .block_highlight()
            .map(|(top, bottom, left, right)| Selection::Block {
                top,
                bottom,
                left,
                right,
                to_eol: self.inner.block_to_eol(),
            })
    }

    /// True while a command waits for its next key: an operator waiting
    /// for a motion, a `g` prefix, `f` waiting for its character. A typed
    /// count alone does not count.
    pub fn is_chord_pending(&self) -> bool {
        self.inner.is_chord_pending()
    }

    /// The clipboard `y` writes to.
    pub fn clipboard(&mut self) -> &mut dyn Clipboard {
        self.inner.host_mut().clipboard.as_mut()
    }

    /// Swaps the clipboard `y` writes to and `p` falls back to.
    pub fn set_clipboard(&mut self, clipboard: Box<dyn Clipboard>) {
        self.inner.host_mut().clipboard = clipboard;
    }

    /// The operator waiting for a motion (`d`, `c`, `y`, ...), if any.
    pub fn pending_op(&self) -> Option<char> {
        self.inner.pending_op()
    }

    /// The unnamed register: its text and whether it holds whole lines.
    /// Linewise text ends in `\n`, one per line.
    pub fn register(&self) -> (String, bool) {
        self.inner
            .with_registers(|r| (r.unnamed.text.clone(), r.unnamed.linewise))
    }

    /// Fills the unnamed register (and `"0`) as a yank would.
    pub fn set_register(&mut self, text: String, linewise: bool) {
        self.inner.record_yank(text, linewise, None);
    }

    /// Fills the unnamed register only, as if pasted in from outside.
    pub fn import_register(&mut self, text: String, linewise: bool) {
        self.inner.set_yank_linewise(linewise);
        self.inner.with_registers_mut(|r| {
            r.unnamed = Slot {
                text,
                linewise,
                ..Slot::default()
            };
        });
    }

    /// After `$`: `j` and `k` keep to the end of each line until a
    /// sideways move, as in vim. hjkl stores the landed column instead.
    pub fn stick_to_line_end(&mut self) {
        self.inner.set_sticky_col(Some(usize::MAX));
    }

    /// True when `u` has something to undo.
    pub fn can_undo(&self) -> bool {
        !self.inner.buffer().undo_stack_is_empty()
    }

    /// True when `Ctrl+R` has something to redo.
    pub fn can_redo(&self) -> bool {
        !self.inner.buffer().redo_stack_is_empty()
    }

    /// Feeds one key to the vim engine. Keys the engine has no name for
    /// (function keys, media keys) are dropped.
    ///
    /// Returns true when the key yanked or deleted text into the register
    /// (hjkl reports both the same way; see [`NmHost`]).
    pub fn send(&mut self, key: KeyEvent) -> bool {
        self.inner.host_mut().stored = false;
        // `content_arc` is cached until the text changes, so motions do
        // not rebuild it.
        let before = (self.mode(), self.inner.content_arc(), self.undo_depth());
        self.dispatch(key);
        self.clamp_normal_cursor();
        self.drop_empty_undo_steps(before);
        std::mem::take(&mut self.inner.host_mut().stored)
    }

    fn undo_depth(&self) -> usize {
        self.inner.buffer().undo_stack_len()
    }

    /// Removes undo steps that changed nothing (`i<Esc>`, `x` on an
    /// empty line, `c$<Esc>` on an empty line add no step).
    ///
    /// hjkl records a step when a command starts, before it knows whether
    /// the text will change: on entering insert mode, and for an edit that
    /// turns out to be a no-op (`dd` on the only, empty line, whose
    /// newline nm puts back). A step whose text equals the text before it
    /// is popped again. An insert session is judged when it ends, and
    /// keeps at most one step.
    fn drop_empty_undo_steps(&mut self, (mode, text, depth): (Mode, Arc<String>, usize)) {
        if self.mode() == Mode::Insert {
            if mode != Mode::Insert {
                self.session_start = Some((text, depth));
            }
            return;
        }
        let (text, depth, keep_one) = if mode == Mode::Insert {
            // The whole insert session is one step, even where
            // hjkl split it (arrow keys start a new undo group, as in
            // vim). The oldest step holds the text from before the
            // session; the newer ones are dropped.
            match self.session_start.take() {
                Some((text, depth)) => (text, depth, true),
                None => return,
            }
        } else {
            (text, depth, false)
        };
        if self.undo_depth() <= depth {
            return;
        }
        let changed = *self.inner.content_arc() != *text;
        if changed && !keep_one {
            return;
        }
        let keep = depth + usize::from(changed);
        while self.undo_depth() > keep && self.inner.pop_last_undo() {}
    }

    /// In normal mode the cursor sits on a character. hjkl leaves it one
    /// past the end after `J` with an empty line, a dot-repeated append,
    /// and a visual delete that ends the line.
    fn clamp_normal_cursor(&mut self) {
        if self.mode() != Mode::Normal {
            return;
        }
        let (row, col) = self.cursor();
        let last = self.line_len(row).saturating_sub(1);
        if col > last {
            self.inner.set_cursor_quiet(row, last);
        }
    }

    fn dispatch(&mut self, key: KeyEvent) {
        let Some(input) = to_input(key) else {
            return;
        };
        let inserting = self.mode() == Mode::Insert;
        let (row, col) = self.cursor();
        if inserting
            && key.code == KeyCode::Delete
            && row + 1 == self.line_count()
            && col >= self.line_len(row)
        {
            // At the end of the last line there is nothing to join; hjkl
            // would join the phantom row and delete an empty last line.
            return;
        }
        hjkl_vim::dispatch_input(&mut self.inner, input);
        self.keep_terminated();

        // In insert mode hjkl moves `<End>`, `<Up>` and `<Down>` with
        // normal-mode rules: `<End>` stops on the last character, and
        // `<Up>`/`<Down>` clamp to it too (after `A` they even land on
        // column 0). nm goes to the end of the line, as vim does, and
        // `<Up>`/`<Down>` keep the column, clamped to the line length. The engine still made
        // the move, so its undo-group break happened; only fix the column.
        if inserting && self.mode() == Mode::Insert {
            let new_row = self.cursor().0;
            let len = self.line_len(new_row);
            match key.code {
                KeyCode::End => self.inner.set_cursor_doc(new_row, len),
                KeyCode::Up | KeyCode::Down => self.inner.set_cursor_doc(new_row, col.min(len)),
                _ => {}
            }
        }
    }
}

/// Converts a crossterm key to an hjkl input. Uppercase letters count as
/// shifted, the way a terminal sends them.
fn to_input(key: KeyEvent) -> Option<Input> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let mut shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let k = match key.code {
        KeyCode::Char(c) => {
            shift |= c.is_uppercase();
            HKey::Char(c)
        }
        KeyCode::Esc => HKey::Esc,
        KeyCode::Enter => HKey::Enter,
        KeyCode::Backspace => HKey::Backspace,
        KeyCode::Delete => HKey::Delete,
        KeyCode::Tab => HKey::Tab,
        KeyCode::Left => HKey::Left,
        KeyCode::Right => HKey::Right,
        KeyCode::Up => HKey::Up,
        KeyCode::Down => HKey::Down,
        KeyCode::Home => HKey::Home,
        KeyCode::End => HKey::End,
        KeyCode::PageUp => HKey::PageUp,
        KeyCode::PageDown => HKey::PageDown,
        _ => return None,
    };
    Some(Input {
        key: k,
        ctrl,
        alt,
        shift,
    })
}
