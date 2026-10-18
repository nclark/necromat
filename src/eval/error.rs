//! Typed evaluation errors with short codes for the results column.

use std::fmt;

/// The class of an evaluation error. Decides the short code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// A name with no value. Short code `undef: NAME`.
    UndefinedVariable,
    /// Division by zero. Short code `div/0`.
    DivisionByZero,
    /// Unbalanced parentheses. Short code `parens`.
    MismatchedParens,
    /// Bad number or unexpected character. Short code `syntax`.
    Syntax,
    /// Anything else (empty or malformed expression, reserved name, ...).
    /// Short code `error`.
    Other,
}

/// An error from evaluating one line.
///
/// `detail` holds the variable name for [`ErrorCode::UndefinedVariable`], is
/// empty for [`ErrorCode::DivisionByZero`] and [`ErrorCode::MismatchedParens`],
/// and holds the full message for the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalError {
    pub code: ErrorCode,
    pub detail: String,
}

impl EvalError {
    pub fn undefined_variable(name: &str) -> Self {
        Self {
            code: ErrorCode::UndefinedVariable,
            detail: name.to_string(),
        }
    }

    pub fn division_by_zero() -> Self {
        Self {
            code: ErrorCode::DivisionByZero,
            detail: String::new(),
        }
    }

    pub fn mismatched_parens() -> Self {
        Self {
            code: ErrorCode::MismatchedParens,
            detail: String::new(),
        }
    }

    pub fn syntax(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Syntax,
            detail: message.into(),
        }
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Other,
            detail: message.into(),
        }
    }

    /// Classifies a message (for example a parse error carried by a line)
    /// into the matching short code.
    pub fn from_message(message: &str) -> Self {
        if message.contains("undefined variable:") {
            let name = message.split_once(':').map_or("", |(_, name)| name).trim();
            Self::undefined_variable(name)
        } else if message.contains("division by zero") {
            Self::division_by_zero()
        } else if message.contains("mismatched parentheses") {
            Self::mismatched_parens()
        } else if message.contains("invalid number") || message.contains("unexpected") {
            Self::syntax(message)
        } else {
            Self::other(message)
        }
    }

    /// The short code shown in the results column.
    pub fn short(&self) -> String {
        match self.code {
            ErrorCode::UndefinedVariable => format!("undef: {}", self.detail),
            ErrorCode::DivisionByZero => "div/0".to_string(),
            ErrorCode::MismatchedParens => "parens".to_string(),
            ErrorCode::Syntax => "syntax".to_string(),
            ErrorCode::Other => "error".to_string(),
        }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code {
            ErrorCode::UndefinedVariable => write!(f, "undefined variable: {}", self.detail),
            ErrorCode::DivisionByZero => f.write_str("division by zero"),
            ErrorCode::MismatchedParens => f.write_str("mismatched parentheses"),
            ErrorCode::Syntax | ErrorCode::Other => f.write_str(&self.detail),
        }
    }
}

impl std::error::Error for EvalError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_messages() {
        assert_eq!(
            EvalError::undefined_variable("x").to_string(),
            "undefined variable: x"
        );
        assert_eq!(
            EvalError::division_by_zero().to_string(),
            "division by zero"
        );
        assert_eq!(
            EvalError::mismatched_parens().to_string(),
            "mismatched parentheses"
        );
        assert_eq!(
            EvalError::syntax("invalid number: 1x").to_string(),
            "invalid number: 1x"
        );
    }

    #[test]
    fn short_codes_from_messages() {
        let cases = [
            ("undefined variable: x", "undef: x"),
            ("undefined variable: tax_rate", "undef: tax_rate"),
            ("division by zero", "div/0"),
            ("mismatched parentheses", "parens"),
            ("invalid number: 1.2.3", "syntax"),
            (
                "invalid number format: 12,34 (commas must be proper thousand separators)",
                "syntax",
            ),
            ("unexpected character: $", "syntax"),
            ("unexpected token", "syntax"),
            ("empty expression", "error"),
            ("invalid expression", "error"),
            ("expected number after unary minus", "error"),
            (
                "'sum' is a reserved keyword and cannot be used as a variable name",
                "error",
            ),
            ("", "error"),
        ];
        for (message, want) in cases {
            let err = EvalError::from_message(message);
            assert_eq!(err.short(), want, "{message:?}");
            assert_eq!(err.to_string(), message, "round trip {message:?}");
        }
    }
}
