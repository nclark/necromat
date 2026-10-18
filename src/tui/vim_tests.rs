//! The vim key surface, one table per key group. The last table replays
//! the editor spike's nvim-verified probes.

// The case tables are tuples, to keep each case on one line.
#![allow(clippy::type_complexity)]

use super::clipboard::{Clipboard, MemoryClipboard};
use super::editor::{Editor, Mode};
use super::harness::Harness;

const TEN: &[&str] = &["l0", "l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9"];

/// (name, lines, start (row, col), keys, want lines, want cursor, want mode)
type Case<'a> = (
    &'a str,
    &'a [&'a str],
    (usize, usize),
    &'a str,
    &'a [&'a str],
    (usize, usize),
    Mode,
);

use Mode::{Insert, Normal, Visual, VisualLine};

/// Runs every case and reports all failures at once.
fn check(cases: &[Case]) {
    let mut failures = Vec::new();
    for &(name, lines, (row, col), keys, want, cursor, mode) in cases {
        let mut h = Harness::at(lines, row, col);
        h.keys(keys);
        let got = (h.lines(), h.cursor(), h.mode());
        let want = (strs(want), cursor, mode);
        if got != want {
            failures.push(format!("{name} ({keys}): got {got:?}, want {want:?}"));
        }
        if h.app.editor.is_chord_pending() || h.app.editor.pending_op().is_some() {
            failures.push(format!("{name} ({keys}): command left pending"));
        }
        if h.app.should_quit() {
            failures.push(format!("{name} ({keys}): quit"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// The unnamed register as nm shows it: linewise text without its final
/// newline.
fn register(h: &Harness) -> (String, bool) {
    let (text, linewise) = h.app.editor.register();
    let text = if linewise {
        text.strip_suffix('\n').unwrap_or(&text).to_string()
    } else {
        text
    };
    (text, linewise)
}

fn clipboard(h: &mut Harness) -> Option<String> {
    h.app.editor.clipboard().get()
}

fn status(h: &Harness) -> &str {
    h.app.status_msg.as_deref().unwrap_or("")
}

/// A harness whose clipboard already holds `text`, so a test can see
/// whether a key wrote to it.
fn with_clipboard(lines: &[&str], row: usize, col: usize, text: &str) -> Harness {
    let mut h = Harness::at(lines, row, col);
    h.app
        .editor
        .clipboard()
        .set(text)
        .expect("memory clipboard");
    h
}

// ------------------------------------------------------------- motions

/// Normal-mode motions, plus `$` keeping to the line end on `j`/`k` as in
/// vim.
#[test]
#[rustfmt::skip]
fn motions_and_counts() {
    check(&[
        ("h", &["abc"], (0, 2), "h", &["abc"], (0, 1), Normal),
        ("l", &["abc"], (0, 0), "l", &["abc"], (0, 1), Normal),
        ("3j", TEN, (0, 0), "3j", TEN, (3, 0), Normal),
        ("j count past end clamps", &["a", "b"], (0, 0), "9j", &["a", "b"], (1, 0), Normal),
        ("2k", TEN, (5, 0), "2k", TEN, (3, 0), Normal),
        ("j keeps column, clamped", &["abcdef", "ab"], (0, 4), "j", &["abcdef", "ab"], (1, 1), Normal),
        ("j then j restores the column", &["abcdef", "ab", "abcdef"], (0, 4), "jj", &["abcdef", "ab", "abcdef"], (2, 4), Normal),
        ("5l clamps at line end", &["abc"], (0, 0), "5l", &["abc"], (0, 2), Normal),
        ("2l", &["abcdef"], (0, 0), "2l", &["abcdef"], (0, 2), Normal),
        ("3h clamps at 0", &["abcdef"], (0, 2), "3h", &["abcdef"], (0, 0), Normal),
        ("$ on last char", &["abcdef"], (0, 0), "$", &["abcdef"], (0, 5), Normal),
        ("$ then j keeps to the line end", &["ab", "abcdef", "abc"], (0, 0), "$jj", &["ab", "abcdef", "abc"], (2, 2), Normal),
        ("End then j keeps to the line end", &["ab", "abcdef"], (0, 0), "<End>j", &["ab", "abcdef"], (1, 5), Normal),
        ("$ then j, k", &["ab", "abcdef"], (0, 0), "$jk", &["ab", "abcdef"], (0, 1), Normal),
        ("l after $ drops the line-end column", &["abcd", "abcdef"], (0, 0), "$hj", &["abcd", "abcdef"], (1, 2), Normal),
        ("0 as motion", &["abcdef"], (0, 4), "0", &["abcdef"], (0, 0), Normal),
        ("0 inside a count", TEN, (0, 0), "10j", TEN, (9, 0), Normal),
        ("^ first non-blank", &["   x = 1"], (0, 6), "^", &["   x = 1"], (0, 3), Normal),
        ("3G goes to line 3", TEN, (0, 0), "3G", TEN, (2, 0), Normal),
        ("G goes to last line", TEN, (0, 0), "G", TEN, (9, 0), Normal),
        ("gg goes to first line", TEN, (7, 0), "gg", TEN, (0, 0), Normal),
        ("4gg goes to line 4", TEN, (0, 0), "4gg", TEN, (3, 0), Normal),
        ("arrow keys are motions", TEN, (0, 0), "2<Down>", TEN, (2, 0), Normal),
        ("arrow right and left", &["abc"], (0, 0), "<Right><Right><Left>", &["abc"], (0, 1), Normal),
    ]);
}

/// `w W e E b B` on calculator lines, across line ends, with counts. An empty
/// line counts as a word.
#[test]
#[rustfmt::skip]
fn word_motions() {
    const L: &[&str] = &["income + tax_rate*2", "", "total = income"];
    check(&[
        ("w", L, (0, 0), "w", L, (0, 7), Normal),
        ("2w", L, (0, 0), "2w", L, (0, 9), Normal),
        ("3w stops on punctuation", L, (0, 0), "3w", L, (0, 17), Normal),
        ("4w", L, (0, 0), "4w", L, (0, 18), Normal),
        ("w onto an empty line", L, (0, 18), "w", L, (1, 0), Normal),
        ("w off an empty line", L, (1, 0), "w", L, (2, 0), Normal),
        ("W", L, (0, 0), "W", L, (0, 7), Normal),
        ("2W skips the whole WORD", L, (0, 0), "3W", L, (1, 0), Normal),
        ("e", L, (0, 0), "e", L, (0, 5), Normal),
        ("2e", L, (0, 0), "2e", L, (0, 7), Normal),
        ("3e ends on tax_rate", L, (0, 0), "3e", L, (0, 16), Normal),
        ("E", L, (0, 9), "E", L, (0, 18), Normal),
        ("3E", L, (0, 0), "3E", L, (0, 18), Normal),
        ("b", L, (0, 18), "b", L, (0, 17), Normal),
        ("2b", L, (0, 18), "2b", L, (0, 9), Normal),
        ("b from mid-word", L, (0, 12), "b", L, (0, 9), Normal),
        ("b onto an empty line", L, (2, 0), "b", L, (1, 0), Normal),
        ("b across a line end", L, (1, 0), "b", L, (0, 18), Normal),
        ("B", L, (0, 18), "B", L, (0, 9), Normal),
        ("2B", L, (0, 18), "2B", L, (0, 7), Normal),
        ("dw", L, (0, 0), "dw", &["+ tax_rate*2", "", "total = income"], (0, 0), Normal),
        ("d2w", L, (0, 0), "d2w", &["tax_rate*2", "", "total = income"], (0, 0), Normal),
        ("dw on the last word does not join", &["total = income", "x"], (0, 8), "dw", &["total = ", "x"], (0, 7), Normal),
        ("cw acts like ce", L, (0, 0), "cwnet<Esc>", &["net + tax_rate*2", "", "total = income"], (0, 2), Normal),
        ("cw leaves insert mode open", L, (0, 0), "cw", &[" + tax_rate*2", "", "total = income"], (0, 0), Insert),
        ("de", L, (0, 0), "de", &[" + tax_rate*2", "", "total = income"], (0, 0), Normal),
        ("db", L, (0, 9), "db", &["income tax_rate*2", "", "total = income"], (0, 7), Normal),
        ("dW", L, (0, 9), "dW", &["income + ", "", "total = income"], (0, 8), Normal),
        ("dB", L, (0, 18), "dB", &["income + 2", "", "total = income"], (0, 9), Normal),
        ("dE", L, (0, 9), "dE", &["income + ", "", "total = income"], (0, 8), Normal),
        ("yw keeps text and cursor", L, (0, 0), "yw", L, (0, 0), Normal),
    ]);
}

/// `f F t T ; ,` and `%`, alone and after operators.
#[test]
#[rustfmt::skip]
fn char_find_and_match() {
    const N: &[&str] = &["a = 100 + 200"];
    const P: &[&str] = &["net = (income - tax) * 2"];
    check(&[
        ("f=", N, (0, 0), "f=", N, (0, 2), Normal),
        ("2f0", N, (0, 0), "2f0", N, (0, 6), Normal),
        ("t+", N, (0, 0), "t+", N, (0, 7), Normal),
        ("F(", P, (0, 23), "F(", P, (0, 6), Normal),
        ("T(", P, (0, 23), "T(", P, (0, 7), Normal),
        ("; repeats", N, (0, 0), "f0;;", N, (0, 11), Normal),
        (", reverses", N, (0, 0), "f0;;,", N, (0, 6), Normal),
        ("; after t steps past the stop", N, (0, 0), "t0;", N, (0, 5), Normal),
        ("not found does nothing", N, (0, 0), "fz", N, (0, 0), Normal),
        ("not found clears the operator", N, (0, 0), "dfzx", &[" = 100 + 200"], (0, 0), Normal),
        ("dt*", P, (0, 0), "dt*", &["* 2"], (0, 0), Normal),
        ("df)", P, (0, 0), "df)", &[" * 2"], (0, 0), Normal),
        ("ct= enters insert", P, (0, 0), "ct=", &["= (income - tax) * 2"], (0, 0), Insert),
        ("ct= then typing", P, (0, 0), "ct=gross <Esc>", &["gross = (income - tax) * 2"], (0, 5), Normal),
        // F and T are exclusive: the cursor character stays.
        ("dT(", P, (0, 12), "dT(", &["net = (e - tax) * 2"], (0, 7), Normal),
        ("dF(", P, (0, 12), "dF(", &["net = e - tax) * 2"], (0, 6), Normal),
        ("d2F space", P, (0, 12), "d2F<Space>", &["nete - tax) * 2"], (0, 3), Normal),
        ("cF(", P, (0, 12), "cF(x<Esc>", &["net = xe - tax) * 2"], (0, 6), Normal),
        ("dF( undo", P, (0, 12), "dF(u", P, (0, 6), Normal),
        ("dF( not found", P, (0, 12), "dF[", P, (0, 12), Normal),
        ("d, after f", P, (0, 12), "f<Space>d,", &["net = - tax) * 2"], (0, 5), Normal),
        ("d; after F", P, (0, 12), "F<Space>d;", &["net (income - tax) * 2"], (0, 3), Normal),
        ("dF(next to the cursor", &["(a(bc"], (0, 4), "dF(", &["(ac"], (0, 2), Normal),
        ("dT(next to the cursor deletes nothing", &["ab(c"], (0, 3), "dT(", &["ab(c"], (0, 3), Normal),
        ("cT(next to the cursor does nothing", &["ab(c"], (0, 3), "cT(", &["ab(c"], (0, 3), Normal),
        ("df(stays inclusive", P, (0, 0), "df(", &["income - tax) * 2"], (0, 0), Normal),
        ("rF is not a find", P, (0, 12), "rFdF(", &["net = F - tax) * 2"], (0, 6), Normal),
        ("%", P, (0, 6), "%", P, (0, 19), Normal),
        ("% back", P, (0, 19), "%", P, (0, 6), Normal),
        ("% from a non-bracket", P, (0, 0), "%", P, (0, 19), Normal),
        ("d%", &["(a + b) * 2"], (0, 0), "d%", &[" * 2"], (0, 0), Normal),
        ("% with no bracket", N, (0, 3), "%", N, (0, 3), Normal),
    ]);
}

// ----------------------------------------------------------- operators

/// Operators and pending state.
#[test]
#[rustfmt::skip]
fn operators_and_shorthands() {
    check(&[
        ("d$", &["abcdef"], (0, 2), "d$", &["ab"], (0, 1), Normal),
        ("D", &["abcdef"], (0, 2), "D", &["ab"], (0, 1), Normal),
        ("2D deletes into next line", &["abc", "def", "ghi"], (0, 1), "2D", &["a", "ghi"], (0, 0), Normal),
        ("d0", &["abcdef"], (0, 3), "d0", &["def"], (0, 0), Normal),
        ("d^", &["  abcdef"], (0, 5), "d^", &["  def"], (0, 2), Normal),
        ("dl", &["abc"], (0, 1), "dl", &["ac"], (0, 1), Normal),
        ("dh", &["abc"], (0, 1), "dh", &["bc"], (0, 0), Normal),
        ("dj deletes 2 lines", &["a", "b", "c", "d"], (1, 0), "dj", &["a", "d"], (1, 0), Normal),
        ("dk deletes 2 lines", &["a", "b", "c", "d"], (2, 0), "dk", &["a", "d"], (1, 0), Normal),
        ("dj on last line does nothing", &["a", "b"], (1, 0), "dj", &["a", "b"], (1, 0), Normal),
        ("2d3j deletes 7 lines", TEN, (0, 0), "2d3j", &["l7", "l8", "l9"], (0, 0), Normal),
        ("d2j deletes 3 lines", TEN, (1, 0), "d2j", &["l0", "l4", "l5", "l6", "l7", "l8", "l9"], (1, 0), Normal),
        ("dG", TEN, (7, 0), "dG", &["l0", "l1", "l2", "l3", "l4", "l5", "l6"], (6, 0), Normal),
        ("dgg", TEN, (2, 0), "dgg", &["l3", "l4", "l5", "l6", "l7", "l8", "l9"], (0, 0), Normal),
        ("dG from first line leaves one empty line", TEN, (0, 0), "dG", &[""], (0, 0), Normal),
        ("d3G", TEN, (5, 0), "d3G", &["l0", "l1", "l6", "l7", "l8", "l9"], (2, 0), Normal),
        ("dd", &["a", "b", "c"], (1, 0), "dd", &["a", "c"], (1, 0), Normal),
        ("dd on last line moves up", &["a", "b", "c"], (2, 0), "dd", &["a", "b"], (1, 0), Normal),
        ("3dd", TEN, (2, 0), "3dd", &["l0", "l1", "l5", "l6", "l7", "l8", "l9"], (2, 0), Normal),
        ("d3d", TEN, (2, 0), "d3d", &["l0", "l1", "l5", "l6", "l7", "l8", "l9"], (2, 0), Normal),
        ("3dd past end deletes to end", &["a", "b", "c"], (1, 0), "3dd", &["a"], (0, 0), Normal),
        ("x", &["abc"], (0, 1), "x", &["ac"], (0, 1), Normal),
        ("3x", &["abcdef"], (0, 1), "3x", &["aef"], (0, 1), Normal),
        ("9x stops at line end", &["abc", "def"], (0, 1), "9x", &["a", "def"], (0, 0), Normal),
        ("x on empty line does not join", &["", "def"], (0, 0), "x", &["", "def"], (0, 0), Normal),
        ("X", &["abc"], (0, 2), "X", &["ac"], (0, 1), Normal),
        ("2X", &["abcd"], (0, 3), "2X", &["ad"], (0, 1), Normal),
        ("X at column 0 does nothing", &["abc"], (0, 0), "X", &["abc"], (0, 0), Normal),
        // Pending state
        ("esc clears a pending d", &["a", "b", "c"], (0, 0), "d<Esc>j", &["a", "b", "c"], (1, 0), Normal),
        ("esc clears a count", TEN, (0, 0), "3<Esc>j", TEN, (1, 0), Normal),
        ("esc clears a pending g", TEN, (4, 0), "g<Esc>j", TEN, (5, 0), Normal),
        ("unknown key after d cancels it", &["a", "b", "c"], (0, 0), "d!j", &["a", "b", "c"], (1, 0), Normal),
        ("mixed operators cancel", &["a", "b", "c"], (0, 0), "dcj", &["a", "b", "c"], (1, 0), Normal),
        // c
        ("c$ enters insert", &["abcdef"], (0, 2), "c$", &["ab"], (0, 2), Insert),
        ("C", &["abcdef"], (0, 2), "Cxy<Esc>", &["abxy"], (0, 3), Normal),
        ("cc leaves empty line in insert", &["a", "bcd", "e"], (1, 1), "cc", &["a", "", "e"], (1, 0), Insert),
        ("2cc", &["a", "b", "c", "d"], (1, 0), "2ccz<Esc>", &["a", "z", "d"], (1, 0), Normal),
        ("S", &["a", "bcd"], (1, 0), "S", &["a", ""], (1, 0), Insert),
        ("cj", &["a", "b", "c"], (0, 0), "cj", &["", "c"], (0, 0), Insert),
        ("s", &["abc"], (0, 1), "sX<Esc>", &["aXc"], (0, 1), Normal),
        ("2s", &["abcd"], (0, 1), "2sX<Esc>", &["aXd"], (0, 1), Normal),
        ("c0", &["abcdef"], (0, 3), "c0", &["def"], (0, 0), Insert),
        // y keeps the cursor and the text
        ("yj keeps cursor", &["a", "b"], (0, 0), "yj", &["a", "b"], (0, 0), Normal),
        ("y$ keeps cursor", &["abc"], (0, 1), "y$", &["abc"], (0, 1), Normal),
        ("yy keeps cursor", &["abc", "d"], (0, 1), "yy", &["abc", "d"], (0, 1), Normal),
        ("yG keeps cursor", &["a", "b", "c"], (1, 0), "yG", &["a", "b", "c"], (1, 0), Normal),
        // A count before a command that ignores it.
        ("3i enters insert", &["abc"], (0, 1), "3i", &["abc"], (0, 1), Insert),
    ]);
}

// ---------------------------------------------------------- line edits

/// `J r ~ ^`, with and without counts, at line ends and on the last line.
/// Each is one undo step.
#[test]
#[rustfmt::skip]
fn line_edits() {
    check(&[
        ("J", &["a", "  b", "c"], (0, 0), "J", &["a b", "c"], (0, 1), Normal),
        ("3J joins 3 lines", &["a", "b", "c", "d"], (0, 0), "3J", &["a b c", "d"], (0, 3), Normal),
        ("J on the last line", &["a", "b"], (1, 0), "J", &["a", "b"], (1, 0), Normal),
        ("J with an empty next line", &["a", "", "b"], (0, 0), "J", &["a", "b"], (0, 0), Normal),
        ("J undo", &["a", "b", "c"], (0, 0), "3Ju", &["a", "b", "c"], (0, 0), Normal),
        ("r", &["abc"], (0, 0), "rX", &["Xbc"], (0, 0), Normal),
        ("2r", &["abc"], (0, 0), "2rX", &["XXc"], (0, 1), Normal),
        ("r count past the end does nothing", &["abc"], (0, 1), "5rX", &["abc"], (0, 1), Normal),
        ("r at the line end", &["abc"], (0, 2), "rX", &["abX"], (0, 2), Normal),
        ("r on the last line", &["a", "bc"], (1, 1), "r9", &["a", "b9"], (1, 1), Normal),
        ("r then Esc cancels", &["abc"], (0, 0), "r<Esc>x", &["bc"], (0, 0), Normal),
        ("r undo", &["abc"], (0, 0), "2rXu", &["abc"], (0, 0), Normal),
        ("~", &["abc"], (0, 0), "~", &["Abc"], (0, 1), Normal),
        ("3~", &["a1B"], (0, 0), "3~", &["A1b"], (0, 2), Normal),
        ("~ at the line end stays", &["abc"], (0, 2), "~", &["abC"], (0, 2), Normal),
        ("~ count past the end", &["abc"], (0, 1), "9~", &["aBC"], (0, 2), Normal),
        ("~ undo", &["abc"], (0, 0), "3~u", &["abc"], (0, 0), Normal),
        ("^", &["  x = 1"], (0, 5), "^", &["  x = 1"], (0, 2), Normal),
        ("d^", &["  abc"], (0, 4), "d^", &["  c"], (0, 2), Normal),
    ]);
}

/// Engine keys beyond the core set that nm documents in `doc/keys.md`.
#[test]
#[rustfmt::skip]
fn other_engine_keys() {
    const L: &[&str] = &["x = 10 + (a * b)", "  y"];
    check(&[
        ("I", L, (1, 2), "I-<Esc>", &["x = 10 + (a * b)", "  -y"], (1, 2), Normal),
        ("Ctrl+A", L, (0, 0), "<C-a>", &["x = 11 + (a * b)", "  y"], (0, 5), Normal),
        ("5 Ctrl+A", L, (0, 0), "5<C-a>", &["x = 15 + (a * b)", "  y"], (0, 5), Normal),
        ("Ctrl+X", L, (0, 4), "<C-x>", &["x = 9 + (a * b)", "  y"], (0, 4), Normal),
        ("ciw", L, (0, 4), "ciw20<Esc>", &["x = 20 + (a * b)", "  y"], (0, 5), Normal),
        ("diw", L, (0, 0), "diw", &[" = 10 + (a * b)", "  y"], (0, 0), Normal),
        ("daw", L, (0, 4), "daw", &["x = + (a * b)", "  y"], (0, 4), Normal),
        ("di(", L, (0, 11), "di(", &["x = 10 + ()", "  y"], (0, 10), Normal),
        ("da(", L, (0, 11), "da(", &["x = 10 + ", "  y"], (0, 8), Normal),
        ("ci(", L, (0, 11), "ci(c<Esc>", &["x = 10 + (c)", "  y"], (0, 10), Normal),
        ("daW", L, (0, 10), "daW", &["x = 10 + * b)", "  y"], (0, 9), Normal),
        ("diW", L, (0, 10), "diW", &["x = 10 +  * b)", "  y"], (0, 9), Normal),
        ("yi( then P", L, (0, 0), "fayi(P", &["x = 10 + (a * ba * b)", "  y"], (0, 14), Normal),
        ("gJ joins without a space", L, (0, 0), "gJ", &["x = 10 + (a * b)  y"], (0, 16), Normal),
    ]);
    // `Y` yanks to the end of the line, as in nvim.
    let mut h = with_clipboard(L, 0, 4, "before");
    h.keys("Y");
    assert_eq!(clipboard(&mut h).as_deref(), Some("10 + (a * b)"));
    assert_eq!(status(&h), "Yanked 12 characters");
}

// ---------------------------------------------------------- dot repeat

/// `.` repeats the last change, with or without a new count.
#[test]
#[rustfmt::skip]
fn dot_repeat() {
    const L: &[&str] = &["income + tax_rate*2"];
    check(&[
        ("dw .", L, (0, 0), "dw.", &["tax_rate*2"], (0, 0), Normal),
        ("3dd .", TEN, (0, 0), "3dd.", &["l6", "l7", "l8", "l9"], (0, 0), Normal),
        ("2. after dd", TEN, (0, 0), "dd2.", &["l3", "l4", "l5", "l6", "l7", "l8", "l9"], (0, 0), Normal),
        ("cw foo then . on another word", &["a b c"], (0, 0), "cwfoo<Esc>w.", &["foo foo c"], (0, 6), Normal),
        ("x .", &["abcd"], (0, 0), "x.", &["cd"], (0, 0), Normal),
        ("J .", &["a", "b", "c"], (0, 0), "J.", &["a b c"], (0, 3), Normal),
        ("A; then . on the next line", &["1", "2"], (0, 0), "A;<Esc>j.", &["1;", "2;"], (1, 1), Normal),
        ("a motion does not replace the change", &["abcd", "efgh"], (0, 0), "xj.", &["bcd", "fgh"], (1, 0), Normal),
        ("~ .", &["abcd"], (0, 0), "~.", &["ABcd"], (0, 2), Normal),
        ("r .", &["abcd"], (0, 0), "rXl.", &["XXcd"], (0, 1), Normal),
        (". then undo is one step", &["abcd"], (0, 0), "2x.u", &["cd"], (0, 0), Normal),
    ]);
}

// --------------------------------------------------------------- visual

/// Visual mode: `v` charwise and inclusive, `V` linewise, `h j k l 0 $ G`
/// extend, `d` deletes and exits, Esc cancels.
#[test]
#[rustfmt::skip]
fn visual_mode() {
    const H: &[&str] = &["hello world"];
    const F: &[&str] = &["first line", "second line", "third line"];
    check(&[
        ("v enters visual", H, (0, 0), "v", H, (0, 0), Visual),
        ("V enters visual line", H, (0, 0), "V", H, (0, 0), VisualLine),
        ("Esc cancels", H, (0, 0), "vll<Esc>", H, (0, 2), Normal),
        ("v from V switches", H, (0, 0), "Vv", H, (0, 0), Visual),
        ("v again leaves", H, (0, 0), "vv", H, (0, 0), Normal),
        ("vd single line", H, (0, 0), "v4ld", &[" world"], (0, 0), Normal),
        ("vd backwards", H, (0, 10), "v4hd", &["hello "], (0, 5), Normal),
        ("Vjd", F, (0, 0), "Vjd", &["third line"], (0, 0), Normal),
        ("vjd across lines", F, (0, 6), "vjjhhd", &["first  line"], (0, 6), Normal),
        ("vkd extends up", &["abc", "def"], (1, 1), "vkd", &["af"], (0, 1), Normal),
        ("vh", &["abcdef"], (0, 2), "vhd", &["adef"], (0, 1), Normal),
        ("v0", &["abcdef"], (0, 3), "v0d", &["ef"], (0, 0), Normal),
        // `$` selects to the end of the line without the newline.
        ("v$d does not join", &["abc", "def"], (0, 1), "v$d", &["a", "def"], (0, 0), Normal),
        ("vG", TEN, (0, 0), "vGd", &["9"], (0, 0), Normal),
        ("VG", TEN, (2, 0), "VGd", &["l0", "l1"], (1, 0), Normal),
        ("Vk", TEN, (2, 0), "Vkd", &["l0", "l3", "l4", "l5", "l6", "l7", "l8", "l9"], (1, 0), Normal),
        ("y exits", H, (0, 0), "vly", H, (0, 0), Normal),
        ("y moves to the start of the selection", H, (0, 4), "vhhy", H, (0, 2), Normal),
        ("x deletes like d", H, (0, 0), "vlx", &["llo world"], (0, 0), Normal),
        ("c changes", H, (0, 0), "v4lcbye<Esc>", &["bye world"], (0, 2), Normal),
        ("counts extend", TEN, (0, 0), "V3jd", &["l4", "l5", "l6", "l7", "l8", "l9"], (0, 0), Normal),
        ("visual undo", F, (0, 0), "Vjdu", F, (1, 0), Normal),
    ]);
}

/// Visual yanks: what `y` yanks, the status message, and the register rule
/// for visual `d`.
#[test]
#[rustfmt::skip]
fn visual_yank() {
    // (lines, start, keys, clipboard text, status)
    let cases: [(&[&str], (usize, usize), &str, &str, &str); 8] = [
        (&["hello world"], (0, 0), "v4ly", "hello", "Yanked 5 characters"),
        (&["hello world"], (0, 10), "v4hy", "world", "Yanked 5 characters"),
        (&["first line", "second line", "third line"], (0, 6), "vjjhhy", "line\nsecond line\nthird", "Yanked 22 characters"),
        (&["first line", "second line", "third line"], (0, 0), "Vjy", "first line\nsecond line", "Yanked 2 lines"),
        (&["100", "200", "300"], (0, 0), "vjlly", "100\n200", "Yanked 7 characters"),
        (&["100", "200", "300"], (0, 0), "V2jy", "100\n200\n300", "Yanked 3 lines"),
        // `v$` stops before the newline.
        (&["income=5000", "x"], (0, 0), "v$y", "income=5000", "Yanked 11 characters"),
        (&["a€b"], (0, 0), "v$y", "a€b", "Yanked 3 characters"),
    ];
    for (lines, (row, col), keys, want, msg) in cases {
        let mut h = with_clipboard(lines, row, col, "before");
        h.keys(keys);
        assert_eq!(h.mode(), Normal, "{keys}");
        assert_eq!(clipboard(&mut h).as_deref(), Some(want), "{keys}");
        assert_eq!(status(&h), msg, "{keys}");
        assert_eq!(h.lines(), strs(lines), "{keys}");
    }

    let mut h = with_clipboard(&["a", "b", "c"], 0, 0, "before");
    h.keys("Vjy");
    assert_eq!(register(&h), ("a\nb".to_string(), true));
    let mut h = with_clipboard(&["abc"], 0, 0, "before");
    h.keys("vld");
    assert_eq!(register(&h), ("ab".to_string(), false));
    assert_eq!(clipboard(&mut h).as_deref(), Some("before"));
    assert_eq!(status(&h), "");
}

// ------------------------------------------------- yank and register rule

/// What each command leaves in the register, and that only yanks reach the
/// clipboard.
#[test]
#[rustfmt::skip]
fn register_rule() {
    // (lines, start, keys, register, linewise, reaches the clipboard)
    let cases: [(&[&str], (usize, usize), &str, &str, bool, bool); 20] = [
        (&["a", "bc", "d"], (1, 0), "yy", "bc", true, true),
        (TEN, (1, 0), "3yy", "l1\nl2\nl3", true, true),
        (TEN, (0, 0), "yj", "l0\nl1", true, true),
        (&["a", "b", "c"], (1, 0), "yG", "b\nc", true, true),
        (&["a", "b", "c"], (1, 0), "ygg", "a\nb", true, true),
        (&["abcdef"], (0, 2), "y$", "cdef", false, true),
        (&["abcdef"], (0, 2), "y0", "ab", false, true),
        (&["ab cd"], (0, 0), "yw", "ab ", false, true),
        (&["ab = cd"], (0, 0), "yt=", "ab ", false, true),
        // `r` as the target of a find is not `yr`.
        (&["abrc"], (0, 0), "yfr", "abr", false, true),
        (&["a rc"], (0, 0), "yiw", "a", false, true),
        (&["a", "bc"], (1, 0), "dd", "bc", true, false),
        (&["abcdef"], (0, 0), "3x", "abc", false, false),
        (&["abcdef"], (0, 4), "D", "ef", false, false),
        (&["abc"], (0, 0), "cc", "abc", true, false),
        (&["abc", "def"], (0, 1), "2D", "bc\ndef", false, false),
        (&["abc"], (0, 2), "X", "b", false, false),
        (&["abc"], (0, 0), "s", "a", false, false),
        (&["ab cd"], (0, 0), "dw", "ab ", false, false),
        (&["ab cd"], (0, 0), "cwx<Esc>", "ab", false, false),
    ];
    for (lines, (row, col), keys, want, linewise, clip) in cases {
        let mut h = with_clipboard(lines, row, col, "before");
        h.keys(keys);
        assert_eq!(register(&h), (want.to_string(), linewise), "{keys}");
        let want_clip = if clip { want } else { "before" };
        assert_eq!(clipboard(&mut h).as_deref(), Some(want_clip), "{keys}");
    }
}

/// `yF` and `yT` stop before the cursor character (hjkl includes it).
#[test]
fn yank_backward_find() {
    let cases = [
        ("yF(", "(income"),
        ("yT(", "income"),
        ("y2F<Space>", " = (income"),
    ];
    for (keys, want) in cases {
        let mut h = with_clipboard(&["net = (income - tax) * 2"], 0, 13, "before");
        h.keys(keys);
        assert_eq!(register(&h), (want.to_string(), false), "{keys}");
        assert_eq!(clipboard(&mut h).as_deref(), Some(want), "{keys}");
        let n = want.chars().count();
        assert_eq!(status(&h), format!("Yanked {n} characters"), "{keys}");
        assert_eq!(h.cursor(), (0, 13 - n), "{keys}");
    }
}

#[test]
fn deletes_do_not_touch_the_clipboard() {
    for keys in [
        "dd", "x", "X", "D", "dj", "cc<Esc>", "S<Esc>", "s<Esc>", "C<Esc>", "dw", "vd", "Vd",
        "vc<Esc>", "J", "rX", "~", "xp", "ddP",
    ] {
        let mut h = with_clipboard(&["abc", "def"], 0, 1, "before");
        h.keys(keys);
        assert_eq!(clipboard(&mut h).as_deref(), Some("before"), "{keys}");
        assert_eq!(status(&h), "", "{keys}");
    }
}

/// A yank says what it took, keeps the cursor, and is not an undo step.
#[test]
fn yank_status_and_cursor() {
    let mut h = Harness::at(TEN, 4, 1);
    h.keys("3yy");
    assert_eq!(status(&h), "Yanked 3 lines");
    assert_eq!((h.lines(), h.cursor()), (strs(TEN), (4, 1)));
    assert!(!h.app.editor.can_undo());
    assert!(!h.app.modified);
    // The message lasts until the next key.
    h.keys("j");
    assert_eq!(status(&h), "");

    let cases = [
        ("yy", "Yanked 1 line"),
        ("yl", "Yanked 1 character"),
        ("y$", "Yanked 3 characters"),
        ("yj", "Yanked 2 lines"),
        ("vy", "Yanked 1 character"),
        ("Vy", "Yanked 1 line"),
    ];
    for (keys, want) in cases {
        let mut h = Harness::new(&["abc", "def"]);
        h.keys(keys);
        assert_eq!(status(&h), want, "{keys}");
    }
}

/// `yr` yanks the formatted result of the line.
#[test]
fn yank_result() {
    let mut h = with_clipboard(&["2 + 3", "hello", "1/0", "x = 1234.5"], 0, 0, "before");
    h.keys("yr");
    assert_eq!(status(&h), "Yanked result");
    assert_eq!(clipboard(&mut h).as_deref(), Some("5"));
    assert_eq!(register(&h), ("5".to_string(), false));
    assert!(h.app.editor.pending_op().is_none());
    assert!(!h.app.editor.is_chord_pending());

    // A line with no value, and an error line, yank nothing.
    for row in [1, 2] {
        let mut h = with_clipboard(&["2 + 3", "hello", "1/0"], row, 0, "before");
        h.keys("yr");
        assert_eq!(status(&h), "No result to yank", "row {row}");
        assert_eq!(clipboard(&mut h).as_deref(), Some("before"));
        h.keys("j");
        assert_eq!(h.cursor().0, (row + 1).min(2), "keys after yr are normal");
    }

    // The formatted value: thousands separators, trimmed decimals.
    let mut h = Harness::at(&["x = 1234.5"], 0, 0);
    h.keys("yr");
    assert_eq!(clipboard(&mut h).as_deref(), Some("1,234.5"));
    // `yr` then `p` pastes the value.
    let mut h = Harness::at(&["2 + 3", ""], 0, 0);
    h.keys("yrjp");
    assert_eq!(h.lines(), ["2 + 3", "5"]);
}

/// A clipboard that always fails, to see the error reach the status bar.
struct BrokenClipboard;

impl Clipboard for BrokenClipboard {
    fn set(&mut self, _text: &str) -> Result<(), String> {
        Err("nope".into())
    }

    fn get(&mut self) -> Option<String> {
        None
    }
}

#[test]
fn clipboard_errors_show_in_status() {
    let mut h = Harness::new(&["1"]);
    h.app.editor = Editor::with_clipboard(&strs(&["1"]), Box::new(BrokenClipboard));
    h.keys("yy");
    assert_eq!(status(&h), "Clipboard error: nope");
    // The register still holds the yank, so `p` works.
    h.keys("p");
    assert_eq!(h.lines(), ["1", "1"]);
}

/// `p`/`P` from the register, linewise and charwise, with counts; one undo
/// step each.
#[test]
#[rustfmt::skip]
fn paste() {
    check(&[
        ("yy p", &["a", "b"], (0, 0), "yyp", &["a", "a", "b"], (1, 0), Normal),
        ("yy P", &["a", "b"], (1, 0), "yyP", &["a", "b", "b"], (1, 0), Normal),
        ("yy 3p", &["a", "b"], (0, 0), "yy3p", &["a", "a", "a", "a", "b"], (1, 0), Normal),
        ("dd p moves a line down", &["a", "b", "c"], (0, 0), "ddp", &["b", "a", "c"], (1, 0), Normal),
        ("2yy p", &["a", "b"], (0, 0), "2yyGp", &["a", "b", "a", "b"], (2, 0), Normal),
        ("yl p", &["abc"], (0, 0), "ylp", &["aabc"], (0, 1), Normal),
        ("yl P", &["abc"], (0, 1), "ylP", &["abbc"], (0, 1), Normal),
        ("yl 3p", &["abc"], (0, 0), "yl3p", &["aaaabc"], (0, 3), Normal),
        ("x p swaps", &["abc"], (0, 0), "xp", &["bac"], (0, 1), Normal),
        ("yw P", &["ab cd"], (0, 3), "0ywP", &["ab ab cd"], (0, 2), Normal),
        ("paste is one undo step", &["a", "b"], (0, 0), "yy3pu", &["a", "b"], (0, 0), Normal),
        ("p with nothing anywhere", &["abc"], (0, 0), "p", &["abc"], (0, 0), Normal),
    ]);
}

/// `p` reads the register, and the clipboard only when the register is
/// empty. Text with a newline pastes as lines.
#[test]
#[rustfmt::skip]
fn paste_falls_back_to_the_clipboard() {
    // (lines, start, clipboard, keys, want lines, want cursor)
    let cases: [(&[&str], (usize, usize), &str, &str, &[&str], (usize, usize)); 6] = [
        (&["abc"], (0, 0), "xyz", "p", &["axyzbc"], (0, 3)),
        (&["abc"], (0, 1), "xyz", "P", &["axyzbc"], (0, 3)),
        (&["a", "b"], (0, 0), "1\n2", "p", &["a", "1", "2", "b"], (1, 0)),
        (&["a", "b"], (1, 0), "1\n2\n", "P", &["a", "1", "2", "b"], (1, 0)),
        // The register wins once it holds something.
        (&["abc"], (0, 0), "xyz", "xp", &["bac"], (0, 1)),
        (&["abc"], (0, 0), "", "p", &["abc"], (0, 0)),
    ];
    for (lines, (row, col), clip, keys, want, cursor) in cases {
        let mut h = with_clipboard(lines, row, col, clip);
        h.keys(keys);
        assert_eq!(
            (h.lines(), h.cursor()),
            (strs(want), cursor),
            "{clip:?} {keys}"
        );
        // Pasting does not change the clipboard.
        assert_eq!(clipboard(&mut h).as_deref(), Some(clip), "{keys}");
    }
    // Visual `p` replaces the selection, from the clipboard too.
    let mut h = with_clipboard(&["abc"], 0, 0, "xyz");
    h.keys("vlp");
    assert_eq!(h.lines(), ["xyzc"]);
}

/// Vim: `p` and `P` with an empty register and nothing in the clipboard
/// show E353 and change nothing. Visual mode keeps the selection (hjkl
/// would delete it); `d` afterwards shows the selection is intact.
#[test]
#[rustfmt::skip]
fn paste_with_nothing_anywhere() {
    // (start, keys before, paste, want cursor, want mode, `d` afterwards)
    let cases: [((usize, usize), &str, &str, (usize, usize), Mode, &str); 7] = [
        ((0, 0), "vl", "p", (0, 1), Visual, "c"),
        ((0, 0), "vl", "P", (0, 1), Visual, "c"),
        ((0, 0), "V", "p", (0, 0), VisualLine, ""),
        ((0, 1), "", "p", (0, 1), Normal, ""),
        ((0, 1), "", "P", (0, 1), Normal, ""),
        ((0, 1), "3", "p", (0, 1), Normal, ""),
        ((0, 1), "", "3p", (0, 1), Normal, ""),
    ];
    let unreadable = || -> Box<dyn Clipboard> { Box::new(BrokenClipboard) };
    let empty = || -> Box<dyn Clipboard> { Box::new(MemoryClipboard::default()) };
    for (clip_name, make) in [("empty", &empty as &dyn Fn() -> Box<dyn Clipboard>), ("unreadable", &unreadable)] {
        for ((row, col), before, paste, cursor, mode, after_d) in cases {
            let name = format!("{clip_name} {before}{paste}");
            let mut h = Harness::at(&["abc"], row, col);
            h.app.editor.set_clipboard(make());
            h.keys(before);
            h.keys(paste);
            assert_eq!(h.lines(), ["abc"], "{name}");
            assert_eq!(h.cursor(), cursor, "{name}");
            assert_eq!(h.mode(), mode, "{name}");
            assert_eq!(status(&h), "E353: Nothing in register \"", "{name}");
            if mode != Normal {
                h.keys("d");
                assert_eq!(h.lines(), [after_d], "{name} then d");
            }
        }
    }
    // The count before a refused `p` does not carry over to the next motion.
    let mut h = Harness::at(&["abcdef"], 0, 0);
    h.keys("3pl");
    assert_eq!(h.cursor(), (0, 1));
}

// ----------------------------------------------------------------- undo

/// Undo and redo.
#[test]
#[rustfmt::skip]
fn undo_and_redo() {
    check(&[
        ("dd", &["a", "bcd", "e"], (1, 2), "dd", &["a", "e"], (1, 0), Normal),
        ("dd u", &["a", "bcd", "e"], (1, 2), "ddu", &["a", "bcd", "e"], (1, 2), Normal),
        ("dd u C-r", &["a", "b"], (0, 0), "ddu<C-r>", &["b"], (0, 0), Normal),
        ("C-r past the newest", &["a", "b"], (0, 0), "ddu<C-r><C-r>", &["b"], (0, 0), Normal),
        ("insert session", &[""], (0, 0), "iabc<Esc>", &["abc"], (0, 2), Normal),
        ("insert session u", &[""], (0, 0), "iabc<Esc>u", &[""], (0, 0), Normal),
        ("two sessions", &[""], (0, 0), "iab<Esc>Acd<Esc>", &["abcd"], (0, 3), Normal),
        ("two sessions u", &[""], (0, 0), "iab<Esc>Acd<Esc>u", &["ab"], (0, 1), Normal),
        ("two sessions uu", &[""], (0, 0), "iab<Esc>Acd<Esc>uu", &[""], (0, 0), Normal),
        // A session with Enter, Backspace and arrows is still one step
        // (hjkl, like vim, splits it at arrow keys).
        ("session with newline", &["a"], (0, 0), "A1<CR>2<BS>3<Esc>u", &["a"], (0, 0), Normal),
        ("session with arrows", &["ab"], (0, 0), "A1<Left><Left>2<Up><End>3<Esc>u", &["ab"], (0, 1), Normal),
        ("session with arrows, redo", &["ab"], (0, 0), "A1<Left>2<Esc>u<C-r>", &["ab21"], (0, 2), Normal),
        ("o", &["a", "b"], (0, 0), "ox<Esc>", &["a", "x", "b"], (1, 0), Normal),
        ("o u", &["a", "b"], (0, 0), "ox<Esc>u", &["a", "b"], (0, 0), Normal),
        ("x u", &["abc"], (0, 1), "xu", &["abc"], (0, 1), Normal),
        ("Vjd u", &["a", "b", "c"], (0, 0), "Vjdu", &["a", "b", "c"], (1, 0), Normal),
        ("new edit clears redo", &["abc"], (0, 0), "xudd<C-r>", &[""], (0, 0), Normal),
        ("3dd u", TEN, (2, 1), "3ddu", TEN, (2, 1), Normal),
        ("2d3j u", TEN, (0, 0), "2d3ju", TEN, (0, 0), Normal),
        ("dG u", TEN, (3, 0), "dGu", TEN, (3, 0), Normal),
        ("3x u", &["abcdef"], (0, 1), "3xu", &["abcdef"], (0, 1), Normal),
        ("D u", &["abcdef"], (0, 1), "Du", &["abcdef"], (0, 1), Normal),
        ("c$ typing u", &["abcdef"], (0, 2), "c$xyz<Esc>u", &["abcdef"], (0, 2), Normal),
        ("2cc typing u", &["a", "b", "c"], (1, 0), "2ccnew<Esc>u", &["a", "b", "c"], (1, 0), Normal),
        ("dw u", &["ab cd"], (0, 0), "dwu", &["ab cd"], (0, 0), Normal),
        // Counts on undo and redo.
        ("2u", &["abcd"], (0, 0), "xxx2u", &["bcd"], (0, 0), Normal),
        ("2 C-r", &["abcd"], (0, 0), "xxxuuu2<C-r>", &["cd"], (0, 0), Normal),
    ]);
}

#[test]
fn undo_steps_and_messages() {
    let mut h = Harness::new(&["a"]);
    h.keys("u");
    assert_eq!(h.lines(), ["a"]);
    assert_eq!(status(&h), "Already at oldest change");
    // A second redo says so.
    let mut h = Harness::new(&["a", "b"]);
    h.keys("ddu<C-r>");
    assert_eq!(status(&h), "");
    h.keys("<C-r>");
    assert_eq!(status(&h), "Already at newest change");
    // Undo marks the buffer modified.
    let mut h = Harness::new(&["a", "b"]);
    h.keys("ddu");
    assert!(h.app.modified);

    // No step for a session or edit that changed nothing.
    for (lines, keys) in [
        (&["a"][..], "i<Esc>"),
        (&["a"][..], "A<Esc>"),
        (&[""][..], "x"),
        (&[""][..], "dd"),
        (&[""][..], "xdd"),
        (&[""][..], "c$<Esc>"),
        (&["a"][..], "dj"),
        (&["a"][..], "yy"),
        (&["a"][..], "vy"),
    ] {
        let mut h = Harness::new(lines);
        h.keys(keys);
        assert!(!h.app.editor.can_undo(), "{keys} left an undo step");
    }

    // Each normal-mode edit is one step.
    let mut h = Harness::new(&["abcdef"]);
    h.keys("xxx");
    h.keys("u");
    assert_eq!(h.lines(), ["cdef"]);

    let mut h = Harness::new(&["abc"]);
    h.keys("xudd");
    assert!(!h.app.editor.can_redo());
}

/// Undo history keeps 200 steps. `x` on one long line builds the same
/// history as `dd` on 210 lines and keeps the test fast in a debug build.
#[test]
fn undo_history_is_capped_at_200() {
    let line = "x".repeat(210);
    let mut h = Harness::new(&[&line]);
    for _ in 0..210 {
        h.keys("x");
    }
    assert_eq!(h.lines(), [""]);
    for _ in 0..200 {
        h.keys("u");
    }
    assert_eq!(h.app.current_line().chars().count(), 200);
    assert_eq!(status(&h), "");
    h.keys("u");
    assert_eq!(h.app.current_line().chars().count(), 200);
    assert_eq!(status(&h), "Already at oldest change");
}

/// Replacing the text clears undo and redo.
#[test]
fn loading_clears_history() {
    let mut h = Harness::new(&["a", "b"]);
    h.keys("ddxu");
    assert!(h.app.editor.can_undo() && h.app.editor.can_redo());
    h.app.editor.set_lines(&strs(&["1", "2"]));
    assert!(!h.app.editor.can_undo());
    assert!(!h.app.editor.can_redo());
    h.keys("u");
    assert_eq!((h.lines(), h.cursor()), (strs(&["1", "2"]), (0, 0)));
}

// ------------------------------------------------------------------- Esc

/// Esc leaving insert steps left (not at column 0); in normal mode it clears
/// pending keys; in visual it cancels.
#[test]
#[rustfmt::skip]
fn esc() {
    check(&[
        ("insert Esc steps left", &["abc"], (0, 0), "A<Esc>", &["abc"], (0, 2), Normal),
        ("insert Esc at column 0", &["abc"], (0, 0), "i<Esc>", &["abc"], (0, 0), Normal),
        ("insert Esc mid-line", &["abc"], (0, 2), "i<Esc>", &["abc"], (0, 1), Normal),
        ("insert Esc on an empty line", &[""], (0, 0), "o<Esc>", &["", ""], (1, 0), Normal),
        ("normal Esc does nothing", &["abc"], (0, 1), "<Esc>", &["abc"], (0, 1), Normal),
        ("Esc clears a count", TEN, (0, 0), "5<Esc>j", TEN, (1, 0), Normal),
        ("Esc clears d", TEN, (0, 0), "d<Esc>j", TEN, (1, 0), Normal),
        ("Esc clears y", TEN, (0, 0), "y<Esc>p", TEN, (0, 0), Normal),
        ("Esc clears f", &["abc"], (0, 0), "f<Esc>l", &["abc"], (0, 1), Normal),
        ("Esc clears r", &["abc"], (0, 0), "r<Esc>l", &["abc"], (0, 1), Normal),
        ("Esc clears g", TEN, (3, 0), "g<Esc>G", TEN, (9, 0), Normal),
        ("visual Esc", &["abc"], (0, 0), "vl<Esc>x", &["ac"], (0, 1), Normal),
        ("visual line Esc", &["a", "b"], (0, 0), "Vj<Esc>x", &["a", ""], (1, 0), Normal),
    ]);
}

// --------------------------------------------------------- spike probes

use Mode as M;

/// The 96 probes from the editor-core spike (nmr-ye9.1): each key
/// sequence run in `nvim --clean` on the same three lines. Where nm
/// and nvim differ, nm's rule wins and the probe is overridden below.
#[rustfmt::skip]
const PROBES: &[(&str, &str, &str, &[&str], (usize, usize), Mode, &str)] = &[
    ("story", "dw", "dw", &["=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income"),
    ("story", "ct=", "ct=salary<Esc>", &["salary=5000", "expenses = 3000", "income - expenses"], (0, 5), M::Normal, "income"),
    ("story", "3j", "3j", &["income=5000", "expenses = 3000", "income - expenses"], (2, 0), M::Normal, ""),
    ("story", "u", "dwu", &["income=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income"),
    ("story", "cw", "cwsalary<Esc>", &["salary=5000", "expenses = 3000", "income - expenses"], (0, 5), M::Normal, "income"),
    ("story", "v l y", "vly", &["income=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "in"),
    ("unicode", "append é", "Aé<Esc>", &["income=5000é", "expenses = 3000", "income - expenses"], (0, 11), M::Normal, ""),
    ("unicode", "é, h, x", "Aé<Esc>hx", &["income=500é", "expenses = 3000", "income - expenses"], (0, 10), M::Normal, "0"),
    ("unicode", "é, x on it", "Aé<Esc>x", &["income=5000", "expenses = 3000", "income - expenses"], (0, 10), M::Normal, "é"),
    ("unicode", "insert Left + BS", "Aé€<Left><BS><Esc>", &["income=5000€", "expenses = 3000", "income - expenses"], (0, 10), M::Normal, ""),
    ("unicode", "insert cursor after é", "Aé", &["income=5000é", "expenses = 3000", "income - expenses"], (0, 12), M::Insert, ""),
    ("unicode", "w after é", "ié <Esc>w", &["é income=5000", "expenses = 3000", "income - expenses"], (0, 2), M::Normal, ""),
    ("unicode", "l onto é then x", "Iaéb<Esc>0lx", &["abincome=5000", "expenses = 3000", "income - expenses"], (0, 1), M::Normal, "é"),
    ("motion", "j l", "jll", &["income=5000", "expenses = 3000", "income - expenses"], (1, 2), M::Normal, ""),
    ("motion", "k", "jlk", &["income=5000", "expenses = 3000", "income - expenses"], (0, 1), M::Normal, ""),
    ("motion", "w", "w", &["income=5000", "expenses = 3000", "income - expenses"], (0, 6), M::Normal, ""),
    ("motion", "b", "$b", &["income=5000", "expenses = 3000", "income - expenses"], (0, 7), M::Normal, ""),
    ("motion", "e", "e", &["income=5000", "expenses = 3000", "income - expenses"], (0, 5), M::Normal, ""),
    ("motion", "W", "jW", &["income=5000", "expenses = 3000", "income - expenses"], (1, 9), M::Normal, ""),
    ("motion", "B", "j$B", &["income=5000", "expenses = 3000", "income - expenses"], (1, 11), M::Normal, ""),
    ("motion", "E", "jE", &["income=5000", "expenses = 3000", "income - expenses"], (1, 7), M::Normal, ""),
    ("motion", "0", "$0", &["income=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, ""),
    ("motion", "^", "$^", &["income=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, ""),
    ("motion", "$", "$", &["income=5000", "expenses = 3000", "income - expenses"], (0, 10), M::Normal, ""),
    ("motion", "gg", "Ggg", &["income=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, ""),
    ("motion", "G", "G", &["income=5000", "expenses = 3000", "income - expenses"], (2, 0), M::Normal, ""),
    ("motion", "f", "f=", &["income=5000", "expenses = 3000", "income - expenses"], (0, 6), M::Normal, ""),
    ("motion", "t", "t=", &["income=5000", "expenses = 3000", "income - expenses"], (0, 5), M::Normal, ""),
    ("motion", "F", "$F=", &["income=5000", "expenses = 3000", "income - expenses"], (0, 6), M::Normal, ""),
    ("motion", "T", "$T=", &["income=5000", "expenses = 3000", "income - expenses"], (0, 7), M::Normal, ""),
    ("motion", ";", "jf ;", &["income=5000", "expenses = 3000", "income - expenses"], (1, 10), M::Normal, ""),
    ("motion", ",", "jf ;,", &["income=5000", "expenses = 3000", "income - expenses"], (1, 8), M::Normal, ""),
    ("motion", "j keeps column", "$jj", &["income=5000", "expenses = 3000", "income - expenses"], (2, 16), M::Normal, ""),
    ("count", "2w", "2w", &["income=5000", "expenses = 3000", "income - expenses"], (0, 7), M::Normal, ""),
    ("count", "3l", "3l", &["income=5000", "expenses = 3000", "income - expenses"], (0, 3), M::Normal, ""),
    ("count", "2j", "2j", &["income=5000", "expenses = 3000", "income - expenses"], (2, 0), M::Normal, ""),
    ("count", "3x", "3x", &["ome=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "inc"),
    ("count", "2dd", "2dd", &["income - expenses"], (0, 0), M::Normal, "income=5000\nexpenses = 3000\n"),
    ("count", "d2w", "d2w", &["5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income="),
    ("count", "2dw", "2dw", &["5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income="),
    ("operator", "x", "x", &["ncome=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "i"),
    ("operator", "X", "$X", &["income=500", "expenses = 3000", "income - expenses"], (0, 9), M::Normal, "0"),
    ("operator", "dd", "dd", &["expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income=5000\n"),
    ("operator", "D", "wD", &["income", "expenses = 3000", "income - expenses"], (0, 5), M::Normal, "=5000"),
    ("operator", "C", "wCX<Esc>", &["incomeX", "expenses = 3000", "income - expenses"], (0, 6), M::Normal, "=5000"),
    ("operator", "s", "sX<Esc>", &["Xncome=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "i"),
    ("operator", "S", "SX<Esc>", &["X", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income=5000\n"),
    ("operator", "cc", "ccX<Esc>", &["X", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income=5000\n"),
    ("operator", "d$", "wd$", &["income", "expenses = 3000", "income - expenses"], (0, 5), M::Normal, "=5000"),
    ("operator", "d0", "$d0", &["0", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income=500"),
    ("operator", "de", "de", &["=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income"),
    ("operator", "db", "$db", &["income=0", "expenses = 3000", "income - expenses"], (0, 7), M::Normal, "500"),
    ("operator", "dj", "dj", &["income - expenses"], (0, 0), M::Normal, "income=5000\nexpenses = 3000\n"),
    ("operator", "dG", "jdG", &["income=5000"], (0, 0), M::Normal, "expenses = 3000\nincome - expenses\n"),
    ("operator", "dgg", "jdgg", &["income - expenses"], (0, 0), M::Normal, "income=5000\nexpenses = 3000\n"),
    ("operator", "dt", "dt0", &["000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income=5"),
    ("operator", "df", "df=", &["5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income="),
    ("operator", "yy p", "yyp", &["income=5000", "income=5000", "expenses = 3000", "income - expenses"], (1, 0), M::Normal, "income=5000\n"),
    ("operator", "yw P", "ywP", &["incomeincome=5000", "expenses = 3000", "income - expenses"], (0, 5), M::Normal, "income"),
    ("operator", "y$", "y$", &["income=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income=5000"),
    ("operator", "J", "J", &["income=5000 expenses = 3000", "income - expenses"], (0, 11), M::Normal, ""),
    ("operator", "r", "rX", &["Xncome=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, ""),
    ("operator", "~", "~", &["Income=5000", "expenses = 3000", "income - expenses"], (0, 1), M::Normal, ""),
    ("textobj", "diw", "diw", &["=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income"),
    ("textobj", "ciw", "ciwX<Esc>", &["X=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income"),
    ("textobj", "daw", "jdaw", &["income=5000", "= 3000", "income - expenses"], (1, 0), M::Normal, "expenses "),
    ("textobj", "diW", "jdiW", &["income=5000", " = 3000", "income - expenses"], (1, 0), M::Normal, "expenses"),
    ("undo", "x u", "xu", &["income=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "i"),
    ("undo", "x u C-r", "xu<C-r>", &["ncome=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "i"),
    ("undo", "insert session is one step", "Aabc<Esc>u", &["income=5000", "expenses = 3000", "income - expenses"], (0, 10), M::Normal, ""),
    ("undo", "x x u", "xxu", &["ncome=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "n"),
    ("dot", "x .", "x.", &["come=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "n"),
    ("dot", "dw .", "dw.", &["5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "="),
    ("dot", "cw .", "cwX<Esc>w.", &["XX5000", "expenses = 3000", "income - expenses"], (0, 1), M::Normal, "="),
    ("visual", "v mode", "v", &["income=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Visual, ""),
    ("visual", "V mode", "V", &["income=5000", "expenses = 3000", "income - expenses"], (0, 0), M::VisualLine, ""),
    ("visual", "v Esc", "vl<Esc>", &["income=5000", "expenses = 3000", "income - expenses"], (0, 1), M::Normal, ""),
    ("visual", "v l d", "vld", &["come=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "in"),
    ("visual", "V d", "Vd", &["expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income=5000\n"),
    ("visual", "v j d", "vjd", &["xpenses = 3000", "income - expenses"], (0, 0), M::Normal, "income=5000\ne"),
    ("visual", "V j d", "Vjd", &["income - expenses"], (0, 0), M::Normal, "income=5000\nexpenses = 3000\n"),
    ("visual", "v $ y", "v$y", &["income=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "income=5000\n"),
    ("visual", "v l c", "vlcX<Esc>", &["Xcome=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, "in"),
    ("insert", "i", "iX<Esc>", &["Xincome=5000", "expenses = 3000", "income - expenses"], (0, 0), M::Normal, ""),
    ("insert", "a", "aX<Esc>", &["iXncome=5000", "expenses = 3000", "income - expenses"], (0, 1), M::Normal, ""),
    ("insert", "A", "AX<Esc>", &["income=5000X", "expenses = 3000", "income - expenses"], (0, 11), M::Normal, ""),
    ("insert", "I", "jwIX<Esc>", &["income=5000", "Xexpenses = 3000", "income - expenses"], (1, 0), M::Normal, ""),
    ("insert", "o", "oX<Esc>", &["income=5000", "X", "expenses = 3000", "income - expenses"], (1, 0), M::Normal, ""),
    ("insert", "O", "jOX<Esc>", &["income=5000", "X", "expenses = 3000", "income - expenses"], (1, 0), M::Normal, ""),
    ("insert", "Enter splits", "wi<CR><Esc>", &["income", "=5000", "expenses = 3000", "income - expenses"], (1, 0), M::Normal, ""),
    ("insert", "BS", "A<BS><Esc>", &["income=500", "expenses = 3000", "income - expenses"], (0, 9), M::Normal, ""),
    ("insert", "BS at col 0 joins", "jI<BS><Esc>", &["income=5000expenses = 3000", "income - expenses"], (0, 10), M::Normal, ""),
    ("insert", "Del at end joins", "A<Del><Esc>", &["income=5000expenses = 3000", "income - expenses"], (0, 10), M::Normal, ""),
    ("insert", "Right", "i<Right><Right>X<Esc>", &["inXcome=5000", "expenses = 3000", "income - expenses"], (0, 2), M::Normal, ""),
    ("paste", "x p", "xp", &["nicome=5000", "expenses = 3000", "income - expenses"], (0, 1), M::Normal, "i"),
    ("paste", "dd p", "ddp", &["expenses = 3000", "income=5000", "income - expenses"], (1, 0), M::Normal, "income=5000\n"),
];

#[test]
fn spike_probes() {
    let start = ["income=5000", "expenses = 3000", "income - expenses"];
    let mut failures = Vec::new();
    for &(area, name, keys, want, cursor, mode, reg) in PROBES {
        // In nm `v$` stops before the newline.
        let reg = if keys == "v$y" { "income=5000" } else { reg };
        let mut h = Harness::new(&start);
        h.keys(keys);
        let got = (h.lines(), h.cursor(), h.mode(), h.app.editor.register().0);
        let want = (strs(want), cursor, mode, reg.to_string());
        if got != want {
            failures.push(format!(
                "{area}/{name} ({keys}): got {got:?}, want {want:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
