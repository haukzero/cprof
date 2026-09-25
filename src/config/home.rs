use std::path::PathBuf;
use std::sync::OnceLock;

use crate::error::{ConfigError, Result};
#[cfg(windows)]
use crate::error::{ElevationError, PathError};

static OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn dir() -> Result<PathBuf> {
    OVERRIDE
        .get()
        .cloned()
        .or_else(dirs::home_dir)
        .ok_or_else(|| ConfigError::NoHomeDir.into())
}

#[cfg(windows)]
pub(crate) fn set_override(path: PathBuf) -> Result<()> {
    if !path.is_absolute() {
        return Err(PathError::Unsafe(path.display().to_string()).into());
    }
    OVERRIDE
        .set(path)
        .map_err(|_| ElevationError::AlreadyInitialized.into())
}
