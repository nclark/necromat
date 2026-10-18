//! Deciding what the editor starts with, from the `nm [file]` argument.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use super::{exists, expand_tilde, load, session_path};

/// What the editor opens with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Startup {
    /// The buffer. Never empty: at least one (possibly empty) line.
    pub lines: Vec<String>,
    /// The file being edited. `None` means session mode: on exit the buffer
    /// is auto-saved to [`session_path`] unless every line is blank.
    pub file_path: Option<PathBuf>,
}

impl Startup {
    /// True when there is no file path, so the buffer belongs to the
    /// auto-saved session.
    pub fn session_mode(&self) -> bool {
        self.file_path.is_none()
    }
}

/// Why the editor cannot start.
#[derive(Debug)]
pub enum StartupError {
    /// The file does not exist and neither does its directory.
    DirectoryMissing(PathBuf),
    /// The file exists but could not be read.
    Load { path: PathBuf, source: io::Error },
}

impl fmt::Display for StartupError {
    /// The full stderr line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryMissing(dir) => {
                write!(f, "Error: directory does not exist: {}", dir.display())
            }
            Self::Load { path, source } => {
                write!(f, "Error loading {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for StartupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DirectoryMissing(_) => None,
            Self::Load { source, .. } => Some(source),
        }
    }
}

/// Resolves the `nm [file]` argument (expanding `~/`) against the real
/// session path.
pub fn resolve_startup(file: Option<&Path>) -> Result<Startup, StartupError> {
    let file = file.map(expand_tilde);
    resolve_startup_with(file.as_deref(), session_path().as_deref())
}

/// [`resolve_startup`] with the session path passed in and no `~`
/// expansion.
///
/// - A file that exists is loaded; a read error is fatal.
/// - A file that does not exist starts as one empty line with that path,
///   if its directory exists; otherwise [`StartupError::DirectoryMissing`].
/// - No file: the session file is loaded if it exists and reads cleanly,
///   else the buffer starts empty. Either way the result is in session mode.
pub fn resolve_startup_with(
    file: Option<&Path>,
    session: Option<&Path>,
) -> Result<Startup, StartupError> {
    let Some(file) = file else {
        let lines = session
            .filter(|p| exists(p))
            .and_then(|p| load(p).ok())
            .unwrap_or_else(empty_buffer);
        return Ok(Startup {
            lines,
            file_path: None,
        });
    };

    if exists(file) {
        let lines = load(file).map_err(|source| StartupError::Load {
            path: file.to_path_buf(),
            source,
        })?;
        return Ok(Startup {
            lines,
            file_path: Some(file.to_path_buf()),
        });
    }

    let dir = match file.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    // Only "not found" is fatal; other stat errors fall through
    // and surface on save.
    if let Err(e) = std::fs::metadata(dir)
        && e.kind() == io::ErrorKind::NotFound
    {
        return Err(StartupError::DirectoryMissing(dir.to_path_buf()));
    }
    Ok(Startup {
        lines: empty_buffer(),
        file_path: Some(file.to_path_buf()),
    })
}

fn empty_buffer() -> Vec<String> {
    vec![String::new()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::save;
    use tempfile::TempDir;

    #[test]
    fn existing_file_is_loaded() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a.nm");
        save(&path, &["x=1", "x*2"]).unwrap();
        let s = resolve_startup_with(Some(&path), None).unwrap();
        assert_eq!(s.lines, ["x=1", "x*2"]);
        assert_eq!(s.file_path.as_deref(), Some(path.as_path()));
        assert!(!s.session_mode());
    }

    #[test]
    fn new_file_in_existing_directory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("new.nm");
        let s = resolve_startup_with(Some(&path), None).unwrap();
        assert_eq!(s.lines, [""]);
        assert_eq!(s.file_path.as_deref(), Some(path.as_path()));
        assert!(!path.exists(), "nothing is written until save");
    }

    #[test]
    fn bare_file_name_uses_current_directory() {
        let s = resolve_startup_with(Some(Path::new("nm-no-such-file-here.nm")), None).unwrap();
        assert_eq!(s.lines, [""]);
    }

    #[test]
    fn missing_directory_is_an_error() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nonexistent").join("dir");
        let err = resolve_startup_with(Some(&missing.join("file.nm")), None).unwrap_err();
        assert!(matches!(&err, StartupError::DirectoryMissing(d) if d == &missing));
        assert_eq!(
            err.to_string(),
            format!("Error: directory does not exist: {}", missing.display())
        );
    }

    #[test]
    fn unreadable_file_is_an_error() {
        let dir = TempDir::new().unwrap();
        let err = resolve_startup_with(Some(dir.path()), None).unwrap_err();
        assert!(matches!(err, StartupError::Load { .. }));
        assert!(err.to_string().starts_with("Error loading "));
    }

    #[test]
    fn session_mode_loads_session_file() {
        let dir = TempDir::new().unwrap();
        let session = dir.path().join("nm").join("last-session.nm");
        save(&session, &["100", "sum"]).unwrap();
        let s = resolve_startup_with(None, Some(&session)).unwrap();
        assert_eq!(s.lines, ["100", "sum"]);
        assert!(s.session_mode());
    }

    #[test]
    fn session_mode_without_session_file() {
        let dir = TempDir::new().unwrap();
        let session = dir.path().join("missing.nm");
        let s = resolve_startup_with(None, Some(&session)).unwrap();
        assert_eq!(s.lines, [""]);
        assert!(s.session_mode());
        let s = resolve_startup_with(None, None).unwrap();
        assert_eq!(s.lines, [""]);
    }

    #[test]
    fn session_mode_ignores_unreadable_session() {
        let dir = TempDir::new().unwrap();
        // A directory where the file should be: exists, but load fails.
        let s = resolve_startup_with(None, Some(dir.path())).unwrap();
        assert_eq!(s.lines, [""]);
        assert!(s.session_mode());
    }
}
