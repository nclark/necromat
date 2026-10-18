//! `nm eval`: evaluate stdin and print one JSON object per input line.
//!
//! This is the interface for the spec tests and editor plugins, so the shape
//! is fixed:
//!
//! ```text
//! {"line":1,"value":5000.0,"text":"5,000","error":null}
//! {"line":3,"value":null,"text":"undef: x","error":"undefined variable: x"}
//! {"line":4,"value":null,"text":"","error":null}
//! ```
//!
//! - `line` is 1-based.
//! - `value` is the number, or `null` for errors, lines with no result, and
//!   non-finite results (JSON has no `inf`/`NaN`; `text` still shows them).
//! - `text` is what the results column shows: the formatted number, the
//!   short error code, or empty.
//! - `error` is the full error message, or `null`.

use std::io::{self, BufWriter, Read, Write};
use std::process::ExitCode;

use serde::Serialize;

use crate::eval::{Evaluator, LineResult, format_number};
use crate::lang::parse_all;
use crate::storage::split_lines;

/// One output record. Field order is the output order.
#[derive(Debug, Serialize)]
struct Record<'a> {
    line: usize,
    value: Option<f64>,
    text: &'a str,
    error: Option<String>,
}

/// Evaluates `input` (file content: CRLF or LF, final newline optional) and
/// returns one JSON object per line, without newlines. Empty input gives no
/// records.
pub fn eval_json_lines(input: &str) -> Vec<String> {
    let lines = split_lines(input);
    let results = Evaluator::new().evaluate_all(&parse_all(&lines));
    results
        .iter()
        .enumerate()
        .map(|(i, r)| record_json(i + 1, r))
        .collect()
}

fn record_json(line: usize, result: &LineResult) -> String {
    let text = match (&result.error, result.value) {
        (Some(e), _) => e.short(),
        (None, Some(v)) => format_number(v),
        (None, None) => String::new(),
    };
    let record = Record {
        line,
        value: result
            .value
            .filter(|v| v.is_finite() && result.error.is_none()),
        text: &text,
        error: result.error.as_ref().map(ToString::to_string),
    };
    // Serializing a struct of plain fields to a string cannot fail; fall
    // back to a hand-built error record rather than panic.
    serde_json::to_string(&record)
        .unwrap_or_else(|_| format!(r#"{{"line":{line},"value":null,"text":"","error":null}}"#))
}

/// Runs the subcommand on the process's stdin and stdout.
pub fn run() -> ExitCode {
    let mut input = Vec::new();
    if let Err(e) = io::stdin().lock().read_to_end(&mut input) {
        eprintln!("nm eval: reading stdin: {e}");
        return ExitCode::FAILURE;
    }
    let input = String::from_utf8_lossy(&input);
    match write_lines(&mut io::stdout().lock(), &eval_json_lines(&input)) {
        Ok(()) => ExitCode::SUCCESS,
        // The reader went away (e.g. `nm eval | head -1`): not an error.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nm eval: writing stdout: {e}");
            ExitCode::FAILURE
        }
    }
}

fn write_lines(out: &mut impl Write, lines: &[String]) -> io::Result<()> {
    let mut out = BufWriter::new(out);
    for line in lines {
        writeln!(out, "{line}")?;
    }
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_line() {
        assert_eq!(
            eval_json_lines("income=5000\n"),
            [r#"{"line":1,"value":5000.0,"text":"5,000","error":null}"#]
        );
    }

    #[test]
    fn error_line() {
        assert_eq!(
            eval_json_lines("1\n2\nx+1\n")[2],
            r#"{"line":3,"value":null,"text":"undef: x","error":"undefined variable: x"}"#
        );
        assert_eq!(
            eval_json_lines("1/0")[0],
            r#"{"line":1,"value":null,"text":"div/0","error":"division by zero"}"#
        );
    }

    #[test]
    fn no_result_lines() {
        let empty = r#"{"line":1,"value":null,"text":"","error":null}"#;
        let comment = r#"{"line":2,"value":null,"text":"","error":null}"#;
        assert_eq!(eval_json_lines("\n# just a note\n"), [empty, comment]);
    }

    #[test]
    fn fractions_and_crlf() {
        assert_eq!(
            eval_json_lines("1.5\r\n0.1+0.2\r\n"),
            [
                r#"{"line":1,"value":1.5,"text":"1.5","error":null}"#,
                r#"{"line":2,"value":0.30000000000000004,"text":"0.3","error":null}"#,
            ]
        );
    }

    #[test]
    fn empty_input_prints_nothing() {
        assert!(eval_json_lines("").is_empty());
    }

    #[test]
    fn strings_are_escaped() {
        let out = eval_json_lines("\"q\\\n");
        assert_eq!(out.len(), 1);
        let v: serde_json::Value = serde_json::from_str(&out[0]).unwrap();
        assert_eq!(v["line"], 1);
    }

    #[test]
    fn write_lines_appends_newlines() {
        let mut buf = Vec::new();
        write_lines(&mut buf, &["a".into(), "b".into()]).unwrap();
        assert_eq!(buf, b"a\nb\n");
    }
}
