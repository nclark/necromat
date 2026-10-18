# Language

Code: `src/lang/`. Everything here is a pure function of one line of text.
Each line is parsed on its own; state between lines (variables, the
accumulator) belongs to the evaluator, see [evaluation.md](evaluation.md).

## Comments

`#` starts a comment that runs to the end of the line. The line splits at the
first `#`:

- the **content** is the text before `#`, trimmed;
- the **comment** is the text after `#`, trimmed, without the `#`.

There is no escape for `#`.

## Line types

The type is found from the content, in this order. The first match wins.

| # | Type | Rule | Examples |
|---|---|---|---|
| 1 | Empty | Content is empty and the comment is empty | ``, `   `, `#` |
| 2 | Comment | Content is empty and the comment is not | `# rent` |
| 3 | Sum | Content is `sum`, any case | `sum`, `SUM`, `Sum` |
| 4 | Assignment | Content holds `=`, and the text before the first `=` (trimmed) is an identifier | `rent = 1200`, `x=sum` |
| 5 | Number | Content is a valid number literal | `100`, `-50`, `5,000`, `1,234.56` |
| 6 | Expression | Content tokenizes without error | `100 + 50`, `x * 2`, `a` |
| 7 | Error | Anything else; carries the tokenizer's message | `100 $ 50`, `12,34` |

Notes:

- A line that is only `#` is Empty, not Comment. It resets the accumulator
  like a blank line.
- For an assignment, the right side is tokenized as an expression. If it
  fails to tokenize, the line is an Error with that message.
- `123=100` and `a+b=100` are not assignments: the left side is not an
  identifier. They fall through to the later rules and end up as Error
  (`unexpected character: =`).
- A lone identifier such as `a` is an Expression. It evaluates to the
  variable's value.
- A parsed line (`ParsedLine`) keeps its type with the data the type needs
  (the number, the name and tokens, the tokens, or the error text), the raw
  line, and the comment.

## Numbers

A number literal matches:

```
^-?([0-9]{1,3}(,[0-9]{3})+|[0-9]+)(\.[0-9]+)?$
```

That is: an optional minus, an integer part, and an optional fraction of one
or more digits. Commas are optional thousands separators. If the integer part
has a comma, it must be a lead group of one to three digits followed by groups
of exactly three.

| Valid | Invalid |
|---|---|
| `5,000` | `1,23,4` |
| `1,234,567.89` | `12,34` |
| `-1,234.56` | `1234,567` |
| `1234567` | `,123` |
| `0.5` | `123,` |
| `100` | `1,,234` |
| | `1,2345` |

- Values are `f64`. Commas are removed before parsing.
- A well-formed literal that does not fit a finite `f64` is an error
  (`invalid number: X`).
- A line that is a bad number is an Error. Inside an expression the
  tokenizer reports `invalid number format`.

## Tokens

| Token | Text |
|---|---|
| Number | A number literal, possibly with a folded unary minus (`-50`) |
| Variable | An identifier |
| Plus, Minus, Multiply, Divide | `+ - * /` |
| LParen, RParen | `( )` |

Each token keeps its exact source text, so a number keeps its commas.

Tokenizer rules:

- Whitespace (any Unicode whitespace) is skipped.
- A digit or `.` starts a number. Digits, `.` and `,` continue it. The whole
  run is then checked against the number rule.
- A letter or `_` starts an identifier. Letters, digits and `_` continue it.
- Any other character is an error.
- Empty or whitespace-only input gives zero tokens and no error.

## Identifiers

An identifier matches `[A-Za-z_][A-Za-z0-9_]*`. Only ASCII letters count.
A non-ASCII letter such as `é` is an unexpected character. Names are case
sensitive, except that the keyword `sum` on a line of its own is matched in
any case.

## Unary minus

A `-` is **unary** when it is the first token, or follows `+ - * / (`.
Otherwise it is binary Minus (`5 - 2`).

A unary minus must be followed, after optional spaces, by a number. The two
fold into one negative Number token:

| Input | Tokens |
|---|---|
| `-50` | `-50` |
| `-3.14` | `-3.14` |
| `-5,000` | `-5,000` |
| `5 * -2` | `5` `*` `-2` |
| `(-2 + 3)` | `(` `-2` `+` `3` `)` |
| `- 5` | `-5` |

A unary minus before anything other than a number is an error: `-x`, `-(2)`
and `--5` are all rejected. Write `0 - x` or `-1 * x` instead.

## Tokenizer errors

| Error | Message |
|---|---|
| Unexpected character | `unexpected character: X` (names the whole character, not a byte) |
| Bad comma grouping | `invalid number format: X (commas must be proper thousand separators)` |
| Unary minus without a number | `expected number after unary minus` |
| Number too large for `f64` | `invalid number: X` |

These messages are the long error text of an Error line. The evaluator maps
them to short codes for the results column, see
[evaluation.md](evaluation.md#errors).
