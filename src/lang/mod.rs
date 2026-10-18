//! Language front end: tokens, number rules, tokenizer, and line parsing.
//! See `doc/design/language.md`.

mod error;
mod line;
mod number;
mod token;
mod tokenizer;

pub use error::LangError;
pub use line::{LineKind, ParsedLine, detect_line_type, parse_all, parse_line, strip_comment};
pub use number::{is_valid_number, parse_number};
pub use token::{Token, TokenKind};
pub use tokenizer::{is_ident_continue, is_ident_start, is_identifier, tokenize};
