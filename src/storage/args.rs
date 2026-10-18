//! Command-line argument parsing for the two forms `nm [file]` and
//! `nm eval`.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

/// One-paragraph usage text for `nm --help` and argument errors.
pub const USAGE: &str = "\
usage: nm [file]     open file in the editor (no file: resume the last session)
       nm eval       evaluate stdin, print one JSON object per line
       nm -h|--help  show this help

Use `nm ./eval` to open a file named eval, and `nm -- -file` for a file
whose name starts with `-`.";

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Run the editor on a file, or in session mode when `None`. The path is
    /// as given; [`super::resolve_startup`] expands `~/`.
    Edit(Option<PathBuf>),
    /// `nm eval`: evaluate stdin and print JSON lines.
    Eval,
    /// `-h` or `--help`.
    Help,
}

/// A command line that could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgsError {
    /// An option other than `-h`/`--help`/`--`.
    UnknownOption(String),
    /// More arguments than the form allows.
    UnexpectedArgument(String),
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOption(o) => write!(f, "unknown option: {o}"),
            Self::UnexpectedArgument(a) => write!(f, "unexpected argument: {a}"),
        }
    }
}

impl std::error::Error for ArgsError {}

/// Parses the arguments after the program name.
///
/// `eval` as the first argument is the subcommand. Otherwise the first
/// argument is the file. `--` ends option parsing. Extra arguments
/// are rejected so a typo does not silently open the wrong thing.
pub fn parse_args<I>(args: I) -> Result<Command, ArgsError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut positional: Vec<OsString> = Vec::new();
    let mut options_done = false;
    for arg in args {
        if !options_done && let Some(text) = arg.to_str() {
            match text {
                "--" => {
                    options_done = true;
                    continue;
                }
                "-h" | "--help" => return Ok(Command::Help),
                _ if text.starts_with('-') && text != "-" => {
                    return Err(ArgsError::UnknownOption(text.to_string()));
                }
                _ => {}
            }
        }
        positional.push(arg);
    }

    let mut rest = positional.into_iter();
    let Some(first) = rest.next() else {
        return Ok(Command::Edit(None));
    };
    if let Some(extra) = rest.next() {
        return Err(ArgsError::UnexpectedArgument(
            extra.to_string_lossy().into_owned(),
        ));
    }
    if !options_done && first == "eval" {
        return Ok(Command::Eval);
    }
    Ok(Command::Edit(Some(PathBuf::from(first))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, ArgsError> {
        parse_args(args.iter().map(OsString::from))
    }

    fn edit(path: &str) -> Result<Command, ArgsError> {
        Ok(Command::Edit(Some(PathBuf::from(path))))
    }

    #[test]
    fn no_arguments_is_session_mode() {
        assert_eq!(parse(&[]), Ok(Command::Edit(None)));
    }

    #[test]
    fn file_argument() {
        assert_eq!(parse(&["budget.nm"]), edit("budget.nm"));
        assert_eq!(parse(&["~/notes/a.nm"]), edit("~/notes/a.nm"));
        assert_eq!(parse(&["-"]), edit("-"));
    }

    #[test]
    fn eval_subcommand() {
        assert_eq!(parse(&["eval"]), Ok(Command::Eval));
        assert_eq!(parse(&["./eval"]), edit("./eval"));
        assert_eq!(parse(&["--", "eval"]), edit("eval"));
    }

    #[test]
    fn help() {
        assert_eq!(parse(&["-h"]), Ok(Command::Help));
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
        assert_eq!(parse(&["a.nm", "--help"]), Ok(Command::Help));
        assert_eq!(parse(&["--", "--help"]), edit("--help"));
    }

    #[test]
    fn errors() {
        assert_eq!(
            parse(&["--bogus"]),
            Err(ArgsError::UnknownOption("--bogus".into()))
        );
        assert_eq!(
            parse(&["a.nm", "b.nm"]),
            Err(ArgsError::UnexpectedArgument("b.nm".into()))
        );
        assert_eq!(
            parse(&["eval", "x"]),
            Err(ArgsError::UnexpectedArgument("x".into()))
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_file_name() {
        use std::os::unix::ffi::OsStringExt;
        let name = OsString::from_vec(vec![b'a', 0xff, b'.', b'n', b'm']);
        assert_eq!(
            parse_args([name.clone()]),
            Ok(Command::Edit(Some(PathBuf::from(name))))
        );
    }
}
