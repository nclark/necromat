//! Storage and start-up: loading and saving `.nm` files, the session path,
//! `~` expansion, command-line parsing, and what the editor opens with. See
//! `doc/design/storage.md`.

mod args;
mod file;
mod path;
mod startup;

pub use args::{ArgsError, Command, USAGE, parse_args};
pub use file::{ensure_dir, exists, load, save, split_lines};
pub use path::{expand_tilde, expand_tilde_with, home_dir, session_path, session_path_from};
pub use startup::{Startup, StartupError, resolve_startup, resolve_startup_with};
