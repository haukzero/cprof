use std::path::PathBuf;

use cprof::config;
use cprof::error::{AppError, PathError, Result, TargetError};
use cprof::format::Format;
use cprof::targets::{ResourceSpec, TargetSpec};

#[test]
fn profile_names_reject_traversal_and_wildcards() {
    for name in ["../escape", "", "literal*star", "literal?mark"] {
        assert!(matches!(
            cprof::profile::validate_name(name),
            Err(AppError::Profile(cprof::error::ProfileError::InvalidName(
                _
            )))
        ));
    }
}

fn target() -> TargetSpec {
    TargetSpec {
        id: "demo".to_string(),
        resources: Vec::new(),
    }
}

fn resource(active_path: Option<&'static str>) -> ResourceSpec {
    ResourceSpec {
        key: "settings".to_string(),
        filename: "settings.json".to_string(),
        active_path: active_path.map(str::to_string),
        absolute_active_path: None,
        required: true,
        template: Vec::new(),
        format: Format::Any,
    }
}

fn assert_invalid_component(value: &'static str) {
    let invalid_target = TargetSpec {
        id: value.to_string(),
        ..target()
    };
    assert!(
        matches!(
            config::profiles_dir(&invalid_target),
            Err(AppError::Target(TargetError::InvalidId(_)))
        ),
        "target id: {value:?}"
    );
    let spec = ResourceSpec {
        filename: value.to_string(),
        ..resource(Some(".demo/settings"))
    };
    assert!(
        matches!(
            config::profile_resource(&target(), "test", &spec),
            Err(AppError::Path(PathError::InvalidFilename(_)))
        ),
        "filename: {value:?}"
    );
}

#[test]
fn target_ids_and_filenames_reject_unsafe_components() {
    for value in ["", ".", "..", "a/b", "a\\b", "/absolute", "a\0b", "a\nb"] {
        assert_invalid_component(value);
    }
}

#[test]
fn windows_prefixes_are_rejected_on_every_platform() {
    let home = tempfile::tempdir().unwrap();
    for value in [
        r"C:\settings",
        r"C:settings",
        r"\settings",
        r"\\server\share\settings",
        r"\\?\C:\settings",
        r"\\.\C:\settings",
    ] {
        assert_invalid_component(value);
        assert!(
            matches!(
                target().active_path(home.path(), &resource(Some(value))),
                Err(AppError::Path(PathError::InvalidActivePath(_)))
            ),
            "active_path: {value:?}"
        );
    }
}

#[test]
fn relative_active_paths_reject_roots_traversal_and_control_characters() {
    let home = tempfile::tempdir().unwrap();
    for value in [
        "",
        ".",
        "..",
        "../file",
        "a/../file",
        r"a\..\file",
        "/file",
        "a/\0file",
        "a/C:file",
    ] {
        assert!(
            matches!(
                target().active_path(home.path(), &resource(Some(value))),
                Err(AppError::Path(PathError::InvalidActivePath(_)))
            ),
            "active_path: {value:?}"
        );
    }
}

#[test]
fn relative_active_paths_normalize_both_separator_styles() -> Result<()> {
    let home = tempfile::tempdir()?;
    let actual = target().active_path(
        home.path(),
        &resource(Some(r"./.config//demo\settings.json")),
    )?;
    assert_eq!(actual, home.path().join(".config/demo/settings.json"));
    Ok(())
}

#[test]
fn invalid_absolute_paths_return_errors() {
    let home = tempfile::tempdir().unwrap();
    for value in [
        "",
        ".",
        "settings",
        "../settings",
        "/",
        r"C:\",
        "/a/../settings",
        r"C:\a\..\settings",
    ] {
        let spec = ResourceSpec {
            absolute_active_path: Some(value.to_string()),
            ..resource(None)
        };
        assert!(
            matches!(
                target().active_path(home.path(), &spec),
                Err(AppError::Path(PathError::InvalidActivePath(_)))
            ),
            "absolute_active_path: {value:?}"
        );
    }
}

#[test]
fn an_active_path_is_required() {
    let home = tempfile::tempdir().unwrap();
    assert!(matches!(
        target().active_path(home.path(), &resource(None)),
        Err(AppError::Path(PathError::InvalidActivePath(_)))
    ));
}

#[test]
fn home_must_be_absolute() {
    assert!(matches!(
        target().active_path(&PathBuf::from("relative-home"), &resource(Some("settings"))),
        Err(AppError::Path(PathError::Unsafe(_)))
    ));
}
