//! Command-line parsing: `mediaplayer [OPTIONS] [FILE]...`.

use std::ffi::OsString;
use std::path::PathBuf;

pub const USAGE: &str = "\
Usage: mediaplayer [OPTIONS] [FILE]...

Opens FILE on start (only the first one until the play queue lands).

Options:
  -h, --help     Print help
  -V, --version  Print version
  --             Treat every following argument as a file name";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Run { files: Vec<PathBuf> },
    Help,
    Version,
}

/// Parses the arguments after the program name. Non-UTF-8 file names are kept as-is.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let mut files = Vec::new();
    let mut options_done = false;
    for arg in args {
        if !options_done {
            match arg.to_str() {
                Some("--") => {
                    options_done = true;
                    continue;
                }
                Some("-h" | "--help") => return Ok(Command::Help),
                Some("-V" | "--version") => return Ok(Command::Version),
                Some(option) if option.starts_with('-') && option.len() > 1 => {
                    return Err(format!("unknown option '{option}'"));
                }
                _ => {}
            }
        }
        files.push(PathBuf::from(arg));
    }
    Ok(Command::Run { files })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn no_arguments_starts_empty() {
        assert_eq!(parse_strs(&[]), Ok(Command::Run { files: vec![] }));
    }

    #[test]
    fn positional_arguments_are_files() {
        assert_eq!(
            parse_strs(&["song.mp3", "movie.mkv"]),
            Ok(Command::Run {
                files: vec!["song.mp3".into(), "movie.mkv".into()]
            })
        );
    }

    #[test]
    fn help_and_version() {
        assert_eq!(parse_strs(&["--help"]), Ok(Command::Help));
        assert_eq!(parse_strs(&["-V"]), Ok(Command::Version));
    }

    #[test]
    fn unknown_options_are_errors() {
        assert!(parse_strs(&["--fullscreen"]).is_err());
    }

    #[test]
    fn double_dash_allows_names_starting_with_a_dash() {
        assert_eq!(
            parse_strs(&["--", "-intro.mp3"]),
            Ok(Command::Run {
                files: vec!["-intro.mp3".into()]
            })
        );
    }

    #[test]
    fn a_lone_dash_is_a_file_name() {
        assert_eq!(
            parse_strs(&["-"]),
            Ok(Command::Run {
                files: vec!["-".into()]
            })
        );
    }
}
