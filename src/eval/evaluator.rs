//! Line evaluation with variables, the `sum` accumulator, and sections.

use std::collections::HashMap;

use super::error::EvalError;
use super::expression::{SUM_KEYWORD, evaluate_expression};
use crate::lang::{LineKind, ParsedLine, Token, TokenKind};

/// The outcome of evaluating one line.
#[derive(Debug, Clone, PartialEq)]
pub struct LineResult {
    /// The numeric result, if the line produced one.
    pub value: Option<f64>,
    /// The error, if evaluation failed.
    pub error: Option<EvalError>,
    /// True when the line read the accumulator (`sum`, `x=sum`, `500-sum`).
    /// Such lines already contain the other lines of their section, so the
    /// section total leaves them out.
    pub used_sum: bool,
    /// Index of the accumulator section the line is in. Sections are split
    /// by runs of blank lines; a blank line belongs to the section above it.
    pub section: usize,
}

impl LineResult {
    /// A line with no value (empty, comment).
    pub fn none() -> Self {
        Self {
            value: None,
            error: None,
            used_sum: false,
            section: 0,
        }
    }

    /// A line with a value.
    pub fn value(v: f64) -> Self {
        Self {
            value: Some(v),
            ..Self::none()
        }
    }

    /// A failed line.
    pub fn error(e: EvalError) -> Self {
        Self {
            error: Some(e),
            ..Self::none()
        }
    }

    pub fn has_value(&self) -> bool {
        self.value.is_some()
    }

    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    fn with_sum(mut self, used_sum: bool) -> Self {
        self.used_sum = used_sum;
        self
    }
}

/// Evaluates parsed lines top to bottom and keeps the state between them.
#[derive(Debug, Default)]
pub struct Evaluator {
    variables: HashMap<String, f64>,
    accumulator: f64,
    section: usize,
    after_blank: bool,
    seen_content: bool,
}

impl Evaluator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears all state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The value of a variable, if set.
    pub fn variable(&self, name: &str) -> Option<f64> {
        self.variables.get(name).copied()
    }

    /// The running sum since the last blank line or `sum`.
    pub fn accumulator(&self) -> f64 {
        self.accumulator
    }

    /// Evaluates one line and updates state.
    pub fn evaluate_line(&mut self, line: &ParsedLine) -> LineResult {
        if matches!(line.kind, LineKind::Empty) {
            self.after_blank = true;
        } else {
            if self.after_blank && self.seen_content {
                self.section += 1;
            }
            self.after_blank = false;
            self.seen_content = true;
        }
        let mut result = self.evaluate_kind(&line.kind);
        result.section = self.section;
        result
    }

    fn evaluate_kind(&mut self, kind: &LineKind) -> LineResult {
        match kind {
            LineKind::Empty => {
                // A blank line is a section divider.
                self.accumulator = 0.0;
                LineResult::none()
            }
            LineKind::Comment => LineResult::none(),
            LineKind::Number(n) => {
                self.accumulator += n;
                LineResult::value(*n)
            }
            LineKind::Expression(expr) => match self.evaluate_expr(expr) {
                Ok((value, used_sum)) => LineResult::value(value).with_sum(used_sum),
                Err(e) => LineResult::error(e),
            },
            LineKind::Assignment { name, expr } => {
                if name == SUM_KEYWORD {
                    return LineResult::error(EvalError::other(
                        "'sum' is a reserved keyword and cannot be used as a variable name",
                    ));
                }
                if let [only] = expr.as_slice()
                    && only.kind == TokenKind::Variable
                    && only.text == SUM_KEYWORD
                {
                    let value = self.accumulator;
                    self.accumulator = 0.0;
                    self.variables.insert(name.clone(), value);
                    return LineResult::value(value).with_sum(true);
                }
                match self.evaluate_expr(expr) {
                    Ok((value, used_sum)) => {
                        self.variables.insert(name.clone(), value);
                        LineResult::value(value).with_sum(used_sum)
                    }
                    Err(e) => LineResult::error(e),
                }
            }
            LineKind::Sum => {
                let value = self.accumulator;
                self.accumulator = 0.0;
                LineResult::value(value).with_sum(true)
            }
            LineKind::Error(message) => LineResult::error(EvalError::from_message(message)),
        }
    }

    /// Evaluates an expression and applies the accumulator rule: reset after
    /// a use of `sum`, otherwise add the value.
    fn evaluate_expr(&mut self, expr: &[Token]) -> Result<(f64, bool), EvalError> {
        let (value, used_sum) = evaluate_expression(expr, &self.variables, self.accumulator)?;
        if used_sum {
            self.accumulator = 0.0;
        } else {
            self.accumulator += value;
        }
        Ok((value, used_sum))
    }

    /// Resets state and evaluates every line in order.
    pub fn evaluate_all(&mut self, lines: &[ParsedLine]) -> Vec<LineResult> {
        self.reset();
        lines.iter().map(|l| self.evaluate_line(l)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::ErrorCode;
    use crate::lang::parse_all;
    use TokenKind::*;

    fn t(kind: TokenKind, text: &str) -> Token {
        Token::new(kind, text)
    }

    fn line(kind: LineKind) -> ParsedLine {
        ParsedLine {
            kind,
            raw: String::new(),
            comment: String::new(),
        }
    }

    fn num(n: f64) -> ParsedLine {
        line(LineKind::Number(n))
    }

    fn expr(tokens: Vec<Token>) -> ParsedLine {
        line(LineKind::Expression(tokens))
    }

    fn assign(name: &str, tokens: Vec<Token>) -> ParsedLine {
        line(LineKind::Assignment {
            name: name.into(),
            expr: tokens,
        })
    }

    fn sum() -> ParsedLine {
        line(LineKind::Sum)
    }

    fn run(input: &[&str]) -> Vec<LineResult> {
        Evaluator::new().evaluate_all(&parse_all(input))
    }

    fn assert_close(actual: Option<f64>, expected: f64) {
        const EPSILON: f64 = 1e-9;
        match actual {
            Some(v) if (v - expected).abs() <= EPSILON => {}
            other => panic!("expected {expected}, got {other:?}"),
        }
    }

    fn message(r: &LineResult) -> String {
        r.error
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default()
    }

    // ---- line results ----

    #[test]
    fn no_result() {
        let r = LineResult::none();
        assert!(!r.has_value());
        assert!(!r.is_error());
    }

    #[test]
    fn value_result() {
        for v in [42.0, -10.5, 0.0] {
            let r = LineResult::value(v);
            assert!(r.has_value());
            assert!(!r.is_error());
            assert_eq!(r.value, Some(v));
        }
    }

    #[test]
    fn error_result() {
        let r = LineResult::error(EvalError::division_by_zero());
        assert!(!r.has_value());
        assert!(r.is_error());
        assert_eq!(message(&r), "division by zero");
    }

    // ---- evaluation ----

    #[test]
    fn new_evaluator() {
        let e = Evaluator::new();
        assert!(e.variables.is_empty());
        assert_eq!(e.accumulator, 0.0);
    }

    #[test]
    fn reset_clears_state() {
        let mut e = Evaluator::new();
        e.variables.insert("x".into(), 100.0);
        e.accumulator = 50.0;
        e.reset();
        assert!(e.variables.is_empty());
        assert_eq!(e.accumulator, 0.0);
    }

    #[test]
    fn evaluate_line_placeholder() {
        let mut e = Evaluator::new();
        assert!(!e.evaluate_line(&line(LineKind::Empty)).has_value());
    }

    #[test]
    fn evaluate_all_resets() {
        let mut e = Evaluator::new();
        e.accumulator = 99.0;
        let lines = [
            line(LineKind::Empty),
            line(LineKind::Comment),
            line(LineKind::Empty),
        ];
        let results = e.evaluate_all(&lines);
        assert_eq!(results.len(), lines.len());
        assert_eq!(e.accumulator, 0.0);
    }

    #[test]
    fn empty_and_comment() {
        let mut e = Evaluator::new();
        assert!(!e.evaluate_line(&line(LineKind::Empty)).has_value());
        assert!(!e.evaluate_line(&line(LineKind::Comment)).has_value());
    }

    #[test]
    fn number_line() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&num(100.0));
        assert_eq!(r.value, Some(100.0));
        assert_eq!(e.accumulator, 100.0);
    }

    #[test]
    fn expression_line() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&expr(vec![t(Number, "50"), t(Plus, "+"), t(Number, "25")]));
        assert_eq!(r.value, Some(75.0));
        assert_eq!(e.accumulator, 75.0);
    }

    #[test]
    fn assignment_line_adds_to_accumulator() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&assign("x", vec![t(Number, "100")]));
        assert_eq!(r.value, Some(100.0));
        assert_eq!(e.accumulator, 100.0);
        assert_eq!(e.variable("x"), Some(100.0));
    }

    #[test]
    fn sum_line() {
        let mut e = Evaluator::new();
        e.accumulator = 300.0;
        let r = e.evaluate_line(&sum());
        assert_eq!(r.value, Some(300.0));
        assert!(r.used_sum);
        assert_eq!(e.accumulator, 0.0);
    }

    #[test]
    fn sum_accumulator_full_workflow() {
        let mut e = Evaluator::new();
        assert_eq!(e.evaluate_line(&num(100.0)).value, Some(100.0));
        assert_eq!(e.evaluate_line(&num(200.0)).value, Some(200.0));
        assert_eq!(e.evaluate_line(&sum()).value, Some(300.0));
        assert_eq!(
            e.evaluate_line(&assign("x", vec![t(Number, "50")])).value,
            Some(50.0)
        );
        assert_eq!(e.evaluate_line(&num(25.0)).value, Some(25.0));
        assert_eq!(e.evaluate_line(&sum()).value, Some(75.0));
    }

    #[test]
    fn sum_accumulator_edge_cases() {
        let mut e = Evaluator::new();
        assert_eq!(e.evaluate_line(&sum()).value, Some(0.0), "sum at start");

        e.accumulator = 100.0;
        e.evaluate_line(&sum());
        assert_eq!(e.evaluate_line(&sum()).value, Some(0.0), "second sum");

        let mut e = Evaluator::new();
        e.evaluate_line(&assign("x", vec![t(Number, "100")]));
        assert_eq!(
            e.evaluate_line(&sum()).value,
            Some(100.0),
            "after assignments"
        );
    }

    #[test]
    fn error_line() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&line(LineKind::Error("parse error".into())));
        assert!(r.is_error());
        assert_eq!(message(&r), "parse error");
    }

    #[test]
    fn sum_in_expressions() {
        let mut e = Evaluator::new();
        for n in [25.0, 100.0, 187.0] {
            e.evaluate_line(&num(n));
        }
        assert_eq!(e.accumulator, 312.0);
        let r = e.evaluate_line(&assign(
            "x",
            vec![t(Number, "500"), t(Minus, "-"), t(Variable, "sum")],
        ));
        assert_eq!(r.value, Some(188.0));
        assert!(r.used_sum);
        assert_eq!(e.accumulator, 0.0);
        assert_eq!(e.variable("x"), Some(188.0));
    }

    #[test]
    fn sum_in_expressions_multiple_uses() {
        let mut e = Evaluator::new();
        e.evaluate_line(&num(50.0));
        let r = e.evaluate_line(&assign(
            "doubled",
            vec![t(Variable, "sum"), t(Plus, "+"), t(Variable, "sum")],
        ));
        assert_eq!(r.value, Some(100.0));
        assert_eq!(e.accumulator, 0.0);
    }

    #[test]
    fn sum_in_expressions_with_variables() {
        let mut e = Evaluator::new();
        e.evaluate_line(&assign("starting_balance", vec![t(Number, "500")]));
        e.evaluate_line(&line(LineKind::Empty));
        for n in [25.0, 100.0, 187.0] {
            e.evaluate_line(&num(n));
        }
        let r = e.evaluate_line(&assign(
            "ending_balance",
            vec![
                t(Variable, "starting_balance"),
                t(Minus, "-"),
                t(Variable, "sum"),
            ],
        ));
        assert_eq!(r.value, Some(188.0));
        assert_eq!(e.accumulator, 0.0);
    }

    #[test]
    fn sum_in_standalone_expression() {
        let mut e = Evaluator::new();
        e.evaluate_line(&num(100.0));
        e.evaluate_line(&num(50.0));
        let r = e.evaluate_line(&expr(vec![
            t(Number, "200"),
            t(Minus, "-"),
            t(Variable, "sum"),
        ]));
        assert_eq!(r.value, Some(50.0));
        assert!(r.used_sum);
        assert_eq!(e.accumulator, 0.0);
    }

    #[test]
    fn sum_as_variable_name_is_an_error() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&assign("sum", vec![t(Number, "100")]));
        assert!(r.is_error());
        assert_eq!(
            message(&r),
            "'sum' is a reserved keyword and cannot be used as a variable name"
        );
    }

    #[test]
    fn sum_before_any_accumulation() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&assign(
            "x",
            vec![t(Number, "100"), t(Plus, "+"), t(Variable, "sum")],
        ));
        assert_eq!(r.value, Some(100.0));
    }

    // ---- errors ----

    #[test]
    fn undefined_variable() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&expr(vec![
            t(Variable, "x"),
            t(Multiply, "*"),
            t(Number, "2"),
        ]));
        assert!(r.is_error());
        assert!(message(&r).contains("undefined variable"));
        assert!(message(&r).contains('x'));
        assert_eq!(r.error.map(|e| e.short()), Some("undef: x".into()));
    }

    #[test]
    fn division_by_zero_literal() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&expr(vec![
            t(Number, "100"),
            t(Divide, "/"),
            t(Number, "0"),
        ]));
        assert!(message(&r).contains("division by zero"));
    }

    #[test]
    fn division_by_zero_variable() {
        let mut e = Evaluator::new();
        e.variables.insert("x".into(), 0.0);
        let r = e.evaluate_line(&expr(vec![
            t(Number, "100"),
            t(Divide, "/"),
            t(Variable, "x"),
        ]));
        assert!(message(&r).contains("division by zero"));
    }

    #[test]
    fn parse_error_is_propagated() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&line(LineKind::Error("unexpected token".into())));
        assert!(r.is_error());
        assert_eq!(message(&r), "unexpected token");
    }

    #[test]
    fn mixed_errors_and_success() {
        let mut e = Evaluator::new();
        assert!(!e.evaluate_line(&num(100.0)).is_error());
        assert!(
            e.evaluate_line(&expr(vec![t(Variable, "undefined")]))
                .is_error()
        );
        assert!(!e.evaluate_line(&num(50.0)).is_error());
        assert_eq!(e.evaluate_line(&sum()).value, Some(150.0));
    }

    #[test]
    fn complex_error_scenario() {
        let mut e = Evaluator::new();
        let div_y = || expr(vec![t(Number, "100"), t(Divide, "/"), t(Variable, "y")]);
        assert!(
            !e.evaluate_line(&assign("y", vec![t(Number, "10")]))
                .is_error()
        );
        assert_eq!(e.evaluate_line(&div_y()).value, Some(10.0));
        assert!(
            !e.evaluate_line(&assign("y", vec![t(Number, "0")]))
                .is_error()
        );
        let r = e.evaluate_line(&div_y());
        assert!(message(&r).contains("division by zero"));
    }

    #[test]
    fn invalid_expression() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&expr(vec![]));
        assert!(r.is_error());
        assert_eq!(message(&r), "empty expression");
        // An empty assignment right side takes the same path.
        let r = e.evaluate_line(&assign("x", vec![]));
        assert!(r.is_error());
        assert_eq!(e.variable("x"), None);
    }

    #[test]
    fn assignment_with_undefined_variable() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&assign(
            "x",
            vec![t(Variable, "y"), t(Plus, "+"), t(Number, "10")],
        ));
        assert!(message(&r).contains("undefined variable"));
        assert_eq!(e.variable("x"), None);
    }

    #[test]
    fn assignment_with_division_by_zero() {
        let mut e = Evaluator::new();
        let r = e.evaluate_line(&assign(
            "x",
            vec![t(Number, "100"), t(Divide, "/"), t(Number, "0")],
        ));
        assert!(r.is_error());
        assert_eq!(e.variable("x"), None);
    }

    #[test]
    fn parse_errors_get_short_codes() {
        let r = run(&["a+b=100", "1,23,4", "5 * -x", "(1"]);
        let shorts: Vec<String> = r
            .iter()
            .map(|r| r.error.as_ref().map(EvalError::short).unwrap_or_default())
            .collect();
        assert_eq!(shorts, ["syntax", "syntax", "error", "parens"]);
        assert_eq!(
            r[3].error.as_ref().map(|e| e.code),
            Some(ErrorCode::MismatchedParens)
        );
    }

    // ---- whole documents ----

    #[test]
    fn worked_example_simple_sum() {
        let r = run(&["100", "-10", "sum"]);
        assert_eq!(r.len(), 3);
        assert_close(r[0].value, 100.0);
        assert_close(r[1].value, -10.0);
        assert_close(r[2].value, 90.0);
    }

    #[test]
    fn worked_example_budget() {
        let r = run(&[
            "starting_balance=5000",
            "taxes=starting_balance*0.28",
            "",
            "90 # Internet",
            "25 # Mint Mobile",
            "85 # Dog food",
            "bills=sum",
            "",
            "balance=starting_balance-taxes-bills",
        ]);
        assert_close(r[0].value, 5000.0);
        assert_close(r[1].value, 1400.0);
        assert!(!r[2].has_value());
        assert_close(r[3].value, 90.0);
        assert_close(r[4].value, 25.0);
        assert_close(r[5].value, 85.0);
        assert_close(r[6].value, 200.0);
        assert!(!r[7].has_value());
        assert_close(r[8].value, 3400.0);
    }

    #[test]
    fn assignments_do_sum() {
        let r = run(&[
            "rent=1200",
            "utilities=150",
            "groceries=400",
            "entertainment=100",
            "total_expenses=sum",
        ]);
        assert_close(r[0].value, 1200.0);
        assert_close(r[1].value, 150.0);
        assert_close(r[2].value, 400.0);
        assert_close(r[3].value, 100.0);
        assert_close(r[4].value, 1850.0);
    }

    #[test]
    fn workflow_mixed_assignments_and_expressions() {
        let r = run(&[
            "hourly_rate=50",
            "hours_worked=40",
            "gross_pay=hourly_rate*hours_worked",
            "",
            "# Deductions",
            "retirement=gross_pay*0.06",
            "200 # health",
            "50  # dental",
            "total_deductions=sum",
            "",
            "net_pay=gross_pay-retirement-total_deductions",
        ]);
        assert_close(r[0].value, 50.0);
        assert_close(r[1].value, 40.0);
        assert_close(r[2].value, 2000.0);
        assert_close(r[5].value, 120.0);
        assert_close(r[6].value, 200.0);
        assert_close(r[7].value, 50.0);
        assert_close(r[8].value, 370.0);
        assert_close(r[10].value, 1510.0);
    }

    #[test]
    fn workflow_with_comments() {
        let r = run(&[
            "# Monthly Budget",
            "income=5000 # After-tax income",
            "",
            "1200 # Rent",
            "300  # Groceries",
            "sum  # Total expenses",
        ]);
        assert!(!r[0].has_value());
        assert_close(r[1].value, 5000.0);
        assert!(!r[2].has_value());
        assert_close(r[3].value, 1200.0);
        assert_close(r[4].value, 300.0);
        assert_close(r[5].value, 1500.0);
    }

    #[test]
    fn workflow_error_recovery() {
        let r = run(&[
            "x=100",
            "y=x*2",
            "z=undefined*5",
            "a=y+50",
            "b=100/0",
            "c=a-50",
            "sum",
        ]);
        assert_close(r[0].value, 100.0);
        assert_close(r[1].value, 200.0);
        assert!(r[2].is_error());
        assert_close(r[3].value, 250.0);
        assert!(r[4].is_error());
        assert_close(r[5].value, 200.0);
        assert_close(r[6].value, 750.0);
    }

    #[test]
    fn workflow_complex_expressions() {
        let r = run(&[
            "base=1000",
            "multiplier=1.5",
            "bonus=base*multiplier+200",
            "tax_rate=0.25",
            "after_tax=bonus*(1-tax_rate)",
        ]);
        assert_close(r[0].value, 1000.0);
        assert_close(r[1].value, 1.5);
        assert_close(r[2].value, 1700.0);
        assert_close(r[3].value, 0.25);
        assert_close(r[4].value, 1275.0);
    }

    #[test]
    fn simple_assignment_sum() {
        let r = run(&["one=1", "two=2", "sum"]);
        assert_close(r[0].value, 1.0);
        assert_close(r[1].value, 2.0);
        assert_close(r[2].value, 3.0);
    }

    #[test]
    fn blank_line_resets_accumulator() {
        let r = run(&["10", "20", "", "5", "3", "sum"]);
        assert_close(r[0].value, 10.0);
        assert_close(r[1].value, 20.0);
        assert!(!r[2].has_value());
        assert_close(r[3].value, 5.0);
        assert_close(r[4].value, 3.0);
        assert_close(r[5].value, 8.0);
    }

    #[test]
    fn blank_lines_section_dividers() {
        let r = run(&["100", "200", "", "50", "25", "", "10", "sum"]);
        assert_close(r[0].value, 100.0);
        assert_close(r[1].value, 200.0);
        assert!(!r[2].has_value());
        assert_close(r[3].value, 50.0);
        assert_close(r[4].value, 25.0);
        assert!(!r[5].has_value());
        assert_close(r[6].value, 10.0);
        assert_close(r[7].value, 10.0);
    }

    #[test]
    fn issue_example_nm_b74_14() {
        let r = run(&[
            "starting_balance=500",
            "",
            "25 # ChatGPT",
            "100 # Claude Max 5x",
            "187 # Snoop Dogg tickets",
            "ending_balance=starting_balance-sum",
        ]);
        assert_close(r[0].value, 500.0);
        assert!(!r[1].has_value());
        assert_close(r[2].value, 25.0);
        assert_close(r[3].value, 100.0);
        assert_close(r[4].value, 187.0);
        assert_close(r[5].value, 188.0);
    }

    // ---- sections ----

    #[test]
    fn section_indices() {
        let r = run(&["", "1", "2", "", "", "# c", "3", "", "4", ""]);
        let sections: Vec<usize> = r.iter().map(|r| r.section).collect();
        assert_eq!(sections, [0, 0, 0, 0, 0, 1, 1, 1, 2, 2]);
    }

    #[test]
    fn used_sum_marks_lines_that_read_the_accumulator() {
        let r = run(&["1", "sum", "x=sum", "y=2-sum", "z=3", "4+5"]);
        let used: Vec<bool> = r.iter().map(|r| r.used_sum).collect();
        assert_eq!(used, [false, true, true, true, false, false]);
    }
}
