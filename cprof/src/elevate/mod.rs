use std::ffi::OsString;

use serde::de::DeserializeOwned;

use crate::error::{AppError, ElevationError, Result};

#[cfg(any(windows, test))]
mod arguments;
#[cfg(windows)]
mod context;
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

/// Preserve useful retry failures even though the child has a separate console.
pub fn error_exit_code(error: &AppError) -> u8 {
    if !is_elevated_child() {
        return 1;
    }
    match error {
        AppError::Elevation(ElevationError::ReplayMismatch) => 3,
        AppError::Elevation(ElevationError::InvalidContext(_)) => 4,
        _ => 1,
    }
}

pub(crate) fn take_context<T: DeserializeOwned>() -> Result<Option<T>> {
    platform::take_context()?
        .map(|value| {
            serde_json::from_value(value)
                .map_err(|error| ElevationError::InvalidContext(error.to_string()).into())
        })
        .transpose()
}

/// Run an operation locally, then retry its command after a recoverable privilege
/// failure. Replay arguments are built only for that retry. The operation must
/// finish rollback before returning an error.
/// `None` means the elevated child succeeded; its return value is not available.
pub(crate) fn run<T>(
    args: impl FnOnce() -> Result<Vec<OsString>>,
    operation: impl FnOnce() -> Result<T>,
) -> Result<Option<T>> {
    run_with_context(args, || Ok(None), operation)
}

/// Build the context lazily, after rollback, just like the replay arguments.
pub(crate) fn run_with_context<T>(
    args: impl FnOnce() -> Result<Vec<OsString>>,
    context: impl FnOnce() -> Result<Option<serde_json::Value>>,
    operation: impl FnOnce() -> Result<T>,
) -> Result<Option<T>> {
    match operation() {
        Ok(value) => Ok(Some(value)),
        Err(error) if is_privilege_error(&error) && !is_elevated_child() => {
            platform::run_as_admin(&args()?, context()?.as_ref()).map(|()| None)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
