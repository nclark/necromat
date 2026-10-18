//! The terminal UI: a vim-style pad on the left, live results on the right.
//! See `doc/design/editor.md`.

mod app;
mod clipboard;
#[cfg(test)]
mod command_tests;
mod commands;
mod completion;
#[cfg(test)]
mod completion_tests;
mod editor;
#[cfg(test)]
mod harness;
mod help;
mod highlight;
#[cfg(test)]
mod highlight_tests;
mod keys;
mod style;
#[cfg(test)]
mod tests;
mod theme;
mod view;
#[cfg(test)]
mod vim_tests;
#[cfg(test)]
mod visual_tests;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::event::{self, Event};
use ratatui::crossterm::execute;

use crate::storage;
use app::App;
use clipboard::SystemClipboard;
use view::CursorShape;

/// Runs the TUI until the user quits.
///
/// `lines` is the starting text (empty means one empty line). `file_path`
/// names the file being edited; `session_mode` means there is no file and
/// the text is the auto-saved session. Uses the alternate screen and does
/// not capture the mouse, so terminal text selection keeps working.
pub fn run(lines: Vec<String>, file_path: Option<PathBuf>, session_mode: bool) -> io::Result<()> {
    theme::set_active(theme::load());
    let mut app = App::new(lines, file_path, session_mode);
    app.session_path = storage::session_path();
    // OSC 52 goes to stdout between frames, never while ratatui draws.
    app.editor
        .set_clipboard(Box::new(SystemClipboard::new(Box::new(io::stdout()))));
    // `try_init` enters raw mode and the alternate screen, and installs a
    // panic hook that restores the terminal. It never enables mouse capture.
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, &mut app);
    let shape = execute!(io::stdout(), SetCursorStyle::DefaultUserShape);
    ratatui::restore();
    // Saved even when the loop failed, so the text is not lost.
    let saved = app.save_session().map(|_| ()).map_err(|(path, e)| {
        io::Error::new(
            e.kind(),
            format!("saving session to {}: {e}", path.display()),
        )
    });
    result.and(shape).and(saved)
}

/// How often the loop looks for a theme change while idle.
const THEME_POLL: Duration = Duration::from_millis(500);

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let mut shape = None;
    let mut theme_watch = theme::Watch::new();
    while !app.should_quit() {
        let size = terminal.size()?;
        app.set_size(size.width, size.height);
        let mut placement = None;
        terminal.draw(|frame| placement = view::draw(frame, app))?;

        let want = placement.map(|p| p.shape);
        if want.is_some() && want != shape {
            let style = match want {
                Some(CursorShape::Bar) => SetCursorStyle::BlinkingBar,
                _ => SetCursorStyle::SteadyBlock,
            };
            execute!(io::stdout(), style)?;
            shape = want;
        }

        // Wait for a key, but wake up now and then to see whether the theme
        // files changed (an Omarchy theme switch, or an edited theme.toml).
        if event::poll(THEME_POLL)? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        } else if theme_watch.changed() {
            theme::set_active(theme::load());
        }
    }
    Ok(())
}
