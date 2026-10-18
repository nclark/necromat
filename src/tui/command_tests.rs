//! Command mode, save, load, quit, the quit guard, the session save on
//! exit, and the help modal.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::editor::Mode;
use super::harness::Harness;
use super::help;
use super::view::{CursorPlacement, CursorShape};

const E37: &str = "E37: No write since last change (add ! to override)";

/// A harness editing the named file `path`, as `nm path` starts.
fn named(lines: &[&str], path: &Path) -> Harness {
    let mut h = Harness::new(lines);
    h.app.file_path = Some(path.to_path_buf());
    h.app.session_mode = false;
    h
}

fn status(h: &Harness) -> &str {
    h.app.status_msg.as_deref().unwrap_or_default()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

/// Runs `:cmd` and Enter.
fn cmd(h: &mut Harness, line: &str) {
    h.keys(":").type_text(line).keys("<CR>");
}

// --------------------------------------------------------------- commands

#[test]
fn write_saves_to_the_current_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.nm");
    let mut h = named(&["x=1", "y=2"], &path);
    h.keys("A<BS>3<Esc>");
    assert!(h.app.modified);
    cmd(&mut h, "w");
    assert_eq!(read(&path), "x=3\ny=2\n");
    assert!(!h.app.modified);
    assert_eq!(status(&h), format!("\"{}\" written", path.display()));
    assert!(!h.app.should_quit());
}

#[test]
fn write_with_a_path_saves_there_and_names_the_buffer() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("newfile.nm");
    // Wide enough for the whole path in the status bar: temp directories
    // can be long (macOS puts them under /var/folders/...).
    let width = u16::try_from(path.display().to_string().len() + 40).unwrap();
    let mut h = Harness::sized(&["x=1"], width, 24);
    h.keys("x");
    assert!(h.app.modified);
    cmd(&mut h, &format!("w {}", path.display()));
    assert_eq!(read(&path), "=1\n");
    assert_eq!(h.app.file_path.as_deref(), Some(path.as_path()));
    assert!(!h.app.session_mode);
    assert!(!h.app.modified);
    assert!(status(&h).contains(&path.display().to_string()));
    // The status bar now names the file.
    h.keys("l");
    let shown = format!("  NORMAL  {} │", path.display());
    assert!(h.render().status().starts_with(&shown));
}

#[test]
fn write_without_a_file_name() {
    let mut h = Harness::new(&["x=1"]);
    cmd(&mut h, "w");
    assert_eq!(status(&h), "E32: No file name");
    assert_eq!(h.app.file_path, None);
}

#[test]
fn write_long_form() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.nm");
    let mut h = named(&["x=1"], &path);
    cmd(&mut h, "write");
    assert_eq!(read(&path), "x=1\n");
}

#[test]
fn quit_command() {
    for line in ["q", "quit", "q!", "quit!"] {
        let mut h = Harness::new(&["1"]);
        cmd(&mut h, line);
        assert!(h.app.should_quit(), "{line}");
    }
}

#[test]
fn write_quit() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.nm");
    let mut h = named(&["x=1"], &path);
    cmd(&mut h, "wq");
    assert_eq!(read(&path), "x=1\n");
    assert!(h.app.should_quit());
}

#[test]
fn unknown_command() {
    for (line, want) in [
        ("unknown", "Unknown command: unknown"),
        ("foo bar", "Unknown command: foo"),
        ("123", "Unknown command: 123"),
        ("wqa", "Unknown command: wqa"),
    ] {
        let mut h = Harness::new(&["1"]);
        cmd(&mut h, line);
        assert_eq!(status(&h), want);
        assert!(!h.app.should_quit());
    }
}

#[test]
fn empty_command_does_nothing() {
    for line in ["", "   "] {
        let mut h = Harness::new(&["1"]);
        cmd(&mut h, line);
        assert_eq!(h.app.status_msg, None);
        assert!(!h.app.should_quit());
        assert_eq!(h.app.command, None);
    }
}

#[test]
fn edit_loads_a_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.nm");
    fs::write(&path, "x=10\ny=20\nz=x+y\n").unwrap();
    let mut h = Harness::new(&["old content"]);
    h.keys("x");
    cmd(&mut h, &format!("e {}", path.display()));
    assert_eq!(h.lines(), ["x=10", "y=20", "z=x+y"]);
    assert_eq!(h.app.file_path.as_deref(), Some(path.as_path()));
    assert!(!h.app.modified);
    assert!(!h.app.session_mode);
    assert_eq!(status(&h), format!("\"{}\" 3 lines", path.display()));
    assert_eq!(h.app.results[2].value, Some(30.0));
}

#[test]
fn edit_long_form() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.nm");
    fs::write(&path, "content\n").unwrap();
    let mut h = Harness::new(&[""]);
    cmd(&mut h, &format!("edit {}", path.display()));
    assert_eq!(h.app.file_path.as_deref(), Some(path.as_path()));
    assert_eq!(h.lines(), ["content"]);
}

#[test]
fn edit_without_an_argument() {
    let mut h = Harness::new(&["1"]);
    cmd(&mut h, "e");
    assert_eq!(status(&h), "E471: Argument required");
}

#[test]
fn edit_a_missing_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nonexistent.nm");
    let mut h = Harness::new(&["keep"]);
    cmd(&mut h, &format!("e {}", path.display()));
    let msg = status(&h);
    assert!(msg.starts_with("Error: "), "{msg}");
    assert!(msg.contains(&path.display().to_string()), "{msg}");
    assert_eq!(h.lines(), ["keep"]);
    assert_eq!(h.app.file_path, None);
}

#[test]
fn write_failure_reports_the_error() {
    let dir = TempDir::new().unwrap();
    // A directory where the file should be.
    let mut h = named(&["x"], dir.path());
    h.keys("x");
    cmd(&mut h, "w");
    assert!(status(&h).starts_with("Error: "), "{}", status(&h));
    assert!(h.app.modified);
    // `:wq` and `ZZ` stay when the write fails.
    cmd(&mut h, "wq");
    assert!(!h.app.should_quit());
    h.keys("ZZ");
    assert!(!h.app.should_quit());
    assert!(status(&h).starts_with("Error: "));
}

#[test]
fn tilde_expands_in_paths() {
    let Some(home) = crate::storage::home_dir() else {
        return;
    };
    let name = "nm-command-test-no-such-file.nm";
    let mut h = Harness::new(&[""]);
    cmd(&mut h, &format!("e ~/{name}"));
    let want = home.join(name).display().to_string();
    assert!(status(&h).contains(&want), "{}", status(&h));
}

#[test]
fn paths_may_hold_spaces_and_unicode() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("my budget é.nm");
    let mut h = Harness::new(&["1"]);
    cmd(&mut h, &format!("w {}", path.display()));
    assert_eq!(read(&path), "1\n");
    // No space needed after the name or the bang.
    let other = dir.path().join("b.nm");
    fs::write(&other, "2\n").unwrap();
    cmd(&mut h, &format!("e!{}", other.display()));
    assert_eq!(h.lines(), ["2"]);
}

// ------------------------------------------------------------- quit guard

#[test]
fn quit_guard_refuses_a_modified_named_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.nm");
    for quit in ["q", ":q<CR>", ":quit<CR>"] {
        let mut h = named(&["1"], &path);
        h.keys("x").keys(quit);
        assert!(!h.app.should_quit(), "{quit}");
        assert_eq!(status(&h), E37, "{quit}");
        // The message lasts until the next key.
        h.keys("l");
        assert_eq!(h.app.status_msg, None);
    }
}

#[test]
fn quit_guard_lets_these_through() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.nm");
    // Unmodified named file.
    for quit in ["q", ":q<CR>", "ZZ", "ZQ", ":x<CR>"] {
        let mut h = named(&["1"], &path);
        h.keys(quit);
        assert!(h.app.should_quit(), "unmodified {quit}");
    }
    // Session mode, modified: quits at once (the session is saved on exit).
    for quit in ["q", ":q<CR>", "ZZ", "ZQ", ":x<CR>", ":wq<CR>"] {
        let mut h = Harness::new(&["1"]);
        h.keys("x").keys(quit);
        assert!(h.app.should_quit(), "session {quit}");
        assert_eq!(h.app.status_msg, None, "session {quit}");
    }
}

#[test]
fn force_quit_discards() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.nm");
    fs::write(&path, "1\n").unwrap();
    for quit in [":q!<CR>", ":quit!<CR>", "ZQ"] {
        let mut h = named(&["1"], &path);
        h.keys("x").keys(quit);
        assert!(h.app.should_quit(), "{quit}");
        assert_eq!(read(&path), "1\n", "{quit}");
    }
}

#[test]
fn save_and_quit() {
    for quit in ["ZZ", ":x<CR>", ":wq<CR>"] {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.nm");
        let mut h = named(&["12"], &path);
        h.keys("x").keys(quit);
        assert!(h.app.should_quit(), "{quit}");
        assert_eq!(read(&path), "2\n", "{quit}");
    }
    // `:x` and `ZZ` write only when there are changes; `:wq` always does.
    for (quit, writes) in [("ZZ", false), (":x<CR>", false), (":wq<CR>", true)] {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.nm");
        let mut h = named(&["1"], &path);
        h.keys(quit);
        assert!(h.app.should_quit(), "{quit}");
        assert_eq!(path.exists(), writes, "{quit}");
    }
    // A path after `:wq` or `:x` writes there, from session mode too.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("b.nm");
    let mut h = Harness::new(&["5"]);
    cmd(&mut h, &format!("x {}", path.display()));
    assert!(h.app.should_quit());
    assert_eq!(read(&path), "5\n");
}

#[test]
fn edit_guard() {
    let dir = TempDir::new().unwrap();
    let a = dir.path().join("a.nm");
    let b = dir.path().join("b.nm");
    fs::write(&a, "1\n").unwrap();
    fs::write(&b, "2\n").unwrap();
    let mut h = named(&["1"], &a);
    h.keys("x");
    cmd(&mut h, &format!("e {}", b.display()));
    assert_eq!(status(&h), E37);
    assert_eq!(h.lines(), [""]);
    cmd(&mut h, &format!("e! {}", b.display()));
    assert_eq!(h.lines(), ["2"]);
    assert_eq!(h.app.file_path.as_deref(), Some(b.as_path()));
    // `:e!` alone reloads the current file, dropping changes.
    h.keys("x");
    cmd(&mut h, "e!");
    assert_eq!(h.lines(), ["2"]);
    assert!(!h.app.modified);
    // ... and needs a file.
    let mut h = Harness::new(&["1"]);
    cmd(&mut h, "e!");
    assert_eq!(status(&h), "E32: No file name");
}

#[test]
fn z_commands_need_their_second_key() {
    let mut h = Harness::new(&["abc"]);
    h.keys("Zx");
    assert!(!h.app.should_quit());
    // The key after `Z` is dropped, as in vim.
    assert_eq!(h.lines(), ["abc"]);
    h.keys("x");
    assert_eq!(h.lines(), ["bc"]);
    // `Z` as a find target is not a command.
    let mut h = Harness::new(&["aZb"]);
    h.keys("fZZ");
    assert_eq!(h.cursor(), (0, 1));
    assert!(!h.app.should_quit());
}

// ------------------------------------------------------- the command line

#[test]
fn command_line_on_the_status_row() {
    let mut h = Harness::new(&["1"]);
    h.keys(":");
    let screen = h.render();
    assert_eq!(screen.status().trim_end(), " :");
    let bar = Some(CursorPlacement {
        x: 2,
        y: 23,
        shape: CursorShape::Bar,
    });
    assert_eq!(screen.cursor, bar);
    h.type_text("wq");
    let screen = h.render();
    assert_eq!(screen.status().trim_end(), " :wq");
    assert_eq!(screen.cursor.map(|c| (c.x, c.y)), Some((4, 23)));
    // Wide characters count by width.
    h.type_text("€表");
    assert_eq!(h.render().cursor.map(|c| c.x), Some(4 + 1 + 2));
}

#[test]
fn command_line_editing() {
    let mut h = Harness::new(&["abc"]);
    h.keys(":dd");
    assert_eq!(h.app.command.as_deref(), Some("dd"));
    assert_eq!(h.lines(), ["abc"], "typed text is the command, not edits");
    h.keys("<BS>");
    assert_eq!(h.app.command.as_deref(), Some("d"));
    h.keys("<BS>");
    assert_eq!(h.app.command.as_deref(), Some(""));
    // Backspace on an empty command leaves command mode.
    h.keys("<BS>");
    assert_eq!(h.app.command, None);
    assert_eq!(h.mode(), Mode::Normal);
    assert!(h.render().status().starts_with("  NORMAL"));
    // Esc leaves too, running nothing.
    h.keys(":q<Esc>");
    assert_eq!(h.app.command, None);
    assert!(!h.app.should_quit());
    // Backspace removes a whole character.
    h.keys(":é€<BS>");
    assert_eq!(h.app.command.as_deref(), Some("é"));
    // Arrows and Ctrl keys do nothing.
    h.keys("<Left><Up><C-a>");
    assert_eq!(h.app.command.as_deref(), Some("é"));
    assert_eq!(h.lines(), ["abc"]);
}

#[test]
fn colon_drops_a_pending_count() {
    let lines = ["l0", "l1", "l2", "l3", "l4"];
    let mut h = Harness::new(&lines);
    h.keys("3:<Esc>j");
    assert_eq!(h.cursor(), (1, 0));
    // A refused `q` drops it too.
    let dir = TempDir::new().unwrap();
    let mut h = named(&lines, &dir.path().join("a.nm"));
    h.keys("x3qj");
    assert_eq!(status(&h), "");
    assert_eq!(h.cursor(), (1, 0));
}

#[test]
fn ctrl_s_saves_from_any_mode() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.nm");
    let mut h = named(&["1"], &path);
    h.keys("A2<C-s>");
    assert_eq!(read(&path), "12\n");
    assert_eq!(h.mode(), Mode::Insert, "Ctrl+S stays in insert mode");
    assert!(!h.app.modified);
    h.type_text("3");
    assert!(h.app.modified);
    h.keys("<Esc>v<C-s>");
    assert_eq!(read(&path), "123\n");
    assert_eq!(h.mode(), Mode::Visual);
    h.keys("<Esc>x:w<C-s>");
    assert_eq!(read(&path), "12\n");
    assert_eq!(h.app.command, None, "Ctrl+S closes the command line");
    assert_eq!(status(&h), format!("\"{}\" written", path.display()));
    // Session mode has no file name.
    let mut h = Harness::new(&["1"]);
    h.keys("<C-s>");
    assert_eq!(status(&h), "E32: No file name");
}

#[test]
fn modified_flag_and_indicator() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.nm");
    let mut h = named(&["1"], &path);
    assert!(
        h.render()
            .status()
            .starts_with(&format!("  NORMAL  {} │", path.display()))
    );
    h.keys("x");
    assert!(h.render().status().contains(" [+] │"));
    cmd(&mut h, "w");
    h.keys("l");
    assert!(!h.render().status().contains("[+]"));
    // Undo is a change too.
    h.keys("u");
    assert!(h.app.modified);
}

#[test]
fn edit_resets_cursor_viewport_and_undo() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.nm");
    fs::write(&path, "1\n2\n").unwrap();
    let lines: Vec<String> = (0..60).map(|i| i.to_string()).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut h = Harness::new(&refs);
    h.keys("Gx");
    assert!(h.app.viewport_start > 0);
    cmd(&mut h, &format!("e {}", path.display()));
    assert_eq!(h.cursor(), (0, 0));
    assert_eq!(h.app.viewport_start, 0);
    h.keys("u");
    assert_eq!(status(&h), "Already at oldest change");
    assert_eq!(h.lines(), ["1", "2"]);
    assert!(!h.app.modified);
}

// ---------------------------------------------------------- session save

fn session(lines: &[&str], dir: &TempDir) -> (Harness, PathBuf) {
    let path = dir.path().join("nm").join("last-session.nm");
    let mut h = Harness::new(lines);
    h.app.session_path = Some(path.clone());
    (h, path)
}

#[test]
fn session_is_saved_on_exit() {
    let dir = TempDir::new().unwrap();
    let (mut h, path) = session(&["x=100", "y=200", "total=x+y"], &dir);
    h.keys("q");
    assert!(h.app.should_quit());
    assert!(matches!(h.app.save_session(), Ok(true)));
    assert_eq!(
        crate::storage::load(&path).unwrap(),
        ["x=100", "y=200", "total=x+y"]
    );
}

#[test]
fn named_file_is_never_auto_saved() {
    let dir = TempDir::new().unwrap();
    let (mut h, path) = session(&["x=100"], &dir);
    let file = dir.path().join("budget.nm");
    h.app.file_path = Some(file.clone());
    h.app.session_mode = false;
    h.keys("x:q!<CR>");
    assert!(matches!(h.app.save_session(), Ok(false)));
    assert!(!path.exists());
    assert!(!file.exists());
}

#[test]
fn blank_session_is_not_saved() {
    let dir = TempDir::new().unwrap();
    let (h, path) = session(&["", "   ", "\t"], &dir);
    assert!(matches!(h.app.save_session(), Ok(false)));
    assert!(!path.exists());
}

#[test]
fn session_save_failure_is_reported() {
    let dir = TempDir::new().unwrap();
    let mut h = Harness::new(&["1"]);
    // A directory where the file should be.
    h.app.session_path = Some(dir.path().to_path_buf());
    let (path, _) = h.app.save_session().unwrap_err();
    assert_eq!(path, dir.path());
}

#[test]
fn edit_from_session_mode_saves_the_session_first() {
    let dir = TempDir::new().unwrap();
    let (mut h, path) = session(&["typed"], &dir);
    let file = dir.path().join("a.nm");
    fs::write(&file, "1\n").unwrap();
    cmd(&mut h, &format!("e {}", file.display()));
    assert_eq!(h.lines(), ["1"]);
    assert_eq!(read(&path), "typed\n");
    // Now a named file: no session save on exit.
    h.keys("x:q!<CR>");
    assert!(matches!(h.app.save_session(), Ok(false)));
    assert_eq!(read(&path), "typed\n");
}

#[test]
fn write_leaves_session_mode() {
    let dir = TempDir::new().unwrap();
    let (mut h, session_path) = session(&["1"], &dir);
    let file = dir.path().join("a.nm");
    cmd(&mut h, &format!("w {}", file.display()));
    h.keys("x");
    // A named file now, so the guard applies and nothing goes to the
    // session.
    h.keys("q");
    assert_eq!(status(&h), E37);
    assert!(matches!(h.app.save_session(), Ok(false)));
    assert!(!session_path.exists());
}

// ------------------------------------------------------------- help modal

/// The help modal at 80x24: the whole box must fit.
#[test]
fn help_modal_fits_80x24() {
    let mut h = Harness::new(&["100", "200"]);
    h.keys("?");
    assert!(h.app.help);
    let screen = h.render();
    assert_eq!(screen.cursor, None, "no cursor while help is shown");
    let text = screen.rows.join("\n");
    for want in [
        "nm Keybindings",
        "NORMAL MODE",
        "INSERT MODE",
        "VISUAL MODE",
        "COMMANDS",
        "Press any key to close",
        "Ctrl+R",
        "Undo/redo",
        "COUNTS & OPERATORS",
        "Delete/change/yank + motion",
        "Go to line 3",
        ":q! ZQ",
        ":wq :x ZZ",
        "Ctrl+S",
        "Tab",
        "Accept completion",
        "Ctrl+N",
        "Ctrl+P",
        "Next/previous match",
    ] {
        assert!(text.contains(want), "help is missing {want:?}\n{text}");
    }
    // The whole rounded box is on screen: top and bottom corners, and
    // side borders on every row between them.
    let top = screen.rows.iter().position(|r| r.contains('╭')).unwrap();
    let bottom = screen.rows.iter().position(|r| r.contains('╰')).unwrap();
    assert!(top < bottom);
    let left = screen.rows[top].chars().position(|c| c == '╭').unwrap();
    let right = screen.rows[top].chars().position(|c| c == '╮').unwrap();
    assert!(screen.rows[bottom].chars().nth(right) == Some('╯'));
    for row in &screen.rows[top + 1..bottom] {
        let chars: Vec<char> = row.chars().collect();
        assert_eq!(chars[left], '│', "{row}");
        assert_eq!(chars[right], '│', "{row}");
    }
    // Centred.
    assert!(left.abs_diff(79 - right) <= 1, "{left} {right}");
    assert!(top.abs_diff(23 - bottom) <= 1, "{top} {bottom}");
}

#[test]
fn any_key_closes_help_and_does_nothing_else() {
    for key in ["x", "j", "<Esc>", "<CR>", "?", ":", "q", "<C-s>"] {
        let mut h = Harness::new(&["abc", "def"]);
        h.keys("?").keys(key);
        assert!(!h.app.help, "{key}");
        assert_eq!(h.lines(), ["abc", "def"], "{key}");
        assert_eq!(h.cursor(), (0, 0), "{key}");
        assert!(!h.app.should_quit(), "{key}");
        assert_eq!(h.app.command, None, "{key}");
        assert_eq!(h.app.status_msg, None, "{key}");
        assert!(h.render().cursor.is_some(), "{key}");
    }
    // Ctrl+C still quits.
    let mut h = Harness::new(&["1"]);
    h.keys("?<C-c>");
    assert!(h.app.should_quit());
}

#[test]
fn question_mark_as_a_find_target_is_not_help() {
    let mut h = Harness::new(&["a?b"]);
    h.keys("f?");
    assert!(!h.app.help);
    assert_eq!(h.cursor(), (0, 1));
}

#[test]
fn help_in_tiny_terminals_does_not_panic() {
    for (w, h) in [(1, 1), (10, 3), (40, 10), (79, 23)] {
        let mut hs = Harness::sized(&["1"], w, h);
        hs.keys("?");
        assert_eq!(hs.render().cursor, None);
    }
}

/// The help text is a pick from `doc/keys.md`: every key it names must be
/// in that file, in a code span.
#[test]
fn help_keys_are_documented() {
    let doc = include_str!("../../doc/keys.md");
    let mut words = std::collections::HashSet::new();
    for (i, span) in doc.split('`').enumerate() {
        if i % 2 == 1 {
            for w in span.split_whitespace() {
                words.insert(w.trim_end_matches("{c}").to_string());
            }
        }
    }
    for row in help::LEFT.iter().chain(help::RIGHT) {
        if let help::Row::Key(_, keys, _) = row {
            for k in keys.split_whitespace() {
                let count = k.starts_with(|c: char| c.is_ascii_digit());
                assert!(
                    count || words.contains(k),
                    "help key {k:?} is not in doc/keys.md"
                );
            }
        }
    }
}
