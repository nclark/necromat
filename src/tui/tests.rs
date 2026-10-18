//! TUI tests: layout, cursor, insert and normal basics, plus checks of
//! the hjkl integration.

// The case tables are tuples, to keep each case on one line.
#![allow(clippy::type_complexity)]

use ratatui::style::Modifier;

use super::app::scroll_to_cursor;
use super::editor::Mode;
use super::harness::{Harness, parse_keys};
use super::style;
use super::view::{CursorPlacement, CursorShape, gutter_width};

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn place(x: u16, y: u16, shape: CursorShape) -> Option<CursorPlacement> {
    Some(CursorPlacement { x, y, shape })
}

// ------------------------------------------------------------------ state

#[test]
fn new_app_has_one_empty_line_in_normal_mode() {
    let h = Harness::new(&[]);
    assert_eq!(h.lines(), [""]);
    assert_eq!(h.mode(), Mode::Normal);
    assert_eq!(h.cursor(), (0, 0));
    assert!(!h.app.modified);
}

#[test]
fn content_joins_lines() {
    let cases: [(&[&str], &str); 3] = [
        (&[""], ""),
        (&["100"], "100"),
        (&["100", "200", "sum"], "100\n200\nsum"),
    ];
    for (lines, want) in cases {
        assert_eq!(Harness::new(lines).app.content(), want, "{lines:?}");
    }
}

/// The cursor line (out of bounds cannot happen: the editor clamps).
#[test]
fn current_line_follows_cursor() {
    let lines = ["100", "200", "sum"];
    for (row, want) in [(0, "100"), (1, "200"), (2, "sum")] {
        assert_eq!(Harness::at(&lines, row, 0).app.current_line(), want);
    }
    assert_eq!(Harness::at(&["100", "200"], 5, 0).app.current_line(), "200");
}

/// Assignments show their values
/// and add to the accumulator.
#[test]
fn assignments_have_results() {
    let h = Harness::new(&["one=1", "two=2", "sum"]);
    let values: Vec<_> = h.app.results.iter().map(|r| r.value).collect();
    assert_eq!(values, [Some(1.0), Some(2.0), Some(3.0)]);
}

// ---------------------------------------------------------------- update

#[test]
fn insert_text() {
    let cases: [(&[&str], usize, usize, &str, &[&str], usize); 5] = [
        (&["world"], 0, 0, "hello ", &["hello world"], 6),
        (&["hello"], 0, 5, " world", &["hello world"], 11),
        (&["helloworld"], 0, 5, " ", &["hello world"], 6),
        (&["80#DogFood"], 0, 2, " ", &["80 #DogFood"], 3),
        (&["100", "200"], 1, 0, "x", &["100", "x200"], 1),
    ];
    for (lines, row, col, text, want, want_col) in cases {
        let mut h = Harness::inserting(lines, row, col);
        h.type_text(text);
        assert_eq!(h.lines(), strs(want), "{lines:?} + {text:?}");
        assert_eq!(h.cursor(), (row, want_col), "{lines:?} + {text:?}");
    }
}

#[test]
fn backspace() {
    let cases: [(&[&str], usize, usize, &[&str], (usize, usize)); 4] = [
        (&["hello"], 0, 5, &["hell"], (0, 4)),
        (&["hello"], 0, 0, &["hello"], (0, 0)),
        (&["hello", "world"], 1, 0, &["helloworld"], (0, 5)),
        (&["100", "200"], 1, 2, &["100", "20"], (1, 1)),
    ];
    for (lines, row, col, want, want_cursor) in cases {
        let mut h = Harness::inserting(lines, row, col);
        h.keys("<BS>");
        assert_eq!(h.lines(), strs(want), "{lines:?} at {row},{col}");
        assert_eq!(h.cursor(), want_cursor, "{lines:?} at {row},{col}");
    }
}

#[test]
fn delete_key() {
    let cases: [(&[&str], usize, &[&str], usize); 4] = [
        (&["hello"], 0, &["ello"], 0),
        (&["hello", "world"], 5, &["helloworld"], 5),
        (&["hello"], 5, &["hello"], 5),
        (&["hello"], 2, &["helo"], 2),
    ];
    for (lines, col, want, want_col) in cases {
        let mut h = Harness::inserting(lines, 0, col);
        h.keys("<Del>");
        assert_eq!(h.lines(), strs(want), "{lines:?} at {col}");
        assert_eq!(h.cursor(), (0, want_col), "{lines:?} at {col}");
    }
}

#[test]
fn enter_splits_line() {
    let cases: [(&[&str], usize, usize, &[&str]); 4] = [
        (&["helloworld"], 0, 5, &["hello", "world"]),
        (&["hello"], 0, 0, &["", "hello"]),
        (&["hello"], 0, 5, &["hello", ""]),
        (&["100", "200", "sum"], 1, 2, &["100", "20", "0", "sum"]),
    ];
    for (lines, row, col, want) in cases {
        let mut h = Harness::inserting(lines, row, col);
        h.keys("<CR>");
        assert_eq!(h.lines(), strs(want), "{lines:?} at {row},{col}");
        assert_eq!(h.cursor(), (row + 1, 0), "{lines:?} at {row},{col}");
    }
}

#[test]
fn scroll_keeps_cursor_visible() {
    // (cursor row, viewport start, height, want viewport start)
    let cases = [
        (5, 10, 12, 5),
        (15, 0, 12, 5),
        (5, 0, 12, 0),
        (10, 10, 12, 10),
        (5, 0, 0, 0),
    ];
    for (row, start, height, want) in cases {
        assert_eq!(
            scroll_to_cursor(row, start, height),
            want,
            "row {row} start {start} height {height}"
        );
    }
}

/// Basic key presses (command mode is tested in `command_tests.rs`).
#[test]
fn typing_backspace_left_delete() {
    let mut h = Harness::new(&[]);
    h.keys("i").type_text("a b");
    assert_eq!(h.app.current_line(), "a b");
    h.keys("<BS><Left><Del>");
    assert_eq!(h.app.current_line(), "a");
    h.keys("<C-c>");
    assert!(h.app.should_quit());
}

// ------------------------------------------------------------------- view

/// Results right-aligned with one trailing
/// space. Width 25 gives an input column of 15 and a results column of 10.
#[test]
fn result_column() {
    let cases = [
        ("100", "      100 "),
        ("1/0", "    div/0 "),
        ("# no result", "          "),
        ("", "          "),
        ("3.14", "     3.14 "),
    ];
    for (line, want) in cases {
        let h = Harness::sized(&[line], 25, 5);
        let screen = h.render();
        let got: String = screen.rows[0].chars().skip(15).collect();
        assert_eq!(got, want, "{line:?}");
    }
}

/// An error result: the short code is red.
#[test]
fn error_result_is_red() {
    let screen = Harness::sized(&["1/0"], 25, 5).render();
    assert_eq!(screen.buffer[(19, 0)].symbol(), "d");
    assert_eq!(screen.buffer[(19, 0)].fg, style::ERROR);
    let screen = Harness::sized(&["100"], 25, 5).render();
    assert_eq!(screen.buffer[(21, 0)].fg, style::RESULT);
}

/// The line text is drawn whole, with no cursor glyph splitting it, in
/// every mode and cursor position.
#[test]
fn input_line_has_no_cursor_glyph() {
    let cases: [(&str, usize); 7] = [
        ("hello", 2),
        ("hello", 5),
        ("200", 1),
        ("abc", 0),
        ("100 # test", 4),
        ("100 # test", 3),
        ("12345", 2),
    ];
    for (line, col) in cases {
        for h in [
            Harness::at(&[line], 0, col),
            Harness::inserting(&[line], 0, col),
        ] {
            let screen = h.render();
            assert!(
                screen.rows[0].starts_with(&format!(" 1 │ {line} ")),
                "{:?}",
                screen.rows[0]
            );
            assert!(!screen.rows.iter().any(|r| r.contains(['▏', '█'])));
        }
    }
}

/// The content shows and the cursor is placed.
#[test]
fn view_smoke() {
    let screen = Harness::new(&["100", "200", "sum"]).render();
    assert!(screen.rows[0].contains("100"));
    assert!(screen.rows[2].trim_end().ends_with("300"));
    assert!(screen.cursor.is_some());
    assert_eq!(screen.rows.len(), 24);
}

/// Cursor placement. The gutter is 5 wide up to 99 lines.
#[test]
fn view_cursor() {
    use CursorShape::{Bar, Block};

    let many = vec!["x = 1"; 30];
    let mut scrolled = Harness::sized(&many, 80, 10);
    scrolled.app.editor.set_cursor(27, 3);
    scrolled.keys("i");
    scrolled.app.viewport_start = 25;

    let mut visual = Harness::at(&["abc"], 0, 1);
    visual.keys("v");

    let cases = [
        (
            "insert mid-line",
            Harness::inserting(&["hello"], 0, 2),
            place(7, 0, Bar),
        ),
        (
            "insert past end of line",
            Harness::inserting(&["hello"], 0, 5),
            place(10, 0, Bar),
        ),
        (
            "insert after = with spaces",
            Harness::inserting(&["x = 1"], 0, 3),
            place(8, 0, Bar),
        ),
        (
            "insert on second line",
            Harness::inserting(&["a", "x = 1"], 1, 4),
            place(9, 1, Bar),
        ),
        ("scrolled viewport", scrolled, place(8, 2, Bar)),
        (
            "normal mode on character",
            Harness::at(&["x = 1"], 0, 4),
            place(9, 0, Block),
        ),
        (
            "normal mode empty line",
            Harness::new(&[""]),
            place(5, 0, Block),
        ),
        ("visual mode", visual, place(6, 0, Block)),
    ];
    for (name, h, want) in cases {
        assert_eq!(h.render().cursor, want, "{name}");
    }
}

/// The cursor cell shows the character
/// under the cursor.
#[test]
fn cursor_cell_matches_content() {
    let line = "x = 1 + 2 # note";
    let screen = Harness::at(&[line], 0, "x = 1 + ".len()).render();
    let p = screen.cursor.expect("cursor");
    assert_eq!(screen.buffer[(p.x, p.y)].symbol(), "2");
}

/// A worked example: a quick sum, then a small budget.
const BUDGET: [&str; 13] = [
    "100",
    "-10",
    "sum",
    "",
    "starting_balance=5000",
    "taxes=starting_balance*0.28",
    "",
    "90 # Internet",
    "25 # Mint Mobile",
    "85 # Dog food",
    "bills=sum",
    "",
    "balance=starting_balance-taxes-bills",
];

#[test]
fn budget_example_layout() {
    let mut h = Harness::sized(&BUDGET, 80, 16);
    let screen = h.render();
    let want_values = [
        "100", "-10", "90", "", "5,000", "1,400", "", "90", "25", "85", "200", "", "3,400",
    ];
    for (i, (line, value)) in BUDGET.iter().zip(want_values).enumerate() {
        let row = &screen.rows[i];
        let gutter = format!("{:>2} │ ", i + 1);
        assert!(row.starts_with(&format!("{gutter}{line}")), "{row:?}");
        // Input column: 60% of 80 = 48 cells, gutter included.
        let input: String = row.chars().take(48).collect();
        assert_eq!(input.trim_end(), format!("{gutter}{line}").trim_end());
        let result: String = row.chars().skip(48).collect();
        assert_eq!(result.len(), 32);
        assert_eq!(result, format!("{value:>31} "), "row {i}");
    }
    assert_eq!(screen.rows[13].trim(), "");
    assert_eq!(screen.rows[14].trim(), "");

    // Status bar: badge, session indicator, total of the cursor's section
    // (100 - 10; `sum` is not counted twice), line count.
    assert_eq!(
        screen.status().trim_end(),
        "  NORMAL  [session] │ total: 90 │ 13 lines"
    );
    assert_eq!(screen.status().chars().count(), 80);

    // Gutter styles: current line number bold gold, others gray,
    // separator dim.
    let cur = &screen.buffer[(1, 0)];
    assert_eq!(cur.fg, style::CURRENT_LINE_NR);
    assert!(cur.modifier.contains(Modifier::BOLD));
    assert_eq!(screen.buffer[(1, 1)].fg, style::GUTTER);
    assert_eq!(screen.buffer[(3, 1)].fg, style::SEPARATOR);
    // Status bar colours and the NORMAL badge.
    assert_eq!(screen.buffer[(0, 15)].bg, style::STATUS_BG);
    assert_eq!(screen.buffer[(79, 15)].bg, style::STATUS_BG);
    assert_eq!(screen.buffer[(2, 15)].bg, style::BADGE_NORMAL);
    assert_eq!(screen.buffer[(2, 15)].fg, style::STATUS_BG);

    // The total follows the cursor's section.
    h.keys("11G");
    assert_eq!(h.cursor(), (10, 0));
    assert!(h.render().status().contains("│ total: 200 │"));
    h.keys("G");
    assert!(h.render().status().contains("│ total: 3,400 │"));
    h.keys("5G");
    assert!(h.render().status().contains("│ total: 6,400 │"));
}

#[test]
fn status_bar_badges_and_indicators() {
    let mut h = Harness::new(&["1"]);
    h.keys("A");
    let screen = h.render();
    assert!(screen.status().starts_with("  INSERT  [session] │"));
    assert_eq!(screen.buffer[(2, 23)].bg, style::BADGE_INSERT);
    h.keys("2<Esc>");
    assert!(
        h.render()
            .status()
            .starts_with("  NORMAL  [session] [+] │ total: 12 │ 1 lines")
    );
    h.keys("V");
    let screen = h.render();
    assert!(
        screen
            .status()
            .starts_with("  VISUAL LINE  [session] [+] │")
    );
    assert_eq!(screen.buffer[(2, 23)].bg, style::BADGE_VISUAL);
    h.keys("<Esc>v");
    assert!(h.render().status().starts_with("  VISUAL  [session] [+] │"));

    // Empty session: no indicator until modified.
    let mut h = Harness::new(&[""]);
    assert!(
        h.render()
            .status()
            .starts_with("  NORMAL  │ total: 0 │ 1 lines")
    );
    h.keys("o<Esc>");
    assert!(
        h.render()
            .status()
            .starts_with("  NORMAL  [+] │ total: 0 │ 2 lines")
    );

    // A named file shows its path.
    let mut h = Harness::new(&["1"]);
    h.app.file_path = Some("budget.nm".into());
    h.app.session_mode = false;
    assert!(h.render().status().starts_with("  NORMAL  budget.nm │"));
    h.keys("x");
    assert!(h.render().status().starts_with("  NORMAL  budget.nm [+] │"));
}

#[test]
fn status_message_replaces_bar_until_next_key() {
    let mut h = Harness::new(&["1"]);
    h.app.status_msg = Some("\"a.nm\" written".into());
    assert_eq!(h.render().status().trim_end(), " \"a.nm\" written");
    h.keys("l");
    assert!(h.render().status().starts_with("  NORMAL"));
}

#[test]
fn gutter_grows_with_line_count() {
    assert_eq!(gutter_width(1), 5);
    assert_eq!(gutter_width(99), 5);
    assert_eq!(gutter_width(100), 6);
    let lines: Vec<String> = (1..=120).map(|i| i.to_string()).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let screen = Harness::new(&refs).render();
    assert!(
        screen.rows[0].starts_with("  1 │ 1"),
        "{:?}",
        screen.rows[0]
    );
    assert_eq!(screen.cursor, place(6, 0, CursorShape::Block));
}

#[test]
fn viewport_scrolls_with_cursor() {
    let lines: Vec<String> = (0..30).map(|i| format!("l{i}")).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut h = Harness::sized(&refs, 40, 10);
    h.keys("G");
    // 9 content rows: lines 21..=29.
    assert_eq!(h.app.viewport_start, 21);
    let screen = h.render();
    assert!(
        screen.rows[0].starts_with("22 │ l21"),
        "{:?}",
        screen.rows[0]
    );
    assert!(screen.rows[8].starts_with("30 │ l29"));
    assert_eq!(screen.cursor, place(5, 8, CursorShape::Block));
    h.keys("gg");
    assert_eq!(h.app.viewport_start, 0);
    assert_eq!(h.render().cursor, place(5, 0, CursorShape::Block));
}

#[test]
fn long_lines_are_clipped_to_the_input_column() {
    let long = "1+".repeat(40) + "1";
    let screen = Harness::sized(&[&long], 40, 5).render();
    // Input column 24 = gutter 5 + 19 cells of text; the result follows.
    let input: String = screen.rows[0].chars().take(24).collect();
    assert_eq!(input, format!(" 1 │ {}", &long[..19]));
    assert!(screen.rows[0].ends_with("41 "));
}

#[test]
fn tiny_terminals_do_not_panic() {
    for (w, h) in [(0, 0), (1, 1), (0, 5), (5, 0), (3, 2), (10, 1)] {
        let mut harness = Harness::sized(&["income=5000", "x"], w, h);
        harness.keys("jA1<Esc>");
        let screen = harness.render();
        if let Some(p) = screen.cursor {
            assert!(p.x < w && p.y < h);
        }
    }
}

// ------------------------------------------------------- insert-mode keys

#[test]
fn insert_arrows_wrap_across_lines() {
    let mut h = Harness::inserting(&["abc", "de"], 1, 0);
    h.keys("<Left>");
    assert_eq!(h.cursor(), (0, 3));
    h.keys("<Right>");
    assert_eq!(h.cursor(), (1, 0));
    // No wrap past the ends of the buffer.
    let mut h = Harness::inserting(&["abc"], 0, 0);
    h.keys("<Left>");
    assert_eq!(h.cursor(), (0, 0));
    h.keys("<End><Right>");
    assert_eq!(h.cursor(), (0, 3));
    // Typing after a wrap lands where the cursor is.
    let mut h = Harness::inserting(&["abc", "de"], 1, 0);
    h.keys("<Left>X<Right>Y");
    assert_eq!(h.lines(), ["abcX", "Yde"]);
    assert_eq!(h.mode(), Mode::Insert);
}

/// Up and Down keep the column, clamped to the
/// line length (which is past the last character in insert mode). hjkl
/// clamps to the last character, and lands on column 0 after `A`.
#[test]
fn insert_up_down_keep_column_clamped() {
    let mut h = Harness::inserting(&["abcdef", "ab"], 0, 4);
    h.keys("<Down>");
    assert_eq!(h.cursor(), (1, 2));
    h.keys("<Up>");
    assert_eq!(h.cursor(), (0, 2));
    let mut h = Harness::new(&["abcdef", "ab", "abcd"]);
    h.keys("A<Down>");
    assert_eq!(h.cursor(), (1, 2));
    h.keys("<Down>");
    assert_eq!(h.cursor(), (2, 2));
    h.keys("<Up><Up><Up>");
    assert_eq!(h.cursor(), (0, 2));
    h.keys("<End>");
    assert_eq!(h.cursor(), (0, 6));
    h.keys("<Home>");
    assert_eq!(h.cursor(), (0, 0));
    // Typing after the moves lands at the cursor.
    h.keys("<Down><End>X");
    assert_eq!(h.lines(), ["abcdef", "abX", "abcd"]);
}

/// hjkl treats a buffer ending in `\n` as a file with a final newline and
/// never moves onto the empty row after it. nm keeps its own trailing
/// `\n` so a real empty last line (a section divider) stays reachable.
#[test]
fn empty_last_line_is_a_real_line() {
    let mut h = Harness::new(&["a", ""]);
    h.keys("G");
    assert_eq!(h.cursor(), (1, 0));
    h.keys("ix");
    assert_eq!(h.lines(), ["a", "x"]);

    let mut h = Harness::new(&["a"]);
    h.keys("o<Esc>kj");
    assert_eq!((h.lines(), h.cursor()), (strs(&["a", ""]), (1, 0)));
    h.keys("kG");
    assert_eq!(h.cursor(), (1, 0));

    // Delete at the end of an empty last line has nothing to join.
    let mut h = Harness::new(&["a", ""]);
    h.keys("GA<Del>");
    assert_eq!(h.lines(), ["a", ""]);
    assert_eq!(h.cursor(), (1, 0));
    // Backspace at the start of it joins upward as usual.
    h.keys("<BS>");
    assert_eq!((h.lines(), h.cursor()), (strs(&["a"]), (0, 1)));
}

/// Deleting every line leaves one empty line, and the buffer keeps working.
#[test]
fn deleting_every_line() {
    for keys in ["dddd", "dG", "VGd", "vGd"] {
        let mut h = Harness::new(&["a", "b"]);
        h.keys(keys);
        assert_eq!((h.lines(), h.cursor()), (strs(&[""]), (0, 0)), "{keys}");
        h.keys("o<Esc>");
        assert_eq!(
            (h.lines(), h.cursor()),
            (strs(&["", ""]), (1, 0)),
            "{keys}o"
        );
        h.keys("kj");
        assert_eq!(h.cursor(), (1, 0), "{keys}okj");
    }
    let mut h = Harness::new(&["a"]);
    h.keys("ddu");
    assert_eq!(h.lines(), ["a"]);
    h.keys("<C-r>");
    assert_eq!(h.lines(), [""]);
    // Redo can leave hjkl's cursor past the last line; nm pulls it back.
    let mut h = Harness::new(&["a", "b"]);
    h.keys("Gddu<C-r>");
    assert_eq!((h.lines(), h.cursor()), (strs(&["a"]), (0, 0)));
}

#[test]
fn esc_leaves_insert_and_steps_left() {
    let mut h = Harness::inserting(&["abc"], 0, 3);
    h.keys("<Esc>");
    assert_eq!((h.mode(), h.cursor()), (Mode::Normal, (0, 2)));
    let mut h = Harness::inserting(&["abc"], 0, 0);
    h.keys("<Esc>");
    assert_eq!((h.mode(), h.cursor()), (Mode::Normal, (0, 0)));
}

/// hjkl turns on autopair and autoindent by default; nm turns them off so
/// typed text goes in exactly as typed.
#[test]
fn typing_is_literal() {
    let mut h = Harness::new(&[]);
    h.keys("i").type_text("(1+2)*[3] \"x\" {y}");
    assert_eq!(h.lines(), ["(1+2)*[3] \"x\" {y}"]);
    let mut h = Harness::new(&[]);
    h.keys("i").type_text("  x = (1");
    h.keys("<CR>").type_text("+2)");
    assert_eq!(h.lines(), ["  x = (1", "+2)"]);
    let mut h = Harness::new(&[]);
    h.keys("i").type_text("# note");
    h.keys("<CR>").type_text("1");
    assert_eq!(h.lines(), ["# note", "1"]);
}

/// The completion keys never reach the engine: with nothing to complete
/// they do nothing (completion itself is tested in `completion_tests.rs`).
#[test]
fn completion_keys_do_not_reach_the_engine() {
    let mut h = Harness::new(&["income=5000", ""]);
    h.keys("Gi").type_text("zz");
    h.keys("<Tab><C-n><C-p>");
    assert_eq!(h.lines(), ["income=5000", "zz"]);
    assert_eq!(h.cursor(), (1, 2));
    assert_eq!(h.mode(), Mode::Insert);
}

// ------------------------------------------------------- normal-mode keys

#[test]
fn normal_mode_basics() {
    let ten: Vec<String> = (0..10).map(|i| format!("l{i}")).collect();
    let ten: Vec<&str> = ten.iter().map(String::as_str).collect();
    // (lines, start row, start col, keys, want lines, want cursor, want mode)
    let cases: Vec<(&[&str], usize, usize, &str, Vec<&str>, (usize, usize), Mode)> = vec![
        (&["abc"], 0, 1, "i", vec!["abc"], (0, 1), Mode::Insert),
        (&["abc"], 0, 1, "a", vec!["abc"], (0, 2), Mode::Insert),
        (&["abc"], 0, 0, "A", vec!["abc"], (0, 3), Mode::Insert),
        (&["", ""], 0, 0, "a", vec!["", ""], (0, 0), Mode::Insert),
        (
            &["a", "b"],
            0,
            0,
            "o",
            vec!["a", "", "b"],
            (1, 0),
            Mode::Insert,
        ),
        (
            &["a", "b"],
            1,
            0,
            "O",
            vec!["a", "", "b"],
            (1, 0),
            Mode::Insert,
        ),
        (&["abc"], 0, 0, "ll", vec!["abc"], (0, 2), Mode::Normal),
        (&["abc"], 0, 0, "lllll", vec!["abc"], (0, 2), Mode::Normal),
        (&["abc"], 0, 2, "hhhh", vec!["abc"], (0, 0), Mode::Normal),
        (
            &["abcdef", "ab"],
            0,
            4,
            "j",
            vec!["abcdef", "ab"],
            (1, 1),
            Mode::Normal,
        ),
        (
            &["a", "b"],
            1,
            0,
            "kk",
            vec!["a", "b"],
            (0, 0),
            Mode::Normal,
        ),
        (&ten, 0, 0, "jjj", ten.clone(), (3, 0), Mode::Normal),
        (
            &ten,
            0,
            0,
            "<Down><Down>",
            ten.clone(),
            (2, 0),
            Mode::Normal,
        ),
        (&["abcdef"], 0, 0, "$", vec!["abcdef"], (0, 5), Mode::Normal),
        (&["abcdef"], 0, 4, "0", vec!["abcdef"], (0, 0), Mode::Normal),
        (&ten, 0, 0, "G", ten.clone(), (9, 0), Mode::Normal),
        (&ten, 7, 0, "gg", ten.clone(), (0, 0), Mode::Normal),
        (&["abc"], 0, 1, "x", vec!["ac"], (0, 1), Mode::Normal),
        (&["abc"], 0, 2, "x", vec!["ab"], (0, 1), Mode::Normal),
        // `x` never joins lines, even on an empty line.
        (
            &["", "def"],
            0,
            0,
            "x",
            vec!["", "def"],
            (0, 0),
            Mode::Normal,
        ),
        (
            &["abc", "def"],
            0,
            2,
            "xx",
            vec!["a", "def"],
            (0, 0),
            Mode::Normal,
        ),
        (
            &["a", "b", "c"],
            1,
            0,
            "dd",
            vec!["a", "c"],
            (1, 0),
            Mode::Normal,
        ),
        (
            &["a", "b", "c"],
            2,
            0,
            "dd",
            vec!["a", "b"],
            (1, 0),
            Mode::Normal,
        ),
        (&["a"], 0, 0, "dd", vec![""], (0, 0), Mode::Normal),
        // Esc clears a pending operator.
        (
            &["a", "b", "c"],
            0,
            0,
            "d<Esc>j",
            vec!["a", "b", "c"],
            (1, 0),
            Mode::Normal,
        ),
    ];
    for (lines, row, col, keys, want, cursor, mode) in cases {
        let mut h = Harness::at(lines, row, col);
        h.keys(keys);
        assert_eq!(h.lines(), want, "{lines:?} {keys}");
        assert_eq!(h.cursor(), cursor, "{lines:?} {keys}");
        assert_eq!(h.mode(), mode, "{lines:?} {keys}");
        assert!(!h.app.should_quit(), "{keys}");
    }
}

#[test]
fn q_and_ctrl_c_quit() {
    let mut h = Harness::new(&["1"]);
    h.keys("q");
    assert!(h.app.should_quit());
    for keys in ["<C-c>", "i<C-c>", "v<C-c>", "d<C-c>"] {
        let mut h = Harness::new(&["1"]);
        h.keys(keys);
        assert!(h.app.should_quit(), "{keys}");
    }
    // `q` in insert mode is text.
    let mut h = Harness::new(&[""]);
    h.keys("iq");
    assert_eq!(h.lines(), ["q"]);
    assert!(!h.app.should_quit());
    // A count before `q` is ignored.
    let mut h = Harness::new(&["1"]);
    h.keys("3q");
    assert!(h.app.should_quit());
    // In visual mode `q` does nothing (hjkl would start a macro).
    let mut h = Harness::new(&["abc"]);
    h.keys("vqlx");
    assert_eq!(h.lines(), ["c"]);
    assert!(!h.app.should_quit());
    // `q` as the target of a pending command is not quit.
    let mut h = Harness::new(&["aqb"]);
    h.keys("fq");
    assert_eq!(h.cursor(), (0, 1));
    assert!(!h.app.should_quit());
}

#[test]
fn search_key_does_not_reach_the_engine() {
    // hjkl would open a search prompt on `/` and swallow the next keys.
    let mut h = Harness::new(&["abc"]);
    h.keys("/lx");
    assert_eq!(h.lines(), ["ac"]);
    assert_eq!(h.mode(), Mode::Normal);
}

// --------------------------------------------- evaluation after every edit

#[test]
fn edits_re_evaluate() {
    let mut h = Harness::new(&["a=2", "a*3"]);
    assert_eq!(h.app.results[1].value, Some(6.0));
    h.keys("A0<Esc>");
    assert_eq!(h.lines(), ["a=20", "a*3"]);
    assert_eq!(h.app.results[1].value, Some(60.0));
    assert!(h.app.modified);
    h.keys("dd");
    assert!(h.app.results[0].error.is_some());
    let screen = h.render();
    assert!(
        screen.rows[0].ends_with("undef: a "),
        "{:?}",
        screen.rows[0]
    );
}

#[test]
fn motions_do_not_mark_modified() {
    let mut h = Harness::new(&["1", "2"]);
    h.keys("jk$0ggGvV<Esc>");
    assert!(!h.app.modified);
}

// ---------------------------------------------------------------- unicode

/// Typing `é` must not corrupt the line. Cursor columns
/// are characters, never bytes.
#[test]
fn unicode_insert_move_delete() {
    let mut h = Harness::new(&["income=5000"]);
    h.keys("A").type_text("é");
    assert_eq!(h.lines(), ["income=5000é"]);
    assert_eq!(h.cursor(), (0, 12));
    assert_eq!(h.render().cursor, place(5 + 12, 0, CursorShape::Bar));
    h.keys("<Esc>");
    assert_eq!(h.cursor(), (0, 11));
    h.keys("hx");
    assert_eq!(h.lines(), ["income=500é"]);
    assert_eq!(h.cursor(), (0, 10));

    // `é` mid-line, so byte and character columns differ after it.
    let mut h = Harness::new(&["income=5000"]);
    h.keys("I").type_text("aéb");
    h.keys("<Esc>");
    assert_eq!(h.lines(), ["aébincome=5000"]);
    assert_eq!(h.cursor(), (0, 2));
    let screen = h.render();
    let p = screen.cursor.expect("cursor");
    assert_eq!(screen.buffer[(p.x, p.y)].symbol(), "b");
    h.keys("hx");
    assert_eq!(h.lines(), ["abincome=5000"]);

    // Backspace and Delete remove whole characters.
    let mut h = Harness::inserting(&["a€b"], 0, 2);
    h.keys("<BS>");
    assert_eq!(h.lines(), ["ab"]);
    let mut h = Harness::inserting(&["a€b"], 0, 1);
    h.keys("<Del>");
    assert_eq!(h.lines(), ["ab"]);
    // Insert-mode arrows step over `é` one character at a time.
    let mut h = Harness::inserting(&["é1", "x"], 1, 0);
    h.keys("<Left><Left><Left>");
    assert_eq!(h.cursor(), (0, 0));
}

/// The cursor column is the display width, so a double-width character
/// moves it two cells.
#[test]
fn wide_characters_place_cursor_by_width() {
    let h = Harness::at(&["日本 = 1"], 0, 2);
    let screen = h.render();
    assert_eq!(screen.cursor, place(5 + 4, 0, CursorShape::Block));
    assert_eq!(screen.buffer[(9, 0)].symbol(), " ");
    assert!(screen.rows[0].starts_with(" 1 │ 日本 = 1"));
}

#[test]
fn key_notation() {
    let keys = parse_keys("2d<Esc>x<C-r><lt><space>A");
    assert_eq!(keys.len(), 8);
}
