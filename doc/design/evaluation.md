# Evaluation

Code: `src/eval/`. The evaluator takes the parsed lines from `src/lang/` and
gives one `LineResult` per line. It runs top to bottom with two pieces of
state: a map of variables and the **accumulator**, a running total.

## Expressions

`evaluate_expression` (`src/eval/expression.rs`) turns tokens into postfix
order with the shunting-yard algorithm, then evaluates the postfix list.

| Precedence | Operators | Associativity |
|---|---|---|
| High | `*` `/` | Left |
| Low | `+` `-` | Left |

Parentheses group. There are no functions, powers or percent signs.

- A Variable token reads the variable map. An unknown name is an error
  (`undefined variable: NAME`).
- The name `sum` inside an expression reads the current accumulator, and
  marks the expression as having used `sum`.
- An empty token list is an error (`empty expression`).
- Division by exactly zero is an error (`division by zero`).
- A `)` with no `(`, or a `(` left open, is an error (`mismatched
  parentheses`).
- An operator without two operands, or operands left over (`2 3`, `()`), is
  an error (`invalid expression`).

## Line evaluation and the accumulator

`Evaluator::evaluate_all` (`src/eval/evaluator.rs`) clears all state, then
evaluates the lines in order.

| Line type | Result | Accumulator |
|---|---|---|
| Empty | None | Set to 0 |
| Comment | None | Unchanged |
| Number | The number | Add the number |
| Expression | The value | If it used `sum`: set to 0. Else add the value |
| Assignment `name = expr` | The value; also stores `name` | Same as Expression |
| Assignment `name = sum` (right side is exactly `sum`) | The accumulator | Set to 0 |
| Sum | The accumulator | Set to 0 |
| Error | The parse error | Unchanged |

Rules and their reasons:

- **A blank line resets the accumulator.** Blank lines split the pad into
  sections, and `sum` totals only its own section.
- **Using `sum` resets the accumulator.** Once a group has been totalled, the
  next `sum` starts a new group. This holds for `sum`, `x = sum`, and any
  expression that reads `sum` (`500 - sum`).
- **Assignments add to the accumulator.** `rent = 1200` is a line item like
  `1200`, so it counts toward the next `sum`. Example: `rent = 1200`,
  `food = 400`, `250`, `total = sum` gives `total` = 1850.
- **A failed line changes nothing.** An error line neither adds to the
  accumulator nor sets a variable, so one typo does not corrupt the lines
  below it.
- **`sum` is reserved.** `sum = 5` is an error: `'sum' is a reserved keyword
  and cannot be used as a variable name`. The variable is not set.
- A variable keeps its value until it is assigned again. Blank lines do not
  clear variables.

`LineResult` holds:

| Field | Meaning |
|---|---|
| `value` | The number, if the line produced one |
| `error` | The `EvalError`, if the line failed |
| `used_sum` | True when the line read the accumulator (`sum`, `x = sum`, `500 - sum`) |
| `section` | Index of the accumulator section. Runs of blank lines split sections; a blank line belongs to the section above it; leading blank lines belong to section 0 |

## Section total (status bar)

`section_total` (`src/eval/section.rs`) gives the `total: N` shown in the
status bar, for the section under the cursor. Here a blank line is one whose
raw text is only whitespace.

- A section is a run of non-blank lines. One blank line directly after it
  also belongs to it, so the cursor on that blank still shows the total.
- If the cursor is on a blank line that has another blank line directly above
  or below it, the total is 0.
- From the cursor, walk up past at most one blank line. If no content is
  found, the total is 0.
- Extend to the first and last non-blank lines of that run. Include one
  trailing blank if it is the last line of the buffer or is followed by
  content.
- The total is the sum of the values in the section, skipping errors and
  skipping lines with `used_sum`. A section of comments only totals 0.

Lines that used `sum` are left out because their value already contains the
lines above them; counting them would count those lines twice. For `90`,
`25`, `85`, `bills = sum` the total is 200, not 400.

## Errors

`EvalError` (`src/eval/error.rs`) has a class, which decides the short code
shown in the results column, and a long message, shown by `nm eval` in its
`error` field.

| Class | Short code | Long message |
|---|---|---|
| Undefined variable | `undef: NAME` | `undefined variable: NAME` |
| Division by zero | `div/0` | `division by zero` |
| Mismatched parentheses | `parens` | `mismatched parentheses` |
| Syntax | `syntax` | `invalid number: X`, `invalid number format: X (...)`, `unexpected character: X` |
| Other | `error` | `empty expression`, `invalid expression`, `expected number after unary minus`, the reserved `sum` message |

An Error line carries the tokenizer's message as text. `EvalError::from_message`
classifies it: a message containing `undefined variable:`, `division by
zero`, `mismatched parentheses`, or `invalid number` / `unexpected` maps to
the class above; anything else is Other.

## Number formatting

`format_number` (`src/eval/format.rs`) gives the text for the results column,
the status-bar total, `yr`, and the `text` field of `nm eval`.

- The integer part gets a comma every three digits.
- The value is rounded to at most five decimals (`MAX_DECIMALS`), and
  trailing zeros are trimmed. An integer shows no decimal point. So `0.5`
  shows `0.5`, `1/3` shows `0.33333`, and `1200.50` shows `1,200.5`.
- The whole value is rounded, not just the fraction, so `1.999999` shows `2`
  and `999.999999` shows `1,000`.
- A negative value has `-` in front, except when the display rounds to zero:
  `-0`, `-0.000001` and `-0.0000049` all show `0`. There is never a `-0`.
- Non-finite values show `inf`, `-inf` and `NaN`. They can come from
  arithmetic that overflows `f64`.
- Large values print every digit of the `f64`, with no exponent:
  `12345678901234567890` shows `12,345,678,901,234,567,168`.

Five decimals is a display choice, not a precision limit. Values and
variables keep full `f64` precision; only the text is rounded.

| Value | Shown |
|---|---|
| `100` | `100` |
| `1000` | `1,000` |
| `0.5` | `0.5` |
| `1234.56` | `1,234.56` |
| `-1234567.89` | `-1,234,567.89` |
| `1/3` | `0.33333` |
| `0.000005` | `0.00001` |
| `0.0000049` | `0` |
| `1.999999` | `2` |
| `-0.000001` | `0` |
