//! The session file path and `~` expansion.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The session file name inside the data directory.
const SESSION_FILE: &str = "last-session.nm";

/// The user's home directory, if known.
pub fn home_dir() -> Option<PathBuf> {
    std::env::home_dir().filter(|p| !p.as_os_str().is_empty())
}

/// The path of the auto-saved session:
/// `$XDG_DATA_HOME/nm/last-session.nm`, or `~/.local/share/nm/last-session.nm`
/// when `XDG_DATA_HOME` is unset, empty, or relative (the XDG spec says to
/// ignore relative values). `None` when neither is available.
pub fn session_path() -> Option<PathBuf> {
    session_path_from(std::env::var_os("XDG_DATA_HOME"), home_dir())
}

/// [`session_path`] with its inputs passed in, so tests need not change the
/// process environment.
pub fn session_path_from(
    xdg_data_home: Option<OsString>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    let data_dir = xdg_data_home
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| h.join(".local").join("share")))?;
    Some(data_dir.join("nm").join(SESSION_FILE))
}

/// Expands a leading `~/` to the home directory. Other paths, and `~/...`
/// when the home directory is unknown, are returned unchanged.
/// `~user/` and a bare `~` are not expanded.
pub fn expand_tilde(path: &Path) -> PathBuf {
    expand_tilde_with(path, home_dir().as_deref())
}

/// [`expand_tilde`] with the home directory passed in.
pub fn expand_tilde_with(path: &Path, home: Option<&Path>) -> PathBuf {
    if let Some(home) = home
        && let Ok(rest) = path.strip_prefix("~")
        && path.as_os_str().len() > 1
    {
        return home.join(rest);
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- session path ----

    #[test]
    fn default_session_path() {
        let path = session_path_from(None, Some(PathBuf::from("/home/u"))).unwrap();
        assert!(path.ends_with(SESSION_FILE));
        assert!(path.ends_with(Path::new(".local/share/nm/last-session.nm")));
        assert_eq!(path, Path::new("/home/u/.local/share/nm/last-session.nm"));
    }

    #[test]
    fn session_path_from_real_environment() {
        // Whatever the environment, the file name is fixed.
        if let Some(path) = session_path() {
            assert!(path.ends_with(Path::new("nm/last-session.nm")));
        }
    }

    #[test]
    fn session_path_honours_xdg_data_home() {
        let path = session_path_from(Some("/xdg/data".into()), Some(PathBuf::from("/home/u")));
        assert_eq!(path.unwrap(), Path::new("/xdg/data/nm/last-session.nm"));
    }

    #[test]
    fn session_path_ignores_empty_or_relative_xdg() {
        for xdg in ["", "relative/data"] {
            let path = session_path_from(Some(xdg.into()), Some(PathBuf::from("/home/u")));
            assert_eq!(
                path.unwrap(),
                Path::new("/home/u/.local/share/nm/last-session.nm"),
                "XDG_DATA_HOME={xdg:?}"
            );
        }
    }

    #[test]
    fn session_path_without_home_or_xdg() {
        assert_eq!(session_path_from(None, None), None);
        assert_eq!(
            session_path_from(Some("/x".into()), None).unwrap(),
            Path::new("/x/nm/last-session.nm")
        );
    }

    // ---- tilde expansion ----

    #[test]
    fn expand_tilde_cases() {
        let home = Some(Path::new("/home/u"));
        let cases = [
            ("~/notes/a.nm", "/home/u/notes/a.nm"),
            ("~/", "/home/u"),
            ("~", "~"),
            ("~user/a.nm", "~user/a.nm"),
            ("/abs/a.nm", "/abs/a.nm"),
            ("rel/~/a.nm", "rel/~/a.nm"),
            ("a.nm", "a.nm"),
        ];
        for (input, want) in cases {
            assert_eq!(
                expand_tilde_with(Path::new(input), home),
                Path::new(want),
                "{input}"
            );
        }
    }

    #[test]
    fn expand_tilde_without_home() {
        assert_eq!(
            expand_tilde_with(Path::new("~/a.nm"), None),
            Path::new("~/a.nm")
        );
    }
}
