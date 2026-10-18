# nm design

nm is a terminal calculator: a text pad with vim keys where every line is
evaluated as you type, and the result shows in a column on the right. Lines
can hold numbers, expressions, variables and comments, and the keyword `sum`
totals the lines above it.

This folder describes how nm is built and the rules it follows. Read this
file first, then the file for the area you are changing.

| File | Covers |
|---|---|
| [language.md](language.md) | Line types, comments, numbers, tokens, identifiers, tokenizer errors |
| [evaluation.md](evaluation.md) | Expressions, variables, the `sum` accumulator, section totals, error codes, number formatting |
| [storage.md](storage.md) | File format, session file, command line, `nm eval` JSON output |
| [editor.md](editor.md) | The TUI: state, layout, the vim engine and its quirks, registers, undo, quitting, completion, colours |
| [testing.md](testing.md) | Where tests live, the headless harness, spec cases, `just ci` |
| [../keys.md](../keys.md) | Every key and command, for users |

## Module map

One binary crate. Modules are split by concern.

| Module | Job |
|---|---|
| `src/lang/` | Comment stripping, line type detection, number rules, tokenizer. Pure functions over strings. |
| `src/eval/` | Expression evaluation, the line evaluator with variables and the `sum` accumulator, section totals, typed errors, number formatting. |
| `src/storage/` | Loading and saving `.nm` files, the session path, `~/` expansion, command-line parsing, what the editor starts with. |
| `src/tui/` | The terminal UI: app state, key handling, the vim engine wrapper, drawing, commands, completion, clipboard, help. |
| `src/eval_cmd.rs` | `nm eval`: read stdin, evaluate, print one JSON object per line. |
| `src/main.rs` | Parse arguments and start either the editor or `nm eval`. |

Dependencies point one way:

```
main.rs ──> storage, eval_cmd, tui
eval_cmd ──> lang, eval, storage
tui ──────> lang, eval, storage
eval ─────> lang
lang, storage: no internal dependencies
```

`lang` and `eval` know nothing about terminals or files. `storage` knows
nothing about the language. Only `tui` knows about the terminal.

## One keypress, end to end

1. `tui::run` reads a key event from crossterm and calls `App::handle_key`
   (`src/tui/app.rs`).
2. `handle_key` deals with the keys that work everywhere (`Ctrl+C`, `Ctrl+S`,
   closing the help modal, the open command line), then hands the key to the
   handler for the current mode in `src/tui/keys.rs`.
3. The mode handler either acts itself (`:`, `?`, `q`, `yr`, completion
   keys, clipboard paste) or passes the key to `Editor::send`
   (`src/tui/editor.rs`), which feeds the vim engine and then fixes the
   engine's known quirks.
4. Back in `App`, `after_key` checks the editor's change counter. If the text
   may have changed, it re-reads the lines. If they did change, it marks the
   buffer modified, re-parses every line (`lang::parse_all`) and re-evaluates
   every line (`Evaluator::evaluate_all`). Then it scrolls the viewport so
   the cursor stays visible, and recomputes the completion.
5. The next pass of the event loop calls `view::draw`, which paints every
   cell from the app state, places the terminal's own cursor, and sets its
   shape.

The whole buffer is re-parsed and re-evaluated on every text change. Each
line depends on the lines above it (variables, the accumulator), and a pad is
small, so there is no incremental evaluation.

## Rules that always hold

- No `unsafe` code.
- No panics on user input. Every parse and evaluation path returns a typed
  error (`LangError`, `EvalError`), and the UI shows it as a result.
- Cursor columns are character indices, never byte offsets. Screen positions
  come from display width, so wide characters place the cursor correctly.
- The editor crate never draws. `src/tui/view.rs` paints every cell.
- Only `src/tui/editor.rs` names types from the editor crate. Upgrading or
  swapping the crate touches that one file.
