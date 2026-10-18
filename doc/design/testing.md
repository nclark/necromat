# Testing

Every behaviour rule in these design files has a test. A change is done when
`just ci` passes.

## `just ci`

```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

All three must pass. Clippy warnings are errors.

## Where tests live

| Area | Tests |
|---|---|
| `src/lang/*.rs` | Inline `#[cfg(test)] mod tests` in each file: number rules, tokenizer, line detection, comments |
| `src/eval/*.rs` | Inline tests: expressions, errors and short codes, the evaluator and accumulator, section totals, formatting |
| `src/storage/*.rs` | Inline tests: load/save round trips, CRLF, session path, `~/` expansion, arguments, start-up (with `tempfile` directories) |
| `src/eval_cmd.rs` | Inline tests of the JSON lines output |
| `src/tui/clipboard.rs` | Inline tests of channel choice and OSC 52, with a fake terminal |
| `src/tui/*_tests.rs`, `src/tui/tests.rs` | TUI tests through the harness (below) |
| `tests/eval_cli.rs` | The built `nm` binary end to end: `nm eval` and the start-up error exit |
| `tests/spec_cases.rs` | The spec cases (below) |

Table-driven tests are the norm: a `const` slice of tuples, one row per case,
with a name in the first field so a failure says which row broke.

## The headless harness

`src/tui/harness.rs` drives the real `App` with no terminal. It is the way to
test anything in the TUI.

```rust
let mut h = Harness::new(&["100", "200"]);
h.keys("jA5<Esc>");
assert_eq!(h.lines(), ["100", "2005"]);
assert_eq!(h.cursor(), (1, 3));
let screen = h.render();
assert!(screen.rows[1].starts_with(" 2 │ 2005"));
assert!(screen.status().contains("total: 2,105"));
```

Building one:

| Constructor | Starts with |
|---|---|
| `Harness::new(lines)` | An 80x24 app in session mode, normal mode, cursor at (0, 0) |
| `Harness::sized(lines, w, h)` | The same at another terminal size |
| `Harness::at(lines, row, col)` | Normal mode with the cursor at (row, col); the column stops on the last character |
| `Harness::inserting(lines, row, col)` | Insert mode at (row, col); `col` may be the line length |

The session path is `None`, so tests never write a session file. To test a
named file, set `h.app.file_path` and `h.app.session_mode = false`, with a
`tempfile` directory for the path.

Driving it:

- `keys("...")` sends keys in vim notation. Plain characters are typed one by
  one. Named keys: `<Esc> <CR> <BS> <Del> <Tab> <Left> <Right> <Up> <Down>
  <Home> <End> <Space>`, `<C-x>` for Ctrl+x, `<lt>` for a literal `<`.
  Names are case-insensitive. An unknown name panics, which fails the test.
- `type_text("...")` types every character literally, `<` included.

Checking it:

- `lines()`, `cursor()` (row and character column), `mode()`.
- `h.app` exposes the rest: `status_msg`, `modified`, `file_path`,
  `completion`, `results`, `command`, `help`.
- `render()` draws through a ratatui `Terminal` on a `TestBackend` and
  returns a `Screen`: `rows` (each row as plain text), `status()` (the last
  row), `cursor` (the native cursor placement, `None` when hidden) and
  `buffer` (the cells, for style checks). `render()` also asserts that the
  backend's cursor matches the placement nm asked for.

The harness editor yanks to a `MemoryClipboard`, never the real one. Read it
with `h.app.editor.clipboard().get()`, preload it with `.set(...)`, or swap
in another `Clipboard` (for example one that always fails) with
`h.app.editor.set_clipboard(...)`.

## TUI test files

| File | Covers |
|---|---|
| `src/tui/tests.rs` | App basics, insert and normal basics, layout, gutter, results column, status bar, viewport scrolling, cursor placement and shape, tiny terminals, the phantom-row fixes, evaluation after edits, Unicode and wide characters, key notation |
| `src/tui/vim_tests.rs` | The vim key surface, one table per group: motions and counts, word motions, finds, operators, line edits, other engine keys, dot repeat, visual mode, the register and clipboard rule, yank messages, paste, undo and redo, Esc. The last table replays 96 key probes checked against `nvim --clean`, with nm's deliberate differences overridden |
| `src/tui/command_tests.rs` | Command mode, write, edit, quit, the quit guard, `ZZ`/`ZQ`, `Ctrl+S`, the modified flag, the session save on exit, the help modal (fits 80x24, closes on any key, every key it names is in `doc/keys.md`) |
| `src/tui/completion_tests.rs` | Completion candidates, prefix rules, matching, cycling, accepting, when it is recomputed, ghost text rendering and clipping, cursor on the ghost |
| `src/tui/highlight_tests.rs` | Highlighting per line type, the rule that spans join back to the raw line (over a wide set of inputs), and styles as rendered |

## Spec cases

`tests/spec/cases/` holds the evaluation spec as data. Each case is a pair:

- `<name>.nm`: a pad, exactly as a user would write it;
- `<name>.jsonl`: the golden `nm eval` output for it.

`tests/spec_cases.rs` runs every `.nm` file through the evaluator and
compares the result with its `.jsonl` file.

To add or change a case, edit or add the `.nm` file, then run
`just spec-update` to regenerate the `.jsonl` files. Review the change with
`git diff`: every changed line of output must be one you meant to change.
Commit the `.nm` and `.jsonl` together.

Use a spec case for any rule about parsing, evaluation, error codes or
number formatting. Use a unit test in the module when the rule is about one
function's edge cases.

## Adding a test for a new key or command

1. Find the table it belongs to in `src/tui/vim_tests.rs` (engine keys) or
   `src/tui/command_tests.rs` (commands, quitting, saving). For a new area,
   add a section with its own table.
2. Add a row: a name, the starting lines, the start cursor, the keys in vim
   notation, and the expected lines, cursor and mode. Checks the table does
   not cover (status message, clipboard, rendered screen) belong in a separate
   `#[test]` using the harness directly.
3. Run it and watch it fail for the right reason, then implement.
4. If the key is user-facing, add it to `doc/keys.md`. If it is in the help
   modal, the help test fails until `doc/keys.md` names it.
5. If the engine gets the key wrong, fix it in `editor.rs` or `keys.rs` and
   add the quirk to the table in [editor.md](editor.md#engine-quirks).
