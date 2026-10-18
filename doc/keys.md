# Keys

`N` is an optional count typed before the key (`3j`, `2dd`). `{motion}` is
any motion from the Normal mode motion table. `{c}` is any character.

## Any mode

| Key | Action |
|---|---|
| `Ctrl+C` | Quit at once (no quit guard; session mode still saves the session) |
| `Ctrl+S` | Save to the current file |

## Normal mode: motions

| Key | Action |
|---|---|
| `h` `l` `←` `→` | Left, right (stays on the line) |
| `j` `k` `↓` `↑` | Down, up (keeps the column) |
| `0` | Start of line |
| `^` | First non-blank character |
| `$` `End` | End of line (`j`/`k` then stay at line ends) |
| `w` `b` `e` | Next word start, previous word start, word end |
| `W` `B` `E` | The same for blank-separated WORDs |
| `gg` | First line (`Ngg`: line N) |
| `G` | Last line (`NG`: line N) |
| `f{c}` `F{c}` | To the next / previous `{c}` on the line |
| `t{c}` `T{c}` | Up to the next / previous `{c}` on the line |
| `;` `,` | Repeat the last `f F t T`, same / opposite direction |
| `%` | Matching bracket |

## Normal mode: editing

| Key | Action |
|---|---|
| `i` `a` | Insert before / after the cursor |
| `I` `A` | Insert at the start / end of the line |
| `o` `O` | Open a line below / above |
| `x` `X` | Delete the character under / before the cursor |
| `s` | Delete the character and insert |
| `S` `cc` | Clear the line and insert |
| `D` `C` | Delete / change to the end of the line |
| `dd` `Ndd` | Delete N lines |
| `d{motion}` | Delete over a motion (`dw`, `d$`, `dj`, `dG`, `dt*`) |
| `c{motion}` | Change over a motion (`cw`, `ct=`) |
| `d` `c` `y` with `iw aw iW aW i( a(` | Act on a word, WORD or parentheses |
| `J` `gJ` | Join with the next line, with / without a space |
| `r{c}` | Replace the character (`Nr{c}`: N characters) |
| `~` | Toggle case and move right |
| `Ctrl+A` `Ctrl+X` | Add / subtract N to the number under or after the cursor |
| `p` `P` | Paste after / before the cursor |
| `.` | Repeat the last change (`N.`: with a new count) |
| `u` | Undo |
| `Ctrl+R` | Redo |
| `Esc` | Cancel a count or a half-typed command |
| `:` | Command mode (see Commands) |
| `?` | Show the key help; any key closes it |
| `q` | Quit (refused with E37 when a named file has unsaved changes) |
| `ZZ` | Save if changed, then quit (same as `:x`) |
| `ZQ` | Quit without saving (same as `:q!`) |

## Normal mode: yank

| Key | Action |
|---|---|
| `yy` `Nyy` | Yank N lines |
| `y{motion}` | Yank over a motion (`yw`, `y$`, `yj`, `yG`, `ygg`) |
| `Y` | Yank to the end of the line |
| `yr` | Yank the line's result, as shown in the results column |

## Insert mode

| Key | Action |
|---|---|
| Text | Inserted as typed |
| `Esc` | Back to normal mode; the cursor steps left |
| `←` `→` | Move; wrap to the previous / next line at the line ends |
| `↑` `↓` | Move; keep the column, clamped to the line |
| `Home` `End` | Start / end of the line |
| `Enter` | Split the line |
| `Backspace` | Delete before the cursor; joins with the line above at column 0 |
| `Delete` | Delete under the cursor; joins the line below at the end |
| `Tab` | Accept the completion shown as dim text at the cursor (never inserts a tab) |
| `Ctrl+N` `Ctrl+P` | Show the next / previous completion match (wraps) |

Completion offers the variables assigned on lines above the cursor,
nearest first, then `sum`. It shows when the cursor is at the end of a
partly typed name, right of any `=` and before any `#`. The rest of the
selected name shows as dim text after the cursor.

## Visual mode

| Key | Action |
|---|---|
| `v` | Start or leave charwise selection (includes the cursor character) |
| `V` | Start or leave linewise selection |
| `h j k l 0 $ G` and the other motions | Extend the selection |
| `y` | Yank the selection and leave |
| `d` `x` | Delete the selection and leave |
| `c` | Delete the selection and insert |
| `p` | Replace the selection with the register |
| `Esc` | Leave without changes |

## Commands

Typed after `:` on the status row. `Enter` runs the command; `Esc`, or
`Backspace` on an empty command, cancels. `~/` in a path is the home
directory. A named file with unsaved changes makes `:q` and `:e` refuse
with `E37: No write since last change (add ! to override)`. Without a file
(session mode) quitting is never refused, and the text is saved to the
session file on exit unless every line is blank.

| Command | Action |
|---|---|
| `:w` `:write` | Save to the current file (`E32: No file name` without one) |
| `:w [file]` | Save to `file`, which becomes the current file |
| `:e file` `:edit file` | Open `file`, replacing the text (undo history is cleared) |
| `:e! file` | Open `file`, dropping unsaved changes |
| `:e!` | Reload the current file, dropping unsaved changes |
| `:q` `:quit` | Quit |
| `:q!` `:quit!` | Quit, dropping unsaved changes |
| `:wq [file]` | Save, then quit |
| `:x [file]` | Save if changed, then quit |

## Registers and the clipboard

| Command | Register | System clipboard |
|---|---|---|
| `y`, `yy`, `Y`, `yr`, visual `y` | Filled | Filled |
| `d`, `c`, `x`, `X`, `s`, `S`, `D`, `C`, visual `d` | Filled | Untouched |
| `p`, `P` | Read | Read only when the register is empty; text with a newline pastes as lines. With both empty, refused with `E353: Nothing in register "` |

## Undo

| Rule |
|---|
| One insert session (from `i a A I o O s S c` to `Esc`) is one step |
| Each normal-mode or visual-mode edit is one step |
| At most 200 steps are kept |
| A new edit clears redo |
| Loading a file clears the history |
