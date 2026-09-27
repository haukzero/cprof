use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};

use super::{
    AppError, ConfigError, EditorError, FormatError, IoContext, TargetError, TransactionError,
};

#[test]
fn io_context_preserves_path_and_original_error() {
    let path = Path::new("settings.toml");
    let result: io::Result<()> = Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
    let error = result.with_path(path).unwrap_err();

    assert_eq!(error.to_string(), "IO error for 'settings.toml': denied");
    assert!(matches!(&error, AppError::IoPath { path: actual, .. } if actual == path));
    assert!(error.is_io_kind(io::ErrorKind::PermissionDenied));
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
}

#[test]
fn successful_recovery_preserves_retryable_io() {
    for error in [
        AppError::from(io::Error::from_raw_os_error(1314)),
        AppError::io(Path::new("active"), io::Error::from_raw_os_error(1314)),
    ] {
        let error = error.with_recovery([]);
        assert_eq!(error.io_source().unwrap().raw_os_error(), Some(1314));
        assert!(matches!(error, AppError::Io(_) | AppError::IoPath { .. }));
    }
}

#[test]
fn contextual_io_errors_are_never_retryable() {
    let cause = || AppError::from(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
    let errors: [AppError; 4] = [
        cause().with_recovery([cause()]),
        TransactionError::CleanupFailed(vec![cause()]).into(),
        EditorError::NotCommitted {
            source: Box::new(cause()),
        }
        .into(),
        TargetError::ResourceValidation {
            path: "settings.toml".into(),
            source: Box::new(cause()),
        }
        .into(),
    ];
    for error in errors {
        assert!(error.io_source().is_none(), "{error}");
        assert!(
            !error.is_io_kind(io::ErrorKind::PermissionDenied),
            "{error}"
        );
    }
}

#[test]
fn nested_validation_and_edit_errors_retain_the_cause_chain() {
    let json_error = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    let json_message = format!("Invalid JSON: {json_error}");
    let validation: AppError = TargetError::ResourceValidation {
        path: "auth.json".into(),
        source: Box::new(json_error.into()),
    }
    .into();
    let error: AppError = EditorError::NotCommitted {
        source: Box::new(validation),
    }
    .into();

    assert_eq!(
        error.to_string(),
        format!(
            "Edit was not committed; original file was kept: Invalid resource at 'auth.json': {json_message}"
        )
    );
    let validation = error.source().unwrap();
    assert!(matches!(
        validation.downcast_ref::<Box<AppError>>().map(Box::as_ref),
        Some(AppError::Target(_))
    ));
    let json = validation.source().unwrap();
    assert!(matches!(
        json.downcast_ref::<Box<AppError>>().map(Box::as_ref),
        Some(AppError::Format(FormatError::Json(_)))
    ));
    assert!(json.source().unwrap().is::<serde_json::Error>());
}

#[test]
fn locked_edit_error_explains_how_to_resume_a_draft() {
    let error: AppError = EditorError::Locked {
        scope: PathBuf::from("/home/example/.cprof/profiles/codex/work"),
        lock: PathBuf::from("/tmp/cprof-edit-example.lock"),
    }
    .into();

    let message = error.to_string();
    assert!(message.contains("another cprof edit session is active"));
    assert!(message.contains("stop the process holding the lock"));
    assert!(message.contains("Existing draft will be reused"));
}

#[test]
fn external_config_errors_retain_parse_context() {
    let cause = toml::from_str::<toml::Value>("[invalid").unwrap_err();
    let expected = format!("Failed to parse 'extra-target.toml': {cause}");
    let error: AppError = ConfigError::Parse {
        path: "extra-target.toml".into(),
        source: cause,
    }
    .into();

    assert_eq!(error.to_string(), expected);
    assert!(error.source().unwrap().is::<toml::de::Error>());
}

#[test]
fn transaction_errors_preserve_primary_and_all_recovery_failures() {
    let errors = || {
        vec![
            AppError::io(Path::new("first"), io::Error::other("cleanup failed")),
            TransactionError::Conflict("second".into()).into(),
        ]
    };
    let details = "IO error for 'first': cleanup failed; Transaction conflict: second";
    let primary: AppError = TransactionError::Conflict("primary".into()).into();
    let error = primary.with_recovery(errors());

    assert_eq!(
        error.to_string(),
        format!("Transaction conflict: primary; rollback or cleanup also failed: {details}")
    );
    assert!(matches!(
        error.source().unwrap().downcast_ref::<Box<AppError>>().map(Box::as_ref),
        Some(AppError::Transaction(TransactionError::Conflict(message))) if message == "primary"
    ));
    let AppError::Transaction(TransactionError::RecoveryFailed {
        errors: retained, ..
    }) = error
    else {
        panic!("expected recovery failure");
    };
    assert_eq!(retained.len(), 2);

    let error: AppError = TransactionError::CleanupFailed(errors()).into();
    assert_eq!(
        error.to_string(),
        format!("Transaction committed, but cleanup failed: {details}")
    );
}
