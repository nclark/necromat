# Editor

Code: `src/tui/`. For the full key list, see [../keys.md](../keys.md). This
file covers how the TUI is built and the rules behind the keys.

## Files

| File | Job |
|---|---|
| `mod.rs` | `tui::run`: enter the alternate screen, run the event loop, restore the terminal, save the session. Sets the cursor shape between frames. |
| `app.rs` | `App`, the whole UI state: the editor, a cached copy of the lines, parsed lines, results, viewport, file path, session mode, modified flag, command line, help flag, status message, completion. `handle_key` is the single entry for keys. |
| `editor.rs` | `Editor`, a wrapper around the vim engine. The only file that names types from the editor crate. Fixes the engine's quirks (below). |
| `keys.rs` | Per-mode key handling: what nm does itself and what goes to the engine. The register and clipboard rule, `yr`, yank messages, backward-find fix, insert-mode arrow wrap. |
| `commands.rs` | Command mode (`:`), write, edit, quit, the quit guard, the session save. |
| `view.rs` | Drawing. Paints every cell from `App` and places the terminal's own cursor. |
| `highlight.rs` | Syntax highlighting of one line into styled spans. |
| `completion.rs` | Variable completion: candidates, prefix, cycling, accepting. |
| `clipboard.rs` | The `Clipboard` trait, `SystemClipboard` (real) and `MemoryClipboard` (tests). |
| `help.rs` | The help modal's text and drawing. |
| `style.rs` | The Tokyo Night constants and the style functions every renderer calls. The functions read the active theme. |
| `theme.rs` | `Theme`, one colour per slot; the active theme; loading it from the nm theme file and Omarchy. |
| `harness.rs` | Test only: drive `App` with keys and render it headless. See [testing.md](testing.md). |

## The update loop

`tui::run` loops: draw, read one event, `App::handle_key`. There is no
timer and no background work; the screen changes only after a key.

`App::handle_key`:

1. Ignores key releases (some terminals report them).
2. Clears the one-shot status message.
3. `Ctrl+C` quits at once, from any mode.
4. If the help modal is open, any other key closes it and does nothing else.
5. `Ctrl+S` saves from any mode; an open command line is dropped so the
   result message shows.
6. If the command line is open, the key edits or runs it.
7. Otherwise the key goes to the handler for the mode: `insert_key`,
   `normal_key` or `visual_key` in `keys.rs`.
8. `after_key` compares the editor's change counter (`Editor::generation`)
   with the last one seen. Only when it moved does the app re-read the
   lines. If they differ from the cache, the buffer is marked modified, and
   every line is re-parsed and re-evaluated.
9. The viewport scrolls so the cursor row is on screen.
10. The completion is recomputed, except after the completion keys
    themselves.

Motions do not change the counter, so they cost no re-read and never mark
the buffer modified.

## Layout

`view::render` paints into a ratatui buffer. Top to bottom: content rows
from the viewport start, blank rows if the text is short, and one status bar
row at the bottom.

Each content row:

- **Gutter.** The line number right-aligned in `max(2, digits of the line
  count)` columns, then a space, `│`, a space. The current line number is
  bold gold; the others are dim gray; the separator is dim.
- **Input column.** The left 60% of the width, gutter included. The line is
  syntax-highlighted and clipped by display width.
- **Results column.** The right 40%. The formatted value, or the short error
  code in red, or nothing. Right-aligned with one trailing space, clipped to
  fit.

Highlighting styles characters in place. The spans, joined, are exactly the
raw line: nothing is trimmed, added or dropped. The cursor is placed from the
raw line, so a column in the raw text must be the same column on screen.
Lines that do not parse are drawn unstyled. A comment tail is always comment
style. `sum` on its own line is keyword red; inside an expression it is
styled as a variable.

**Status bar.** A mode badge (` NORMAL `, ` INSERT `, ` VISUAL `,
` VISUAL LINE `, ` VISUAL BLOCK `), then the file indicator, then
` │ total: N │ L lines`. The file indicator is ` <path>` for a named file,
` [session]` in session mode when any line has text, and ` [+]` after either
when the buffer is modified. `total` is the section total for the cursor
line ([evaluation.md](evaluation.md#section-total-status-bar)).

- While the command line is open, the bar shows `:` and the typed text.
- A status message (`"file" written`, `E32: No file name`, `Yanked 3
  lines`) replaces the whole bar until the next key.

**Cursor.** nm uses the terminal's own cursor and never draws a cursor glyph
into a cell, so terminal text selection and copying see only the text.

| State | Cursor |
|---|---|
| Insert mode | Blinking bar at the insertion point |
| Normal and visual | Steady block on the character |
| Command line | Bar after the typed text on the status row |
| Help modal open | Hidden |
| Cursor row scrolled off | Hidden |

The x position is the gutter width plus the display width of the text left
of the cursor, so wide characters count as two cells. It is clamped to the
last column. The shape escape is written only when the shape changes, and the
user's default shape is restored on exit.

Other rules: the alternate screen is used, and mouse capture is never turned
on, so the terminal's own selection keeps working. A terminal of any size,
0x0 included, must draw without a panic.

**Visual selection.** The selected cells get the visual style (bg and fg
both set) over the syntax colours; the characters never change. Charwise
(`v`) covers the anchor to the cursor, the cursor character included, by
the display width of each character. Linewise (`V`) covers the whole input
column of each line, the space after the text included. Block (`Ctrl+V`)
covers the same character columns on each row, or each row to its end after
`$`. A charwise or block selection stops where the clipped text stops; an
empty line in a charwise selection shows no selected cell.

## The editor crate

The vim engine is **hjkl-engine + hjkl-buffer + hjkl-vim**, pinned to
`=0.42.2` in `Cargo.toml`. It was chosen because:

- it runs headless, with no terminal, so every key can be tested in-process;
- it draws nothing of its own, so nm paints every cell and keeps full control
  of the layout;
- it indexes the cursor by character, not byte;
- it matched nvim on 94 of 96 key probes run against `nvim --clean`;
- 2000 random runs of 40 keys each gave zero panics;
- the clipboard goes through a `Host` trait that nm implements.

The pin is exact because nm works around engine behaviour (below) that could
change in any release. Upgrading means re-running the full test suite,
including the probe table in `src/tui/vim_tests.rs`.

### Engine options

hjkl defaults to an IDE-like setup. nm wants plain vim, so `editor.rs` sets:

| Option | Value | Why |
|---|---|---|
| `motion_sneak` | `false` | `s` and `S` substitute, as in vim, instead of vim-sneak |
| `autoindent`, `smartindent` | `false` | Enter starts the new line at column 0 |
| `formatoptions` | empty | No comment-leader continuation |
| `textwidth` | 0 | No hard wrap while typing |
| `modeline` | `false` | File content never changes settings |
| `undo_levels` | 200 | Undo cap |
| `autopair`, `autoclose_tag` | `false` | Typing `(` inserts only `(`. These are not in `Options`; they are set through `settings_mut()` after the editor is built |

### Engine quirks

Every item here is engine behaviour that differs from vim, and what nm does
about it. Most fixes are in `editor.rs`; the rest are in `keys.rs`.

| Engine behaviour | What nm does |
|---|---|
| The clipboard hook receives deletes as well as yanks | The host only records that the register was filled. `keys.rs` decides from the key whether it was a yank, and only a yank reaches the system clipboard |
| A buffer ending in a newline has an unreachable last row ("phantom row") | nm stores its lines with one extra trailing newline and leaves that row out when reading lines back. Without it, a real empty last line (a section divider) could not be reached with `j` or `G` |
| Deleting every line leaves an empty rope with no trailing newline | nm puts the newline back straight into the buffer, so it is not an undo step and the cursor does not move |
| Redo can leave the cursor on the phantom row | nm pulls the cursor back to the last real line |
| Insert-mode `Delete` at the end of the last line joins the phantom row | nm drops the key |
| Insert-mode `Left` at column 0 and `Right` at the end of a line do not wrap | `keys.rs` sends `Up`+`End` or `Down`+`Home` instead, so the move still goes through the engine's insert motions |
| Insert-mode `End`, `Up` and `Down` use normal-mode column rules (stop on the last character; after `A`, `Up`/`Down` can land on column 0) | nm lets the engine move, then sets the column: `End` goes to the end of the line, `Up`/`Down` keep the column clamped to the line length |
| `/`, `?`, `q` and `:` start search, macro and command prompts inside the engine | nm intercepts them in normal mode (`?` is help, `q` quits, `:` opens nm's command line, `/` does nothing) and drops `/ ? q` in visual mode |
| `F` and `T` after an operator are inclusive, so `dF(` also deletes the cursor character | `keys.rs` lets the engine run, reads how much it took from the register, then undoes and redoes the operator over the right range with a visual selection. For `y` it only fixes the register and clipboard |
| `T` with its target right next to the cursor counts as not found | Accepted; the operator does nothing |
| The cursor can end one past the end of the line after `J` with an empty line, after `.` repeating `A`, and after a visual delete that ends the line | In normal mode nm clamps the cursor onto the last character |
| An undo step is recorded on entering insert mode and for edits that change nothing | nm pops steps whose text equals the text before them |
| Arrow keys in insert mode split the session into several undo steps | nm merges them: one insert session is one step |
| `v` in charwise visual and `V` in linewise visual do not leave visual mode | nm sends `Esc` |
| Visual `p` with nothing to paste deletes the selection | nm refuses with `E353` and keeps the selection |
| After `$`, `j`/`k` keep the landed column instead of sticking to line ends | nm sets the sticky column to "end of line" after `$` and `End` |
| A count typed before a key nm handles itself would apply to the next motion | nm sends `Esc` first to drop it |
| `Ctrl+D`, `Ctrl+U`, `H`, `M`, `L` use the engine's own screen model, which nm does not keep in sync with its viewport | Not supported and not documented |
| `undo_stack_len` is not documented | nm reads it through `hjkl_buffer::View` |
| Keys it has no name for (function keys, media keys) | Dropped before they reach the engine |

Two engine behaviours nm keeps as they are: `Y` yanks to the end of the
line (as in nvim, not vim's `yy`), and `R` (replace) reports itself as
insert mode, so the badge shows INSERT.

## Registers and the clipboard

| Key | Unnamed register | System clipboard |
|---|---|---|
| `y` with any motion, `yy`, `Y`, visual `y` | Filled | Filled |
| `yr` | Filled with the line's formatted result | Filled |
| `d c x X s S D C`, visual `d x c` | Filled | Untouched |
| `p` `P` | Read | Read only when the register is empty |

The reason for the split: deleting text to retype it is common, and it
should not wipe what the user copied from another program.

- Linewise text goes to the clipboard without its final newline.
- When the register is empty, `p` fills it from the clipboard. Clipboard
  text with a newline in it pastes as whole lines.
- With both empty, `p` refuses with `E353: Nothing in register "`.
- A yank reports `Yanked 1 line`, `Yanked 3 lines` or `Yanked 5 characters`
  (characters, not bytes; visual `V` counts lines). `yr` reports `Yanked
  result`, or `No result to yank` on a line without a value.
- A failed clipboard write shows `Clipboard error: <reason>`.

### System clipboard channels

`SystemClipboard` writes through up to three channels:

1. **arboard**, the native clipboard. It is tried only when `WAYLAND_DISPLAY`
   or `DISPLAY` is set (always on macOS and Windows); without a display it can
   only fail or stall.
2. **A clipboard tool**, as a last resort when arboard is missing or fails:
   `wl-copy` (Wayland), then `xclip`, then `xsel`; `pbcopy` on macOS. Each
   run is killed after 500 ms, because yanks run on the UI thread.
3. **OSC 52**, an escape sequence that asks the terminal to set the
   clipboard on its host. It is written on every yank, alongside the native
   channel, because nm cannot tell whether it runs over SSH, and it is the
   only channel that reaches the user's own machine from a remote host. It
   is written to stdout between frames, never while drawing. It is skipped
   when `TERM` is unset or `dumb`.

OSC 52 is sent plain, with no tmux passthrough wrapping: tmux forwards the
plain form when `set-clipboard` is on, and the wrapped form needs
`allow-passthrough`, which is also off by default.

A write succeeds when any channel took the text. Reading uses arboard, then
the tool's paste command (`wl-paste`, `xclip -out`, `xsel --output`,
`pbpaste`). There is no OSC 52 read, since few terminals answer the query.

## Undo

- One insert session, from `i a A I o O s S c` to `Esc`, is one step, even
  when arrow keys moved the cursor inside it.
- Each normal-mode or visual-mode edit is one step.
- An edit or insert session that changes nothing adds no step.
- At most 200 steps are kept.
- A new edit clears redo.
- Loading a file (`:e`) clears the history.
- Undo puts the cursor where it was before the change, or at the start of the
  changed region.
- `u` with nothing to undo shows `Already at oldest change`; `Ctrl+R` with
  nothing to redo shows `Already at newest change`.

## Command mode

`:` opens a command line on the status row. Enter runs it; `Esc`, or
`Backspace` on an empty line, closes it. Only plain characters are typed
into it.

The command name is the leading letters, then an optional `!`, then the
argument: the rest of the line, trimmed. Paths may hold spaces, and `e!x.nm`
works as well as `e! x.nm`. `~/` in a path is expanded.

| Command | Does |
|---|---|
| `w`, `write [path]` | Write to `path` or the current file. `path` becomes the current file and session mode ends |
| `e`, `edit path` | Load `path`, replacing the buffer |
| `e! [path]` | Load `path`, dropping changes; with no path, reload the current file |
| `q`, `quit` | Quit |
| `q!`, `quit!` | Quit, dropping changes |
| `wq [path]` | Write, then quit |
| `x [path]` | Write if changed, then quit |

Messages: `"path" written`, `"path" N lines`, `E32: No file name`, `E471:
Argument required` (`:e` with no path), `Unknown command: NAME` (the first
word typed), `Error: <path>: <reason>` for I/O errors. An empty command does
nothing.

## Quitting and the session

The **quit guard**: with a named file that has unsaved changes, `q`, `:q` and
`:e` refuse with `E37: No write since last change (add ! to override)`.

| Key or command | Named file, modified | Named file, unchanged | Session mode |
|---|---|---|---|
| `q`, `:q` | Refused (E37) | Quit | Quit |
| `:q!`, `ZQ` | Quit, changes dropped | Quit | Quit |
| `ZZ`, `:x` | Write, then quit | Quit, no write | Quit |
| `:wq` | Write, then quit | Write, then quit | Quit |
| `Ctrl+C` | Quit, no guard | Quit | Quit |

- A write that fails shows `Error: ...` and does not quit.
- `ZZ` and `:x` write only when there are changes, as in vim.
- In session mode, quitting is never refused, and `:wq` / `:x` with no path
  just quit: there is no file to write, and the session save covers it.
- On exit in session mode, the text is saved to the session file unless every
  line is blank. This happens however nm exits, `Ctrl+C` included, and even
  when the event loop failed.
- `:e path` from session mode saves the session first, so its text is not
  lost.
- `Z` followed by anything but `Z` or `Q` does nothing.

## Help modal

`?` in normal mode shows a centred box with rounded borders listing the main
keys by mode. Any key closes it and does nothing else. It is laid out in two
columns so it fits an 80x24 terminal; in a smaller terminal it shows its
top-left part.

The help text in `help.rs` is a short pick from `doc/keys.md`, kept in sync
by hand. A test checks that every key it names appears in `doc/keys.md`.

## Completion

In insert mode, nm offers variable names as dim "ghost text" at the cursor.

**Candidates:** names from Assignment lines above the cursor line, nearest
first, without repeats, then `sum` last. Lines that are not assignments
(error lines included) are skipped. A variable named `sum` is never offered
as a variable.

**Prefix:** the identifier characters just left of the cursor. There is no
completion when:

- the prefix is empty or starts with a digit;
- there is a `#` left of the cursor (inside a comment);
- the line has `=` and the cursor is at or left of the first one (the name
  being assigned);
- the character under the cursor is an identifier character, so the cursor
  is inside a word rather than at its end.

**Matches:** candidates that start with the prefix and are longer than it,
in candidate order. An exact match has nothing to add and does not count.

**When it is computed:** after every insert-mode key except `Tab`, `Ctrl+N`
and `Ctrl+P`. Arrows and Enter recompute it too, so moving to the end of a
partial name offers it. Entering insert mode is not an insert-mode key, so
`i` on a partial word shows nothing until a key is typed. Leaving insert mode
clears it.

**Display:** the rest of the selected match, in the ghost colour, at the
cursor. The text after the cursor moves right to make room. The ghost is
clipped to the room left in the input column after the real text, so real
text is never cut for it. The bar cursor sits on the ghost's first cell.

**Keys:**

| Key | Does |
|---|---|
| `Tab` | Accept: types the ghost text through the engine, so it stays in the same insert session (one undo step). Does nothing when there is no completion; never inserts a tab |
| `Ctrl+N`, `Ctrl+P` | Next / previous match, wrapping; the completion stays active |

These three keys never reach the engine.

## Colours

`theme.rs` holds `Theme`, one `Color` per slot. The style functions in
`style.rs` (`number()`, `status_bar()`, `badge(..)` and so on) keep no
colours of their own: each reads the active theme. The active theme lives in
a thread-local that starts as Tokyo Night. `tui::run` sets it from
`theme::load()` before the first draw, and the event loop reloads it when a
theme file changes: while waiting for a key it wakes every 500 ms and asks
`theme::Watch` whether any source file's modification time or size moved.
An Omarchy theme switch replaces the whole `current/theme` directory, so
the switch shows in nm within a second, no restart. Tests never read the
real files:
each test runs on its own thread, sees Tokyo Night, and may call
`theme::set_active` to try another theme.

**Where the colours come from.** Each slot takes the first source that sets
it:

1. The nm theme file: `$XDG_CONFIG_HOME/nm/theme.toml`, or
   `~/.config/nm/theme.toml` when `XDG_CONFIG_HOME` is unset, empty or
   relative.
2. The active Omarchy theme: `$XDG_STATE_HOME/omarchy/current/theme/colors.toml`,
   or the same path under `~/.local/state`. Mapped by role (table below).
3. Tokyo Night, `Theme::tokyo_night()`.

Loading never fails. A missing or unreadable file, a file that is not UTF-8,
or one over 64 KiB is ignored as a whole. A line that is not
`known_key = "#rrggbb"` is skipped.

**The nm theme file** is flat TOML, one `slot = "#rrggbb"` per line, any
subset of the slots. Hex is upper or lower case; single or double quotes;
blank lines, `#` comment lines and a `#` comment after the value are fine.
Unknown keys are ignored.

```toml
# ~/.config/nm/theme.toml
number = "#7dcfff"
status_bg = "#1A1B26"
```

**Slots**, with the Tokyo Night default and the Omarchy keys that can fill
each, in order of preference. For a text slot (marked *text*), the first key
whose WCAG contrast against the theme's `background` reaches 4.5 wins; if
none does, the most contrasting one wins. Other slots take the first key the
theme defines. A slot with none of its keys present keeps Tokyo Night.
Numbers are the pad's body text, so they use `foreground`, the one role
every Omarchy theme keeps readable; the coloured roles drop below 4.5 in
several stock themes.

| Slot | Paints | Tokyo Night | Omarchy keys |
|---|---|---|---|
| `number` | Numbers | `#7DCFFF` | *text* `foreground` |
| `operator` | Operators, `=`, parentheses | `#BB9AF7` | *text* `magenta`, `bright_magenta`, `accent`, `light_foreground`, `foreground` |
| `variable` | Variables | `#9ECE6A` | *text* `green`, `bright_green`, `cyan`, `bright_cyan`, `light_foreground`, `foreground` |
| `comment` | Comments (always italic) | `#565F89` | `muted` |
| `keyword` | `sum` keyword | `#F7768E` | *text* `red`, `bright_red`, `orange`, `yellow`, `foreground` |
| `error` | Error results | `#F7768E` | *text* `red`, `bright_red`, `orange`, `yellow`, `foreground` |
| `result` | Results | `#A9B1D6` | *text* `foreground` |
| `status_bg` | Status bar background; badge text | `#1A1B26` | `dark_background` |
| `status_fg` | Status bar text | `#A9B1D6` | `foreground` |
| `badge_normal` | NORMAL badge (bold) | `#7AA2F7` | `blue` |
| `badge_insert` | INSERT badge (bold) | `#9ECE6A` | `green` |
| `badge_visual` | VISUAL badges (bold) | `#FF9E64` | `orange` |
| `gutter` | Line numbers | `#666666` | `muted` |
| `current_line_nr` | Current line number (bold) | `#E0AF68` | *text* `yellow`, `bright_yellow`, `orange`, `accent`, `foreground` |
| `separator` | Gutter separator | `#3B4261` | `selection` |
| `ghost` | Completion ghost text | `#565F89` | `muted` |
| `visual_bg` | Visual selection background | `#3B4261` | `selection` |
| `visual_fg` | Visual selection text | `#E0AF68` | `yellow` |

The help modal uses the same theme: border `badge_normal`; normal-mode keys
`badge_visual`, insert-mode keys `badge_insert`, commands `number`; footer
`comment`.
