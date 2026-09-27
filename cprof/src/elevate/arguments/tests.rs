use std::ffi::OsString;
use std::iter::once;
use std::path::PathBuf;

use clap::Parser;

use crate::cli::{Cli, RootCommand, TargetCommand, command_with_extra_targets};
use crate::elevate::prepare_args;
#[cfg(windows)]
use crate::error::{AppError, PathError};

use super::{ELEVATED_CONTEXT_ARG, quote_arg, split_context_prefix, split_elevation_prefix};

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

#[test]
fn context_prefix_is_private_and_preserves_public_arguments() {
    let digest = "a".repeat(64);
    let args = vec![
        ELEVATED_CONTEXT_ARG.into(),
        "C:\\temporary file.json".into(),
        digest.clone().into(),
        "codex".into(),
        "adopt".into(),
        "--".into(),
        ELEVATED_CONTEXT_ARG.into(),
    ];
    let (reference, remaining) = split_context_prefix(args.clone()).unwrap();
    let reference = reference.unwrap();
    assert_eq!(reference.digest, digest);
    assert_eq!(reference.path, PathBuf::from("C:\\temporary file.json"));
    assert_eq!(remaining, args[3..]);
    assert!(split_context_prefix(remaining.clone()).unwrap().0.is_none());
    let cli = Cli::try_parse_from(once(OsString::from("cprof")).chain(remaining)).unwrap();
    assert!(matches!(cli.command, RootCommand::Codex(_)));
    assert!(
        !command_with_extra_targets([])
            .render_help()
            .to_string()
            .contains("elevated-context")
    );
    for invalid in [
        vec![ELEVATED_CONTEXT_ARG.into()],
        vec![ELEVATED_CONTEXT_ARG.into(), "file".into()],
        vec![
            ELEVATED_CONTEXT_ARG.into(),
            "file".into(),
            "bad-digest".into(),
        ],
    ] {
        assert!(split_context_prefix(invalid).is_err());
    }
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
