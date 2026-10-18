//! Syntax highlighting for the input column.
//!
//! [`highlight_line`] cuts the raw line into styled spans. The spans always
//! hold exactly the characters of the line, in order: nothing is trimmed,
//! dropped or added. The cursor is placed from the raw line, so a column in
//! the raw text must be the same column on screen.

use ratatui::style::Style;
use ratatui::text::Span;

use super::style;
use crate::lang::{LineKind, ParsedLine, TokenKind, tokenize};

/// Styles `line` (parsed as `parsed`) into spans whose text, joined, is
/// `line`.
///
/// - Comment lines, and blank lines holding a bare `#`, are all comment.
/// - Otherwise the line splits at the first `#`; the tail is comment.
/// - `sum` is a keyword; a plain number is a number; an assignment is the
///   name, `=`, and the expression; an expression is styled token by token.
/// - Lines that do not parse (and expressions that do not tokenize) are left
///   in the default style.
pub fn highlight_line(line: &str, parsed: &ParsedLine) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    match parsed.kind {
        LineKind::Comment => {
            push(&mut out, line, style::comment());
            return out;
        }
        LineKind::Empty => {
            let st = if line.contains('#') {
                style::comment()
            } else {
                Style::new()
            };
            push(&mut out, line, st);
            return out;
        }
        _ => {}
    }

    let (code, comment) = line.find('#').map_or((line, ""), |i| line.split_at(i));

    match &parsed.kind {
        LineKind::Sum => push_trimmed(&mut out, code, style::keyword()),
        LineKind::Number(_) => push_trimmed(&mut out, code, style::number()),
        LineKind::Assignment { .. } => match code.split_once('=') {
            Some((name, expr)) => {
                push_trimmed(&mut out, name, style::variable());
                push(&mut out, "=", style::operator());
                push_expression(&mut out, expr);
            }
            None => push(&mut out, code, Style::new()),
        },
        LineKind::Expression(_) => push_expression(&mut out, code),
        _ => push(&mut out, code, Style::new()),
    }

    push(&mut out, comment, style::comment());
    out
}

/// Appends `text` as one span, skipping empty text.
fn push(out: &mut Vec<Span<'static>>, text: &str, st: Style) {
    if !text.is_empty() {
        out.push(Span::styled(text.to_string(), st));
    }
}

/// Appends `s` with `st` on its trimmed middle; the leading and trailing
/// whitespace stays in place, unstyled.
fn push_trimmed(out: &mut Vec<Span<'static>>, s: &str, st: Style) {
    let rest = s.trim_start();
    let lead = &s[..s.len() - rest.len()];
    let middle = rest.trim_end();
    let trail = &rest[middle.len()..];
    push(out, lead, Style::new());
    push(out, middle, st);
    push(out, trail, Style::new());
}

/// Appends `expr` styled token by token, copying the whitespace between
/// tokens unchanged. If `expr` does not tokenize, or a token cannot be found
/// in the raw text, the whole of `expr` goes in unstyled.
fn push_expression(out: &mut Vec<Span<'static>>, expr: &str) {
    match expression_spans(expr) {
        Some(spans) => out.extend(spans),
        None => push(out, expr, Style::new()),
    }
}

fn expression_spans(expr: &str) -> Option<Vec<Span<'static>>> {
    let tokens = tokenize(expr).ok()?;
    let mut out = Vec::new();
    let mut rest = expr;

    for tok in tokens {
        rest = skip_space(&mut out, rest);
        let len = if tok.kind == TokenKind::Number && tok.text.starts_with('-') {
            // The tokenizer folds a unary minus and any spaces after it into
            // one number token ("- 5" becomes "-5"). Cover the whole raw
            // span so the spaces stay.
            let digits = &tok.text[1..];
            let after_minus = rest.strip_prefix('-')?.trim_start();
            if !after_minus.starts_with(digits) {
                return None;
            }
            rest.len() - after_minus.len() + digits.len()
        } else {
            if !rest.starts_with(tok.text.as_str()) {
                return None;
            }
            tok.text.len()
        };
        let (text, tail) = rest.split_at(len);
        let st = match tok.kind {
            TokenKind::Number => style::number(),
            TokenKind::Variable => style::variable(),
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Multiply
            | TokenKind::Divide
            | TokenKind::LParen
            | TokenKind::RParen => style::operator(),
        };
        push(&mut out, text, st);
        rest = tail;
    }
    push(&mut out, rest, Style::new());
    Some(out)
}

/// Appends the leading whitespace of `s` unstyled and returns the rest.
fn skip_space<'a>(out: &mut Vec<Span<'static>>, s: &'a str) -> &'a str {
    let rest = s.trim_start();
    push(out, &s[..s.len() - rest.len()], Style::new());
    rest
}
