use std::path::PathBuf;

use cprof::config;
use cprof::error::{AppError, Result};
use cprof::targets::{ResourceSpec, TargetSpec};

const TARGET: TargetSpec = TargetSpec {
    id: "demo",
    resources: &[],
};

fn resource(active_path: Option<&'static str>) -> ResourceSpec {
    ResourceSpec {
        key: "settings",
        filename: "settings.json",
        active_path,
        absolute_active_path: None,
        required: true,
        template: b"",
        validate: |_| Ok(()),
    }
}

fn assert_invalid_component(value: &'static str) {
    let target = TargetSpec {
        id: value,
        ..TARGET
    };
    assert!(
        matches!(
            config::profiles_dir(&target),
            Err(AppError::InvalidTargetId(_))
        ),
        "target id: {value:?}"
    );
    let spec = ResourceSpec {
        filename: value,
        ..resource(Some(".demo/settings"))
    };
    assert!(
        matches!(
            config::profile_resource(&TARGET, "test", &spec),
            Err(AppError::InvalidFilename(_))
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
                TARGET.active_path(home.path(), &resource(Some(value))),
                Err(AppError::InvalidActivePath(_))
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
                TARGET.active_path(home.path(), &resource(Some(value))),
                Err(AppError::InvalidActivePath(_))
            ),
            "active_path: {value:?}"
        );
    }
}

#[test]
fn relative_active_paths_normalize_both_separator_styles() -> Result<()> {
    let home = tempfile::tempdir()?;
    let actual = TARGET.active_path(
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
            absolute_active_path: Some(value),
            ..resource(None)
        };
        assert!(
            matches!(
                TARGET.active_path(home.path(), &spec),
                Err(AppError::InvalidActivePath(_))
            ),
            "absolute_active_path: {value:?}"
        );
    }
}

#[test]
fn an_active_path_is_required() {
    let home = tempfile::tempdir().unwrap();
    assert!(matches!(
        TARGET.active_path(home.path(), &resource(None)),
        Err(AppError::InvalidActivePath(_))
    ));
}

#[test]
fn home_must_be_absolute() {
    assert!(matches!(
        TARGET.active_path(&PathBuf::from("relative-home"), &resource(Some("settings"))),
        Err(AppError::UnsafePath(_))
    ));
}
