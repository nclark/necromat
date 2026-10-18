//! Expression tokens.

/// The kind of a single expression token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Number,
    Variable,
    Plus,
    Minus,
    Multiply,
    Divide,
    LParen,
    RParen,
}

/// One token of an expression. `text` is the exact source text, so a number
/// keeps its commas (`5,000`) and a unary minus is folded into it (`-50`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
}

impl Token {
    pub fn new(kind: TokenKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }
}
