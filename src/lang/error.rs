//! Typed errors from the tokenizer and line parser.

use std::fmt;

/// An error found while tokenizing or parsing one line. `Display` gives the
/// message shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LangError {
    /// A character that cannot start any token.
    UnexpectedChar(char),
    /// A number whose commas are not proper thousands separators.
    InvalidNumberFormat(String),
    /// A unary minus that is not followed by a number.
    ExpectedNumberAfterUnaryMinus,
    /// A well-formed number that does not fit in an `f64`.
    InvalidNumber(String),
}

impl fmt::Display for LangError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedChar(c) => write!(f, "unexpected character: {c}"),
            Self::InvalidNumberFormat(s) => write!(
                f,
                "invalid number format: {s} (commas must be proper thousand separators)"
            ),
            Self::ExpectedNumberAfterUnaryMinus => f.write_str("expected number after unary minus"),
            Self::InvalidNumber(s) => write!(f, "invalid number: {s}"),
        }
    }
}

impl std::error::Error for LangError {}
