use std::ffi::OsString;

use crate::error::Result;

#[cfg(any(windows, test))]
mod arguments;
#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
use self::unsupported as platform;
#[cfg(windows)]
use self::windows as platform;

// Expose one cross-platform API; callers do not select platform modules.
pub use platform::{is_elevated_child, is_privilege_error, prepare_args};

/// Run an operation locally, then retry its command after a recoverable privilege
/// failure. Replay arguments are built only for that retry. The operation must
/// finish rollback before returning an error.
/// `None` means the elevated child succeeded; its return value is not available.
pub(crate) fn run<T>(
    args: impl FnOnce() -> Result<Vec<OsString>>,
    operation: impl FnOnce() -> Result<T>,
) -> Result<Option<T>> {
    match operation() {
        Ok(value) => Ok(Some(value)),
        Err(error) if is_privilege_error(&error) && !is_elevated_child() => {
            platform::run_as_admin(&args()?).map(|()| None)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::error::Error;
    use std::io;

    use super::*;
    use crate::error::{AppError, ProfileError, TransactionError};

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
    }

    #[test]
    fn failed_recovery_and_committed_transactions_are_never_retried() {
        // ERROR_PRIVILEGE_NOT_HELD on Windows. Nesting it in these errors must
        // not launch another process even on that platform.
        let privilege = || AppError::Io(io::Error::from_raw_os_error(1314));
        let error = privilege()
            .with_recovery([TransactionError::Conflict("recovery failed".into()).into()]);
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
}
