# Agent Instructions

`nm` is a terminal calculator: a vim-style text pad where every line is
evaluated live and the result is shown in a right-hand column. The goal is a
solid editor core, then publication.

Architecture and behaviour: `doc/design/README.md` first, then the file for
the area you are changing.

## Toolchain and layout

- Rust stable via mise (`mise.toml`); `cargo build`, `cargo test`,
  `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` must all pass
  before a story is marked for review.
- Crate layout (one binary crate, modules by concern):
  - `src/lang/` tokenizer, parser, line detection, number rules
  - `src/eval/` evaluator, sum accumulator, sections, typed errors, formatting
  - `src/storage/` load/save, session path, CLI arguments
  - `src/tui/` ratatui view, modes, key handling, completion, clipboard
  - `src/main.rs` argument parsing and program start; `nm eval` subcommand
- Editor core: hjkl, see `doc/design/editor.md`.
- No `unsafe`. No panics on user input: every parse/eval path returns a typed
  error.

## Issue tracking (br)

This project uses **br** (beads_rust), installed via mise. `.beads/issues.jsonl`
is committed; the SQLite database is local (`br sync --import-only` rebuilds it).
Only run `br` from this directory, never from a git worktree.

```bash
br ready                              # Find available work (open, unblocked)
br show <id>                          # View issue details
br update <id> -s in_progress         # Claim work (add --force if blockers are in_progress)
br close <id> --reason="<what landed>"   # Close once merged and ci passes
br q "<title>" -p <0-4> -t <type> -d "<why>" --parent <epic>   # Capture follow-up
br sync --flush-only                  # Export DB to issues.jsonl
```

## Per-story workflow

1. Claim: `br update <id> -s in_progress`, then `br show <id>`.
2. Tests first: write the failing test, then implement.
3. Verify: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`.
4. Commit only the files you changed (`git add <files>`, never `-A`), message
   `<story-id>: <brief description>`, ending with
   `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
5. Once the story is merged to master and `just ci` passes, close it:
   `br close <id> --reason="<what landed>"`. Put findings, spec deviations
   and quirks in a `br comments add <id>` on the story. Epics close when their
   last story does.

Rules: only work on stories from `br ready`; one story at a time per agent;
follow the story text, it contains the design; a story is not done until its
tests pass.

## Landing the plane (end of session)

Work is not complete until `git push` succeeds:

```bash
br sync --flush-only
git add .beads/issues.jsonl && git commit -m "br sync" || true
git pull --rebase=merges   # plain --rebase replays merge commits and re-raises resolved conflicts
git push
git status   # must be up to date with origin
```

`br sync` never runs git for you. If there is no remote yet, say so in the
report instead of pretending.

After merging anything that changes the binary, run `just install` so the
user's `nm` on PATH is current. A stale installed binary has already been
mistaken for a bug once.

## Checking the real terminal

Unit tests render through the headless harness; a tmux pane checks the rest.
Send keys with pauses: `Esc` followed at once by a letter arrives as Alt+letter
and is ignored, so `tmux send-keys Escape; sleep 0.3; tmux send-keys q`.
`tmux capture-pane -e -p` shows colours as escape codes. Point the session
file somewhere disposable with `XDG_DATA_HOME`, never at the user's real one.

## Releasing

The crate is `necromat`, the command is `nm`, the repo is
`github.com/nclark/necromat`. A release is a version bump and a tag; CI does
the rest:

```bash
sed -i 's/^version = ".*"$/version = "X.Y.Z"/' Cargo.toml && cargo build
git commit -am "X.Y.Z" && git push
git tag vX.Y.Z && git push origin vX.Y.Z
```

The tag runs `release.yml` (cargo-dist: four targets, GitHub Release, shell
installer, then `cargo publish` through crates.io Trusted Publishing, no
stored secret) and `docker.yml` (`ghcr.io/nclark/necromat:X.Y.Z` and
`:latest`). A push to master publishes `:edge`. dist refuses a tag whose
version differs from Cargo.toml. `release.yml` is generated: change
`dist-workspace.toml` and run `dist generate`, never edit it by hand. A
reusable workflow called from it must not declare `permissions`; it inherits
them.

## Decisions and history

The record lives in br, which is committed. Decisions, spike results, crate
quirks and review notes are comments on the story that produced them: start
with `br show <epic>` and read the comments on its stories. `br search
<word>` searches comment text too. Do not restate that history here.

- Editor core: hjkl-engine + hjkl-buffer + hjkl-vim, pinned `=0.42.2`,
  `motion_sneak: false` (why: nmr-ye9.1 comments; quirks: nmr-b19.3 comments
  and `src/tui/editor.rs`).
- Issue prefix is `nmr`.
- License: WTFPL.

<!-- br-agent-instructions-v1 -->

---

## Beads Workflow Integration

This project uses [beads_rust](https://github.com/Dicklesworthstone/beads_rust) (`br`/`bd`) for issue tracking. Issues are stored in `.beads/` and tracked in git.

### Essential Commands

```bash
# View ready issues (open, unblocked, not deferred)
br ready              # or: bd ready

# List and search
br list --status=open # All open issues
br show <id>          # Full issue details with dependencies
br search "keyword"   # Full-text search

# Create and update
br create --title="..." --description="..." --type=task --priority=2
br update <id> --status=in_progress
br close <id> --reason="Completed"
br close <id1> <id2>  # Close multiple issues at once

# Sync with git
br sync --flush-only  # Export DB to JSONL
br sync --status      # Check sync status
```

### Workflow Pattern

1. **Start**: Run `br ready` to find actionable work
2. **Claim**: Use `br update <id> --status=in_progress`
3. **Work**: Implement the task
4. **Complete**: Use `br close <id>`
5. **Sync**: Always run `br sync --flush-only` at session end

### Key Concepts

- **Dependencies**: Issues can block other issues. `br ready` shows only open, unblocked work.
- **Priority**: P0=critical, P1=high, P2=medium, P3=low, P4=backlog (use numbers 0-4, not words)
- **Types**: task, bug, feature, epic, chore, docs, question
- **Blocking**: `br dep add <issue> <depends-on>` to add dependencies

### Session Protocol

**Before ending any session, run this checklist:**

```bash
git status              # Check what changed
git add <files>         # Stage code changes
br sync --flush-only    # Export beads changes to JSONL
git commit -m "..."     # Commit everything
git push                # Push to remote
```

### Best Practices

- Check `br ready` at session start to find available work
- Update status as you work (in_progress → closed)
- Create new issues with `br create` when you discover tasks
- Use descriptive titles and set appropriate priority/type
- Always sync before ending session

<!-- end-br-agent-instructions -->
