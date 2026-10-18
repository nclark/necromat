//! Key handling per mode: which keys reach the vim engine, which nm keeps
//! from it, and the nm commands the engine does not have.
//!
//! The engine (behind `editor.rs`) does the vim work. What lives here:
//! - the register rule: only a yank reaches the system
//!   clipboard, and `p` falls back to the clipboard when the register is
//!   empty;
//! - `yr`, which yanks the line's formatted result;
//! - the status messages shown after a yank and at the ends of the
//!   undo history;
//! - insert-mode arrows that wrap across lines, `$` that sticks to the
//!   line end on `j`/`k`;
//! - `:` command mode, `?` help, `q`, `ZZ` and `ZQ` (the commands
//!   themselves are in `commands.rs`);
//! - the completion keys `Tab`, `Ctrl+N`, `Ctrl+P` (the completion
//!   itself is in `completion.rs`);
//! - engine prompts nm has no UI for (`/`, `?`, `q` macros).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::App;
use super::editor::Mode;
use crate::eval::format_number;

/// Shorthand for the bare character of a key with no Ctrl or Alt.
fn plain_char(key: KeyEvent) -> Option<char> {
    let mods = key.modifiers - KeyModifiers::SHIFT;
    match key.code {
        KeyCode::Char(c) if mods.is_empty() => Some(c),
        _ => None,
    }
}

/// What nm remembers about `f F t T ; ,` (see [`App::send_vim_key`]).
#[derive(Debug, Default)]
pub(super) struct Finds {
    /// The next key is the character a find looks for; true when the
    /// find goes forward (`f`, `t`).
    waiting: Option<bool>,
    /// The direction of the last completed find, for `;` and `,`.
    last_forward: Option<bool>,
    /// The last key was `i` or `a` after an operator: the next key names
    /// a text object, not a motion.
    text_object: bool,
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

impl App {
    /// Handles an insert-mode key. Returns false for the completion keys,
    /// which must not recompute the completion they act on.
    pub(super) fn insert_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let (row, col) = self.cursor();
        match key.code {
            // Completion keys. The engine never sees these: Tab would
            // insert a tab, and Ctrl+N/Ctrl+P mean other things to it.
            KeyCode::Char('n') if ctrl => {
                self.cycle_completion(1);
                return false;
            }
            KeyCode::Char('p') if ctrl => {
                self.cycle_completion(-1);
                return false;
            }
            KeyCode::Tab => {
                self.accept_completion();
                return false;
            }
            // Left at column 0 goes to the end of the
            // previous line, Right at the end goes to the start of the
            // next. Vim (and hjkl) stop instead, so send the engine keys
            // that do the same thing through its own insert motions.
            KeyCode::Left if col == 0 && row > 0 => {
                self.editor.send(KeyEvent::from(KeyCode::Up));
                self.editor.send(KeyEvent::from(KeyCode::End));
            }
            KeyCode::Right
                if col >= self.editor.line_len(row) && row + 1 < self.editor.line_count() =>
            {
                self.editor.send(KeyEvent::from(KeyCode::Down));
                self.editor.send(KeyEvent::from(KeyCode::Home));
            }
            _ => {
                self.editor.send(key);
            }
        }
        true
    }

    pub(super) fn normal_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let motion_next = self.finds.waiting.is_none() && !self.finds.text_object;
        if self.editor.pending_op() == Some('y') && motion_next && plain_char(key) == Some('r') {
            // `yr`: nm's own command. Drop the engine's pending `y`.
            self.editor.send(KeyEvent::from(KeyCode::Esc));
            self.yank_result();
            return;
        }
        if std::mem::take(&mut self.pending_z) {
            match plain_char(key) {
                Some('Z') => self.exit(None),
                Some('Q') => self.quit(true),
                // Vim drops an unknown `Z` command.
                _ => {}
            }
            return;
        }
        if self.editor.is_chord_pending() {
            self.send_vim_key(key);
            return;
        }
        // The keys nm handles itself ignore a count; Esc drops
        // the engine's half-typed count so it does not apply to the next
        // motion. hjkl would start a macro on `q`, a search prompt on `?`
        // (nm has no UI for one) and nothing useful on `:` or `Z`.
        match key.code {
            KeyCode::Char('q') if !ctrl => self.drop_count().quit(false),
            KeyCode::Char(':') => self.drop_count().command = Some(String::new()),
            KeyCode::Char('?') => self.drop_count().help = true,
            KeyCode::Char('Z') if !ctrl => self.drop_count().pending_z = true,
            KeyCode::Char('/') => {}
            KeyCode::Char('u') if !ctrl => {
                if !self.editor.can_undo() {
                    self.status_msg = Some("Already at oldest change".into());
                }
                self.editor.send(key);
            }
            KeyCode::Char('r') if ctrl => {
                if !self.editor.can_redo() {
                    self.status_msg = Some("Already at newest change".into());
                }
                self.editor.send(key);
            }
            KeyCode::Char('p' | 'P') if !ctrl => {
                if self.fill_register_from_clipboard() {
                    self.editor.send(key);
                } else {
                    self.drop_count().nothing_to_paste();
                }
            }
            KeyCode::Char('$') | KeyCode::End => {
                self.editor.send(key);
                self.editor.stick_to_line_end();
            }
            _ => self.send_vim_key(key),
        }
    }

    /// Drops a count typed before a key nm handles itself.
    fn drop_count(&mut self) -> &mut Self {
        self.editor.send(KeyEvent::from(KeyCode::Esc));
        self
    }

    pub(super) fn visual_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if self.editor.is_chord_pending() {
            self.send_vim_key(key);
            return;
        }
        match (key.code, self.mode()) {
            // Search prompt and macro recording: nm has neither.
            (KeyCode::Char('/' | '?' | 'q'), _) => {}
            // Vim: `v` in charwise visual and `V` in linewise visual
            // leave it. hjkl stays.
            (KeyCode::Char('v'), Mode::Visual) | (KeyCode::Char('V'), Mode::VisualLine)
                if !ctrl =>
            {
                self.editor.send(KeyEvent::from(KeyCode::Esc));
            }
            // With nothing to paste, vim refuses and keeps the selection;
            // hjkl would delete it.
            (KeyCode::Char('p' | 'P'), _) if !ctrl => {
                if self.fill_register_from_clipboard() {
                    self.editor.send(key);
                } else {
                    self.nothing_to_paste();
                }
            }
            _ => self.send_vim_key(key),
        }
    }

    /// Sends a normal- or visual-mode key to the engine, keeping track of
    /// `f F t T ; ,` so an operator with a backward find can be fixed.
    ///
    /// hjkl treats every find as inclusive, so `dF(` and `dT(` also
    /// delete the character under the cursor. In vim (and in nm) `F`
    /// and `T` are exclusive. nm lets the engine run the
    /// command, then redoes it over the right range.
    fn send_vim_key(&mut self, key: KeyEvent) {
        let c = plain_char(key);
        let op = self.editor.pending_op();
        let free = !self.editor.is_chord_pending();
        let waiting = self.finds.waiting.take();
        let after_object_key = std::mem::take(&mut self.finds.text_object);

        let mut backward = false;
        if let Some(forward) = waiting {
            // This key is the character the find looks for.
            if c.is_some() {
                self.finds.last_forward = Some(forward);
                backward = !forward;
            }
        } else if free || (op.is_some() && !after_object_key) {
            match c {
                Some(ch @ ('f' | 'F' | 't' | 'T')) => {
                    self.finds.waiting = Some(matches!(ch, 'f' | 't'));
                }
                Some(ch @ (';' | ',')) if op.is_some() => {
                    if let Some(forward) = self.finds.last_forward {
                        backward = forward == (ch == ',');
                    }
                }
                Some('i' | 'a') if op.is_some() => self.finds.text_object = true,
                _ => {}
            }
        }

        match op {
            Some(op @ ('d' | 'c' | 'y')) if backward => self.backward_find_op(op, key),
            _ => self.send_tracking_yank(key),
        }
    }

    /// Runs the key that completes `dF{c}`, `cT{c}`, `y,` and the like,
    /// then corrects the range to stop before the cursor character.
    fn backward_find_op(&mut self, op: char, key: KeyEvent) {
        let (row, col) = self.cursor();
        if !self.editor.send(key) {
            // Not found: the engine did nothing.
            return;
        }
        // The engine took `target..=col`; its register says how much.
        // (The cursor cannot tell: it is clamped when the line got
        // shorter.)
        let (taken, _) = self.editor.register();
        let n = taken.chars().count();
        // One character means the range was empty; hjkl already treats `T`
        // next to its target as not found, so this does not happen.
        if taken.contains('\n') || n < 2 || n > col + 1 {
            return;
        }
        let target = col + 1 - n;
        let text: String = taken.chars().take(n - 1).collect();
        if op == 'y' {
            self.editor.set_register(text.clone(), false);
            self.yank_to_clipboard(&text, false);
            return;
        }
        if op == 'c' {
            self.editor.send(KeyEvent::from(KeyCode::Esc));
        }
        // Take the engine's edit back and redo it on a visual selection
        // from just before the old cursor back to the target. Selecting
        // backwards leaves the cursor at the target, which is where undo
        // will put it back.
        self.editor.send(KeyEvent::from(KeyCode::Char('u')));
        self.editor.set_cursor(row, col - 1);
        self.editor.send(KeyEvent::from(KeyCode::Char('v')));
        self.editor.set_cursor(row, target);
        self.editor.send(KeyEvent::from(KeyCode::Char(op)));
    }

    /// Sends a normal- or visual-mode key and, if it was a yank, copies
    /// the register to the clipboard and says so in the status bar.
    ///
    /// A yank is the key that completes a pending `y` (`yy`, `y$`, `yG`),
    /// `Y`, or `y`/`Y` in visual mode. hjkl reports deletes through the
    /// same hook, so the key decides, not the hook.
    fn send_tracking_yank(&mut self, key: KeyEvent) {
        let c = plain_char(key);
        let yank_key = match self.mode() {
            Mode::Normal => self.editor.pending_op() == Some('y') || c == Some('Y'),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => matches!(c, Some('y' | 'Y')),
            Mode::Insert => false,
        };
        let stored = self.editor.send(key);
        if yank_key && stored {
            let (text, linewise) = self.editor.register();
            self.yank_to_clipboard(&text, linewise);
        }
    }

    /// `yr`: yank the formatted result of the cursor line.
    fn yank_result(&mut self) {
        let row = self.cursor().0;
        let value = self
            .results
            .get(row)
            .filter(|r| r.error.is_none())
            .and_then(|r| r.value);
        match value {
            Some(v) => {
                let text = format_number(v);
                self.editor.set_register(text.clone(), false);
                self.status_msg = Some("Yanked result".into());
                self.write_clipboard(&text);
            }
            None => self.status_msg = Some("No result to yank".into()),
        }
    }

    /// Writes a yank to the clipboard and reports it: `Yanked 3 lines`,
    /// `Yanked 5 characters`. Linewise text goes out without its final
    /// newline.
    fn yank_to_clipboard(&mut self, text: &str, linewise: bool) {
        let (out, msg) = if linewise {
            let body = text.strip_suffix('\n').unwrap_or(text);
            let n = body.split('\n').count();
            (body, format!("Yanked {}", plural(n, "line", "lines")))
        } else {
            let n = text.chars().count();
            (
                text,
                format!("Yanked {}", plural(n, "character", "characters")),
            )
        };
        self.status_msg = Some(msg);
        self.write_clipboard(out);
    }

    fn write_clipboard(&mut self, text: &str) {
        if let Err(e) = self.editor.clipboard().set(text) {
            self.status_msg = Some(format!("Clipboard error: {e}"));
        }
    }

    /// Vim's refusal of `p`/`P` when there is nothing to paste.
    fn nothing_to_paste(&mut self) {
        self.status_msg = Some("E353: Nothing in register \"".into());
    }

    /// Before `p`/`P`: an empty register is filled from the clipboard.
    /// Text with a newline in it pastes as whole lines.
    /// Returns false when there is still nothing to paste.
    fn fill_register_from_clipboard(&mut self) -> bool {
        if !self.editor.register().0.is_empty() {
            return true;
        }
        let Some(text) = self.editor.clipboard().get() else {
            return false;
        };
        if text.is_empty() {
            return false;
        }
        if text.contains('\n') {
            let mut text = text;
            if !text.ends_with('\n') {
                text.push('\n');
            }
            self.editor.import_register(text, true);
        } else {
            self.editor.import_register(text, false);
        }
        true
    }
}
