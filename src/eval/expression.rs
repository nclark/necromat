//! Expression evaluation by shunting-yard.

use std::collections::HashMap;

use super::error::EvalError;
use crate::lang::{Token, TokenKind, parse_number};

/// The name that reads the accumulator inside an expression.
pub const SUM_KEYWORD: &str = "sum";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl BinOp {
    fn precedence(self) -> u8 {
        match self {
            Self::Add | Self::Sub => 1,
            Self::Mul | Self::Div => 2,
        }
    }

    fn apply(self, a: f64, b: f64) -> Result<f64, EvalError> {
        Ok(match self {
            Self::Add => a + b,
            Self::Sub => a - b,
            Self::Mul => a * b,
            Self::Div if b == 0.0 => return Err(EvalError::division_by_zero()),
            Self::Div => a / b,
        })
    }
}

/// An entry on the operator stack.
#[derive(Debug, Clone, Copy)]
enum StackOp {
    LParen,
    Bin(BinOp),
}

/// An entry of the postfix output.
#[derive(Debug, Clone, Copy)]
enum Item {
    Num(f64),
    Op(BinOp),
}

/// Evaluates a token list. `sum` reads `accumulator`. Returns the value and
/// whether `sum` was used.
pub fn evaluate_expression(
    tokens: &[Token],
    variables: &HashMap<String, f64>,
    accumulator: f64,
) -> Result<(f64, bool), EvalError> {
    if tokens.is_empty() {
        return Err(EvalError::other("empty expression"));
    }
    let (postfix, used_sum) = to_postfix(tokens, variables, accumulator)?;
    let value = eval_postfix(&postfix)?;
    Ok((value, used_sum))
}

fn to_postfix(
    tokens: &[Token],
    variables: &HashMap<String, f64>,
    accumulator: f64,
) -> Result<(Vec<Item>, bool), EvalError> {
    let mut output = Vec::with_capacity(tokens.len());
    let mut stack: Vec<StackOp> = Vec::new();
    let mut used_sum = false;

    for token in tokens {
        let op = match token.kind {
            TokenKind::Number => {
                let n = parse_number(&token.text)
                    .ok_or_else(|| EvalError::syntax(format!("invalid number: {}", token.text)))?;
                output.push(Item::Num(n));
                continue;
            }
            TokenKind::Variable => {
                let value = if token.text == SUM_KEYWORD {
                    used_sum = true;
                    accumulator
                } else {
                    *variables
                        .get(&token.text)
                        .ok_or_else(|| EvalError::undefined_variable(&token.text))?
                };
                output.push(Item::Num(value));
                continue;
            }
            TokenKind::LParen => {
                stack.push(StackOp::LParen);
                continue;
            }
            TokenKind::RParen => {
                loop {
                    match stack.pop() {
                        Some(StackOp::LParen) => break,
                        Some(StackOp::Bin(op)) => output.push(Item::Op(op)),
                        None => return Err(EvalError::mismatched_parens()),
                    }
                }
                continue;
            }
            TokenKind::Plus => BinOp::Add,
            TokenKind::Minus => BinOp::Sub,
            TokenKind::Multiply => BinOp::Mul,
            TokenKind::Divide => BinOp::Div,
        };
        while let Some(&StackOp::Bin(top)) = stack.last() {
            if top.precedence() < op.precedence() {
                break;
            }
            stack.pop();
            output.push(Item::Op(top));
        }
        stack.push(StackOp::Bin(op));
    }

    while let Some(top) = stack.pop() {
        match top {
            StackOp::LParen => return Err(EvalError::mismatched_parens()),
            StackOp::Bin(op) => output.push(Item::Op(op)),
        }
    }

    Ok((output, used_sum))
}

fn eval_postfix(postfix: &[Item]) -> Result<f64, EvalError> {
    let mut stack: Vec<f64> = Vec::new();
    for item in postfix {
        match *item {
            Item::Num(n) => stack.push(n),
            Item::Op(op) => {
                let (Some(b), Some(a)) = (stack.pop(), stack.pop()) else {
                    return Err(EvalError::other("invalid expression"));
                };
                stack.push(op.apply(a, b)?);
            }
        }
    }
    match stack.as_slice() {
        [value] => Ok(*value),
        _ => Err(EvalError::other("invalid expression")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::ErrorCode;
    use crate::lang::tokenize;
    use TokenKind::*;

    fn t(kind: TokenKind, text: &str) -> Token {
        Token::new(kind, text)
    }

    fn eval(tokens: &[Token]) -> Result<(f64, bool), EvalError> {
        evaluate_expression(tokens, &HashMap::new(), 0.0)
    }

    fn eval_str(s: &str) -> Result<f64, EvalError> {
        let tokens = tokenize(s).map_err(|e| EvalError::from_message(&e.to_string()))?;
        evaluate_expression(&tokens, &HashMap::new(), 0.0).map(|(v, _)| v)
    }

    fn check_table(cases: &[(&str, Vec<Token>, f64)], vars: &HashMap<String, f64>) {
        for (name, tokens, want) in cases {
            match evaluate_expression(tokens, vars, 0.0) {
                Ok((got, _)) => assert_eq!(got, *want, "{name}"),
                Err(e) => panic!("{name}: unexpected error {e}"),
            }
        }
    }

    #[test]
    #[allow(clippy::approx_constant)] // 3.14 is a literal, not PI
    fn numbers() {
        check_table(
            &[
                ("simple number", vec![t(Number, "100")], 100.0),
                ("negative number", vec![t(Number, "-50")], -50.0),
                ("decimal number", vec![t(Number, "3.14")], 3.14),
            ],
            &HashMap::new(),
        );
    }

    #[test]
    fn basic_operations() {
        check_table(
            &[
                (
                    "addition",
                    vec![t(Number, "100"), t(Plus, "+"), t(Number, "50")],
                    150.0,
                ),
                (
                    "subtraction",
                    vec![t(Number, "100"), t(Minus, "-"), t(Number, "50")],
                    50.0,
                ),
                (
                    "multiplication",
                    vec![t(Number, "10"), t(Multiply, "*"), t(Number, "5")],
                    50.0,
                ),
                (
                    "division",
                    vec![t(Number, "100"), t(Divide, "/"), t(Number, "4")],
                    25.0,
                ),
            ],
            &HashMap::new(),
        );
    }

    #[test]
    fn precedence() {
        check_table(
            &[
                (
                    "multiply before add",
                    vec![
                        t(Number, "100"),
                        t(Plus, "+"),
                        t(Number, "50"),
                        t(Multiply, "*"),
                        t(Number, "2"),
                    ],
                    200.0,
                ),
                (
                    "left to right subtraction",
                    vec![
                        t(Number, "10"),
                        t(Minus, "-"),
                        t(Number, "5"),
                        t(Minus, "-"),
                        t(Number, "2"),
                    ],
                    3.0,
                ),
            ],
            &HashMap::new(),
        );
    }

    #[test]
    fn parentheses() {
        check_table(
            &[(
                "parentheses override precedence",
                vec![
                    t(LParen, "("),
                    t(Number, "100"),
                    t(Plus, "+"),
                    t(Number, "50"),
                    t(RParen, ")"),
                    t(Multiply, "*"),
                    t(Number, "2"),
                ],
                300.0,
            )],
            &HashMap::new(),
        );
    }

    #[test]
    fn variables() {
        let vars = HashMap::from([("a".to_string(), 10.0), ("b".to_string(), 20.0)]);
        check_table(
            &[
                (
                    "variable addition",
                    vec![t(Variable, "a"), t(Plus, "+"), t(Variable, "b")],
                    30.0,
                ),
                (
                    "variable and number",
                    vec![t(Variable, "a"), t(Multiply, "*"), t(Number, "5")],
                    50.0,
                ),
            ],
            &vars,
        );
    }

    #[test]
    fn negative_numbers() {
        check_table(
            &[
                (
                    "negative number after multiplication",
                    vec![t(Number, "5"), t(Multiply, "*"), t(Number, "-2")],
                    -10.0,
                ),
                (
                    "negative number in parentheses",
                    vec![
                        t(LParen, "("),
                        t(Number, "-2"),
                        t(Plus, "+"),
                        t(Number, "3"),
                        t(RParen, ")"),
                    ],
                    1.0,
                ),
                (
                    "negative number after addition",
                    vec![t(Number, "10"), t(Plus, "+"), t(Number, "-5")],
                    5.0,
                ),
                (
                    "negative number with commas",
                    vec![t(Number, "-2,000")],
                    -2000.0,
                ),
            ],
            &HashMap::new(),
        );
    }

    #[test]
    fn errors() {
        let div0 = eval(&[t(Number, "100"), t(Divide, "/"), t(Number, "0")]);
        assert_eq!(div0, Err(EvalError::division_by_zero()));
        let undef = eval(&[t(Variable, "x")]);
        assert_eq!(undef, Err(EvalError::undefined_variable("x")));
        let empty = eval(&[]);
        assert_eq!(
            empty.map_err(|e| e.to_string()),
            Err("empty expression".into())
        );
    }

    #[test]
    fn malformed_expressions() {
        let cases = [
            ("(1 + 2", ErrorCode::MismatchedParens, "parens"),
            ("1 + 2)", ErrorCode::MismatchedParens, "parens"),
            (")(", ErrorCode::MismatchedParens, "parens"),
            ("1 +", ErrorCode::Other, "error"),
            ("* 2", ErrorCode::Other, "error"),
            ("1 2", ErrorCode::Other, "error"),
            ("()", ErrorCode::Other, "error"),
            ("x / 0", ErrorCode::UndefinedVariable, "undef: x"),
            ("5 / (3 - 3)", ErrorCode::DivisionByZero, "div/0"),
        ];
        for (input, code, short) in cases {
            let err = eval_str(input).expect_err(input);
            assert_eq!(err.code, code, "{input}");
            assert_eq!(err.short(), short, "{input}");
        }
        assert_eq!(
            eval_str("1 +").map_err(|e| e.to_string()),
            Err("invalid expression".into())
        );
    }

    #[test]
    fn sum_reads_the_accumulator() {
        let tokens = [t(Number, "500"), t(Minus, "-"), t(Variable, "sum")];
        assert_eq!(
            evaluate_expression(&tokens, &HashMap::new(), 312.0),
            Ok((188.0, true))
        );
        assert_eq!(
            evaluate_expression(&[t(Number, "1")], &HashMap::new(), 312.0),
            Ok((1.0, false))
        );
    }

    #[test]
    fn invalid_number_token() {
        let err = eval(&[t(Number, "1.2.3")]).expect_err("bad number");
        assert_eq!(err.to_string(), "invalid number: 1.2.3");
        assert_eq!(err.short(), "syntax");
    }
}
