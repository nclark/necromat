//! Splits an expression into tokens.

use super::error::LangError;
use super::number::is_valid_number;
use super::token::{Token, TokenKind};

/// True for a character that may start an identifier.
pub fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

/// True for a character that may continue an identifier.
pub fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// True when `s` is a whole identifier: `[A-Za-z_][A-Za-z0-9_]*`.
pub fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(is_ident_start) && chars.all(is_ident_continue)
}

fn is_number_start(c: char) -> bool {
    c.is_ascii_digit() || c == '.'
}

fn is_number_continue(c: char) -> bool {
    c.is_ascii_digit() || c == '.' || c == ','
}

/// Converts an expression into tokens.
///
/// A `-` is unary when it is the first token or follows `+ - * / (`; a unary
/// minus must be followed (after optional whitespace) by a number and folds
/// into one negative Number token. Empty or whitespace-only input gives no
/// tokens.
pub fn tokenize(expr: &str) -> Result<Vec<Token>, LangError> {
    let chars: Vec<char> = expr.trim().chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut i = 0;

    // Collects chars from `start` while `pred` holds; returns the text and
    // the index after it.
    let take_while = |start: usize, pred: fn(char) -> bool| -> (String, usize) {
        let end = chars[start..]
            .iter()
            .position(|&c| !pred(c))
            .map_or(chars.len(), |n| start + n);
        (chars[start..end].iter().collect(), end)
    };

    while let Some(&ch) = chars.get(i) {
        if ch.is_whitespace() {
            i += 1;
        } else if is_number_start(ch) {
            let (text, end) = take_while(i, is_number_continue);
            if !is_valid_number(&text) {
                return Err(LangError::InvalidNumberFormat(text));
            }
            tokens.push(Token::new(TokenKind::Number, text));
            i = end;
        } else if is_ident_start(ch) {
            let (text, end) = take_while(i, is_ident_continue);
            tokens.push(Token::new(TokenKind::Variable, text));
            i = end;
        } else if ch == '-' {
            let unary = tokens.last().is_none_or(|t| {
                matches!(
                    t.kind,
                    TokenKind::Plus
                        | TokenKind::Minus
                        | TokenKind::Multiply
                        | TokenKind::Divide
                        | TokenKind::LParen
                )
            });
            if !unary {
                tokens.push(Token::new(TokenKind::Minus, "-"));
                i += 1;
                continue;
            }
            i += 1;
            while chars.get(i).is_some_and(|c| c.is_whitespace()) {
                i += 1;
            }
            if !chars.get(i).is_some_and(|&c| is_number_start(c)) {
                return Err(LangError::ExpectedNumberAfterUnaryMinus);
            }
            let (digits, end) = take_while(i, is_number_continue);
            let text = format!("-{digits}");
            if !is_valid_number(&text) {
                return Err(LangError::InvalidNumberFormat(text));
            }
            tokens.push(Token::new(TokenKind::Number, text));
            i = end;
        } else {
            let kind = match ch {
                '+' => TokenKind::Plus,
                '*' => TokenKind::Multiply,
                '/' => TokenKind::Divide,
                '(' => TokenKind::LParen,
                ')' => TokenKind::RParen,
                other => return Err(LangError::UnexpectedChar(other)),
            };
            tokens.push(Token::new(kind, ch.to_string()));
            i += 1;
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    fn t(kind: TokenKind, text: &str) -> Token {
        Token::new(kind, text)
    }

    /// Tokens for valid input; errors for invalid input.
    #[test]
    fn tokenize_table() {
        let ok_cases: Vec<(&str, &str, Vec<Token>)> = vec![
            ("simple number", "100", vec![t(Number, "100")]),
            (
                "addition",
                "100 + 50",
                vec![t(Number, "100"), t(Plus, "+"), t(Number, "50")],
            ),
            (
                "variable multiplication",
                "x * 2",
                vec![t(Variable, "x"), t(Multiply, "*"), t(Number, "2")],
            ),
            (
                "parentheses expression",
                "(a + b) * c",
                vec![
                    t(LParen, "("),
                    t(Variable, "a"),
                    t(Plus, "+"),
                    t(Variable, "b"),
                    t(RParen, ")"),
                    t(Multiply, "*"),
                    t(Variable, "c"),
                ],
            ),
            ("negative number", "-50", vec![t(Number, "-50")]),
            ("decimal number", "3.14", vec![t(Number, "3.14")]),
            ("number with commas", "5,000", vec![t(Number, "5,000")]),
            (
                "all operators",
                "a + b - c * d / e",
                vec![
                    t(Variable, "a"),
                    t(Plus, "+"),
                    t(Variable, "b"),
                    t(Minus, "-"),
                    t(Variable, "c"),
                    t(Multiply, "*"),
                    t(Variable, "d"),
                    t(Divide, "/"),
                    t(Variable, "e"),
                ],
            ),
            (
                "variable with underscore and numbers",
                "tax_rate_2024",
                vec![t(Variable, "tax_rate_2024")],
            ),
            (
                "complex expression",
                "(100 + x) * 0.5",
                vec![
                    t(LParen, "("),
                    t(Number, "100"),
                    t(Plus, "+"),
                    t(Variable, "x"),
                    t(RParen, ")"),
                    t(Multiply, "*"),
                    t(Number, "0.5"),
                ],
            ),
            (
                "no spaces",
                "a+b*c",
                vec![
                    t(Variable, "a"),
                    t(Plus, "+"),
                    t(Variable, "b"),
                    t(Multiply, "*"),
                    t(Variable, "c"),
                ],
            ),
            (
                "division",
                "100 / 4",
                vec![t(Number, "100"), t(Divide, "/"), t(Number, "4")],
            ),
            ("empty expression", "", vec![]),
            ("whitespace only", "   ", vec![]),
            (
                "negative number in expression",
                "-2000",
                vec![t(Number, "-2000")],
            ),
            (
                "binary minus (subtraction)",
                "5 - 2",
                vec![t(Number, "5"), t(Minus, "-"), t(Number, "2")],
            ),
            (
                "negative number after multiplication",
                "5 * -2",
                vec![t(Number, "5"), t(Multiply, "*"), t(Number, "-2")],
            ),
            (
                "negative number in parentheses",
                "(-2 + 3)",
                vec![
                    t(LParen, "("),
                    t(Number, "-2"),
                    t(Plus, "+"),
                    t(Number, "3"),
                    t(RParen, ")"),
                ],
            ),
            ("negative decimal", "-3.14", vec![t(Number, "-3.14")]),
            (
                "negative number with commas",
                "-5,000",
                vec![t(Number, "-5,000")],
            ),
        ];
        for (name, input, want) in ok_cases {
            assert_eq!(tokenize(input), Ok(want), "{name}: tokenize({input:?})");
        }

        let err_cases = [
            ("invalid character", "100 $ 50"),
            ("invalid character @", "a @ b"),
            ("malformed comma - irregular grouping", "1,23,4 + 5"),
            ("malformed comma - only 2 digits after comma", "12,34"),
            ("malformed comma - 4 digits before comma", "1234,567"),
            ("malformed comma - trailing comma", "123, + 5"),
            ("malformed comma - double comma", "1,,234"),
            ("malformed comma - 4 digits after comma", "1,2345"),
            (
                "malformed comma - negative number with bad placement",
                "-1,23,4",
            ),
        ];
        for (name, input) in err_cases {
            assert!(tokenize(input).is_err(), "{name}: tokenize({input:?})");
        }
    }

    #[test]
    fn error_messages_name_the_problem() {
        let msg = |s: &str| tokenize(s).map_err(|e| e.to_string());
        assert_eq!(msg("100 $ 50"), Err("unexpected character: $".into()));
        assert_eq!(msg("a @ b"), Err("unexpected character: @".into()));
        assert_eq!(
            msg("12,34"),
            Err("invalid number format: 12,34 (commas must be proper thousand separators)".into())
        );
        assert_eq!(
            msg("-1,23,4"),
            Err(
                "invalid number format: -1,23,4 (commas must be proper thousand separators)".into()
            )
        );
        assert_eq!(
            msg("5 * -x"),
            Err("expected number after unary minus".into())
        );
        assert_eq!(msg("-"), Err("expected number after unary minus".into()));
    }

    #[test]
    fn unary_minus_skips_spaces() {
        assert_eq!(tokenize("- 5"), Ok(vec![t(Number, "-5")]));
        assert_eq!(
            tokenize("x - 5"),
            Ok(vec![t(Variable, "x"), t(Minus, "-"), t(Number, "5")])
        );
    }

    #[test]
    fn non_ascii_is_an_unexpected_character() {
        assert_eq!(tokenize("5 € 3"), Err(LangError::UnexpectedChar('€')));
        assert_eq!(tokenize("é"), Err(LangError::UnexpectedChar('é')));
        assert_eq!(
            tokenize("café"),
            Err(LangError::UnexpectedChar('é')),
            "identifiers stay ASCII"
        );
    }

    #[test]
    fn identifiers() {
        for s in ["x", "_", "_var", "tax_rate_2024", "A1"] {
            assert!(is_identifier(s), "{s}");
        }
        for s in ["", "1a", "a b", "a-b", "é", "a+b"] {
            assert!(!is_identifier(s), "{s}");
        }
    }
}
