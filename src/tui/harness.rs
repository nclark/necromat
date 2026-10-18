//! Headless test harness: build an [`App`], feed it keys written in vim
//! notation, inspect the buffer and cursor, and render it to a ratatui
//! `TestBackend`. All the TUI tests use this.
//!
//! ```ignore
//! let mut h = Harness::new(&["100", "200"]);
//! h.keys("jA5<Esc>");
//! assert_eq!(h.lines(), ["100", "2005"]);
//! let screen = h.render();
//! assert!(screen.rows[1].starts_with(" 2 │ 2005"));
//! ```

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Span;

use super::app::App;
use super::editor::Mode;
use super::view::{self, CursorPlacement};

pub const WIDTH: u16 = 80;
pub const HEIGHT: u16 = 24;

pub struct Harness {
    pub app: App,
}

/// One rendered frame.
pub struct Screen {
    /// Each row as plain text (styling dropped).
    pub rows: Vec<String>,
    /// Where the terminal cursor was placed, if it is shown.
    pub cursor: Option<CursorPlacement>,
    /// The cells, for style checks.
    pub buffer: Buffer,
}

impl Screen {
    /// The status bar row.
    pub fn status(&self) -> &str {
        self.rows.last().map_or("", String::as_str)
    }
}

impl Harness {
    /// An 80x24 app holding `lines`, in session mode, cursor at the origin.
    pub fn new(lines: &[&str]) -> Self {
        Self::sized(lines, WIDTH, HEIGHT)
    }

    pub fn sized(lines: &[&str], width: u16, height: u16) -> Self {
        let lines = lines.iter().map(|s| s.to_string()).collect();
        let mut app = App::new(lines, None, true);
        app.set_size(width, height);
        Self { app }
    }

    /// Like [`Harness::new`] with the cursor moved to `(row, col)` in
    /// normal mode (the column stops on the last character).
    pub fn at(lines: &[&str], row: usize, col: usize) -> Self {
        let mut h = Self::new(lines);
        h.app.editor.set_cursor(row, col);
        h
    }

    /// Insert mode with the cursor at `(row, col)`; `col` may be the line
    /// length (end of line).
    pub fn inserting(lines: &[&str], row: usize, col: usize) -> Self {
        let mut h = Self::at(lines, row, col);
        if col >= h.app.editor.line_len(row) {
            h.keys("A");
        } else {
            h.keys("i");
        }
        assert_eq!(h.cursor(), (row, col), "inserting() start");
        h
    }

    /// Sends keys in vim notation (see [`parse_keys`]).
    pub fn keys(&mut self, seq: &str) -> &mut Self {
        for k in parse_keys(seq) {
            self.app.handle_key(k);
        }
        self
    }

    /// Sends each character of `text` as a typed key.
    pub fn type_text(&mut self, text: &str) -> &mut Self {
        for c in text.chars() {
            self.app.handle_key(char_key(c));
        }
        self
    }

    pub fn lines(&self) -> Vec<String> {
        self.app.lines().to_vec()
    }

    pub fn cursor(&self) -> (usize, usize) {
        self.app.cursor()
    }

    pub fn mode(&self) -> Mode {
        self.app.mode()
    }

    /// Renders through a ratatui `Terminal` on a `TestBackend` of the
    /// app's size.
    pub fn render(&self) -> Screen {
        let (width, height) = self.app.size();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
        let mut cursor = None;
        terminal
            .draw(|f| cursor = view::draw(f, &self.app))
            .expect("draw");
        let backend = terminal.backend();
        if let Some(p) = cursor {
            assert!(backend.cursor_visible(), "placement without visible cursor");
            assert_eq!(backend.cursor_position(), (p.x, p.y).into());
        } else {
            assert!(!backend.cursor_visible(), "cursor shown without placement");
        }
        let buffer = backend.buffer().clone();
        Screen {
            rows: rows(&buffer),
            cursor,
            buffer,
        }
    }
}

/// Plain text of each buffer row. A wide character takes two cells; the
/// cell it covers is skipped.
pub fn rows(buf: &Buffer) -> Vec<String> {
    let area = buf.area;
    (area.top()..area.bottom())
        .map(|y| {
            let mut row = String::new();
            let mut x = area.left();
            while x < area.right() {
                let symbol = buf[(x, y)].symbol();
                row.push_str(symbol);
                x += u16::try_from(Span::raw(symbol).width().max(1)).unwrap_or(1);
            }
            row
        })
        .collect()
}

/// A typed character, shifted when uppercase as a terminal sends it.
pub fn char_key(c: char) -> KeyEvent {
    let mods = if c.is_uppercase() {
        KeyModifiers::SHIFT
    } else {
        KeyModifiers::NONE
    };
    KeyEvent::new(KeyCode::Char(c), mods)
}

/// Parses vim key notation. Plain characters are typed one by one;
/// `<Esc> <CR> <BS> <Del> <Tab> <Left> <Right> <Up> <Down> <Home> <End>`
/// name keys (case-insensitive), `<C-x>` is Ctrl+x, `<lt>` is a literal `<`.
pub fn parse_keys(seq: &str) -> Vec<KeyEvent> {
    let mut out = Vec::new();
    let mut chars = seq.chars();
    while let Some(c) = chars.next() {
        if c != '<' {
            out.push(char_key(c));
            continue;
        }
        let name: String = chars.by_ref().take_while(|&c| c != '>').collect();
        let lower = name.to_lowercase();
        let code = match lower.as_str() {
            "esc" => KeyCode::Esc,
            "cr" | "enter" => KeyCode::Enter,
            "bs" | "backspace" => KeyCode::Backspace,
            "del" => KeyCode::Delete,
            "tab" => KeyCode::Tab,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "lt" => {
                out.push(char_key('<'));
                continue;
            }
            "space" => {
                out.push(char_key(' '));
                continue;
            }
            n => {
                let mut rest = n.strip_prefix("c-").unwrap_or_default().chars();
                match (rest.next(), rest.next()) {
                    (Some(c), None) => {
                        out.push(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
                        continue;
                    }
                    _ => panic!("unknown key name <{name}>"),
                }
            }
        };
        out.push(KeyEvent::from(code));
    }
    out
}
