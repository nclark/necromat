//! The system clipboard as nm sees it: a place `y` writes to and `p` reads
//! from when the register is empty.
//!
//! [`Clipboard`] is the trait the editor talks to. [`SystemClipboard`] is
//! the real one; [`MemoryClipboard`] stands in for it in tests.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::Command as _;
use crossterm::clipboard::CopyToClipboard;

/// A clipboard nm can write text to and read text from.
pub trait Clipboard: Send {
    /// Replaces the clipboard text. The error is shown in the status bar.
    fn set(&mut self, text: &str) -> Result<(), String>;

    /// The clipboard text, or `None` when it is empty or unreadable.
    fn get(&mut self) -> Option<String>;
}

/// A clipboard that keeps its text in memory. Tests use it to see what `y`
/// wrote.
#[derive(Debug, Default)]
pub struct MemoryClipboard {
    text: Option<String>,
}

impl Clipboard for MemoryClipboard {
    fn set(&mut self, text: &str) -> Result<(), String> {
        self.text = Some(text.to_string());
        Ok(())
    }

    fn get(&mut self) -> Option<String> {
        self.text.clone()
    }
}

/// How long a clipboard tool (`wl-copy`, `xclip`, ...) may run before nm
/// kills it. Yanks happen on the UI thread, so this caps the stall.
const TOOL_TIMEOUT: Duration = Duration::from_millis(500);

/// The platform clipboard.
///
/// Writing uses up to three channels:
///
/// 1. **arboard**, the native clipboard (Wayland through the data-control
///    protocol, X11, macOS, Windows).
/// 2. **A clipboard tool** (`wl-copy`, `xclip`, `xsel`, `pbcopy`), run only
///    when arboard could not start or its write failed. arboard already
///    covers every platform these tools do, so this is a last resort, for
///    example a Wayland compositor without the data-control protocol, where
///    `wl-copy` still works.
/// 3. **OSC 52**, an escape sequence asking the terminal to set its host's
///    clipboard. This is the only channel that reaches the user's own
///    machine over SSH.
///
/// OSC 52 is always written alongside the native channel, not only when it
/// fails. nm cannot tell whether it runs locally or over SSH (a forwarded X
/// display makes arboard succeed on the remote host), and the terminal
/// never says whether it accepted the sequence. Writing both means a local
/// paste and a paste on the SSH client both get the text; when both reach
/// the same clipboard they write the same text, so nothing is lost. The terminal-owned copy also outlives nm, while an arboard copy on
/// Wayland or X11 disappears when nm exits unless a clipboard manager
/// takes it over.
///
/// OSC 52 is written as plain `ESC ] 52 ; c ; <base64> ESC \`, with no tmux
/// passthrough wrapping. tmux turns that into its own buffer and forwards
/// it to the outer terminal when `set-clipboard` is `on`; the wrapped form
/// would need `allow-passthrough`, which is also off by default, so
/// wrapping gains nothing.
///
/// A write succeeds when any channel took the text. Reading uses arboard,
/// then the read side of the clipboard tool. Terminals rarely answer an
/// OSC 52 query, so there is no OSC 52 read; with no other channel,
/// [`Clipboard::get`] returns `None` and `p` with an empty register does
/// nothing.
pub struct SystemClipboard {
    native: Option<arboard::Clipboard>,
    /// Where OSC 52 goes: the terminal nm draws on. `None` turns it off.
    terminal: Option<Box<dyn Write + Send>>,
    tools: &'static [Tool],
}

/// The parts of the environment that choose clipboard channels.
#[derive(Debug, Clone, Default)]
struct Env {
    wayland: bool,
    x11: bool,
    term: Option<String>,
}

impl Env {
    fn current() -> Self {
        let set = |name| std::env::var_os(name).is_some_and(|v| !v.is_empty());
        Self {
            wayland: set("WAYLAND_DISPLAY"),
            x11: set("DISPLAY"),
            term: std::env::var("TERM").ok(),
        }
    }

    /// True when a native clipboard may exist. On Linux and the BSDs that
    /// needs a display; elsewhere the clipboard is part of the OS.
    fn has_display(&self) -> bool {
        if cfg!(any(target_os = "macos", windows)) {
            return true;
        }
        self.wayland || self.x11
    }

    /// True when the terminal can take an escape sequence. Terminals that
    /// do not know OSC 52 ignore it; only `TERM=dumb` (or no `TERM`) means
    /// there is no terminal to talk to.
    fn has_terminal(&self) -> bool {
        self.term
            .as_deref()
            .is_some_and(|t| !t.is_empty() && t != "dumb")
    }

    /// The clipboard tools to try, in order.
    fn tools(&self) -> &'static [Tool] {
        if cfg!(target_os = "macos") {
            &[PBCOPY]
        } else if cfg!(windows) {
            &[]
        } else if self.wayland {
            &[WL_COPY, XCLIP, XSEL]
        } else if self.x11 {
            &[XCLIP, XSEL]
        } else {
            &[]
        }
    }
}

/// An external clipboard program: how to write to it and read from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tool {
    copy: &'static [&'static str],
    paste: &'static [&'static str],
}

const WL_COPY: Tool = Tool {
    copy: &["wl-copy"],
    paste: &["wl-paste", "--no-newline"],
};
const XCLIP: Tool = Tool {
    copy: &["xclip", "-selection", "clipboard", "-in"],
    paste: &["xclip", "-selection", "clipboard", "-out"],
};
const XSEL: Tool = Tool {
    copy: &["xsel", "--clipboard", "--input"],
    paste: &["xsel", "--clipboard", "--output"],
};
const PBCOPY: Tool = Tool {
    copy: &["pbcopy"],
    paste: &["pbpaste"],
};

impl SystemClipboard {
    /// The platform clipboard, sending OSC 52 to `terminal` (stdout in the
    /// TUI). Never fails: a channel that cannot start is left out, and
    /// [`Clipboard::set`] reports the error when no channel is left.
    pub fn new(terminal: Box<dyn Write + Send>) -> Self {
        Self::with_env(&Env::current(), terminal)
    }

    fn with_env(env: &Env, terminal: Box<dyn Write + Send>) -> Self {
        // Without a display arboard can only fail (or, on X11, wait for a
        // connection timeout), so it is not tried.
        let native = if env.has_display() {
            arboard::Clipboard::new().ok()
        } else {
            None
        };
        Self {
            native,
            terminal: env.has_terminal().then_some(terminal),
            tools: env.tools(),
        }
    }

    /// Writes `text` through the first clipboard tool that works.
    fn set_with_tool(&self, text: &str) -> Result<(), String> {
        let mut last = Err("no clipboard tool found".to_string());
        for tool in self.tools {
            last = run_tool(tool.copy, Some(text)).map(|_| ());
            if last.is_ok() {
                break;
            }
        }
        last
    }

    fn get_with_tool(&self) -> Option<String> {
        self.tools
            .iter()
            .find_map(|tool| run_tool(tool.paste, None).ok().flatten())
    }

    fn set_with_osc52(&mut self, text: &str) -> Result<(), String> {
        let Some(terminal) = self.terminal.as_mut() else {
            return Err("no terminal for OSC 52".to_string());
        };
        terminal
            .write_all(osc52(text).as_bytes())
            .and_then(|()| terminal.flush())
            .map_err(|e| format!("OSC 52: {e}"))
    }
}

impl Clipboard for SystemClipboard {
    fn set(&mut self, text: &str) -> Result<(), String> {
        let native = match self.native.as_mut() {
            Some(clip) => clip.set_text(text).map_err(|e| e.to_string()),
            None => Err("no display".to_string()),
        };
        let native = native.or_else(|e| self.set_with_tool(text).map_err(|_| e));
        let osc = self.set_with_osc52(text);
        match (native, osc) {
            (Ok(()), _) | (_, Ok(())) => Ok(()),
            (Err(e), Err(_)) => Err(e),
        }
    }

    fn get(&mut self) -> Option<String> {
        let native = self.native.as_mut().and_then(|clip| clip.get_text().ok());
        native
            .or_else(|| self.get_with_tool())
            .filter(|text| !text.is_empty())
    }
}

/// The OSC 52 sequence that puts `text` on the terminal's clipboard.
fn osc52(text: &str) -> String {
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = CopyToClipboard::to_clipboard_from(text).write_ansi(&mut out);
    out
}

/// Runs a clipboard program, feeding it `input` or reading its output,
/// and kills it after [`TOOL_TIMEOUT`]. Its stderr is discarded so nothing
/// prints over the TUI. Returns what it printed when `input` is `None`.
fn run_tool(argv: &[&str], input: Option<&str>) -> Result<Option<String>, String> {
    let (program, args) = argv.split_first().ok_or("empty command")?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(if input.is_some() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;

    // Feed and drain on threads so a full pipe cannot stall the wait.
    if let (Some(mut stdin), Some(text)) = (child.stdin.take(), input) {
        let text = text.to_string();
        thread::spawn(move || stdin.write_all(text.as_bytes()));
    }
    let output = child.stdout.take().map(|mut stdout| {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = tx.send(stdout.read_to_end(&mut buf).map(|_| buf));
        });
        rx
    });

    let deadline = Instant::now() + TOOL_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program} timed out"));
            }
            Err(e) => return Err(format!("{program}: {e}")),
        }
    };
    if !status.success() {
        return Err(format!("{program} failed: {status}"));
    }
    let Some(rx) = output else {
        return Ok(None);
    };
    let left = deadline.saturating_duration_since(Instant::now());
    match rx.recv_timeout(left.max(Duration::from_millis(50))) {
        Ok(Ok(bytes)) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Ok(Err(e)) => Err(format!("{program}: {e}")),
        Err(_) => Err(format!("{program} timed out")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A terminal that records what was written to it.
    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<u8>>>);

    impl Recorder {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl Write for Recorder {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn headless(term: Option<&str>) -> Env {
        Env {
            wayland: false,
            x11: false,
            term: term.map(str::to_string),
        }
    }

    /// `p` and `P` with an empty register and a clipboard that has nothing
    /// to give (`get` returns `None`, as [`SystemClipboard`] does when it
    /// cannot read) leave the text alone. `vim_tests.rs` covers an empty
    /// string, not `None`.
    #[test]
    fn paste_with_nothing_anywhere_does_nothing() {
        for keys in ["p", "P"] {
            let mut h = crate::tui::harness::Harness::at(&["abc", "d"], 0, 1);
            h.keys(keys);
            assert_eq!(h.lines(), ["abc", "d"], "{keys}");
            assert_eq!(h.app.editor.clipboard().get(), None, "{keys}");
        }
    }

    #[test]
    fn osc52_is_base64_between_the_prefix_and_st() {
        let cases = [
            ("", "\x1b]52;c;\x1b\\"),
            ("hello", "\x1b]52;c;aGVsbG8=\x1b\\"),
            ("10 + 2", "\x1b]52;c;MTAgKyAy\x1b\\"),
            ("a\nb", "\x1b]52;c;YQpi\x1b\\"),
            // UTF-8 bytes, not characters.
            ("é€", "\x1b]52;c;w6nigqw=\x1b\\"),
        ];
        for (text, want) in cases {
            assert_eq!(osc52(text), want, "{text:?}");
        }
    }

    #[test]
    fn without_a_display_writes_go_to_the_terminal_only() {
        if cfg!(any(target_os = "macos", windows)) {
            return;
        }
        let out = Recorder::default();
        let mut clip = SystemClipboard::with_env(&headless(Some("xterm")), Box::new(out.clone()));
        assert!(clip.native.is_none());
        assert!(clip.tools.is_empty());
        assert_eq!(clip.set("hello"), Ok(()));
        assert_eq!(out.text(), "\x1b]52;c;aGVsbG8=\x1b\\");
        // OSC 52 cannot be read back.
        assert_eq!(clip.get(), None);
    }

    #[test]
    fn with_no_channel_at_all_the_write_fails_cleanly() {
        if cfg!(any(target_os = "macos", windows)) {
            return;
        }
        for term in [None, Some(""), Some("dumb")] {
            let out = Recorder::default();
            let mut clip = SystemClipboard::with_env(&headless(term), Box::new(out.clone()));
            assert_eq!(clip.set("hello"), Err("no display".to_string()), "{term:?}");
            assert_eq!(out.text(), "", "{term:?}");
            assert_eq!(clip.get(), None, "{term:?}");
        }
    }

    #[test]
    fn tools_follow_the_display() {
        if cfg!(any(target_os = "macos", windows)) {
            return;
        }
        let env = |wayland, x11| Env {
            wayland,
            x11,
            term: None,
        };
        assert_eq!(env(true, true).tools(), &[WL_COPY, XCLIP, XSEL]);
        assert_eq!(env(false, true).tools(), &[XCLIP, XSEL]);
        assert!(env(false, false).tools().is_empty());
    }

    #[test]
    fn a_missing_tool_is_an_error_not_a_panic() {
        let missing = ["nm-test-no-such-clipboard-tool"];
        assert!(run_tool(&missing, Some("x")).is_err());
        assert!(run_tool(&missing, None).is_err());
        assert!(run_tool(&[], None).is_err());
    }

    #[test]
    fn tool_output_is_read_and_slow_tools_are_killed() {
        if cfg!(windows) {
            return;
        }
        assert_eq!(
            run_tool(&["printf", "a\nb"], None),
            Ok(Some("a\nb".to_string()))
        );
        assert_eq!(run_tool(&["cat"], Some("text")), Ok(None));
        assert!(run_tool(&["false"], None).is_err());
        let start = Instant::now();
        assert_eq!(
            run_tool(&["sleep", "5"], None),
            Err("sleep timed out".to_string())
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    /// The real constructor, with the real environment, when there is no
    /// display: arboard is not reached, nothing panics. Run in a child
    /// process by [`real_construction_without_a_display`] with the display
    /// variables removed; it does nothing when a display is present.
    #[test]
    fn real_construction_in_this_environment() {
        let env = Env::current();
        if env.has_display() {
            return;
        }
        assert!(arboard::Clipboard::new().is_err());
        let out = Recorder::default();
        let mut clip = SystemClipboard::new(Box::new(out.clone()));
        assert!(clip.native.is_none());
        assert_eq!(clip.get(), None);
        let _ = clip.set("x");
    }

    #[test]
    fn real_construction_without_a_display() {
        if cfg!(any(target_os = "macos", windows)) {
            return;
        }
        let exe = std::env::current_exe().unwrap();
        let out = Command::new(exe)
            .args([
                "--exact",
                "tui::clipboard::tests::real_construction_in_this_environment",
            ])
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .stderr(Stdio::null())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{stdout}");
        // The filter matched the test, so it really ran.
        assert!(stdout.contains("1 passed"), "{stdout}");
    }
}
