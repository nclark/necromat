//! Comment stripping, line type detection, and the parsed-line type.

use super::error::LangError;
use super::number::{is_valid_number, parse_number};
use super::token::Token;
use super::tokenizer::{is_identifier, tokenize};

/// What a line is, with the data each kind needs for evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum LineKind {
    /// Blank line, or whitespace only. Acts as a section divider.
    Empty,
    /// Comment-only line (`# ...`).
    Comment,
    /// Plain number: `100`, `-50`, `5,000`.
    Number(f64),
    /// `name = expr`.
    Assignment { name: String, expr: Vec<Token> },
    /// Any other expression that tokenizes: `100 + 50`, `x * 2`, `a`.
    Expression(Vec<Token>),
    /// The `sum` keyword on its own (any case).
    Sum,
    /// A line that does not parse; carries the error message.
    Error(String),
}

/// One parsed input line.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLine {
    pub kind: LineKind,
    /// The original line text.
    pub raw: String,
    /// The trimmed trailing comment without `#`; empty if none.
    pub comment: String,
}

/// Splits a line at the first `#` into trimmed content and trimmed comment
/// (without the `#`).
pub fn strip_comment(line: &str) -> (&str, &str) {
    match line.split_once('#') {
        Some((content, comment)) => (content.trim(), comment.trim()),
        None => (line.trim(), ""),
    }
}

/// Detects the kind of `content`, which should already have its comment
/// stripped. Never returns [`LineKind::Comment`]: that needs the comment,
/// which [`parse_line`] has.
pub fn detect_line_type(content: &str) -> LineKind {
    let content = content.trim();

    if content.is_empty() {
        return LineKind::Empty;
    }

    if content.eq_ignore_ascii_case("sum") {
        return LineKind::Sum;
    }

    if let Some((name, rhs)) = content.split_once('=') {
        let name = name.trim();
        if is_identifier(name) {
            return match tokenize(rhs.trim()) {
                Ok(expr) => LineKind::Assignment {
                    name: name.to_string(),
                    expr,
                },
                Err(e) => LineKind::Error(e.to_string()),
            };
        }
    }

    if is_valid_number(content) {
        return match parse_number(content) {
            Some(n) => LineKind::Number(n),
            None => LineKind::Error(LangError::InvalidNumber(content.to_string()).to_string()),
        };
    }

    match tokenize(content) {
        Ok(expr) => LineKind::Expression(expr),
        Err(e) => LineKind::Error(e.to_string()),
    }
}

/// Parses a single line of input.
pub fn parse_line(line: &str) -> ParsedLine {
    let (content, comment) = strip_comment(line);
    let kind = if content.is_empty() && !comment.is_empty() {
        LineKind::Comment
    } else {
        detect_line_type(content)
    };
    ParsedLine {
        kind,
        raw: line.to_string(),
        comment: comment.to_string(),
    }
}

/// Parses every line.
pub fn parse_all<S: AsRef<str>>(lines: &[S]) -> Vec<ParsedLine> {
    lines.iter().map(|l| parse_line(l.as_ref())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::token::TokenKind;

    /// The variant name, for comparing kinds without their payload.
    fn tag(kind: &LineKind) -> &'static str {
        match kind {
            LineKind::Empty => "Empty",
            LineKind::Comment => "Comment",
            LineKind::Number(_) => "Number",
            LineKind::Assignment { .. } => "Assignment",
            LineKind::Expression(_) => "Expression",
            LineKind::Sum => "Sum",
            LineKind::Error(_) => "Error",
        }
    }

    #[test]
    fn strip_comment_table() {
        let cases = [
            ("100", "100", ""),
            ("100 # note", "100", "note"),
            ("# just comment", "", "just comment"),
            ("  x = 50  # half  ", "x = 50", "half"),
            ("", "", ""),
        ];
        for (input, content, comment) in cases {
            assert_eq!(strip_comment(input), (content, comment), "{input:?}");
        }
    }

    #[test]
    fn strip_comment_unicode() {
        assert_eq!(strip_comment("5 € # café ☕"), ("5 €", "café ☕"));
        assert_eq!(strip_comment("1 # a # b"), ("1", "a # b"));
    }

    #[test]
    fn detect_line_type_table() {
        let cases = [
            // Numbers - valid
            ("100", "Number"),
            ("-50", "Number"),
            ("3.14", "Number"),
            ("5,000", "Number"),
            ("1,234,567.89", "Number"),
            ("123", "Number"),
            ("1234", "Number"),
            ("12,345", "Number"),
            ("123,456", "Number"),
            ("1,234,567", "Number"),
            ("-1,234.56", "Number"),
            // Numbers - malformed comma placement
            ("1,23,4", "Error"),
            ("12,34", "Error"),
            ("1234,567", "Error"),
            (",123", "Error"),
            ("123,", "Error"),
            ("1,,234", "Error"),
            ("1,2345", "Error"),
            // Sum keyword
            ("sum", "Sum"),
            ("SUM", "Sum"),
            ("Sum", "Sum"),
            // Assignments
            ("x=100", "Assignment"),
            ("tax_rate=0.28", "Assignment"),
            ("_var=50", "Assignment"),
            ("var123=100", "Assignment"),
            ("result = 100 + 50", "Assignment"),
            // Expressions
            ("100+50", "Expression"),
            ("100 + 50", "Expression"),
            ("x*2", "Expression"),
            ("x * 2", "Expression"),
            ("(a+b)*c", "Expression"),
            ("(a + b) * c", "Expression"),
            ("a", "Expression"),
            ("100 - 50", "Expression"),
            ("100 / 4", "Expression"),
            // Empty
            ("", "Empty"),
            ("   ", "Empty"),
            ("\t\t", "Empty"),
            // Invalid assignments (no valid variable name on left)
            ("123=100", "Error"),
            ("a+b=100", "Error"),
        ];
        for (input, want) in cases {
            assert_eq!(
                tag(&detect_line_type(input)),
                want,
                "detect_line_type({input:?})"
            );
        }
    }

    #[test]
    fn error_lines_carry_the_tokenizer_message() {
        assert_eq!(
            detect_line_type("a+b=100"),
            LineKind::Error("unexpected character: =".into())
        );
        assert_eq!(
            detect_line_type("x = 1 $ 2"),
            LineKind::Error("unexpected character: $".into())
        );
        assert_eq!(
            detect_line_type("12,34"),
            LineKind::Error(
                "invalid number format: 12,34 (commas must be proper thousand separators)".into()
            )
        );
        let huge = "9".repeat(400);
        assert_eq!(
            detect_line_type(&huge),
            LineKind::Error(format!("invalid number: {huge}"))
        );
    }

    #[test]
    #[allow(clippy::approx_constant)] // 3.14 is a literal, not PI
    fn parse_table() {
        type Check = fn(&ParsedLine);
        let cases: Vec<(&str, &str, &str, Option<Check>)> = vec![
            (
                "plain number",
                "100",
                "Number",
                Some(|p| assert_eq!(p.kind, LineKind::Number(100.0))),
            ),
            (
                "negative number",
                "-10",
                "Number",
                Some(|p| assert_eq!(p.kind, LineKind::Number(-10.0))),
            ),
            (
                "number with commas",
                "5,000",
                "Number",
                Some(|p| assert_eq!(p.kind, LineKind::Number(5000.0))),
            ),
            (
                "decimal number",
                "3.14",
                "Number",
                Some(|p| assert_eq!(p.kind, LineKind::Number(3.14))),
            ),
            ("sum keyword", "sum", "Sum", None),
            ("sum uppercase", "SUM", "Sum", None),
            ("empty line", "", "Empty", None),
            ("whitespace only", "   ", "Empty", None),
            (
                "comment only",
                "# just a comment",
                "Comment",
                Some(|p| assert_eq!(p.comment, "just a comment")),
            ),
            (
                "number with comment",
                "90 # Internet",
                "Number",
                Some(|p| {
                    assert_eq!(p.kind, LineKind::Number(90.0));
                    assert_eq!(p.comment, "Internet");
                }),
            ),
            (
                "simple assignment",
                "x=5000",
                "Assignment",
                Some(|p| match &p.kind {
                    LineKind::Assignment { name, expr } => {
                        assert_eq!(name, "x");
                        assert_eq!(expr.len(), 1);
                    }
                    other => panic!("not an assignment: {other:?}"),
                }),
            ),
            (
                "assignment with expression",
                "y=x*0.28",
                "Assignment",
                Some(|p| match &p.kind {
                    LineKind::Assignment { name, expr } => {
                        assert_eq!(name, "y");
                        assert_eq!(expr.len(), 3);
                    }
                    other => panic!("not an assignment: {other:?}"),
                }),
            ),
            (
                "assignment with sum",
                "bills=sum",
                "Assignment",
                Some(|p| match &p.kind {
                    LineKind::Assignment { name, .. } => assert_eq!(name, "bills"),
                    other => panic!("not an assignment: {other:?}"),
                }),
            ),
            (
                "assignment with negative number",
                "expenses=-2000",
                "Assignment",
                Some(|p| {
                    assert_eq!(
                        p.kind,
                        LineKind::Assignment {
                            name: "expenses".into(),
                            expr: vec![Token::new(TokenKind::Number, "-2000")],
                        }
                    )
                }),
            ),
            (
                "expression",
                "100 + 50",
                "Expression",
                Some(|p| assert!(matches!(&p.kind, LineKind::Expression(e) if e.len() == 3))),
            ),
            (
                "expression with parens",
                "(a + b) * c",
                "Expression",
                Some(|p| assert!(matches!(&p.kind, LineKind::Expression(e) if e.len() == 7))),
            ),
        ];
        for (name, input, want, check) in cases {
            let p = parse_line(input);
            assert_eq!(tag(&p.kind), want, "{name}: parse_line({input:?}).kind");
            assert_eq!(p.raw, input, "{name}: raw");
            if let Some(check) = check {
                check(&p);
            }
        }
    }

    #[test]
    fn parse_all_lines() {
        let input = [
            "100",
            "-10",
            "sum",
            "",
            "x=5000",
            "y=x*0.28",
            "90 # Internet",
            "bills=sum",
        ];
        let r = parse_all(&input);
        assert_eq!(r.len(), input.len());
        assert_eq!(r[0].kind, LineKind::Number(100.0));
        assert_eq!(r[1].kind, LineKind::Number(-10.0));
        assert_eq!(r[2].kind, LineKind::Sum);
        assert_eq!(r[3].kind, LineKind::Empty);
        assert!(matches!(&r[4].kind, LineKind::Assignment { name, .. } if name == "x"));
        assert!(matches!(&r[5].kind, LineKind::Assignment { name, .. } if name == "y"));
        assert_eq!(r[6].kind, LineKind::Number(90.0));
        assert_eq!(r[6].comment, "Internet");
        assert!(matches!(&r[7].kind, LineKind::Assignment { name, .. } if name == "bills"));
    }

    #[test]
    fn lone_hash_is_empty() {
        // `#` with no comment text is an Empty line, not a Comment.
        assert_eq!(parse_line("#").kind, LineKind::Empty);
        assert_eq!(parse_line("  #  ").kind, LineKind::Empty);
    }

    #[test]
    fn assignment_with_empty_rhs_has_no_tokens() {
        assert_eq!(
            parse_line("x =").kind,
            LineKind::Assignment {
                name: "x".into(),
                expr: vec![]
            }
        );
    }
}
