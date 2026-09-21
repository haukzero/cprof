use std::ffi::OsString;
use std::iter::repeat_n;
use std::path::PathBuf;

use crate::error::{ElevationError, Result};

pub(super) const ELEVATED_HOME_ARG: &str = "--elevated-home";

/// Only consume the private prefix prepended by run_as_admin. Later arguments
/// may legitimately contain the same text as an editor argument or profile name.
pub(super) fn split_elevation_prefix(
    args: Vec<OsString>,
) -> Result<(Option<PathBuf>, Vec<OsString>)> {
    if !args.first().is_some_and(|arg| arg == ELEVATED_HOME_ARG) {
        return Ok((None, args));
    }
    let mut args = args.into_iter().skip(1);
    let home = args.next().ok_or(ElevationError::MissingHome)?;
    Ok((Some(home.into()), args.collect()))
}

pub(super) fn quote_arg(value: &OsString) -> String {
    let value = value.to_string_lossy();
    if !value.is_empty() && !value.chars().any(|ch| ch.is_whitespace() || ch == '"') {
        return value.into_owned();
    }

    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    let mut backslashes = 0;
    for character in value.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(repeat_n('\\', backslashes));
                quoted.push(character);
                backslashes = 0;
            }
        }
    }
    quoted.extend(repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::iter::once;
    use std::path::PathBuf;

    use clap::Parser;

    use crate::cli::{Cli, RootCommand, TargetCommand, command_with_extra_targets};
    use crate::elevate::prepare_args;
    #[cfg(windows)]
    use crate::error::{AppError, PathError};

    use super::{quote_arg, split_elevation_prefix};

    #[test]
    fn quote_arg_preserves_windows_argument_boundaries() {
        assert_eq!(quote_arg(&"a\"b".into()), "\"a\\\"b\"");
        assert_eq!(quote_arg(&"".into()), "\"\"");
        assert_eq!(
            quote_arg(&"C:\\User Name\\".into()),
            "\"C:\\User Name\\\\\""
        );
        assert_eq!(quote_arg(&"profile name".into()), "\"profile name\"");
        assert_eq!(
            quote_arg(&"profile \\\"name".into()),
            "\"profile \\\\\\\"name\""
        );
    }

    #[test]
    fn internal_prefix_is_removed_before_public_cli_parsing() {
        let args = [
            "--elevated-home",
            "C:\\Users\\Original User",
            "codex",
            "adopt",
            "work profile",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        let (home, args) = split_elevation_prefix(args).unwrap();
        assert_eq!(home, Some(PathBuf::from("C:\\Users\\Original User")));
        let cli = Cli::try_parse_from(once(OsString::from("cprof")).chain(args)).unwrap();
        let RootCommand::Codex(args) = cli.command else {
            panic!("expected codex")
        };
        assert!(
            matches!(args.command, TargetCommand::Adopt { name: Some(name) } if name == "work profile")
        );
        assert!(
            !command_with_extra_targets([])
                .render_help()
                .to_string()
                .contains("elevated-home")
        );
    }

    #[test]
    fn ordinary_arguments_and_flag_literals_are_preserved() {
        for args in [
            vec![],
            vec!["--help"],
            vec!["codex", "which"],
            vec!["codex", "adopt", "--", "--elevated-home"],
            vec![
                "edit-extra",
                "--editor",
                "editor",
                "--editor-arg",
                "--elevated-home",
            ],
        ] {
            let args = args.into_iter().map(OsString::from).collect::<Vec<_>>();
            let (home, remaining) = split_elevation_prefix(args.clone()).unwrap();
            assert!(home.is_none());
            assert_eq!(remaining, args);
            assert_eq!(prepare_args(args.clone()).unwrap(), args);
        }
    }

    #[test]
    fn internal_prefix_requires_a_home_value() {
        let error = split_elevation_prefix(vec!["--elevated-home".into()]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Missing original home directory")
        );
    }

    #[cfg(windows)]
    #[test]
    fn elevated_retry_rejects_nonabsolute_home_paths() {
        for home in ["", "relative-home", "C:relative-home"] {
            assert!(matches!(
                prepare_args(vec![
                    "--elevated-home".into(),
                    home.into(),
                    "targets".into()
                ]),
                Err(AppError::Path(PathError::Unsafe(_)))
            ));
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn platforms_without_elevation_do_not_consume_the_internal_prefix() {
        let args = vec![
            "--elevated-home".into(),
            "/original/home".into(),
            "targets".into(),
        ];
        assert_eq!(prepare_args(args.clone()).unwrap(), args);
    }
}
