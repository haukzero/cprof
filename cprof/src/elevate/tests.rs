use std::cell::Cell;
use std::error::Error;
use std::io;

use crate::error::{AppError, ProfileError, TransactionError};

use super::{is_privilege_error, run, run_with_context};

#[test]
fn ordinary_errors_are_returned_without_elevation() {
    assert!(matches!(
        run::<()>(|| Ok(vec![]), || Err(ProfileError::Exists("existing".into()).into())),
        Err(AppError::Profile(ProfileError::Exists(name))) if name == "existing"
    ));
}

#[test]
fn successful_operations_do_not_build_replay_arguments() {
    let built = Cell::new(false);
    let result = run(
        || {
            built.set(true);
            Ok(vec!["switch".into()])
        },
        || Ok(42),
    );
    assert!(matches!(result, Ok(Some(42))));
    assert!(!built.get());
    assert_eq!(
        run_with_context(
            || panic!("args built"),
            || panic!("context built"),
            || Ok(7)
        )
        .unwrap(),
        Some(7)
    );
    assert!(
        run_with_context::<()>(
            || panic!("args built"),
            || panic!("context built"),
            || Err(ProfileError::Exists("existing".into()).into()),
        )
        .is_err()
    );
}

#[test]
fn failed_recovery_and_committed_transactions_are_never_retried() {
    // ERROR_PRIVILEGE_NOT_HELD on Windows. Nesting it in these errors must
    // not launch another process even on that platform.
    let privilege = || AppError::Io(io::Error::from_raw_os_error(1314));
    let error =
        privilege().with_recovery([TransactionError::Conflict("recovery failed".into()).into()]);
    assert!(!is_privilege_error(&error));
    assert!(error.source().is_some());
    assert!(matches!(
        run::<()>(|| Ok(vec![]), || Err(error)),
        Err(AppError::Transaction(
            TransactionError::RecoveryFailed { .. }
        ))
    ));

    let error = TransactionError::CleanupFailed(vec![privilege()]).into();
    assert!(!is_privilege_error(&error));
    assert!(matches!(
        run::<()>(|| Ok(vec![]), || Err(error)),
        Err(AppError::Transaction(TransactionError::CleanupFailed(_)))
    ));
}
