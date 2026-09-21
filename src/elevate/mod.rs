use std::ffi::OsString;

use crate::error::{AppError, Result};

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

/// Retry only privilege failures after successful rollback. Elevated children
/// must never request another elevation; other errors retain their original type.
pub(crate) fn retry_as_admin(error: AppError, args: &[OsString]) -> Result<()> {
    if !is_privilege_error(&error) || is_elevated_child() {
        return Err(error);
    }
    platform::run_as_admin(args)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;

    use super::*;

    #[test]
    fn ordinary_errors_are_returned_without_elevation() {
        assert!(matches!(
            retry_as_admin(AppError::ProfileExists("existing".into()), &[]),
            Err(AppError::ProfileExists(name)) if name == "existing"
        ));
    }

    #[test]
    fn failed_recovery_and_committed_transactions_are_never_retried() {
        // ERROR_PRIVILEGE_NOT_HELD on Windows. Nesting it in these errors must
        // not launch another process even on that platform.
        let privilege = || AppError::Io(io::Error::from_raw_os_error(1314));
        let error =
            privilege().with_recovery([AppError::TransactionConflict("recovery failed".into())]);
        assert!(!is_privilege_error(&error));
        assert!(error.source().is_some());
        assert!(matches!(
            retry_as_admin(error, &[]),
            Err(AppError::RecoveryFailed { .. })
        ));

        let error = AppError::TransactionCleanupFailed(vec![privilege()]);
        assert!(!is_privilege_error(&error));
        assert!(matches!(
            retry_as_admin(error, &[]),
            Err(AppError::TransactionCleanupFailed(_))
        ));
    }
}
