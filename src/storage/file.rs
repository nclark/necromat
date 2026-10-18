//! Reading and writing `.nm` files.
//!
//! The format is plain text, one line per line, LF line endings, and a
//! trailing newline on save.

use std::fs;
use std::io;
use std::path::Path;

/// Splits file content into lines.
///
/// CRLF is turned into LF, the text is split on LF, and one trailing empty
/// element (from the final newline) is dropped. Empty input gives no lines.
/// A lone `\r` is kept as text.
pub fn split_lines(content: &str) -> Vec<String> {
    let content = content.replace("\r\n", "\n");
    let mut lines: Vec<String> = content.split('\n').map(str::to_string).collect();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// Reads the lines of a file. An empty file loads as one empty line.
///
/// Bytes that are not valid UTF-8 are replaced with U+FFFD rather than
/// failing the load.
pub fn load(path: &Path) -> io::Result<Vec<String>> {
    let data = fs::read(path)?;
    let mut lines = split_lines(&String::from_utf8_lossy(&data));
    if lines.is_empty() {
        lines.push(String::new());
    }
    Ok(lines)
}

/// Writes lines to a file: joined with `\n`, plus a final `\n`. Parent
/// directories are created as needed.
///
/// The final `\n` is always added, so trailing blank lines survive a save
/// and load: `["x", ""]` is written as `x\n\n`, not `x\n`, which would load
/// back as `["x"]`.
pub fn save<S: AsRef<str>>(path: &Path, lines: &[S]) -> io::Result<()> {
    ensure_dir(path)?;
    let mut content = String::new();
    for line in lines {
        content.push_str(line.as_ref());
        content.push('\n');
    }
    if content.is_empty() {
        content.push('\n');
    }
    fs::write(path, content)
}

/// Creates the parent directory of `path` if it is missing.
pub fn ensure_dir(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => fs::create_dir_all(dir),
        _ => Ok(()),
    }
}

/// True when something exists at `path` (follows symlinks).
pub fn exists(path: &Path) -> bool {
    fs::metadata(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    // ---- load ----

    #[test]
    fn load_basic() {
        let dir = TempDir::new().unwrap();
        let path = write(
            &dir,
            "test.nm",
            "# My Budget\nincome=5000\ntaxes=income*0.28\n",
        );
        assert_eq!(
            load(&path).unwrap(),
            ["# My Budget", "income=5000", "taxes=income*0.28"]
        );
    }

    #[test]
    fn load_nonexistent() {
        let dir = TempDir::new().unwrap();
        assert!(load(&dir.path().join("nonexistent.nm")).is_err());
    }

    #[test]
    fn load_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = write(&dir, "empty.nm", "");
        assert_eq!(load(&path).unwrap(), [""]);
    }

    #[test]
    fn load_windows_line_endings() {
        let dir = TempDir::new().unwrap();
        let path = write(&dir, "windows.nm", "line1\r\nline2\r\nline3\r\n");
        assert_eq!(load(&path).unwrap(), ["line1", "line2", "line3"]);
    }

    #[test]
    fn load_no_trailing_newline() {
        let dir = TempDir::new().unwrap();
        let path = write(&dir, "notrailing.nm", "line1\nline2\nline3");
        assert_eq!(load(&path).unwrap(), ["line1", "line2", "line3"]);
    }

    #[test]
    fn exists_true_and_false() {
        let dir = TempDir::new().unwrap();
        let path = write(&dir, "exists.nm", "test");
        assert!(exists(&path));
        assert!(!exists(&dir.path().join("nonexistent.nm")));
    }

    #[test]
    fn load_save_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("roundtrip.nm");
        let original = ["# Test File", "x=42", "y=x*2", "", "total=x+y"];
        save(&path, &original).unwrap();
        assert_eq!(load(&path).unwrap(), original);
    }

    // ---- extra load cases ----

    #[test]
    fn load_single_newline_is_one_empty_line() {
        let dir = TempDir::new().unwrap();
        let path = write(&dir, "nl.nm", "\n");
        assert_eq!(load(&path).unwrap(), [""]);
    }

    #[test]
    fn load_drops_only_one_trailing_empty_line() {
        let dir = TempDir::new().unwrap();
        let path = write(&dir, "blank.nm", "x\n\n");
        assert_eq!(load(&path).unwrap(), ["x", ""]);
    }

    #[test]
    fn load_keeps_lone_carriage_return() {
        let dir = TempDir::new().unwrap();
        let path = write(&dir, "cr.nm", "a\rb\n");
        assert_eq!(load(&path).unwrap(), ["a\rb"]);
    }

    #[test]
    fn load_unicode() {
        let dir = TempDir::new().unwrap();
        let path = write(&dir, "u.nm", "café=3 # ünïcode 🎉\r\n");
        assert_eq!(load(&path).unwrap(), ["café=3 # ünïcode 🎉"]);
    }

    #[test]
    fn load_invalid_utf8_is_lossy() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bad.nm");
        fs::write(&path, b"x=1\n\xff\n").unwrap();
        assert_eq!(load(&path).unwrap(), ["x=1", "\u{fffd}"]);
    }

    #[test]
    fn load_directory_is_error() {
        let dir = TempDir::new().unwrap();
        assert!(load(dir.path()).is_err());
    }

    #[test]
    fn split_lines_empty_input_is_no_lines() {
        assert!(split_lines("").is_empty());
    }

    // ---- save ----

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    #[test]
    fn save_basic() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.nm");
        let lines = [
            "# My Budget",
            "income=5000",
            "taxes=income*0.28",
            "net=income-taxes",
        ];
        save(&path, &lines).unwrap();
        assert_eq!(read(&path), lines.join("\n") + "\n");
    }

    #[test]
    fn save_creates_directory() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested").join("dirs").join("test.nm");
        save(&path, &["x=42"]).unwrap();
        assert!(path.exists());
        assert_eq!(read(&path), "x=42\n");
    }

    #[test]
    fn save_overwrite() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.nm");
        save(&path, &["x=1"]).unwrap();
        save(&path, &["x=2", "y=3"]).unwrap();
        assert_eq!(read(&path), "x=2\ny=3\n");
    }

    #[test]
    fn save_empty_lines() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.nm");
        save(&path, &[""]).unwrap();
        assert_eq!(read(&path), "\n");
    }

    #[test]
    fn save_adds_trailing_newline() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.nm");
        save(&path, &["x=1", "y=2"]).unwrap();
        assert_eq!(read(&path), "x=1\ny=2\n");
    }

    // ---- extra save cases ----

    #[test]
    fn save_no_lines_writes_one_newline() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.nm");
        save::<&str>(&path, &[]).unwrap();
        assert_eq!(read(&path), "\n");
        assert_eq!(load(&path).unwrap(), [""]);
    }

    #[test]
    fn save_keeps_trailing_blank_lines() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.nm");
        save(&path, &["x", "", ""]).unwrap();
        assert_eq!(read(&path), "x\n\n\n");
        assert_eq!(load(&path).unwrap(), ["x", "", ""]);
    }

    #[test]
    fn save_accepts_owned_strings() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.nm");
        save(&path, &[String::from("a"), String::from("b")]).unwrap();
        assert_eq!(read(&path), "a\nb\n");
    }

    // ---- directory creation ----

    #[test]
    fn ensure_dir_creates_and_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let path = dir
            .path()
            .join("some")
            .join("nested")
            .join("dirs")
            .join("file.nm");
        ensure_dir(&path).unwrap();
        assert!(path.parent().unwrap().is_dir());
        ensure_dir(&path).unwrap();
    }

    #[test]
    fn ensure_dir_with_existing_dir() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("existing").join("path");
        fs::create_dir_all(&nested).unwrap();
        ensure_dir(&nested.join("file.nm")).unwrap();
    }

    #[test]
    fn ensure_dir_bare_file_name() {
        ensure_dir(Path::new("file.nm")).unwrap();
    }
}
