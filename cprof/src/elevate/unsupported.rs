use std::ffi::OsString;

use crate::error::{AppError, ElevationError, Result};

pub fn prepare_args(args: Vec<OsString>) -> Result<Vec<OsString>> {
    Ok(args)
}

pub fn is_elevated_child() -> bool {
    false
}

pub fn is_privilege_error(_: &AppError) -> bool {
    false
}

pub(super) fn take_context() -> Result<Option<serde_json::Value>> {
    Ok(None)
}

pub(super) fn run_as_admin(_: &[OsString], _: Option<&serde_json::Value>) -> Result<()> {
    Err(ElevationError::Failed.into())
}
