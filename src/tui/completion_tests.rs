//! Completion tests: candidates, the prefix rule (no completion inside an
//! identifier), the keys, and the ghost text on screen.

// The case tables are tuples, to keep each case on one line.
#![allow(clippy::type_complexity)]

use super::completion::{Completion, candidates, prefix};
use super::editor::Mode;
use super::harness::{Harness, Screen};
use super::style;
use super::view::{CursorShape, gutter_width};
use crate::lang::{LineKind, ParsedLine, parse_line};

fn assign(name: &str) -> ParsedLine {
    parse_line(&format!("{name} = 1"))
}

fn ghost(h: &Harness) -> Option<String> {
    h.app
        .completion
        .as_ref()
        .map(|c| c.ghost_text().to_string())
}

/// Insert mode at `(row, col)` with the completion computed (entering
/// insert mode alone computes nothing).
fn ghost_harness(lines: &[&str], row: usize, col: usize) -> Harness {
    let mut h = Harness::inserting(lines, row, col);
    h.app.refresh_completion();
    h
}

/// Plain text of the input column (gutter left out) on screen row `y`.
fn input_text(screen: &Screen, y: usize, gutter: usize, input_width: usize) -> String {
    screen.rows[y]
        .chars()
        .skip(gutter)
        .take(input_width - gutter)
        .collect()
}

/// The characters on row `y` drawn in the ghost colour.
fn ghost_cells(screen: &Screen, y: u16) -> String {
    let area = screen.buffer.area;
    (area.left()..area.right())
        .filter(|&x| screen.buffer[(x, y)].fg == style::GHOST)
        .map(|x| screen.buffer[(x, y)].symbol().to_string())
        .collect()
}

// ------------------------------------------------------------- candidates

/// Candidates come from the lines above, nearest first. Rows are `usize`,
/// so there is no negative-cursor case.
#[test]
fn candidates_table() {
    let err_line = ParsedLine {
        kind: LineKind::Error("syntax".into()),
        raw: "bad = 1 +".into(),
        comment: String::new(),
    };
    let expr = parse_line("1 + 2");
    let cases: Vec<(&str, Vec<ParsedLine>, usize, &[&str])> = vec![
        ("empty doc", vec![], 0, &["sum"]),
        ("empty doc cursor past end", vec![], 5, &["sum"]),
        (
            "cursor on line 0",
            vec![assign("x"), assign("y")],
            0,
            &["sum"],
        ),
        (
            "single assignment above",
            vec![assign("x"), expr.clone()],
            1,
            &["x", "sum"],
        ),
        (
            "assignment below cursor excluded",
            vec![assign("x"), expr.clone(), assign("y")],
            1,
            &["x", "sum"],
        ),
        (
            "current line excluded",
            vec![assign("x"), assign("y")],
            1,
            &["x", "sum"],
        ),
        (
            "nearest first",
            vec![assign("a"), assign("b"), assign("c"), expr.clone()],
            3,
            &["c", "b", "a", "sum"],
        ),
        (
            "duplicates collapse to nearest",
            vec![assign("a"), assign("b"), assign("a"), expr.clone()],
            3,
            &["a", "b", "sum"],
        ),
        (
            "error lines skipped",
            vec![assign("x"), err_line, expr.clone()],
            2,
            &["x", "sum"],
        ),
        (
            "assignment that does not tokenize skipped",
            vec![assign("x"), parse_line("bad = 1 $"), expr.clone()],
            2,
            &["x", "sum"],
        ),
        (
            "variable named sum excluded",
            vec![assign("sum"), assign("x"), expr.clone()],
            2,
            &["x", "sum"],
        ),
        (
            "cursor past end uses all lines",
            vec![assign("x"), assign("y")],
            10,
            &["y", "x", "sum"],
        ),
    ];
    for (name, parsed, row, want) in cases {
        assert_eq!(candidates(&parsed, row), want, "{name}");
    }
}

// ----------------------------------------------------------------- prefix

/// The prefix left of the cursor. A cursor inside an identifier
/// (`x = in|come`) gives nothing. Columns are `usize`, so there is no
/// negative-cursor case.
#[test]
fn prefix_table() {
    let cases: [(&str, &str, usize, Option<(&str, usize)>); 22] = [
        ("bare identifier", "inc", 3, Some(("inc", 0))),
        ("rhs of assignment", "x = inc", 7, Some(("inc", 4))),
        ("lhs of assignment", "inc = 5", 3, None),
        ("after operator", "100 + ta", 8, Some(("ta", 6))),
        ("inside comment", "100 # ta", 8, None),
        ("no identifier before cursor", "100 +", 5, None),
        ("empty line", "", 0, None),
        ("underscore start with digit", "_a1", 3, Some(("_a1", 0))),
        ("digit-leading run", "5x", 2, None),
        ("longer digit-leading run", "12ab", 4, None),
        ("cursor mid identifier", "x = income", 6, None),
        ("cursor exactly on equals", "x = y", 2, None),
        ("cursor before equals", "abc = y", 2, None),
        ("second equals uses first", "a = b = c", 9, Some(("c", 8))),
        ("cursor beyond end clamps", "abc", 99, Some(("abc", 0))),
        (
            "comment after cursor ignored",
            "ta # note",
            2,
            Some(("ta", 0)),
        ),
        // No completion inside an identifier.
        ("in|co", "inco", 2, None),
        ("inco|", "inco", 4, Some(("inco", 0))),
        ("in| + 1", "in + 1", 2, Some(("in", 0))),
        ("before a digit", "x = in2", 6, None),
        // Columns are characters, not bytes.
        ("after a wide character", "€ + inc", 7, Some(("inc", 4))),
        ("non-ASCII letter is not an identifier", "é", 1, None),
    ];
    for (name, line, col, want) in cases {
        let got = prefix(line, col);
        let got = got.as_ref().map(|(p, s)| (p.as_str(), *s));
        assert_eq!(got, want, "{name}: prefix({line:?}, {col})");
    }
}

// ----------------------------------------------------- completion.rs units

#[test]
fn refresh_needs_a_proper_prefix_match() {
    let parsed = [assign("tax"), assign("taxes")];
    let c = Completion::refresh(&parsed, "ta", 2, 2).unwrap();
    assert_eq!(c.matches, ["taxes", "tax"]);
    assert_eq!((c.prefix.as_str(), c.start, c.selected), ("ta", 0, 0));
    assert_eq!(c.ghost_text(), "xes");
    // "tax" equals a candidate: only "taxes" extends it.
    let c = Completion::refresh(&parsed, "tax", 2, 3).unwrap();
    assert_eq!(c.matches, ["taxes"]);
    assert_eq!(Completion::refresh(&parsed, "taxes", 2, 5), None);
    assert_eq!(Completion::refresh(&parsed, "zz", 2, 2), None);
}

#[test]
fn cycle_wraps_both_ways() {
    let parsed = [assign("ab"), assign("ac"), assign("ad")];
    let mut c = Completion::refresh(&parsed, "a", 3, 1).unwrap();
    assert_eq!(c.matches, ["ad", "ac", "ab"]);
    c.cycle(-1);
    assert_eq!(c.current(), "ab");
    c.cycle(1);
    assert_eq!(c.current(), "ad");
    c.cycle(4);
    assert_eq!(c.current(), "ac");
}

// ------------------------------------------------------------------- keys

#[test]
fn tab_accepts() {
    let mut h = Harness::new(&[]);
    h.keys("i").type_text("income=5000").keys("<CR>");
    assert_eq!(h.app.completion, None, "after Enter on an empty line");
    h.type_text("inc");
    assert_eq!(ghost(&h).as_deref(), Some("ome"));
    h.keys("<Tab>");
    assert_eq!(h.app.current_line(), "income");
    assert_eq!(h.cursor(), (1, 6));
    assert_eq!(h.mode(), Mode::Insert);
    assert_eq!(h.app.completion, None);
    assert!(h.app.modified);
    // The accepted line is re-evaluated.
    assert_eq!(h.app.results[1].value, Some(5000.0));
}

#[test]
fn tab_without_completion_does_nothing() {
    let mut h = Harness::new(&[]);
    h.keys("i").type_text("income=5000").keys("<CR>");
    h.type_text("incx");
    assert_eq!(h.app.completion, None);
    h.keys("<Tab>");
    assert_eq!(h.app.current_line(), "incx");
    assert_eq!(h.cursor(), (1, 4));
}

#[test]
fn tab_on_empty_line_inserts_nothing() {
    let mut h = Harness::new(&[]);
    h.keys("i<Tab>");
    assert_eq!(h.lines(), [""]);
    assert!(!h.app.modified);
}

#[test]
fn exact_match_is_inactive() {
    let mut h = Harness::new(&[]);
    h.keys("i").type_text("tax=1").keys("<CR>").type_text("tax");
    assert_eq!(h.app.completion, None);
}

#[test]
fn ctrl_n_and_ctrl_p_cycle() {
    let mut h = Harness::new(&[]);
    h.keys("i").type_text("tax=1").keys("<CR>");
    h.type_text("taxes=2").keys("<CR>").type_text("ta");
    let c = h.app.completion.clone().expect("active");
    // Nearest definition first.
    assert_eq!(c.matches, ["taxes", "tax"]);
    assert_eq!(ghost(&h).as_deref(), Some("xes"));
    h.keys("<C-n>");
    assert_eq!(ghost(&h).as_deref(), Some("x"));
    // Ctrl+N wraps around.
    h.keys("<C-n>");
    assert_eq!(ghost(&h).as_deref(), Some("xes"));
    // Ctrl+P wraps backwards from 0 to the last match, then returns.
    h.keys("<C-p>");
    assert_eq!(ghost(&h).as_deref(), Some("x"));
    h.keys("<C-n><C-p>");
    assert_eq!(ghost(&h).as_deref(), Some("x"));
    assert_eq!(h.app.current_line(), "ta", "cycling does not edit");
    h.keys("<Tab>");
    assert_eq!(h.app.current_line(), "tax");
    assert_eq!(h.cursor(), (2, 3));
}

#[test]
fn cycle_without_completion_does_nothing() {
    let mut h = Harness::new(&[]);
    h.keys("i").type_text("12").keys("<C-n><C-p>");
    assert_eq!(h.lines(), ["12"]);
    assert_eq!(h.cursor(), (0, 2));
    assert_eq!(h.app.completion, None);
}

#[test]
fn completion_mid_line() {
    let mut h = Harness::inserting(&["income = 5000", "x = i + 1"], 1, 5);
    h.type_text("n"); // x = in| + 1
    let c = h.app.completion.clone().expect("active mid-line");
    assert_eq!((c.prefix.as_str(), c.start), ("in", 4));
    h.keys("<Tab>");
    assert_eq!(h.app.current_line(), "x = income + 1");
    assert_eq!(h.cursor(), (1, 10));
}

#[test]
fn esc_clears() {
    let mut h = Harness::new(&[]);
    h.keys("i")
        .type_text("income=5000")
        .keys("<CR>")
        .type_text("inc");
    assert!(h.app.completion.is_some());
    h.keys("<Esc>");
    assert_eq!(h.mode(), Mode::Normal);
    assert_eq!(h.app.completion, None);
}

/// Entering insert mode shows no completion. `A` puts the cursor at the
/// end of a partial word.
#[test]
fn not_shown_on_entering_insert() {
    let mut h = Harness::at(&["income = 5000", "inc"], 1, 2);
    h.keys("A");
    assert_eq!(h.mode(), Mode::Insert);
    assert_eq!(h.cursor(), (1, 3));
    assert_eq!(h.app.completion, None, "right after A");
    // Any insert-mode key recomputes it; Right at the end of the last
    // line does not move.
    h.keys("<Right>");
    assert_eq!(h.cursor(), (1, 3));
    assert_eq!(ghost(&h).as_deref(), Some("ome"));

    // The same with `i`, at the end of a word followed by more text.
    let mut h = Harness::at(&["income = 5000", "inc + 1"], 1, 3);
    h.keys("i");
    assert_eq!(h.cursor(), (1, 3));
    assert_eq!(h.app.completion, None, "right after i");
    h.type_text("o");
    assert_eq!(ghost(&h).as_deref(), Some("me"));
}

/// Enter and the arrows recompute the completion. `Left` from `inc|`
/// puts the cursor inside the word, so nothing is offered until it is back at the end.
#[test]
fn enter_and_arrows_recompute() {
    let mut h = Harness::new(&[]);
    h.keys("i")
        .type_text("income=5000")
        .keys("<CR>")
        .type_text("inc");
    h.keys("<Left>");
    assert_eq!(h.cursor(), (1, 2));
    assert_eq!(h.app.completion, None, "after Left: in|c");
    h.keys("<Right>");
    assert_eq!(ghost(&h).as_deref(), Some("ome"), "after Right: inc|");

    // Moving up lands on the assignment's left side: no completion.
    h.keys("<Up>");
    assert_eq!(h.cursor(), (0, 3));
    assert_eq!(h.app.completion, None, "after Up");
    h.keys("<Down>");
    assert_eq!(h.cursor(), (1, 3));
    assert_eq!(ghost(&h).as_deref(), Some("ome"), "after Down");

    // Enter at the end of a partial identifier splits the line; the new
    // empty line has nothing to complete.
    h.keys("<CR>");
    assert_eq!(h.app.completion, None, "after Enter");

    // Typing a non-identifier character dismisses.
    h.type_text("inc+");
    assert_eq!(h.app.completion, None, "after inc+");
}

#[test]
fn only_from_lines_above() {
    let mut h = Harness::new(&["", "income = 5000"]);
    h.keys("i").type_text("inc");
    assert_eq!(h.app.completion, None);
}

#[test]
fn new_app_has_no_completion() {
    assert_eq!(Harness::new(&[]).app.completion, None);
}

/// `in|co` below `income=5000` shows no ghost, and Tab
/// does not produce `incomeco`.
#[test]
fn no_completion_inside_an_identifier() {
    let mut h = Harness::new(&["income=5000"]);
    h.keys("o").type_text("inco");
    assert_eq!(ghost(&h).as_deref(), Some("me"));
    h.keys("<Left><Left>");
    assert_eq!(h.cursor(), (1, 2));
    assert_eq!(h.app.completion, None);
    assert!(!h.render().rows[1].contains("incomeco"));
    h.keys("<Tab>");
    assert_eq!(h.app.current_line(), "inco");
    assert_eq!(h.cursor(), (1, 2));
}

/// An accepted completion is part of the insert session: one undo step.
#[test]
fn accept_is_part_of_the_insert_undo_step() {
    let mut h = Harness::new(&["income=5000"]);
    h.keys("o").type_text("inc").keys("<Tab><Esc>");
    assert_eq!(h.lines(), ["income=5000", "income"]);
    h.keys("u");
    assert_eq!(h.lines(), ["income=5000"]);
}

/// Leaving insert mode any way clears the completion; normal-mode keys
/// never compute one.
#[test]
fn only_insert_mode_has_a_completion() {
    let mut h = Harness::new(&["income=5000", "inc"]);
    h.keys("jA<Right>");
    assert!(h.app.completion.is_some());
    h.keys("<C-s>");
    assert_eq!(h.mode(), Mode::Insert);
    h.keys("<Esc>");
    assert_eq!(h.app.completion, None);
    h.keys("$");
    assert_eq!(h.app.completion, None);
}

// ---------------------------------------------------------- ghost on screen

#[test]
fn render_ghost_at_end_of_line() {
    let h = ghost_harness(&["income=5000", "inc"], 1, 3);
    assert_eq!(ghost(&h).as_deref(), Some("ome"));
    let screen = h.render();
    assert_eq!(ghost_cells(&screen, 1), "ome");
    let gutter = gutter_width(2);
    assert!(input_text(&screen, 1, gutter, 48).starts_with("income "));
    // The real text keeps its own colour.
    assert_ne!(screen.buffer[(5, 1)].fg, style::GHOST);
}

/// The text after the cursor moves right and keeps its characters and
/// spacing.
#[test]
fn render_ghost_mid_line() {
    let h = ghost_harness(&["income = 5000", "x=in+1"], 1, 4);
    assert_eq!(ghost(&h).as_deref(), Some("come"));
    let screen = h.render();
    assert_eq!(ghost_cells(&screen, 1), "come");
    assert!(input_text(&screen, 1, 5, 48).starts_with("x=income+1 "));

    let h = ghost_harness(&["income = 5000", "x = in + 1"], 1, 6);
    let screen = h.render();
    assert!(input_text(&screen, 1, 5, 48).starts_with("x = income + 1 "));
    // `+` and `1` after the ghost keep their highlighting.
    let plus = 5 + "x = income ".len() as u16;
    assert_eq!(screen.buffer[(plus, 1)].fg, style::OPERATOR);
    assert_eq!(screen.buffer[(plus + 2, 1)].fg, style::NUMBER);
}

#[test]
fn render_no_ghost_in_normal_mode_or_inactive() {
    // Normal mode: even a left-over completion is not drawn.
    let mut h = ghost_harness(&["income=5000", "inc"], 1, 3);
    let left_over = h.app.completion.clone();
    h.keys("<Esc>");
    h.app.completion = left_over;
    let screen = h.render();
    assert_eq!(ghost_cells(&screen, 1), "");
    assert!(!screen.rows[1].contains("income"));

    // Insert mode with no match.
    let h = ghost_harness(&["income=5000", "zz"], 1, 2);
    assert_eq!(h.app.completion, None);
    let screen = h.render();
    assert_eq!(input_text(&screen, 1, 5, 48).trim_end(), "zz");

    // The ghost is drawn on the cursor line only.
    let h = ghost_harness(&["income=5000", "inc", "inc"], 1, 3);
    let screen = h.render();
    assert_eq!(ghost_cells(&screen, 1), "ome");
    assert_eq!(ghost_cells(&screen, 2), "");
    assert!(!screen.rows[2].contains("income"));
}

/// The ghost never runs past
/// the input column.
#[test]
fn render_ghost_clipped_to_content_width() {
    // Content width is `width * 60 / 100 - 5`.
    for (width, content) in [(15, 4), (17, 5), (19, 6), (22, 8)] {
        let mut h = Harness::sized(&["incomeFromAllSources=5000", "inc"], width, 5);
        h.keys("jA<Right>");
        let screen = h.render();
        let input_width = usize::from(width) * 60 / 100;
        assert_eq!(input_width - 5, content);
        let text = input_text(&screen, 1, 5, input_width);
        let want: String = "incomeFromAllSources".chars().take(content).collect();
        assert_eq!(text, want, "width {width}");
        assert_eq!(ghost_cells(&screen, 1).chars().count(), content - 3);
        // Nothing in the ghost colour right of the input column.
        for x in input_width..usize::from(width) {
            assert_ne!(screen.buffer[(x as u16, 1)].fg, style::GHOST, "x {x}");
        }
    }
    // A line that fills the column leaves no room: no ghost at all.
    let mut h = Harness::sized(&["income=5000", "x = 1 + inc"], 19, 5);
    h.keys("jA<Right>");
    assert!(h.app.completion.is_some());
    assert_eq!(ghost_cells(&h.render(), 1), "");
}

/// The bar cursor sits on the first ghost cell.
#[test]
fn cursor_on_ghost_text() {
    let h = ghost_harness(&["income=5000", "inco"], 1, 4);
    let screen = h.render();
    let cursor = screen.cursor.expect("cursor");
    assert_eq!(cursor.shape, CursorShape::Bar);
    assert_eq!((cursor.x, cursor.y), (5 + 4, 1));
    let from_cursor: String = screen.rows[1].chars().skip(9).collect();
    assert!(from_cursor.starts_with("me"), "{from_cursor:?}");
}

/// End to end: type a prefix, see the ghost in the
/// ghost colour, press Tab, get the whole name with the cursor after it.
#[test]
fn type_prefix_see_ghost_tab_accepts() {
    let mut h = Harness::new(&["monthly_rent = 1200"]);
    h.keys("o").type_text("x = mon");
    let screen = h.render();
    assert!(screen.rows[1].starts_with(" 2 │ x = monthly_rent "));
    assert_eq!(ghost_cells(&screen, 1), "thly_rent");
    let cursor = screen.cursor.expect("cursor");
    assert_eq!((cursor.x, cursor.y), (5 + 7, 1));
    assert_eq!(screen.buffer[(12, 1)].fg, style::GHOST);
    assert_ne!(screen.buffer[(11, 1)].fg, style::GHOST);

    h.keys("<Tab>");
    assert_eq!(h.lines(), ["monthly_rent = 1200", "x = monthly_rent"]);
    assert_eq!(h.cursor(), (1, 16));
    assert_eq!(h.mode(), Mode::Insert);
    let screen = h.render();
    assert_eq!(ghost_cells(&screen, 1), "");
    assert_eq!(screen.cursor.map(|c| c.x), Some(5 + 16));
    assert!(screen.rows[1].contains("1,200"));
}
