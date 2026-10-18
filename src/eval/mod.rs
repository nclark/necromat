//! Evaluation: expressions, the line evaluator with the `sum` accumulator
//! and sections, typed errors, and number formatting. See
//! `doc/design/evaluation.md`.

mod error;
mod evaluator;
mod expression;
mod format;
mod section;

pub use error::{ErrorCode, EvalError};
pub use evaluator::{Evaluator, LineResult};
pub use expression::{SUM_KEYWORD, evaluate_expression};
pub use format::format_number;
pub use section::section_total;
