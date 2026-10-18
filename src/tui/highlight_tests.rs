//! Syntax highlighting tests: token styles, the raw-text property over a
//! wide set of lines, and rendering checks through the harness.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use super::harness::Harness;
use super::highlight::highlight_line;
use super::style;
use super::theme::{self, Theme};
use crate::lang::parse_line;

fn hl(line: &str) -> Vec<Span<'static>> {
    highlight_line(line, &parse_line(line))
}

fn joined(spans: &[Span]) -> String {
    spans.iter().map(|s| s.content.as_ref()).collect()
}

/// The spans as (text, style) pairs, for exact comparisons.
fn pairs(line: &str) -> Vec<(String, Style)> {
    hl(line)
        .into_iter()
        .map(|s| (s.content.into_owned(), s.style))
        .collect()
}

fn p(text: &str, st: Style) -> (String, Style) {
    (text.to_string(), st)
}

fn plain(text: &str) -> (String, Style) {
    p(text, Style::new())
}

fn assert_preserved(line: &str) {
    let spans = hl(line);
    assert_eq!(joined(&spans), line, "spans for {line:?}: {spans:?}");
    assert!(
        spans.iter().all(|s| !s.content.is_empty()),
        "empty span for {line:?}"
    );
}

/// The spans hold exactly the raw line, including unicode, multiple and
/// trailing spaces, lines that fail to tokenize, and `sum` inside an
/// identifier.
#[test]
fn preserves_raw_text() {
    let basic = [
        "",
        "   ",
        "x = 1",
        "x=1",
        "  income  =  5000  ",
        "income = 5000 # monthly",
        "100 + 50",
        "100+50",
        " a * ( b - 2 ) ",
        "x = - 5",
        "3 - -2",
        "sum",
        "  sum  ",
        "5,000",
        " 42 ",
        "100 #",
        "100   #   spaced comment  ",
        "# just a comment",
        "   #",
        "x = 1 +",
        "bad $ line",
        "x = $",
    ];
    let extra = [
        "#",
        "##",
        "# a # b",
        "SUM",
        "Sum # total",
        "summary",
        "summary + 1",
        "sum + 1",
        "x = sum",
        "sum = 5",
        "café = 5",
        "日本 = 1",
        "5 € 3",
        "100 # €uro note",
        "  x  =   -   5   +  y  ",
        "1 +    2      ",
        "a    *b",
        "1..2",
        "1,2,3,4",
        "- x",
        "x = ",
        "= 5",
        "x == 5",
        "(((",
        "(1 + 2) * -3",
        "rent=1,500#monthly",
        "  \u{a0}5\u{a0} ",
        "1 + 2 = 3",
        "é",
    ];
    for line in basic.iter().chain(&extra) {
        assert_preserved(line);
    }
}

/// Token styles, compared exactly.
#[test]
fn styles_tokens() {
    assert_eq!(
        pairs("x = 1 + y"),
        [
            p("x", style::variable()),
            plain(" "),
            p("=", style::operator()),
            plain(" "),
            p("1", style::number()),
            plain(" "),
            p("+", style::operator()),
            plain(" "),
            p("y", style::variable()),
        ]
    );
    assert_eq!(
        pairs("100 # note"),
        [
            p("100", style::number()),
            plain(" "),
            p("# note", style::comment()),
        ]
    );
}

#[test]
fn partial_comment_lines_are_comment_style() {
    for line in ["#", "   #", "# just a comment", "  # x  "] {
        assert_eq!(pairs(line), [p(line, style::comment())], "{line:?}");
    }
    assert_eq!(
        pairs("100 #"),
        [
            p("100", style::number()),
            plain(" "),
            p("#", style::comment())
        ]
    );
    assert_eq!(pairs("   "), [plain("   ")]);
    assert!(pairs("").is_empty());
}

#[test]
fn comment_style_is_italic_gray() {
    let st = style::comment();
    assert_eq!(st.fg, Some(style::COMMENT));
    assert!(st.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn sum_keyword_versus_identifier() {
    assert_eq!(
        pairs("  SuM  "),
        [plain("  "), p("SuM", style::keyword()), plain("  ")]
    );
    assert_eq!(pairs("summary"), [p("summary", style::variable())]);
    assert_eq!(
        pairs("sum + 1"),
        [
            p("sum", style::variable()),
            plain(" "),
            p("+", style::operator()),
            plain(" "),
            p("1", style::number()),
        ]
    );
    assert_eq!(style::keyword().fg, Some(style::ERROR));
}

#[test]
fn numbers_keep_commas_and_spacing() {
    assert_eq!(
        pairs(" 5,000 "),
        [plain(" "), p("5,000", style::number()), plain(" ")]
    );
    assert_eq!(pairs("-50"), [p("-50", style::number())]);
}

/// The tokenizer folds `- 5` into one number token; the span keeps the
/// space and is styled as one number.
#[test]
fn unary_minus_with_spaces_is_one_number_span() {
    assert_eq!(
        pairs("x = - 5"),
        [
            p("x", style::variable()),
            plain(" "),
            p("=", style::operator()),
            plain(" "),
            p("- 5", style::number()),
        ]
    );
    assert_eq!(
        pairs("3 - -2"),
        [
            p("3", style::number()),
            plain(" "),
            p("-", style::operator()),
            plain(" "),
            p("-2", style::number()),
        ]
    );
}

#[test]
fn assignment_without_spaces() {
    assert_eq!(
        pairs("rent=1,500#monthly"),
        [
            p("rent", style::variable()),
            p("=", style::operator()),
            p("1,500", style::number()),
            p("#monthly", style::comment()),
        ]
    );
}

/// The code part of an error line is left unstyled and still styles a
/// trailing comment.
#[test]
fn error_lines_are_unstyled() {
    for line in ["bad $ line", "x = $", "café = 5", "5 € 3", "1..2"] {
        assert_eq!(pairs(line), [plain(line)], "{line:?}");
    }
    assert_eq!(
        pairs("bad $ # why"),
        [plain("bad $ "), p("# why", style::comment())]
    );
}

/// An assignment whose expression is incomplete still tokenizes, so its
/// tokens are styled (evaluation reports the error, not the highlighter).
#[test]
fn incomplete_assignment_is_styled() {
    assert_eq!(
        pairs("x = 1 +"),
        [
            p("x", style::variable()),
            plain(" "),
            p("=", style::operator()),
            plain(" "),
            p("1", style::number()),
            plain(" "),
            p("+", style::operator()),
        ]
    );
}

/// Every string up to length 4 over an alphabet that hits every token kind,
/// comments, `=`, spaces and a non-ASCII character keeps its text.
#[test]
fn preserves_raw_text_exhaustive_short_lines() {
    let alphabet = ['1', ',', '.', '-', '+', '(', 'x', '=', '#', ' ', 'é', '$'];
    let mut line = String::new();
    fn walk(alphabet: &[char], line: &mut String, depth: usize) {
        assert_preserved(line);
        if depth == 0 {
            return;
        }
        for &c in alphabet {
            line.push(c);
            walk(alphabet, line, depth - 1);
            line.pop();
        }
    }
    walk(&alphabet, &mut line, 4);
}

/// Longer pseudo-random lines built from words and spaces.
#[test]
fn preserves_raw_text_random_lines() {
    let words = [
        "1", "5,000", "-", "- ", "+", "*", "/", "(", ")", "x", "sum", "summary", "=", "#", " ",
        "   ", "3.5", "日本", "€", "$", "income", "-2",
    ];
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..5000 {
        let n = next() % 10;
        let line: String = (0..n)
            .map(|_| words[(next() % words.len() as u64) as usize])
            .collect();
        assert_preserved(&line);
    }
}

// ------------------------------------------------------------- rendering

#[test]
fn rendered_cells_carry_token_colours() {
    let screen = Harness::new(&["x = 1 + y # note"]).render();
    assert!(screen.rows[0].starts_with(" 1 │ x = 1 + y # note "));
    let fg = |x: u16| screen.buffer[(x, 0)].fg;
    assert_eq!(fg(5), style::VARIABLE);
    assert_eq!(fg(7), style::OPERATOR);
    assert_eq!(fg(9), style::NUMBER);
    assert_eq!(fg(11), style::OPERATOR);
    assert_eq!(fg(13), style::VARIABLE);
    assert_eq!(fg(15), style::COMMENT);
    assert!(screen.buffer[(15, 0)].modifier.contains(Modifier::ITALIC));
}

#[test]
fn rendered_sum_is_keyword_red() {
    let screen = Harness::new(&["100", "sum"]).render();
    assert_eq!(screen.buffer[(5, 0)].fg, style::NUMBER);
    assert_eq!(screen.buffer[(5, 1)].fg, style::KEYWORD);
}

/// Every corpus line renders as its raw text after the gutter.
#[test]
fn rendered_text_equals_raw_line() {
    for line in [
        "  income  =  5000  ",
        "x = - 5",
        "100   #   spaced comment",
        "bad $ line",
        "summary + 1",
        "café = 5",
    ] {
        let screen = Harness::new(&[line]).render();
        let text: String = screen.rows[0]
            .chars()
            .skip(5)
            .take(line.chars().count())
            .collect();
        assert_eq!(text, line);
    }
}

/// Styles survive clipping: the cells kept at the input column edge keep
/// their token colours, and nothing spills into the results column.
#[test]
fn styled_text_is_clipped_to_the_input_column() {
    let long = "1+".repeat(40) + "1";
    let screen = Harness::sized(&[&long], 40, 5).render();
    let input: String = screen.rows[0].chars().take(24).collect();
    assert_eq!(input, format!(" 1 │ {}", &long[..19]));
    // Text cells 5..24: '1' at even offsets, '+' at odd.
    for x in 5u16..24 {
        let want = if (x - 5) % 2 == 0 {
            style::NUMBER
        } else {
            style::OPERATOR
        };
        assert_eq!(screen.buffer[(x, 0)].fg, want, "cell {x}");
    }
    assert!(screen.rows[0].ends_with("41 "));
}

/// Wide characters take two cells; the colour after them lines up with the
/// display width and the cursor placement is unchanged.
#[test]
fn wide_characters_keep_styles_aligned() {
    let line = "x = 1 # 日本 y";
    let h = Harness::at(&[line], 0, 0);
    let screen = h.render();
    assert!(screen.rows[0].starts_with(" 1 │ x = 1 # 日本 y"));
    // "x = 1 " is 6 cells, so the comment starts at 5 + 6 and runs past the
    // two double-width characters to the final `y` at 5 + 13.
    assert_eq!(screen.buffer[(11, 0)].fg, style::COMMENT);
    assert_eq!(screen.buffer[(18, 0)].symbol(), "y");
    assert_eq!(screen.buffer[(18, 0)].fg, style::COMMENT);
}

/// A wide character that straddles the input column edge is dropped, not
/// split, and the results column is untouched.
#[test]
fn wide_character_at_clip_edge() {
    // Input column 24 = gutter 5 + 19 cells. 18 ASCII cells then a
    // double-width character that would need cells 18 and 19.
    let line = format!("{}日", "a".repeat(18));
    let screen = Harness::sized(&[&line], 40, 5).render();
    let input: String = screen.rows[0].chars().take(24).collect();
    assert_eq!(input, format!(" 1 │ {} ", "a".repeat(18)));
}

#[test]
fn rendered_cells_use_the_active_theme() {
    let custom = Theme {
        number: Color::Rgb(0x3F, 0x8F, 0x8A),
        badge_normal: Color::Rgb(0x01, 0x02, 0x03),
        ..Theme::tokyo_night()
    };
    theme::set_active(custom);
    let screen = Harness::new(&["100", "x = 1"]).render();
    theme::set_active(Theme::tokyo_night());
    assert_eq!(screen.buffer[(5, 0)].fg, custom.number);
    assert_eq!(screen.buffer[(9, 1)].fg, custom.number);
    assert_eq!(
        screen.buffer[(5, 1)].fg,
        style::VARIABLE,
        "unset slots keep Tokyo Night"
    );
    assert_eq!(screen.buffer[(2, 23)].bg, custom.badge_normal);
}
