//! The colour theme: one colour per thing nm paints, and where it comes from.
//!
//! [`load`] builds the theme once at start-up. Each slot is taken from the
//! first source that sets it:
//!
//! 1. The nm theme file, `$XDG_CONFIG_HOME/nm/theme.toml`, or
//!    `~/.config/nm/theme.toml` when `XDG_CONFIG_HOME` is unset, empty or
//!    relative.
//! 2. The active Omarchy theme, `$XDG_STATE_HOME/omarchy/current/theme/colors.toml`,
//!    or `~/.local/state/omarchy/current/theme/colors.toml`, mapped by role
//!    (see [`OMARCHY_ROLES`]).
//! 3. Tokyo Night, [`Theme::tokyo_night`].
//!
//! # The nm theme file
//!
//! Flat TOML: one `slot = "#rrggbb"` per line. The slots are the fields of
//! [`Theme`]: `number`, `operator`, `variable`, `comment`, `keyword`,
//! `error`, `result`, `status_bg`, `status_fg`, `badge_normal`,
//! `badge_insert`, `badge_visual`, `gutter`, `current_line_nr`, `separator`,
//! `ghost`, `visual_bg`, `visual_fg`. Any subset may be set; the rest fall
//! through to the next source. Hex digits
//! may be upper or lower case. Blank lines and `#` comments are allowed,
//! and so is a `#` comment after the value. A line that is not
//! `known_slot = "#rrggbb"` is skipped. A file that cannot be read as UTF-8
//! text, or is larger than 64 KiB, is ignored as a whole.
//!
//! ```toml
//! # ~/.config/nm/theme.toml
//! number = "#7dcfff"
//! status_bg = "#1A1B26"
//! ```
//!
//! The theme in use is held per thread ([`active`], [`set_active`]). The TUI
//! draws on one thread, so `run` sets it once; each test runs on its own
//! thread and sees Tokyo Night unless it sets a theme itself.

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};

use ratatui::style::Color;

use super::style;
use crate::storage;

/// One colour per nm slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub number: Color,
    pub operator: Color,
    pub variable: Color,
    pub comment: Color,
    pub keyword: Color,
    pub error: Color,
    pub result: Color,
    pub status_bg: Color,
    pub status_fg: Color,
    pub badge_normal: Color,
    pub badge_insert: Color,
    pub badge_visual: Color,
    pub gutter: Color,
    pub current_line_nr: Color,
    pub separator: Color,
    pub ghost: Color,
    pub visual_bg: Color,
    pub visual_fg: Color,
}

/// Which Omarchy `colors.toml` keys fill each nm slot, in order of
/// preference. For a text slot (`check_contrast`), the first candidate whose
/// contrast against `background` reaches [`MIN_CONTRAST`] wins; when none
/// does, the most contrasting one wins. Other slots take the first key the
/// theme defines. Numbers are the pad's body text, so they use `foreground`,
/// the one role every theme keeps readable.
pub const OMARCHY_ROLES: [OmarchyRole; 18] = [
    OmarchyRole::text("number", &["foreground"]),
    OmarchyRole::text(
        "operator",
        &[
            "magenta",
            "bright_magenta",
            "accent",
            "light_foreground",
            "foreground",
        ],
    ),
    OmarchyRole::text(
        "variable",
        &[
            "green",
            "bright_green",
            "cyan",
            "bright_cyan",
            "light_foreground",
            "foreground",
        ],
    ),
    OmarchyRole::plain("comment", &["muted"]),
    OmarchyRole::text(
        "keyword",
        &["red", "bright_red", "orange", "yellow", "foreground"],
    ),
    OmarchyRole::text(
        "error",
        &["red", "bright_red", "orange", "yellow", "foreground"],
    ),
    OmarchyRole::text("result", &["foreground"]),
    OmarchyRole::plain("status_bg", &["dark_background"]),
    OmarchyRole::plain("status_fg", &["foreground"]),
    OmarchyRole::plain("badge_normal", &["blue"]),
    OmarchyRole::plain("badge_insert", &["green"]),
    OmarchyRole::plain("badge_visual", &["orange"]),
    OmarchyRole::plain("gutter", &["muted"]),
    OmarchyRole::text(
        "current_line_nr",
        &["yellow", "bright_yellow", "orange", "accent", "foreground"],
    ),
    OmarchyRole::plain("separator", &["selection"]),
    OmarchyRole::plain("ghost", &["muted"]),
    OmarchyRole::plain("visual_bg", &["selection"]),
    OmarchyRole::plain("visual_fg", &["yellow"]),
];

/// One row of [`OMARCHY_ROLES`].
#[derive(Debug, Clone, Copy)]
pub struct OmarchyRole {
    pub slot: &'static str,
    pub keys: &'static [&'static str],
    /// Pick by contrast against `background` rather than by order alone.
    pub check_contrast: bool,
}

impl OmarchyRole {
    const fn text(slot: &'static str, keys: &'static [&'static str]) -> Self {
        Self {
            slot,
            keys,
            check_contrast: true,
        }
    }

    const fn plain(slot: &'static str, keys: &'static [&'static str]) -> Self {
        Self {
            slot,
            keys,
            check_contrast: false,
        }
    }
}

/// The least contrast a text colour may have against the background before
/// nm looks for another role (WCAG AA for normal text).
pub const MIN_CONTRAST: f64 = 4.5;

/// WCAG contrast ratio between two colours, from 1 (same) to 21.
pub fn contrast(a: Color, b: Color) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Relative luminance of an RGB colour; other colour kinds count as
/// mid-grey, so a theme that uses them is neither favoured nor rejected.
fn luminance(c: Color) -> f64 {
    let Color::Rgb(r, g, b) = c else {
        return 0.2;
    };
    let channel = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// Theme files larger than this are ignored.
const MAX_FILE_BYTES: u64 = 64 * 1024;

impl Theme {
    /// The built-in default.
    pub const fn tokyo_night() -> Self {
        Self {
            number: style::NUMBER,
            operator: style::OPERATOR,
            variable: style::VARIABLE,
            comment: style::COMMENT,
            keyword: style::KEYWORD,
            error: style::ERROR,
            result: style::RESULT,
            status_bg: style::STATUS_BG,
            status_fg: style::STATUS_FG,
            badge_normal: style::BADGE_NORMAL,
            badge_insert: style::BADGE_INSERT,
            badge_visual: style::BADGE_VISUAL,
            gutter: style::GUTTER,
            current_line_nr: style::CURRENT_LINE_NR,
            separator: style::SEPARATOR,
            ghost: style::GHOST,
            visual_bg: style::VISUAL_BG,
            visual_fg: style::VISUAL_FG,
        }
    }

    /// The slot called `name`, or `None` for an unknown name.
    fn slot_mut(&mut self, name: &str) -> Option<&mut Color> {
        Some(match name {
            "number" => &mut self.number,
            "operator" => &mut self.operator,
            "variable" => &mut self.variable,
            "comment" => &mut self.comment,
            "keyword" => &mut self.keyword,
            "error" => &mut self.error,
            "result" => &mut self.result,
            "status_bg" => &mut self.status_bg,
            "status_fg" => &mut self.status_fg,
            "badge_normal" => &mut self.badge_normal,
            "badge_insert" => &mut self.badge_insert,
            "badge_visual" => &mut self.badge_visual,
            "gutter" => &mut self.gutter,
            "current_line_nr" => &mut self.current_line_nr,
            "separator" => &mut self.separator,
            "ghost" => &mut self.ghost,
            "visual_bg" => &mut self.visual_bg,
            "visual_fg" => &mut self.visual_fg,
            _ => return None,
        })
    }

    /// Sets every slot named in `colors`; other names are ignored.
    fn apply_nm(&mut self, colors: &HashMap<String, Color>) {
        for (name, color) in colors {
            if let Some(slot) = self.slot_mut(name) {
                *slot = *color;
            }
        }
    }

    /// Sets each slot whose Omarchy role is present in `colors`.
    fn apply_omarchy(&mut self, colors: &HashMap<String, Color>) {
        let background = colors.get("background").copied();
        for role in OMARCHY_ROLES {
            let candidates = role.keys.iter().filter_map(|k| colors.get(*k).copied());
            let pick = match (role.check_contrast, background) {
                (true, Some(bg)) => pick_readable(candidates, bg),
                _ => candidates.into_iter().next(),
            };
            if let (Some(color), Some(slot)) = (pick, self.slot_mut(role.slot)) {
                *slot = color;
            }
        }
    }
}

/// The first colour whose contrast against `bg` reaches [`MIN_CONTRAST`],
/// otherwise the most contrasting one, otherwise `None`.
fn pick_readable(candidates: impl Iterator<Item = Color>, bg: Color) -> Option<Color> {
    let mut best: Option<(f64, Color)> = None;
    for c in candidates {
        let ratio = contrast(c, bg);
        if ratio >= MIN_CONTRAST {
            return Some(c);
        }
        if best.is_none_or(|(b, _)| ratio > b) {
            best = Some((ratio, c));
        }
    }
    best.map(|(_, c)| c)
}

impl Default for Theme {
    fn default() -> Self {
        Self::tokyo_night()
    }
}

thread_local! {
    static ACTIVE: Cell<Theme> = const { Cell::new(Theme::tokyo_night()) };
}

/// The theme the style functions paint with on this thread.
pub fn active() -> Theme {
    ACTIVE.with(Cell::get)
}

/// Makes `theme` the one this thread paints with.
pub fn set_active(theme: Theme) {
    ACTIVE.with(|t| t.set(theme));
}

/// The theme from the user's files, falling back to Tokyo Night. Never
/// fails: missing, unreadable or malformed files are ignored.
pub fn load() -> Theme {
    let home = storage::home_dir();
    load_from(
        config_path_from(std::env::var_os("XDG_CONFIG_HOME"), home.clone()).as_deref(),
        omarchy_path_from(std::env::var_os("XDG_STATE_HOME"), home).as_deref(),
    )
}

/// [`load`] with the nm theme file and the Omarchy `colors.toml` passed in.
pub fn load_from(nm_file: Option<&Path>, omarchy_file: Option<&Path>) -> Theme {
    let mut theme = Theme::tokyo_night();
    if let Some(text) = omarchy_file.and_then(read_small) {
        theme.apply_omarchy(&parse(&text));
    }
    if let Some(text) = nm_file.and_then(read_small) {
        theme.apply_nm(&parse(&text));
    }
    theme
}

/// The theme files nm reads, in the order [`load`] reads them.
pub fn source_paths() -> Vec<PathBuf> {
    let home = storage::home_dir();
    [
        config_path_from(std::env::var_os("XDG_CONFIG_HOME"), home.clone()),
        omarchy_path_from(std::env::var_os("XDG_STATE_HOME"), home),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Notices when any theme file changes on disk, so the event loop can reload
/// the theme while nm runs (an Omarchy theme switch replaces the whole
/// `current/theme` directory, which changes the file's modification time).
///
/// A file is "changed" when its modification time or size differs from the
/// last look, or when it appears or disappears.
pub struct Watch {
    paths: Vec<PathBuf>,
    stamps: Vec<Option<(std::time::SystemTime, u64)>>,
}

impl Watch {
    /// Watches the files [`load`] reads.
    pub fn new() -> Self {
        Self::of(source_paths())
    }

    /// Watches `paths`, taking their current state as the baseline.
    pub fn of(paths: Vec<PathBuf>) -> Self {
        let stamps = paths.iter().map(|p| Self::stamp(p)).collect();
        Self { paths, stamps }
    }

    /// True when any watched file changed since the last call (or since
    /// construction). The new state becomes the baseline.
    pub fn changed(&mut self) -> bool {
        let mut changed = false;
        for (path, old) in self.paths.iter().zip(self.stamps.iter_mut()) {
            let now = Self::stamp(path);
            if now != *old {
                *old = now;
                changed = true;
            }
        }
        changed
    }

    fn stamp(path: &Path) -> Option<(std::time::SystemTime, u64)> {
        let meta = std::fs::metadata(path).ok()?;
        Some((meta.modified().ok()?, meta.len()))
    }
}

/// The nm theme file path: `$XDG_CONFIG_HOME/nm/theme.toml`, or
/// `~/.config/nm/theme.toml`.
pub fn config_path_from(
    xdg_config_home: Option<OsString>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    xdg_dir(xdg_config_home, home, ".config").map(|d| d.join("nm").join("theme.toml"))
}

/// The active Omarchy theme's colours:
/// `$XDG_STATE_HOME/omarchy/current/theme/colors.toml`, or the same under
/// `~/.local/state`.
pub fn omarchy_path_from(
    xdg_state_home: Option<OsString>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    xdg_dir(xdg_state_home, home, ".local/state")
        .map(|d| d.join("omarchy/current/theme/colors.toml"))
}

/// An XDG base directory: the variable when it is an absolute path,
/// otherwise `home/fallback`. Relative values are ignored, as the XDG spec
/// says.
fn xdg_dir(var: Option<OsString>, home: Option<PathBuf>, fallback: &str) -> Option<PathBuf> {
    var.map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| h.join(fallback)))
}

/// The file's text, or `None` when it cannot be read, is not UTF-8, or is
/// larger than [`MAX_FILE_BYTES`].
fn read_small(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Every `key = "#rrggbb"` line in `text`, by key. Other lines are
/// skipped; a later line wins over an earlier one with the same key.
pub fn parse(text: &str) -> HashMap<String, Color> {
    text.lines().filter_map(parse_line).collect()
}

/// One `key = "#rrggbb"` line, with an optional trailing `# comment`.
fn parse_line(line: &str) -> Option<(String, Color)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let value = value.trim_start();
    let quote = value.chars().next().filter(|&c| c == '"' || c == '\'')?;
    let rest = &value[1..];
    let end = rest.find(quote)?;
    let after = rest[end + 1..].trim_start();
    if !(after.is_empty() || after.starts_with('#')) {
        return None;
    }
    Some((key.to_string(), parse_hex(&rest[..end])?))
}

/// `#rrggbb`, either case.
fn parse_hex(s: &str) -> Option<Color> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(Color::Rgb(byte(0)?, byte(2)?, byte(4)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RETRO_82: &str = include_str!("theme/fixtures/retro-82.toml");

    /// Every slot name.
    const SLOTS: [&str; 18] = [
        "number",
        "operator",
        "variable",
        "comment",
        "keyword",
        "error",
        "result",
        "status_bg",
        "status_fg",
        "badge_normal",
        "badge_insert",
        "badge_visual",
        "gutter",
        "current_line_nr",
        "separator",
        "ghost",
        "visual_bg",
        "visual_fg",
    ];

    fn rgb(hex: u32) -> Color {
        Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    // ---- parsing ----

    #[test]
    fn parse_line_cases() {
        let cases: [(&str, Option<(&str, u32)>); 18] = [
            ("number = \"#7dcfff\"", Some(("number", 0x7DCFFF))),
            ("number=\"#7dcfff\"", Some(("number", 0x7DCFFF))),
            ("  number   =   \"#7DCFFF\"  ", Some(("number", 0x7DCFFF))),
            ("number = '#7dcfff'", Some(("number", 0x7DCFFF))),
            ("number = \"#7dcfff\" # blue", Some(("number", 0x7DCFFF))),
            ("number = \"#7dcfff\"\r", Some(("number", 0x7DCFFF))),
            ("", None),
            ("   ", None),
            ("# number = \"#7dcfff\"", None),
            ("number = \"7dcfff\"", None),
            ("number = \"#7dcfgg\"", None),
            ("number = \"#7dcff\"", None),
            ("number = \"#7dcfff0\"", None),
            ("number = #7dcfff", None),
            ("number = \"#7dcfff", None),
            ("number = \"#7dcfff\" junk", None),
            ("= \"#7dcfff\"", None),
            ("[colors]", None),
        ];
        for (line, want) in cases {
            let got = parse_line(line);
            let want = want.map(|(k, c)| (k.to_string(), rgb(c)));
            assert_eq!(got, want, "{line:?}");
        }
    }

    #[test]
    fn parse_non_ascii_is_skipped_not_a_panic() {
        assert_eq!(parse_line("number = \"#é7dcff\""), None);
        assert_eq!(parse_line("nümber = \"#7dcfff\""), None);
        assert_eq!(parse_line("number = \"é\""), None);
    }

    #[test]
    fn parse_file_with_comments_blanks_and_crlf() {
        let text = "# my theme\r\n\r\nnumber = \"#112233\"\r\nmode = \"dark\"\r\nbogus line\r\nnumber = \"#445566\"\r\n";
        let map = parse(text);
        assert_eq!(map.len(), 1);
        assert_eq!(map["number"], rgb(0x445566), "later line wins");
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let mut theme = Theme::tokyo_night();
        theme.apply_nm(&parse("nope = \"#112233\"\nnumber = \"#445566\"\n"));
        assert_eq!(
            theme,
            Theme {
                number: rgb(0x445566),
                ..Theme::tokyo_night()
            }
        );
    }

    #[test]
    fn every_slot_name_is_settable() {
        for name in SLOTS {
            let mut theme = Theme::tokyo_night();
            theme.apply_nm(&parse(&format!("{name} = \"#010203\"")));
            assert_ne!(theme, Theme::tokyo_night(), "{name}");
        }
        for role in OMARCHY_ROLES {
            let slot = role.slot;
            assert!(SLOTS.contains(&slot), "{slot}");
        }
    }

    // ---- Omarchy mapping ----

    #[test]
    fn omarchy_retro_82_maps_by_role() {
        let mut theme = Theme::tokyo_night();
        theme.apply_omarchy(&parse(RETRO_82));
        let want = Theme {
            // foreground: numbers are body text.
            number: rgb(0xF6DCAC),
            // magenta, contrast 4.7 against the background: kept.
            operator: rgb(0x3F8F8A),
            // green reads at 4.0, bright_green is the same colour, so cyan.
            variable: rgb(0x8CBFB8),
            comment: rgb(0x2A6B78),
            keyword: rgb(0xF85525),
            error: rgb(0xF85525),
            result: rgb(0xF6DCAC),
            status_bg: rgb(0x031222),
            status_fg: rgb(0xF6DCAC),
            badge_normal: rgb(0x3F8F8A),
            badge_insert: rgb(0x028391),
            badge_visual: rgb(0xFAA968),
            gutter: rgb(0x2A6B78),
            current_line_nr: rgb(0xE97B3C),
            separator: rgb(0x134E5A),
            ghost: rgb(0x2A6B78),
            visual_bg: rgb(0x134E5A),
            visual_fg: rgb(0xE97B3C),
        };
        assert_eq!(theme, want);
    }

    #[test]
    fn omarchy_missing_key_falls_through_to_tokyo_night() {
        let text: String = RETRO_82
            .lines()
            .filter(|l| !l.starts_with("orange "))
            .map(|l| format!("{l}\n"))
            .collect();
        let mut theme = Theme::tokyo_night();
        theme.apply_omarchy(&parse(&text));
        assert_eq!(theme.badge_visual, style::BADGE_VISUAL);
        assert_eq!(theme.number, rgb(0xF6DCAC));
    }

    #[test]
    fn omarchy_low_contrast_roles_fall_back() {
        // A light theme whose magenta, bright_magenta and accent all read
        // below 4.5 on the background; light_foreground is the first that
        // does not. Green reads well, so variables keep it.
        let text = "background = \"#faf4ed\"\nforeground = \"#575279\"\n\
            light_foreground = \"#6e6a86\"\naccent = \"#56949f\"\n\
            magenta = \"#907aa9\"\nbright_magenta = \"#907aa9\"\n\
            green = \"#286983\"\nred = \"#b4637a\"\nyellow = \"#ea9d34\"\n";
        let mut theme = Theme::tokyo_night();
        theme.apply_omarchy(&parse(text));
        assert_eq!(theme.operator, rgb(0x6E6A86));
        assert_eq!(theme.variable, rgb(0x286983));
        assert_eq!(theme.number, rgb(0x575279));
        // red reads at 3.8 and nothing later is better except foreground.
        assert_eq!(theme.keyword, rgb(0x575279));
        // yellow reads at 2.1; orange and accent are absent or worse;
        // foreground wins.
        assert_eq!(theme.current_line_nr, rgb(0x575279));
    }

    #[test]
    fn omarchy_without_background_takes_the_first_key() {
        let text = "magenta = \"#000001\"\nforeground = \"#000002\"\n";
        let mut theme = Theme::tokyo_night();
        theme.apply_omarchy(&parse(text));
        assert_eq!(theme.operator, rgb(0x000001));
        assert_eq!(theme.number, rgb(0x000002));
    }

    #[test]
    fn contrast_ratio_matches_wcag() {
        let white = rgb(0xFFFFFF);
        let black = rgb(0x000000);
        assert!((contrast(white, black) - 21.0).abs() < 0.01);
        assert!((contrast(black, white) - 21.0).abs() < 0.01);
        assert!((contrast(white, white) - 1.0).abs() < 0.01);
        // retro-82 blue on its background, as measured.
        assert!((contrast(rgb(0x3F8F8A), rgb(0x05182E)) - 4.68).abs() < 0.01);
    }

    // ---- precedence ----

    #[test]
    fn nm_file_beats_omarchy_beats_default_per_slot() {
        let dir = tempfile::tempdir().unwrap();
        let omarchy = write(
            dir.path(),
            "colors.toml",
            "blue = \"#000001\"\nmagenta = \"#000002\"\n",
        );
        let nm = write(dir.path(), "theme.toml", "number = \"#0000ff\"\n");
        let theme = load_from(Some(&nm), Some(&omarchy));
        assert_eq!(theme.number, rgb(0x0000FF), "nm file");
        assert_eq!(theme.operator, rgb(0x000002), "Omarchy");
        assert_eq!(theme.variable, style::VARIABLE, "default");
    }

    #[test]
    fn missing_files_give_tokyo_night() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("gone.toml");
        assert_eq!(load_from(Some(&gone), Some(&gone)), Theme::tokyo_night());
        assert_eq!(load_from(None, None), Theme::tokyo_night());
        // A directory is not a readable file.
        assert_eq!(
            load_from(Some(dir.path()), Some(dir.path())),
            Theme::tokyo_night()
        );
    }

    #[test]
    fn malformed_file_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("theme.toml");
        let mut bytes = b"number = \"#0000ff\"\n".to_vec();
        bytes.extend([0xFF, 0xFE, b'\n']);
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(load_from(Some(&path), None), Theme::tokyo_night());

        let big = "number = \"#0000ff\"\n".repeat(4000);
        let path = write(dir.path(), "big.toml", &big);
        assert_eq!(load_from(Some(&path), None), Theme::tokyo_night());
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let nm = write(
            dir.path(),
            "theme.toml",
            "number = \"0000ff\"\noperator = \"#00ff00\"\n",
        );
        let theme = load_from(Some(&nm), None);
        assert_eq!(theme.number, style::NUMBER);
        assert_eq!(theme.operator, rgb(0x00FF00));
    }

    // ---- paths ----

    #[test]
    fn config_path_cases() {
        let home = || Some(PathBuf::from("/home/u"));
        assert_eq!(
            config_path_from(None, home()).unwrap(),
            Path::new("/home/u/.config/nm/theme.toml")
        );
        assert_eq!(
            config_path_from(Some("/xdg/cfg".into()), home()).unwrap(),
            Path::new("/xdg/cfg/nm/theme.toml")
        );
        for xdg in ["", "relative"] {
            assert_eq!(
                config_path_from(Some(xdg.into()), home()).unwrap(),
                Path::new("/home/u/.config/nm/theme.toml"),
                "{xdg:?}"
            );
        }
        assert_eq!(config_path_from(None, None), None);
    }

    #[test]
    fn omarchy_path_cases() {
        let home = || Some(PathBuf::from("/home/u"));
        assert_eq!(
            omarchy_path_from(None, home()).unwrap(),
            Path::new("/home/u/.local/state/omarchy/current/theme/colors.toml")
        );
        assert_eq!(
            omarchy_path_from(Some("/xdg/state".into()), home()).unwrap(),
            Path::new("/xdg/state/omarchy/current/theme/colors.toml")
        );
        assert_eq!(
            omarchy_path_from(Some("rel".into()), home()).unwrap(),
            Path::new("/home/u/.local/state/omarchy/current/theme/colors.toml")
        );
        assert_eq!(omarchy_path_from(None, None), None);
    }

    // ---- active theme ----

    #[test]
    fn active_theme_defaults_to_tokyo_night_and_is_per_thread() {
        assert_eq!(active(), Theme::tokyo_night());
        let custom = Theme {
            number: rgb(0x010203),
            ..Theme::tokyo_night()
        };
        std::thread::spawn(move || {
            set_active(custom);
            assert_eq!(active(), custom);
        })
        .join()
        .unwrap();
        assert_eq!(active(), Theme::tokyo_night());
    }

    #[test]
    fn watch_notices_edits_appearance_and_removal() {
        let dir = tempfile::tempdir().unwrap();
        let present = dir.path().join("theme.toml");
        let absent = dir.path().join("missing.toml");
        std::fs::write(&present, "number = \"#000000\"\n").unwrap();
        let mut watch = Watch::of(vec![present.clone(), absent.clone()]);
        assert!(!watch.changed(), "nothing changed yet");

        // A rewrite with a different size.
        std::fs::write(&present, "number = \"#000000\"\noperator = \"#111111\"\n").unwrap();
        assert!(watch.changed());
        assert!(!watch.changed(), "baseline moved to the new state");

        // Same size, newer modification time (what a theme switch does).
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&present)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert!(watch.changed());

        // A file appearing, then disappearing.
        std::fs::write(&absent, "x").unwrap();
        assert!(watch.changed());
        std::fs::remove_file(&absent).unwrap();
        assert!(watch.changed());
        assert!(!watch.changed());
    }
}
