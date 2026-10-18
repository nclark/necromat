# Storage and the command line

Code: `src/storage/` (files, paths, arguments, start-up), `src/eval_cmd.rs`
(`nm eval`), `src/main.rs` (exit codes).

## File format

A `.nm` file is plain UTF-8 text, one pad line per file line, LF line
endings.

Loading (`storage::load`, `storage::split_lines`):

- CRLF becomes LF. A lone `\r` is kept as text.
- The text is split on LF. One trailing empty piece (from the final newline)
  is dropped.
- An empty file loads as one empty line.
- Bytes that are not valid UTF-8 become U+FFFD instead of failing the load.

Saving (`storage::save`):

- Every line is written followed by `\n`, so the file always ends in a
  newline. An empty list writes a single `\n`.
- Parent directories are created as needed.

Because every line gets its own `\n`, a trailing blank line survives a save
and a load: `["x", ""]` saves as `x\n\n` and loads back as `["x", ""]`.
Trailing blank lines matter, since a blank line resets the accumulator.

## Session file

With no file argument, nm works in **session mode**: the text has no file
name, and it is saved to the session file on exit.

The session file is `$XDG_DATA_HOME/nm/last-session.nm` when `XDG_DATA_HOME`
is set to an absolute path. Otherwise it is
`~/.local/share/nm/last-session.nm`. An empty or relative `XDG_DATA_HOME` is
ignored, as the XDG spec says. With neither available, there is no session
file and nothing is saved.

- On start in session mode, the session file is loaded if it exists and
  reads cleanly. Otherwise the pad starts with one empty line.
- On exit in session mode, the text is saved to the session file unless every
  line is blank. A named file is never saved on exit.
- Leaving session mode with `:e file` saves the session first, so its text
  is not lost. `:w file` also leaves session mode.

## `~/` expansion

A leading `~/` in a path from the command line or a `:w` / `:e` command is
replaced by the home directory. A bare `~` and `~user/` are not expanded.
If the home directory is unknown, the path is used as typed.

## Command line

| Form | Does |
|---|---|
| `nm` | Open the editor in session mode |
| `nm <file>` | Open the editor on `file` |
| `nm eval` | Evaluate stdin and print JSON lines (below) |
| `nm -h`, `nm --help` | Print usage, exit 0 |

- `eval` as the first argument is the subcommand. To open a file named
  `eval`, write `nm ./eval` or `nm -- eval`.
- `--` ends option parsing, so `nm -- -file` opens a file named `-file`. A
  lone `-` is a file name.
- Any other argument starting with `-` is an unknown option. More than one
  file argument is an error. Both print `nm: <reason>` and the usage text to
  stderr and exit 2.

`nm <file>` start-up:

- If the file exists, it is loaded. A read error prints `Error loading
  <path>: <reason>` and exits 1.
- If the file does not exist but its directory does, the editor starts with
  one empty line and that path. The file is created on the first save.
- If the directory does not exist either, nm prints `Error: directory does
  not exist: <dir>` and exits 1.

| Exit code | When |
|---|---|
| 0 | Normal exit, `--help`, `nm eval` done (also when stdout closes early, as in `nm eval \| head -1`) |
| 1 | Start-up error, terminal error, a failed session save, or an I/O error in `nm eval` |
| 2 | Bad arguments |

A terminal error or a failed session save prints `nm: <reason>` after the
terminal has been restored.

## `nm eval` output

`nm eval` reads all of stdin, splits it into lines the same way as loading a
file (CRLF or LF, final newline optional, invalid UTF-8 replaced), evaluates
every line, and prints one JSON object per input line. Empty input prints
nothing.

This output is a stable interface for scripts, editor plugins and the spec
tests. Treat any change to its shape as a breaking change.

```
$ printf 'income = 5,000\nrent = 1,200\n\nx + 1\n' | nm eval
{"line":1,"value":5000.0,"text":"5,000","error":null}
{"line":2,"value":1200.0,"text":"1,200","error":null}
{"line":3,"value":null,"text":"","error":null}
{"line":4,"value":null,"text":"undef: x","error":"undefined variable: x"}
```

Keys, always in this order:

| Key | Type | Meaning |
|---|---|---|
| `line` | integer | 1-based line number |
| `value` | number or `null` | The result. `null` for errors, for lines with no result (empty, comment), and for non-finite results |
| `text` | string | Exactly what the results column shows: the formatted number, the short error code, or `""` |
| `error` | string or `null` | The long error message, or `null` |

- JSON has no infinity or NaN, so a non-finite result has `"value":null`
  and `text` holds `inf`, `-inf` or `NaN`, with `"error":null`.
- A line with no result gives `"value":null,"text":"","error":null`.
- `value` is printed by serde_json, so whole numbers keep a `.0`.
