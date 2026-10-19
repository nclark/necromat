//! Command mode (`:`), saving, loading and quitting.
//!
//! The quit guard: with a named file that has unsaved changes, `q`, `:q`
//! and `:e` refuse with E37. Without a file (session mode) quitting is
//! always allowed, because the session is saved on exit.

use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::App;
use crate::storage;

const E32: &str = "E32: No file name";
const E37: &str = "E37: No write since last change (add ! to override)";
const E471: &str = "E471: Argument required";

/// A parsed command line: the name, whether it ends in `!`, and the
/// argument (the rest of the line, trimmed; may contain spaces).
struct Parsed<'a> {
    name: &'a str,
    bang: bool,
    arg: Option<&'a str>,
}

/// Splits `e! ~/a.nm` into `e`, bang, `~/a.nm`. The name is the leading
/// letters, so `e!x.nm` and `w x.nm` both work.
fn parse(line: &str) -> Parsed<'_> {
    let line = line.trim();
    let end = line
        .char_indices()
        .find(|(_, c)| !c.is_ascii_alphabetic())
        .map_or(line.len(), |(i, _)| i);
    let (name, rest) = line.split_at(end);
    let (bang, rest) = match rest.strip_prefix('!') {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let rest = rest.trim();
    Parsed {
        name,
        bang,
        arg: (!rest.is_empty()).then_some(rest),
    }
}

impl App {
    /// A key while the command line is open. Enter runs it; Esc, or
    /// Backspace on an empty line, closes it.
    pub(super) fn command_key(&mut self, key: KeyEvent) {
        let Some(input) = self.command.as_mut() else {
            return;
        };
        let plain = (key.modifiers - KeyModifiers::SHIFT).is_empty();
        match key.code {
            KeyCode::Esc => self.command = None,
            KeyCode::Enter => {
                let line = std::mem::take(input);
                self.command = None;
                self.execute_command(&line);
            }
            KeyCode::Backspace => {
                if input.pop().is_none() {
                    self.command = None;
                }
            }
            KeyCode::Char(c) if plain => input.push(c),
            _ => {}
        }
    }

    /// Runs one command line. An empty line does
    /// nothing.
    pub(super) fn execute_command(&mut self, line: &str) {
        let Parsed { name, bang, arg } = parse(line);
        match name {
            "" if line.trim().is_empty() => {}
            "w" | "write" => {
                self.write(arg);
            }
            "e" | "edit" => self.edit(arg, bang),
            "q" | "quit" => self.quit(bang),
            "wq" => self.write_quit(arg),
            "x" => self.exit(arg),
            _ => {
                let word = line.split_whitespace().next().unwrap_or_default();
                self.status_msg = Some(format!("Unknown command: {word}"));
            }
        }
    }

    /// `q`, `:q`: quits unless a named file has unsaved changes. With
    /// `force` (`:q!`, `ZQ`) quits anyway and drops the changes.
    pub(super) fn quit(&mut self, force: bool) {
        if !force && self.modified && self.file_path.is_some() {
            self.status_msg = Some(E37.into());
        } else {
            self.should_quit = true;
        }
    }

    /// `:w [path]`, `Ctrl+S`: writes the lines to `path` (with `~/`
    /// expanded) or to the current file, which then becomes the current
    /// file. Returns true when the file was written.
    pub(super) fn write(&mut self, path: Option<&str>) -> bool {
        let Some(path) = path.map(expand).or_else(|| self.file_path.clone()) else {
            self.status_msg = Some(E32.into());
            return false;
        };
        if let Err(e) = storage::save(&path, self.lines()) {
            self.status_msg = Some(io_error(&path, &e));
            return false;
        }
        self.status_msg = Some(format!("\"{}\" written", path.display()));
        self.file_path = Some(path);
        self.session_mode = false;
        self.modified = false;
        true
    }

    /// `:wq [path]`: writes, then quits if the write worked. In session
    /// mode with no path there is nothing to write to: quit, and the
    /// session is saved on exit.
    fn write_quit(&mut self, path: Option<&str>) {
        if (path.is_none() && self.file_path.is_none()) || self.write(path) {
            self.should_quit = true;
        }
    }

    /// `:x [path]`, `ZZ`: like `:wq`, but a named file is only written
    /// when it has changes (vim).
    pub(super) fn exit(&mut self, path: Option<&str>) {
        if path.is_none() && !(self.modified && self.file_path.is_some()) {
            self.should_quit = true;
        } else {
            self.write_quit(path);
        }
    }

    /// `:e path`, `:e! [path]`: loads a file, replacing the buffer. Refused
    /// with E37 when a named file has unsaved changes, unless `force`.
    /// `:e!` with no path reloads the current file.
    ///
    /// Leaving session mode this way saves the session first, as quitting
    /// would, so its text is not lost.
    fn edit(&mut self, path: Option<&str>, force: bool) {
        let path = match (path, force) {
            (Some(p), _) => expand(p),
            (None, false) => {
                self.status_msg = Some(E471.into());
                return;
            }
            (None, true) => match self.file_path.clone() {
                Some(p) => p,
                None => {
                    self.status_msg = Some(E32.into());
                    return;
                }
            },
        };
        if !force && self.modified && self.file_path.is_some() {
            self.status_msg = Some(E37.into());
            return;
        }
        let lines = match storage::load(&path) {
            Ok(lines) => lines,
            Err(e) => {
                self.status_msg = Some(io_error(&path, &e));
                return;
            }
        };
        if let Err((session, e)) = self.save_session() {
            self.status_msg = Some(io_error(&session, &e));
            return;
        }
        self.status_msg = Some(format!("\"{}\" {} lines", path.display(), lines.len()));
        self.replace_lines(&lines);
        self.file_path = Some(path);
        self.session_mode = false;
    }

    /// Saves the buffer to the session path when in session mode. When every
    /// line is blank, nothing is written and any existing session file is
    /// removed, so clearing the pad and quitting forgets the session. A
    /// named file is never saved here. Returns whether anything was written;
    /// on failure, the session path and the error.
    pub fn save_session(&self) -> Result<bool, (PathBuf, std::io::Error)> {
        if self.file_path.is_some() {
            return Ok(false);
        }
        let Some(path) = &self.session_path else {
            return Ok(false);
        };
        if self.lines().iter().all(|l| l.trim().is_empty()) {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err((path.clone(), e)),
            }
            return Ok(false);
        }
        storage::save(path, self.lines()).map_err(|e| (path.clone(), e))?;
        Ok(true)
    }
}

fn expand(path: &str) -> PathBuf {
    storage::expand_tilde(Path::new(path))
}

/// `Error: PATH: REASON`.
fn io_error(path: &Path, e: &std::io::Error) -> String {
    format!("Error: {}: {e}", path.display())
}
